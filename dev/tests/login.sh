#!/bin/bash -p
# `make test-login` (Phase 43; R14, D9; V6.3, V6.10, V6.11, V6.15, V6.16): `ward login`
# end to end. The extension of `extension/dist/` in the real Chrome (headless, temporary
# profile in `.dev/login.*`), the test `ward-nm` opened by its service worker and a test
# `wardd` of this script only with the extension variable set to `chrome` (the child of
# the installed Chrome gets the extension role without the egid; only in `test-mode`).
# The test `ward` CLI talks to that daemon through `WARD_DEV_SOCKET_DIR`.
#
# - V6.10: tab on `localhost:8081/wp-login.php`, `ward login acme-wp-prod` exits 0 and the
#   tab reaches the dashboard; a page that refuses the login (served by CDP on the right
#   origin) and a page that does not navigate: the fields are empty after the submit and
#   the CLI exits ≠ 0.
# - V6.3: tab on the phishing (8082): exits ≠ 0 with the domain mismatch message,
#   `posts.log` unchanged and nothing filled.
# - V6.15: `delta-redir` with the tab on 8084 (which goes to 8082): the same.
# - Form attacks on the right origin: `action`/`formaction` to the phishing, `GET`,
#   `target`, two password fields, masked `action`: nothing sent outside.
# - V6.11 (daemon side): `acme-wp-dev` (with approval) denied in the window fills
#   nothing.
# - Phase 44c (D9): every `http` entry goes through the window, with the origin in view.
#   The window is the test one (the test approver binary, opened by the daemon through
#   `WARD_DEV_APP`, which answers what its mode file in `$WORK_DIR` says); the `[::1]`
#   attack of the M6 audit (`dev/tests/fake_v6.py` on `[::1]:8081`, WordPress only on
#   127.0.0.1) with the window denied takes nothing to the fake.
# - V6.16: logs of the daemon, of Chrome and of `ward-nm` and the extension console
#   without a pattern of `.dev/variations.txt`; the daemon log without the origins.
#
# Never touches the real Chrome profile, `~/.config/google-chrome` nor the daemon of
# `make dev-daemon`. Needs `make dev-vault`, `make dev-variations`,
# `make dev-services-up`, `make app-front` (fonts) and Chrome at
# /opt/google/chrome/chrome; out of `make check`.
#
# Usage: dev/tests/login.sh <wardd test-mode> <ward-nm test-mode> <ward test-mode>
#      <test approver>

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
cd "$ROOT"

abs() { case "$1" in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$ROOT" "$1" ;; esac; }
USAGE="uso: login.sh <wardd> <ward-nm> <ward> <wardd-aprovador-teste>"
WARDD="$(abs "${1:?$USAGE}")"
NM="$(abs "${2:?$USAGE}")"
WARD="$(abs "${3:?$USAGE}")"
APPROVER="$(abs "${4:?$USAGE}")"
for b in "$WARDD" "$NM" "$WARD" "$APPROVER"; do [ -x "$b" ] || { echo "falta $b" >&2; exit 1; }; done
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
WORK_DIR="$(mktemp -d "$ROOT/.dev/login.XXXXXX")"
SOCKDIR="$RUNTIME/ward-login-test-$$-${WORK_DIR##*.}"
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
printf 'ward-state 2\nseen 0\nburned 0\nagent_failures 0\n' > "$WORK_DIR/test-vault.state"
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
/usr/bin/python3 -I -B "$ROOT/dev/tests/login.py" "$ROOT" "$WORK_DIR" "$SOCKDIR" "$NM" "$CHROME" "$PATTERN_LIST" "$EXT" "$WARD" \
    || failures=$((failures + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; failures=$((failures + 1)); }
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
for f in "$LOG" "$WORK_DIR/chrome.log" "$WORK_DIR/console.log" "$WORK_DIR/cli.log" "$WORK_DIR/logs/wardd-aprovador-teste.log"; do
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
# The daemon log does not carry the tab origin (S13): none of the origins used in the test.
if grep -qaE 'localhost:808[1-4]|127\.0\.0\.1:808[1-4]' -- "$LOG"; then
    echo "FALHA o log do daemon tem uma origem" >&2
    failures=$((failures + 1))
else
    echo "ok    log do daemon sem as origens"
fi
grep -qa 'login acme-wp-prod: formulário enviado' -- "$LOG" \
    && echo "ok    o daemon registrou o login enviado (só o nome da entrada)" \
    || { echo "FALHA o daemon não registrou o login enviado" >&2; failures=$((failures + 1)); }

if [ "$failures" -ne 0 ]; then
    echo "test-login: $failures parte(s) com falha" >&2
    exit 1
fi
echo "test-login: tudo certo"
