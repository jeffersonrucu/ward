#!/bin/bash -p
# `make dev-daemon` / `make dev-daemon-parar` (Fase 22; D7, V3.30).
#
# Sobe o `wardd` de TESTE (feature `modo-teste`) com o cofre de teste de `.dev/` e o
# socket em `$XDG_RUNTIME_DIR/ward-teste/wardd.sock`. Nunca o cofre real.
#
# Ambiente limpo (V3.30): o `wardd` recusa subir com `TMPDIR` ou `LD_*` no ambiente
# (V3.25), e a sessão do desenvolvedor costuma ter os dois. Por isso ele sobe por
# `env -i` com uma lista fixa: HOME, PATH, XDG_RUNTIME_DIR, LANG e WARD_DEV_DIR, mais
# WARD_DEV_DBUS_SISTEMA só se vier definida (Fase 24: socket do barramento que o daemon
# de teste vigia para o logind; sem ela, o daemon de teste não liga o logind. Para o
# V3.19 com o `loginctl lock-session` de verdade, passe
# WARD_DEV_DBUS_SISTEMA=/run/dbus/system_bus_socket; isso trava a tela do usuário).
#
# WARD_DEV_APP (Fase 23a, D63): caminho absoluto do binário que o `abrir_app` do daemon
# de teste abre (a `ward-ponte-teste` ou o `ward-app` de teste), dentro do repositório;
# o daemon confere o resto e recusa subir fora da regra. Só esse processo, filho do
# daemon, recebe o papel app. O valor usado fica em .dev/wardd.app: com o daemon já no
# ar com outro WARD_DEV_APP (ou sem), o `subir` falha em vez de fingir que trocou.
#
# Uso: dev/daemon.sh subir | parar | estado

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
DEV="$RAIZ/.dev"
BIN="$RAIZ/target/debug/wardd"
COFRE="$DEV/cofre-teste.ward"
ESTADO="$DEV/cofre-teste.estado"
LOGS="$DEV/logs"
LOG="$LOGS/wardd.log"
PID="$DEV/wardd.pid"
MARCA_APP="$DEV/wardd.app"
APP="${WARD_DEV_APP:-}"
case "$APP" in
    ""|/*) ;;
    *) echo "WARD_DEV_APP não é absoluto" >&2; exit 1 ;;
esac

UID_ATUAL="$(id -u)"
CASA="$(getent passwd "$UID_ATUAL" | cut -d: -f6)"
[ -n "$CASA" ] || { echo "home do passwd indefinido" >&2; exit 1; }
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$UID_ATUAL}"
case "$RUNTIME" in
    /*) ;;
    *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;;
esac
SOCKET="$RUNTIME/ward-teste/wardd.sock"

# PID do daemon de teste que está no ar, se for mesmo ele (o binário deste repositório):
# um PID velho reaproveitado por outro programa não é morto nem tomado por daemon.
pid_vivo() {
    [ -f "$PID" ] || return 1
    local p
    p="$(head -c 16 -- "$PID")"
    [[ "$p" =~ ^[1-9][0-9]*$ ]] || return 1
    [ "$(cat "/proc/$p/comm" 2>/dev/null)" = "wardd" ] || return 1
    # O wardd se marca não-dumpable (V3.7): `/proc/<pid>/exe` fica do root e não dá
    # para ler; o primeiro argumento do `cmdline` dá.
    local arg0
    arg0="$(tr '\0' '\n' < "/proc/$p/cmdline" 2>/dev/null | head -n 1)"
    [ "$arg0" = "$BIN" ] || return 1
    echo "$p"
}

subir() {
    [ -x "$BIN" ] || { echo "falta $BIN (o make compila antes)" >&2; exit 1; }
    [ -f "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-cofre'" >&2; exit 1; }
    if p="$(pid_vivo)"; then
        local atual=""
        [ -f "$MARCA_APP" ] && atual="$(head -n 1 -- "$MARCA_APP")"
        if [ "$atual" != "$APP" ]; then
            echo "o daemon de teste já está no ar (pid $p) com outro WARD_DEV_APP; rode 'make dev-daemon-parar' antes" >&2
            exit 1
        fi
        echo "o daemon de teste já está no ar (pid $p, socket $SOCKET)"
        return 0
    fi
    umask 077
    mkdir -p -- "$LOGS"
    # Primeiro uso do cofre de teste (ou recém-recriado pelo `make dev-cofre`, que
    # apaga o estado velho): sem estado, o daemon recusa abrir (D39). O de teste começa
    # em 0 e sobe para a geração do arquivo no primeiro destravar. Só aqui, só no
    # cofre de teste: o daemon real nunca cria esse arquivo sozinho.
    if [ ! -e "$ESTADO" ]; then
        printf 'ward-estado 1\nvista 0\nqueimada 0\n' > "$ESTADO"
    fi
    env -i \
        HOME="$CASA" \
        PATH=/usr/bin:/bin \
        XDG_RUNTIME_DIR="$RUNTIME" \
        LANG="${LANG:-C.UTF-8}" \
        WARD_DEV_DIR="$DEV" \
        ${WARD_DEV_DBUS_SISTEMA:+"WARD_DEV_DBUS_SISTEMA=$WARD_DEV_DBUS_SISTEMA"} \
        ${APP:+"WARD_DEV_APP=$APP"} \
        "$BIN" </dev/null >>"$LOG" 2>&1 &
    local p=$!
    echo "$p" > "$PID"
    printf '%s\n' "$APP" > "$MARCA_APP"
    for _ in $(seq 1 100); do
        if [ -S "$SOCKET" ] && pid_vivo >/dev/null; then
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
    rm -f -- "$PID" "$MARCA_APP"
    exit 1
}

parar() {
    local p
    if ! p="$(pid_vivo)"; then
        rm -f -- "$PID" "$MARCA_APP"
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
    rm -f -- "$PID" "$MARCA_APP"
    echo "daemon de teste parado"
}

case "${1:-}" in
    subir) subir ;;
    parar) parar ;;
    estado)
        if p="$(pid_vivo)"; then echo "no ar (pid $p)"; else echo "parado"; fi
        ;;
    *) echo "uso: dev/daemon.sh subir|parar|estado" >&2; exit 2 ;;
esac
