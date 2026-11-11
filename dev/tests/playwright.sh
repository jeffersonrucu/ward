#!/bin/bash -p
# `make test-playwright` (Phase 44): `ward playwright` end to end, with the installed
# Chrome and the test services (WordPress 8081, phish 8082, redir 8084).
#
# - V6.12: `ward playwright acme-wp-prod` prints `<rt>playwright/acme-wp-prod.json`
#   (0600) and a Playwright test in Python with that `storage_state` opens `/wp-admin/`
#   logged in (the Python Playwright comes from PW_PYTHONPATH, outside the repository;
#   without it the test fails, except with NO_PLAYWRIGHT_PYTHON=1, which skips only that
#   check).
# - V6.13: no pattern of `.dev/variations.txt` in the storage state nor in the temporary
#   profile (read every 50 ms while Chrome runs); the profile is gone at the end and
#   `ward travar` deletes the storage states.
# - V6.14: while Chrome runs, none of its processes listens on a TCP port
#   (`/proc/net/tcp*` and `ss -ltnp`), the argv has `--remote-debugging-pipe` and no
#   secret, and the debugging pipe is only in the main Chrome.
# - V6.15 (`ward playwright` part): `delta-redir` (8084 → 8082) refuses by origin, and
#   the phish `posts.log` gets no line nor vault pattern.
# - V6.16 (daemon part): the daemon log (`RUST_LOG=trace`) without a pattern of
#   `.dev/variations.txt`.
#
# Never brings down the test daemon of another process: starts its own with a copy of the
# test vault in its own folder in `.dev/` and the socket in its own folder in
# `$XDG_RUNTIME_DIR` (`WARD_DEV_SOCKET_DIR`), and only kills the pid it created.
# Needs `make dev-vault`, `make dev-variations` and the services up.
#
# Phase 44c (D9): every `http` entry goes through the window (only the origin
# confirmation, at the free level). The window is the test one (`wardd-aprovador-teste`,
# opened by the daemon through `WARD_DEV_APP`, which answers what is in
# `$WORK_DIR/aprovador.modo`). The `[::1]` attack of the M6 audit (`dev/tests/fake_v6.py`
# on `[::1]:8081`, WordPress only on 127.0.0.1): denied, Chrome does not even open;
# confirmed, `--host-resolver-rules` takes Chrome to 127.0.0.1 and the fake gets nothing.
#
# Usage: dev/tests/playwright.sh <wardd with test-mode> <ward with test-mode>
#      <wardd-aprovador-teste>

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
cd "$ROOT"

WARDD="${1:?uso: playwright.sh <wardd> <ward> <wardd-aprovador-teste>}"
WARD="${2:?uso: playwright.sh <wardd> <ward> <wardd-aprovador-teste>}"
APPROVER="${3:?uso: playwright.sh <wardd> <ward> <wardd-aprovador-teste>}"
case "$WARDD" in /*) ;; *) WARDD="$ROOT/$WARDD" ;; esac
case "$WARD" in /*) ;; *) WARD="$ROOT/$WARD" ;; esac
case "$APPROVER" in /*) ;; *) APPROVER="$ROOT/$APPROVER" ;; esac
[ -x "$WARDD" ] && [ -x "$WARD" ] && [ -x "$APPROVER" ] || { echo "falta $WARDD, $WARD ou $APPROVER" >&2; exit 1; }
VAULT="$ROOT/.dev/test-vault.ward"
PATTERN_LIST="$ROOT/.dev/variations.txt"
[ -f "$VAULT" ] && [ ! -L "$VAULT" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$PATTERN_LIST" ] && [ ! -L "$PATTERN_LIST" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }
[ -x /opt/google/chrome/chrome ] || { echo "falta o Chrome em /opt/google/chrome/chrome" >&2; exit 1; }

CURRENT_UID="$(id -u)"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$CURRENT_UID}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
WORK_DIR="$(mktemp -d "$ROOT/.dev/playwright.XXXXXX")"
# Short: Chrome creates a Unix socket inside the profile (`<rt>pw-*/t/…`), with at most
# 107 bytes of path.
SOCKDIR="$RUNTIME/ward-pw-$$"
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
echo "wardd de teste só deste script: pid $PID, socket $SOCKET"

failures=0
/usr/bin/python3 -I -B "$ROOT/dev/tests/playwright.py" "$WARD" "$ROOT" "$SOCKDIR" "$WORK_DIR" \
    || failures=$((failures + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; failures=$((failures + 1)); }
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
if grep -qaF -f "$PATTERN_LIST" -- "$LOG"; then
    echo "FALHA padrão de .dev/variations.txt no log do daemon" >&2
    failures=$((failures + 1))
else
    echo "ok    V6.16 log do daemon ($(wc -l < "$LOG") linhas) sem padrão de .dev/variations.txt"
fi
if [ "$failures" -ne 0 ]; then
    # The log was already checked for secrets above; its tail helps to find the failure.
    tail -n 30 -- "$LOG" >&2
    echo "test-playwright: falhou" >&2
    exit 1
fi
echo "test-playwright: ok"
