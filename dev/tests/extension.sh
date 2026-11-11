#!/bin/bash -p
# `make test-extension` (Phase 41; D5, D9, D47; V6.2, V6.4–V6.7): the MV3 extension of
# `extension/dist/` in the real Chrome (headless, temporary profile in `.dev/extension.*`),
# through the test `ward-nm`, against a test `wardd` of this script only with the
# extension variable set to `chrome` (the child of the installed Chrome gets the extension
# role without the egid of the `ward-nm` group; only in `test-mode`) and the test
# services (8081–8084).
#
# - V6.7: popup on `localhost:8081` with `acme-wp-dev` and `acme-wp-prod`; clicking
#   `acme-wp-prod` fills user and password; on `localhost:8083`, `acme-totp-dev` fills
#   user, password and TOTP and the login gets in. Popup measures checked against the
#   FigCraft nodes (17332, 17392) and screenshots in docs/mission/evidence/test-extension/.
# - V6.2: on `127.0.0.1:8081`, nothing suggested; forced fill: the daemon refuses.
# - V6.4: page of the right origin with a phishing iframe: only the main document.
# - V6.5/V6.6: nothing stored, a page that sends a message/postMessage/reads the popup
#   gets nothing; DOM clobbering; hidden password field; phishing (8082) and redir
#   (8084): nothing suggested, `posts.log` unchanged.
# - Logs of the daemon, of Chrome (stderr of `ward-nm`) and the extension console without
#   a pattern of `.dev/variations.txt` (V6.16 partial).
#
# Never touches the real Chrome profile, `~/.config/google-chrome` nor the daemon of
# `make dev-daemon`. Needs `make dev-vault`, `make dev-variations`,
# `make dev-services-up`, `make app-front` (fonts) and Chrome at
# /opt/google/chrome/chrome; out of `make check`.
#
# Phase 42 (V6.8, V6.9): submitting a new login on the sign-up page of `localhost:8083`
# offers to save (popup opened by the service worker), asks for type, group and
# environment and the entry shows up in the JSON list of the `ward` CLI with the use
# asking for approval; attacks: submit forged by the page script, login already saved,
# submit in an iframe of another origin, vault locked at submit, messages from the
# content script to the service worker; light system theme in the three popup states.
# Screenshots in docs/mission/evidence/test-extension/popup/.
#
# Phase 44c (D9): the popup fill on an `http` origin only after the origin is confirmed in
# the window; the window is the test one (the test approver binary, opened by the daemon
# through `WARD_DEV_APP`, which answers what its mode file in `$WORK_DIR` says): denied,
# nothing is filled and the popup warns; confirmed, it fills.
#
# Usage: dev/tests/extension.sh <wardd test-mode> <ward-nm test-mode> <fake-chrome> <ward test-mode>
#      <test approver>

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
cd "$ROOT"

abs() { case "$1" in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$ROOT" "$1" ;; esac; }
USAGE="uso: extension.sh <wardd> <ward-nm> <fake-chrome> <ward> <wardd-aprovador-teste>"
WARDD="$(abs "${1:?$USAGE}")"
NM="$(abs "${2:?$USAGE}")"
FAKE="$(abs "${3:?$USAGE}")"
WARD="$(abs "${4:?$USAGE}")"
APPROVER="$(abs "${5:?$USAGE}")"
for b in "$WARDD" "$NM" "$FAKE" "$WARD" "$APPROVER"; do [ -x "$b" ] || { echo "falta $b" >&2; exit 1; }; done
CHROME=/opt/google/chrome/chrome
[ -x "$CHROME" ] || { echo "falta o Chrome em $CHROME" >&2; exit 1; }
VAULT="$ROOT/.dev/test-vault.ward"
PATTERN_LIST="$ROOT/.dev/variations.txt"
EXT="$ROOT/extension/dist"
[ -f "$VAULT" ] && [ ! -L "$VAULT" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$PATTERN_LIST" ] && [ ! -L "$PATTERN_LIST" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }
[ -f "$EXT/manifest.json" ] || { echo "falta extension/dist: rode 'make extension'" >&2; exit 1; }
for port in 8081 8082 8083 8084; do
    /usr/bin/python3 -I -c "import socket,sys; socket.create_connection(('127.0.0.1', int(sys.argv[1])), 2).close()" "$port" 2>/dev/null \
        || { echo "serviço de teste na porta $port fora do ar: rode 'make dev-services-up'" >&2; exit 1; }
done

CURRENT_UID="$(id -u)"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$CURRENT_UID}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
WORK_DIR="$(mktemp -d "$ROOT/.dev/extension.XXXXXX")"
SOCKDIR="$RUNTIME/ward-extension-test-$$-${WORK_DIR##*.}"
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
# Phase 44c: the test window, inside the daemon's "repository" (the folder above
# WARD_DEV_DIR, D63); starts confirming the origin.
mkdir -m 0700 "$WORK_DIR/logs"
cp -- "$APPROVER" "$WORK_DIR/wardd-aprovador-teste"
printf 'confirmar\n' > "$WORK_DIR/aprovador.modo"
printf 'ward-estado 1\nvista 0\nqueimada 0\n' > "$WORK_DIR/test-vault.state"
LOG="$WORK_DIR/wardd.log"
env -i \
    HOME="$WORK_DIR" \
    PATH=/usr/bin:/bin \
    XDG_RUNTIME_DIR="$RUNTIME" \
    LANG=C.UTF-8 \
    WARD_DEV_DIR="$WORK_DIR" \
    WARD_DEV_SOCKET_DIR="$SOCKDIR" \
    WARD_DEV_EXTENSAO=chrome \
    WARD_DEV_APP="$WORK_DIR/wardd-aprovador-teste" \
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
echo "wardd de teste só deste script (WARD_DEV_EXTENSAO=chrome): pid $PID, socket $SOCKET"

failures=0
/usr/bin/python3 -I -B "$ROOT/dev/tests/extension.py" "$ROOT" "$WORK_DIR" "$SOCKDIR" "$NM" "$FAKE" "$CHROME" "$PATTERN_LIST" "$EXT" "$WARD" \
    || failures=$((failures + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; failures=$((failures + 1)); }
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
for f in "$LOG" "$WORK_DIR/chrome.log" "$WORK_DIR/console.log" "$WORK_DIR/logs/wardd-aprovador-teste.log"; do
    if [ ! -f "$f" ]; then
        echo "FALHA ${f##*/} não existe" >&2
        failures=$((failures + 1))
    elif grep -qaF -f "$PATTERN_LIST" -- "$f"; then
        echo "FALHA padrão de .dev/variations.txt em ${f##*/}" >&2
        failures=$((failures + 1))
    else
        echo "ok    ${f##*/} sem padrão de .dev/variations.txt"
    fi
done
# The daemon log does not carry the requested origin (S13): none of the origins used in the test.
if grep -qaE 'localhost:808[1-4]|127\.0\.0\.1:808[1-4]' -- "$LOG"; then
    echo "FALHA o log do daemon tem uma origem pedida" >&2
    failures=$((failures + 1))
else
    echo "ok    log do daemon sem as origens pedidas"
fi
grep -qa 'papel extensão de teste' -- "$LOG" \
    && echo "ok    o daemon de teste deu o papel extensão ao ward-nm filho do Chrome" \
    || { echo "FALHA o daemon não registrou o papel extensão de teste" >&2; failures=$((failures + 1)); }

if [ "$failures" -ne 0 ]; then
    echo "test-extension: $failures parte(s) com falha" >&2
    exit 1
fi
echo "test-extension: tudo certo"
