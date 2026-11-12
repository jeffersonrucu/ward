#!/bin/bash -p
# `make test-socket-attacks` (Phase 24a, M3 scrutiny): V3.3 and V3.4 through the socket
# of a test `wardd` that only this script uses.
#
# - V3.4 (`socket_attack.py`): 2 MiB frames, negative size, invalid JSON, unknown
#   operation, cut frame and 10 000 connections opened without talking; the daemon
#   refuses or closes each one and keeps answering a normal client.
# - V3.3 (`socket_matrix.py`): operation × entry × field matrix as agent; no reply with a
#   pattern of `.dev/variations.txt`, app and extension operations answer
#   "not authorized".
# - The daemon log (`RUST_LOG=trace`) without a pattern of `.dev/variations.txt`.
#
# Never brings down the test daemon of another process: starts its own with a copy of the
# test vault in its own folder in `.dev/` (`socket-attacks.XXXXXX`) and the socket in its
# own folder in `$XDG_RUNTIME_DIR` (`WARD_DEV_SOCKET_DIR`), and only kills the pid it
# created. Needs `make dev-vault` and `make dev-variations`. Stays out of `make check`
# (the 10 000 connections); the short version (70 connections) is in the `wardd`
# `cargo test`.
#
# Usage: dev/tests/socket-attacks.sh <wardd with test-mode> [connections]

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
cd "$ROOT"

WARDD="${1:?uso: socket-attacks.sh <wardd> [conexões]}"
CONNECTIONS="${2:-10000}"
[[ "$CONNECTIONS" =~ ^[1-9][0-9]{0,5}$ ]] || { echo "conexões inválido" >&2; exit 2; }
case "$WARDD" in /*) ;; *) WARDD="$ROOT/$WARDD" ;; esac
[ -x "$WARDD" ] || { echo "falta $WARDD" >&2; exit 1; }
VAULT="$ROOT/.dev/test-vault.ward"
PATTERN_LIST="$ROOT/.dev/variations.txt"
[ -f "$VAULT" ] && [ ! -L "$VAULT" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$PATTERN_LIST" ] && [ ! -L "$PATTERN_LIST" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }

CURRENT_UID="$(id -u)"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$CURRENT_UID}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
WORK_DIR="$(mktemp -d "$ROOT/.dev/socket-attacks.XXXXXX")"
# The socket folder must not exist before: the daemon creates it 0700 (the name is unique).
SOCKDIR="$RUNTIME/ward-socket-attacks-$$-${WORK_DIR##*.}"
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
# New state (D39): generation 0, goes up on the first unlock (only in the test vault).
printf 'ward-state 2\nseen 0\nburned 0\nagent_failures 0\n' > "$WORK_DIR/test-vault.state"
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
echo "==> V3.4: quadros ruins e $CONNECTIONS conexões caladas"
/usr/bin/python3 -I -B "$ROOT/dev/tests/socket_attack.py" "$SOCKET" "$PID" "$CONNECTIONS" || failures=$((failures + 1))
echo "==> V3.3: matriz operação × entrada × campo (agente)"
/usr/bin/python3 -I -B "$ROOT/dev/tests/socket_matrix.py" "$SOCKET" "$ROOT" "$PATTERN_LIST" || failures=$((failures + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; failures=$((failures + 1)); }
echo "==> log do daemon (RUST_LOG=trace) sem segredo"
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
if grep -qaF -f "$PATTERN_LIST" -- "$LOG"; then
    echo "FALHA padrão de .dev/variations.txt no log do daemon" >&2
    failures=$((failures + 1))
else
    echo "ok    log do daemon ($(wc -l < "$LOG") linhas) sem padrão de .dev/variations.txt"
fi

if [ "$failures" -ne 0 ]; then
    echo "test-socket-attacks: $failures parte(s) com falha" >&2
    exit 1
fi
echo "test-socket-attacks: V3.3 e V3.4 ok"
