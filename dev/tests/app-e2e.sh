#!/bin/bash -p
# `make test-app-e2e` (Phase 39c; V5.10, V5.16, V5.24; D2, D3, D63, S10).
#
# The app end to end with the test daemon and the bridge, without the native window:
# starts a test `wardd` of this test only (socket in WARD_DEV_SOCKET_DIR, default: a
# folder of its own in `/run/user/<uid>/`; never the fixed socket of another checkout)
# with WARD_DEV_APP = the bridge, asks the daemon to open the app (dev/bridge.sh) and runs
# `dev/tests/app_e2e.py` (Python Playwright, headless Chrome) with the real test CLI:
# `ward run` with the approval window (deny, close, once, 1 hour with lock and unlock,
# always) and the Connect button with `ward ssh --token-fd` against the jump server (2224),
# plus the attacks on the token. At the end, the log scan (V5.27) of .dev/logs, and the
# test vault goes back to what it was before (the "always" changes it).
#
# Needs: `make dev-vault`, `make dev-variations`, `make app-front`, the test services up
# (`make dev-services-up`), Chrome in /opt/google/chrome and Python Playwright in
# PW_PYTHONPATH. No test daemon of this repository may be running
# (`make dev-daemon-stop`).
# Usage: PW_PYTHONPATH=<site-packages> dev/tests/app-e2e.sh

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$ROOT/dev/env.sh"

PW="${PW_PYTHONPATH:-}"
[[ "$PW" == /* && -d "$PW/playwright" ]] || { echo "app-e2e: PW_PYTHONPATH=<site-packages com o playwright> (absoluto)" >&2; exit 1; }
[[ -x /opt/google/chrome/chrome ]] || { echo "app-e2e: falta o Chrome em /opt/google/chrome" >&2; exit 1; }
DEV="$ROOT/.dev"
for f in "$DEV/test-vault.ward" "$DEV/variations.txt" "$ROOT/app/dist-teste/index.html" \
    "$ROOT/target/debug/wardd" "$ROOT/target/debug/ward" "$ROOT/target/debug/ward-test-bridge"; do
    [[ -f "$f" ]] || { echo "app-e2e: falta ${f#"$ROOT"/} (make dev-vault, dev-variations, app-front)" >&2; exit 1; }
done
(exec 3<>/dev/tcp/127.0.0.1/2224) 2> /dev/null \
    || { echo "app-e2e: o ssh-salto (2224) não responde; rode 'make dev-services-up'" >&2; exit 1; }

UID_="$(id -u)"
SOCKDIR="${WARD_DEV_SOCKET_DIR:-/run/user/$UID_/ward-teste-e2e}"
[[ "$SOCKDIR" == /run/user/"$UID_"/* && "$SOCKDIR" != */../* ]] \
    || { echo "app-e2e: WARD_DEV_SOCKET_DIR fora de /run/user/$UID_/" >&2; exit 1; }
[[ "$SOCKDIR" != "/run/user/$UID_/ward-teste" ]] \
    || { echo "app-e2e: use uma pasta própria em WARD_DEV_SOCKET_DIR (a ward-teste é de todos os checkouts)" >&2; exit 1; }
if [[ -f "$DEV/wardd.pid" ]] && p="$(head -c 16 "$DEV/wardd.pid")" && [[ "$p" =~ ^[0-9]+$ ]] \
    && [[ "$(cat "/proc/$p/comm" 2> /dev/null)" == wardd ]]; then
    echo "app-e2e: o daemon de teste deste repositório já está no ar (pid $p); rode 'make dev-daemon-stop'" >&2
    exit 1
fi

# The test vault goes back to what it was ("always allow" changes the level of an entry).
umask 077
BACKUP="$DEV/e2e-backup"
rm -rf "$BACKUP"
mkdir -m 0700 "$BACKUP"
cp -p "$DEV/test-vault.ward" "$BACKUP/"
[[ -f "$DEV/test-vault.state" ]] && cp -p "$DEV/test-vault.state" "$BACKUP/"
rm -f "$DEV/logs/terminal-teste.log" "$DEV/e2e-mark" "$DEV/terminal-teste.modo"

cleanup() {
    WARD_DEV_SOCKET_DIR="$SOCKDIR" "$ROOT/dev/daemon.sh" stop > /dev/null 2>&1 || true
    cp -p "$BACKUP/test-vault.ward" "$DEV/test-vault.ward"
    if [[ -f "$BACKUP/test-vault.state" ]]; then
        cp -p "$BACKUP/test-vault.state" "$DEV/test-vault.state"
    else
        rm -f "$DEV/test-vault.state"
    fi
    rm -rf "$BACKUP"
    rm -f "$DEV/e2e-mark" "$DEV/terminal-teste.modo"
}
trap cleanup EXIT

echo "==> daemon de teste em $SOCKDIR com a ponte (D63)"
WARD_DEV_SOCKET_DIR="$SOCKDIR" WARD_DEV_APP="$ROOT/target/debug/ward-test-bridge" "$ROOT/dev/daemon.sh" up
URL="$(WARD_DEV_SOCKET_DIR="$SOCKDIR" "$ROOT/dev/bridge.sh")"

echo "==> app de ponta a ponta (Chrome headless)"
set +e
printf '%s\n' "$URL" | /usr/bin/python3 -I -B "$ROOT/dev/tests/app_e2e.py" "$ROOT" "$SOCKDIR" "$PW"
qa=$?
set -e
unset URL

echo "==> varredura de logs (V5.27): .dev/logs com .dev/variations.txt"
WARD_DEV_SOCKET_DIR="$SOCKDIR" "$ROOT/dev/daemon.sh" stop > /dev/null 2>&1 || true
/usr/bin/python3 -I -B "$ROOT/dev/tests/log-scan.py" "$DEV/logs"
exit "$qa"
