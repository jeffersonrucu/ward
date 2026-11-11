#!/bin/bash -p
# Inside part of `make test-docker-uid` (Phase 22; D49; V3.2). Runs as root **only
# inside** the disposable `ubuntu:24.04` container; from the host come only the read-only
# test mount under /opt (test `wardd` and minimal client) and this script.
#
# The test `wardd` starts as uid 1000 (without any group) with the socket in the test
# socket folder under /run/user/1000/. Clients:
# - uid 4242: the `connect` fails with EACCES (13) because of the 0700 folder;
# - uid 4242 with CAP_DAC_OVERRIDE (gets past the 0700/0600): connects, the daemon closes
#   without sending a byte and the log has the uid refusal with uid 4242;
# - root (uid 0, with Docker's CAP_DAC_OVERRIDE): the same, with uid 0;
# - uid 1000: served.
# Each check prints "ok:" or the failure marker; any failure exits ≠ 0.

export PATH=/usr/sbin:/usr/bin:/bin
set -euo pipefail

WARDD=/opt/teste/wardd
CLIENT=/opt/teste/cliente
DIR=/run/user/1000/ward-test
SOCK=$DIR/wardd.sock
LOG=/tmp/wardd.log
failures=0

ok() { echo "ok: $*"; }
failed() { echo "FALHOU: $*"; failures=$((failures + 1)); }
check() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else failed "$1 (esperado '$3', veio '$2')"; fi
}

[ "$(id -u)" = 0 ] || { echo "inside.sh roda como root, só no contêiner" >&2; exit 1; }
[ -f /.dockerenv ] || { echo "inside.sh só roda dentro do contêiner" >&2; exit 1; }

# Folders of the test user (as systemd-logind and the home would give on the host).
install -d -m 0700 -o 1000 -g 1000 /run/user/1000
install -d -m 0700 -o 1000 -g 1000 /tmp/dev

setpriv --reuid=1000 --regid=1000 --clear-groups -- \
    env -i PATH=/usr/bin:/bin WARD_DEV_DIR=/tmp/dev WARD_DEV_SOCKET_DIR="$DIR" RUST_LOG=debug \
    "$WARDD" </dev/null >/dev/null 2>"$LOG" &
PID=$!
for _ in $(seq 1 100); do
    [ -S "$SOCK" ] && break
    kill -0 "$PID" 2>/dev/null || break
    sleep 0.1
done
[ -S "$SOCK" ] || { echo "o wardd não subiu:"; cat "$LOG"; exit 1; }
ok "wardd de teste no ar como uid 1000"

# uid 4242 without capability: stopped by the 0700 folder.
r="$(setpriv --reuid=4242 --regid=4242 --clear-groups -- "$CLIENT" "$SOCK" estado)"
check "uid 4242: connect recusado (EACCES)" "$r" "connect: 13"

# uid 4242 with CAP_DAC_OVERRIDE (ambient): gets past the 0700/0600; SO_PEERCRED stops it.
r="$(setpriv --reuid=4242 --regid=4242 --clear-groups \
    --inh-caps=+dac_override --ambient-caps=+dac_override -- "$CLIENT" "$SOCK" lista)"
check "uid 4242 com CAP_DAC_OVERRIDE: fechado sem nenhum byte" "$r" "bytes: 0"

# root: the same.
r="$("$CLIENT" "$SOCK" lista)"
check "root: fechado sem nenhum byte" "$r" "bytes: 0"

# The owner is served.
r="$(setpriv --reuid=1000 --regid=1000 --clear-groups -- "$CLIENT" "$SOCK" estado)"
check "uid 1000: atendido" "$r" "resposta: status"

sleep 0.3
log="$(cat "$LOG")"
check "log: uid 4242 recusado por SO_PEERCRED" "$log" "conexão recusada (UidRefused): uid 4242"
check "log: root recusado por SO_PEERCRED" "$log" "conexão recusada (UidRefused): uid 0"

kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true

if [ "$failures" -ne 0 ]; then
    echo "--- log do wardd ---"
    cat "$LOG"
    echo "$failures conferência(s) falharam"
    exit 1
fi
echo "test-docker-uid: tudo ok"
