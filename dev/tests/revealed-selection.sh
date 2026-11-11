#!/bin/bash -p
# `make test-revealed-selection` (Phase 39g; S2, S12; 2nd audit of M5).
#
# The value revealed in the secret field does not go to PRIMARY nor to CLIPBOARD by
# selection, Ctrl+A, Ctrl+C, drag or triple click, in the real WebKitGTK: starts a test
# `wardd` of this test only (socket in WARD_DEV_SOCKET_DIR, default: a folder of its own
# in `/run/user/<uid>/`; never the fixed socket of another checkout) with
# WARD_DEV_APP = the bridge, asks the daemon to open the app (dev/bridge.sh) and runs
# `dev/tests/revealed_selection.py`, which opens the test interface in a WebKitGTK 4.1
# inside a private Xwayland (the clipboard of the user's session is neither read nor
# written). At the end, the log scan (V5.27) of .dev/logs, and the test vault goes back
# to what it was.
#
# Needs: `make dev-vault`, `make dev-variations`, `make app-front`, Xwayland, xauth,
# python3-gi with WebKit2 4.1 and the Wayland session (an Xwayland window shows up for
# about 20 s). No test daemon of this repository may be running
# (`make dev-daemon-stop`). Out of `make check`.
# Usage: dev/tests/revealed-selection.sh

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$ROOT/dev/env.sh"

DEV="$ROOT/.dev"
for f in "$DEV/test-vault.ward" "$DEV/variations.txt" "$ROOT/app/dist-teste/index.html" \
    "$ROOT/target/debug/wardd" "$ROOT/target/debug/ward-test-bridge"; do
    [[ -f "$f" ]] || { echo "selecao-revelado: falta ${f#"$ROOT"/} (make dev-vault, dev-variations, app-front)" >&2; exit 1; }
done

UID_="$(id -u)"
SOCKDIR="${WARD_DEV_SOCKET_DIR:-/run/user/$UID_/ward-test-selection}"
[[ "$SOCKDIR" == /run/user/"$UID_"/* && "$SOCKDIR" != */../* ]] \
    || { echo "selecao-revelado: WARD_DEV_SOCKET_DIR fora de /run/user/$UID_/" >&2; exit 1; }
[[ "$SOCKDIR" != "/run/user/$UID_/ward-test" ]] \
    || { echo "selecao-revelado: use uma pasta própria em WARD_DEV_SOCKET_DIR (a ward-test é de todos os checkouts)" >&2; exit 1; }
if [[ -f "$DEV/wardd.pid" ]] && p="$(head -c 16 "$DEV/wardd.pid")" && [[ "$p" =~ ^[0-9]+$ ]] \
    && [[ "$(cat "/proc/$p/comm" 2> /dev/null)" == wardd ]]; then
    echo "selecao-revelado: o daemon de teste deste repositório já está no ar (pid $p); rode 'make dev-daemon-stop'" >&2
    exit 1
fi

umask 077
BACKUP="$DEV/selection-backup"
rm -rf "$BACKUP"
mkdir -m 0700 "$BACKUP"
cp -p "$DEV/test-vault.ward" "$BACKUP/"
[[ -f "$DEV/test-vault.state" ]] && cp -p "$DEV/test-vault.state" "$BACKUP/"

cleanup() {
    WARD_DEV_SOCKET_DIR="$SOCKDIR" "$ROOT/dev/daemon.sh" stop > /dev/null 2>&1 || true
    cp -p "$BACKUP/test-vault.ward" "$DEV/test-vault.ward"
    if [[ -f "$BACKUP/test-vault.state" ]]; then
        cp -p "$BACKUP/test-vault.state" "$DEV/test-vault.state"
    else
        rm -f "$DEV/test-vault.state"
    fi
    rm -rf "$BACKUP" "$DEV/revealed-selection"
}
trap cleanup EXIT

echo "==> daemon de teste em $SOCKDIR com a ponte (D63)"
WARD_DEV_SOCKET_DIR="$SOCKDIR" WARD_DEV_APP="$ROOT/target/debug/ward-test-bridge" "$ROOT/dev/daemon.sh" up
URL="$(WARD_DEV_SOCKET_DIR="$SOCKDIR" "$ROOT/dev/bridge.sh")"

echo "==> valor revelado contra seleção e cópia (WebKitGTK num X privado)"
set +e
printf '%s\n' "$URL" | /usr/bin/python3 -I -B "$ROOT/dev/tests/revealed_selection.py"
qa=$?
set -e
unset URL

echo "==> varredura de logs (V5.27): .dev/logs com .dev/variations.txt"
WARD_DEV_SOCKET_DIR="$SOCKDIR" "$ROOT/dev/daemon.sh" stop > /dev/null 2>&1 || true
/usr/bin/python3 -I -B "$ROOT/dev/tests/log-scan.py" "$DEV/logs"
exit "$qa"
