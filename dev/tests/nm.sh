#!/bin/bash -p
# `make test-nm` (Phase 40; D5, D9, D47; V6.1, V6.2 host side): the test `ward-nm`
# against a test `wardd` of this script only, and with the real Chrome.
#
# - V6.1: the `ward-nm` run directly by a user process (here without the setgid, as on
#   every development host) gets the agent role; `preencher` and `sugerir` answer
#   `nao_autorizado` and nothing of the list `.dev/variations.txt` reaches stdout.
# - Real Chrome (headless, temporary profile in `.dev/nm.*`, test extension loaded through
#   `--remote-debugging-pipe`, native messaging manifest only inside the temporary
#   profile): Chrome opens the host as a direct child of the browser process, the
#   daemon's `PeerProcess::verify_parent` accepts that parent against
#   `/opt/google/chrome/chrome` and refuses the same host opened by another process; the
#   browser is dumpable (D47: the check through `/proc` holds); and `ward-nm` answers
#   through Chrome (status, fill refused to the agent, message too large).
# - The daemon and Chrome logs (stderr of `ward-nm`) and the extension report without a
#   pattern of `.dev/variations.txt`.
#
# Never touches the real Chrome profile nor `~/.config/google-chrome/NativeMessagingHosts`,
# nor the daemon of `make dev-daemon`: the daemon, the socket, the profile and the Chrome
# HOME belong to this script and are gone at the end. Needs `make dev-vault`,
# `make dev-variations` and Chrome at /opt/google/chrome/chrome; out of `make check`.
#
# Usage: dev/tests/nm.sh <wardd test-mode> <ward-nm test-mode> <chrome-parent example>

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
cd "$ROOT"

abs() { case "$1" in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$ROOT" "$1" ;; esac; }
WARDD="$(abs "${1:?uso: nm.sh <wardd> <ward-nm> <chrome-parent>}")"
NM="$(abs "${2:?uso: nm.sh <wardd> <ward-nm> <chrome-parent>}")"
PROBE="$(abs "${3:?uso: nm.sh <wardd> <ward-nm> <chrome-parent>}")"
for b in "$WARDD" "$NM" "$PROBE"; do [ -x "$b" ] || { echo "falta $b" >&2; exit 1; }; done
CHROME=/opt/google/chrome/chrome
[ -x "$CHROME" ] || { echo "falta o Chrome em $CHROME" >&2; exit 1; }
VAULT="$ROOT/.dev/test-vault.ward"
PATTERN_LIST="$ROOT/.dev/variations.txt"
[ -f "$VAULT" ] && [ ! -L "$VAULT" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$PATTERN_LIST" ] && [ ! -L "$PATTERN_LIST" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }

CURRENT_UID="$(id -u)"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$CURRENT_UID}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
WORK_DIR="$(mktemp -d "$ROOT/.dev/nm.XXXXXX")"
SOCKDIR="$RUNTIME/ward-nm-test-$$-${WORK_DIR##*.}"
PID=""
cleanup() {
    if [ -n "$PID" ] && kill -0 "$PID" 2>/dev/null; then
        kill -TERM "$PID" 2>/dev/null || true
        for _ in $(seq 1 50); do kill -0 "$PID" 2>/dev/null || break; sleep 0.1; done
        kill -KILL "$PID" 2>/dev/null || true
        wait "$PID" 2>/dev/null || true
    fi
    rm -rf -- "$WORK_DIR" "$SOCKDIR"
}
trap cleanup EXIT

cp -- "$VAULT" "$WORK_DIR/test-vault.ward"
printf 'ward-estado 1\nvista 0\nqueimada 0\n' > "$WORK_DIR/test-vault.state"
LOG="$WORK_DIR/wardd.log"
env -i \
    HOME="$WORK_DIR" \
    PATH=/usr/bin:/bin \
    XDG_RUNTIME_DIR="$RUNTIME" \
    LANG=C.UTF-8 \
    WARD_DEV_DIR="$WORK_DIR" \
    WARD_DEV_SOCKET_DIR="$SOCKDIR" \
    RUST_LOG=trace \
    "$WARDD" </dev/null >"$LOG" 2>&1 &
PID=$!
SOCKET="$SOCKDIR/wardd.sock"
for _ in $(seq 1 200); do
    [ -S "$SOCKET" ] && break
    kill -0 "$PID" 2>/dev/null || { echo "o wardd de teste não subiu:" >&2; tail -n 20 -- "$LOG" >&2; exit 1; }
    sleep 0.05
done
[ -S "$SOCKET" ] || { echo "o wardd de teste não criou o socket" >&2; exit 1; }
echo "wardd de teste só deste script: pid $PID, socket $SOCKET"

failures=0
/usr/bin/python3 -I -B "$ROOT/dev/tests/nm.py" "$ROOT" "$WORK_DIR" "$SOCKDIR" "$NM" "$PROBE" "$CHROME" "$PATTERN_LIST" \
    || failures=$((failures + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; failures=$((failures + 1)); }
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
for f in "$LOG" "$WORK_DIR/chrome.log"; do
    if [ -f "$f" ] && grep -qaF -f "$PATTERN_LIST" -- "$f"; then
        echo "FALHA padrão de .dev/variations.txt em ${f##*/}" >&2
        failures=$((failures + 1))
    else
        echo "ok    ${f##*/} sem padrão de .dev/variations.txt"
    fi
done
if grep -qa "preencher acme-wp-dev$" -- "$LOG"; then
    echo "FALHA o daemon atendeu um preencher do papel agente" >&2
    failures=$((failures + 1))
fi

if [ "$failures" -ne 0 ]; then
    echo "test-nm: $failures parte(s) com falha" >&2
    exit 1
fi
echo "test-nm: tudo certo"
