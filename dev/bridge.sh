#!/bin/bash -p
# `make app-bridge` (Phase 23a, D63): the test bridge opened by the test daemon.
#
# `make` already started the test daemon with WARD_DEV_APP = the bridge (dev/daemon.sh).
# Here: asks for `abrir_app` (dev/open-app.py, no field) and waits for the URL the bridge
# prints in its log (.dev/logs/ward-test-bridge.log, 0600). The bridge is a child of the
# daemon, gets the app role (the launcher's pid), checks the role on the first reply and
# exits when the daemon stops. Run by hand, it gets the agent role and exits.
#
# The URL carries the bridge token: it goes only to this command's output. Any process
# of the uid that reads the log sees it too (residue of D63, only with the test vault).

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
. "$ROOT/dev/env.sh"

LOG="$ROOT/.dev/logs/ward-test-bridge.log"
# WARD_DEV_SOCKET_DIR (Phase 39c): the test daemon in another folder (dev/daemon.sh); the
# daemon passes the same folder to the bridge it opens.
case "${WARD_DEV_SOCKET_DIR:-}" in
    "") SOCKET="/run/user/$(id -u)/ward-teste/wardd.sock" ;;
    /*) SOCKET="$WARD_DEV_SOCKET_DIR/wardd.sock" ;;
    *) echo "WARD_DEV_SOCKET_DIR não é absoluto" >&2; exit 1 ;;
esac
URL_RE='^http://127\.0\.0\.1:[0-9]{1,5}/#token=[0-9a-f]{64}$'

before=0
[ -f "$LOG" ] && before="$(stat -c %s -- "$LOG")"

/usr/bin/python3 -I -B "$ROOT/dev/open-app.py" "$SOCKET" >/dev/null

new_output() { tail -c "+$((before + 1))" -- "$LOG" 2>/dev/null || true; }

for _ in $(seq 1 150); do
    url="$(new_output | grep -E -- "$URL_RE" | tail -n 1 || true)"
    if [ -n "$url" ]; then
        echo "$url"
        exit 0
    fi
    if new_output | grep -q -- '^ward-ponte-teste:'; then
        echo "a ponte não subiu; saída dela:" >&2
        new_output | grep -- '^ward-ponte-teste:' >&2
        exit 1
    fi
    sleep 0.1
done

# No new URL: the bridge was already open (one at a time, D57); the last one in the log holds.
url="$(grep -E -- "$URL_RE" "$LOG" 2>/dev/null | tail -n 1 || true)"
if [ -n "$url" ]; then
    echo "$url"
    echo "(a ponte já estava aberta)" >&2
    exit 0
fi
echo "a ponte não imprimiu a URL; veja .dev/logs/wardd.log e $LOG" >&2
exit 1
