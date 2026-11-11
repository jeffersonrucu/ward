#!/bin/bash -p
# `make dev-daemon` / `make dev-daemon-stop` (Phase 22; D7, V3.30).
#
# Starts the TEST `wardd` (feature `test-mode`) with the test vault of `.dev/` and the
# socket `wardd.sock` in the test socket folder under `$XDG_RUNTIME_DIR`. Never the real
# vault.
#
# Clean environment (V3.30): `wardd` refuses to start with `TMPDIR` or `LD_*` in the
# environment (V3.25), and the developer's session usually has both. So it starts through
# `env -i` with a fixed list: HOME, PATH, XDG_RUNTIME_DIR, LANG, WARD_DEV_DIR and RUST_LOG
# (default `trace`), plus the system-bus variable (`WARD_DEV_DBUS_*`) only if set
# (Phase 24: socket of the bus the test daemon watches for logind; without it, the test
# daemon does not turn logind on. For V3.19 with the real `loginctl lock-session`, set it
# to /run/dbus/system_bus_socket; that locks the user's screen).
#
# WARD_DEV_APP (Phase 23a, D63): absolute path of the binary the test daemon opens on the
# open-app request (the `ward-test-bridge` or the test `ward-app`), inside the
# repository; the daemon checks the rest and refuses to start outside the rule. Only that
# process, child of the daemon, gets the app role. The value used stays in .dev/wardd.app:
# with the daemon already up with another WARD_DEV_APP (or none), `up` fails instead of
# pretending it switched.
#
# The extension variable (`WARD_DEV_EXT*`) set to `chrome` (Phase 41): only if set; the
# test daemon gives the extension role to the test `ward-nm` child of the installed
# Chrome (parent checked through `/proc`), without the egid of the `ward-nm` group, for
# the extension QA in the test Chrome. Any other value: the daemon refuses to start.
#
# WARD_DEV_SOCKET_DIR (Phase 27a): 0700 socket folder in place of the test socket folder
# under `$XDG_RUNTIME_DIR` (absolute), to start the daemon of a worktree next to the
# test daemon of another checkout without bringing it down. It reaches the daemon through
# `env -i`; the test CLI and `dev/tests/cli.py` use the same variable.
#
# Usage: dev/daemon.sh up | stop | status

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
DEV="$ROOT/.dev"
BIN="$ROOT/target/debug/wardd"
VAULT="$DEV/test-vault.ward"
STATE="$DEV/test-vault.state"
LOGS="$DEV/logs"
LOG="$LOGS/wardd.log"
PID="$DEV/wardd.pid"
APP_MARK="$DEV/wardd.app"
APP="${WARD_DEV_APP:-}"
case "$APP" in
    ""|/*) ;;
    *) echo "WARD_DEV_APP não é absoluto" >&2; exit 1 ;;
esac

# Log of the test daemon: `trace` by default (V4.33: the suite checks that not even then
# does a secret reach the log). Only the known levels; the daemon never prints the value.
LOG_LEVEL="${RUST_LOG:-trace}"
case "$LOG_LEVEL" in
    error|warn|info|debug|trace) ;;
    *) echo "RUST_LOG fora de error|warn|info|debug|trace" >&2; exit 1 ;;
esac

CURRENT_UID="$(id -u)"
PASSWD_HOME="$(getent passwd "$CURRENT_UID" | cut -d: -f6)"
[ -n "$PASSWD_HOME" ] || { echo "home do passwd indefinido" >&2; exit 1; }
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$CURRENT_UID}"
case "$RUNTIME" in
    /*) ;;
    *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;;
esac
SOCKET_DIR="${WARD_DEV_SOCKET_DIR:-}"
case "$SOCKET_DIR" in
    "") ;;
    /*) ;;
    *) echo "WARD_DEV_SOCKET_DIR não é absoluto" >&2; exit 1 ;;
esac
SOCKET="${SOCKET_DIR:-$RUNTIME/ward-teste}/wardd.sock"

# PID of the test daemon that is up, if it really is it (the binary of this repository):
# an old PID reused by another program is neither killed nor taken for the daemon.
live_pid() {
    [ -f "$PID" ] || return 1
    local p
    p="$(head -c 16 -- "$PID")"
    [[ "$p" =~ ^[1-9][0-9]*$ ]] || return 1
    [ "$(cat "/proc/$p/comm" 2>/dev/null)" = "wardd" ] || return 1
    # wardd marks itself non-dumpable (V3.7): `/proc/<pid>/exe` belongs to root and
    # cannot be read; the first argument of `cmdline` can.
    local arg0
    arg0="$(tr '\0' '\n' < "/proc/$p/cmdline" 2>/dev/null | head -n 1)"
    [ "$arg0" = "$BIN" ] || return 1
    echo "$p"
}

start() {
    [ -x "$BIN" ] || { echo "falta $BIN (o make compila antes)" >&2; exit 1; }
    [ -f "$VAULT" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
    if p="$(live_pid)"; then
        local current=""
        [ -f "$APP_MARK" ] && current="$(head -n 1 -- "$APP_MARK")"
        if [ "$current" != "$APP" ]; then
            echo "o daemon de teste já está no ar (pid $p) com outro WARD_DEV_APP; rode 'make dev-daemon-stop' antes" >&2
            exit 1
        fi
        echo "o daemon de teste já está no ar (pid $p, socket $SOCKET)"
        return 0
    fi
    umask 077
    mkdir -p -- "$LOGS"
    # First use of the test vault (or just recreated by `make dev-vault`, which
    # deletes the old state): without a state, the daemon refuses to open (D39). The test
    # one starts at 0 and goes up to the file's generation on the first unlock. Only here,
    # only for the test vault: the real daemon never creates that file by itself.
    if [ ! -e "$STATE" ]; then
        printf 'ward-estado 1\nvista 0\nqueimada 0\n' > "$STATE"
    fi
    env -i \
        HOME="$PASSWD_HOME" \
        PATH=/usr/bin:/bin \
        XDG_RUNTIME_DIR="$RUNTIME" \
        LANG="${LANG:-C.UTF-8}" \
        WARD_DEV_DIR="$DEV" \
        RUST_LOG="$LOG_LEVEL" \
        ${WARD_DEV_DBUS_SISTEMA:+"WARD_DEV_DBUS_SISTEMA=$WARD_DEV_DBUS_SISTEMA"} \
        ${SOCKET_DIR:+"WARD_DEV_SOCKET_DIR=$SOCKET_DIR"} \
        ${APP:+"WARD_DEV_APP=$APP"} \
        ${WARD_DEV_EXTENSAO:+"WARD_DEV_EXTENSAO=$WARD_DEV_EXTENSAO"} \
        "$BIN" </dev/null >>"$LOG" 2>&1 &
    local p=$!
    echo "$p" > "$PID"
    # 1st line: the WARD_DEV_APP; 2nd: the socket (`make test-cli` checks both).
    printf '%s\n%s\n' "$APP" "$SOCKET" > "$APP_MARK"
    for _ in $(seq 1 100); do
        if [ -S "$SOCKET" ] && live_pid >/dev/null; then
            echo "daemon de teste no ar: pid $p, socket $SOCKET, log $LOG"
            return 0
        fi
        if ! kill -0 "$p" 2>/dev/null; then
            break
        fi
        sleep 0.1
    done
    echo "o daemon de teste não subiu; fim do log ($LOG):" >&2
    tail -n 20 -- "$LOG" >&2 || true
    rm -f -- "$PID" "$APP_MARK"
    exit 1
}

stop_daemon() {
    local p
    if ! p="$(live_pid)"; then
        rm -f -- "$PID" "$APP_MARK"
        echo "o daemon de teste não está no ar"
        return 0
    fi
    kill -TERM "$p"
    for _ in $(seq 1 50); do
        kill -0 "$p" 2>/dev/null || break
        sleep 0.1
    done
    if kill -0 "$p" 2>/dev/null; then
        kill -KILL "$p"
    fi
    rm -f -- "$PID" "$APP_MARK"
    echo "daemon de teste parado"
}

case "${1:-}" in
    up) start ;;
    stop) stop_daemon ;;
    status)
        if p="$(live_pid)"; then echo "no ar (pid $p)"; else echo "parado"; fi
        ;;
    *) echo "uso: dev/daemon.sh up|stop|status" >&2; exit 2 ;;
esac
