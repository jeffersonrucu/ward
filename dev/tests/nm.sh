#!/bin/bash -p
# `make test-nm` (Fase 40; D5, D9, D47; V6.1, V6.2 lado do host): o `ward-nm` de teste
# contra um `wardd` de teste só deste script, e com o Chrome real.
#
# - V6.1: o `ward-nm` executado direto por um processo do usuário (aqui sem o setgid,
#   como em todo host de desenvolvimento) recebe o papel agente; `preencher` e `sugerir`
#   respondem `nao_autorizado` e nada da lista `.dev/variations.txt` sai no stdout.
# - Chrome real (headless, perfil temporário em `.dev/nm.*`, extensão de teste carregada
#   pelo `--remote-debugging-pipe`, manifesto de native messaging só dentro do perfil
#   temporário): o Chrome abre o host como filho direto do processo do navegador, o
#   `PeerProcess::verify_parent` do daemon aceita esse pai contra
#   `/opt/google/chrome/chrome` e recusa o mesmo host aberto por outro processo; o
#   navegador é dumpable (D47: a conferência pelo `/proc` vale); e o `ward-nm` responde
#   pelo Chrome (estado, preencher recusado ao agente, mensagem grande demais).
# - Os logs do daemon e do Chrome (stderr do `ward-nm`) e o relatório da extensão sem
#   padrão de `.dev/variations.txt`.
#
# Nunca toca o perfil real do Chrome nem `~/.config/google-chrome/NativeMessagingHosts`,
# nem o daemon de `make dev-daemon`: o daemon, o socket, o perfil e o HOME do Chrome são
# deste script e somem no fim. Precisa de `make dev-vault` e `make dev-variations` e do
# Chrome em /opt/google/chrome/chrome; fora do `make check`.
#
# Uso: dev/tests/nm.sh <wardd test-mode> <ward-nm test-mode> <exemplo pai-chrome>

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$RAIZ/dev/env.sh"
cd "$RAIZ"

abs() { case "$1" in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$RAIZ" "$1" ;; esac; }
WARDD="$(abs "${1:?uso: nm.sh <wardd> <ward-nm> <pai-chrome>}")"
NM="$(abs "${2:?uso: nm.sh <wardd> <ward-nm> <pai-chrome>}")"
SONDA="$(abs "${3:?uso: nm.sh <wardd> <ward-nm> <pai-chrome>}")"
for b in "$WARDD" "$NM" "$SONDA"; do [ -x "$b" ] || { echo "falta $b" >&2; exit 1; }; done
CHROME=/opt/google/chrome/chrome
[ -x "$CHROME" ] || { echo "falta o Chrome em $CHROME" >&2; exit 1; }
COFRE="$RAIZ/.dev/test-vault.ward"
LISTA="$RAIZ/.dev/variations.txt"
[ -f "$COFRE" ] && [ ! -L "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$LISTA" ] && [ ! -L "$LISTA" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }

UID_ATUAL="$(id -u)"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$UID_ATUAL}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
TRAB="$(mktemp -d "$RAIZ/.dev/nm.XXXXXX")"
SOCKDIR="$RUNTIME/ward-nm-teste-$$-${TRAB##*.}"
PID=""
limpar() {
    if [ -n "$PID" ] && kill -0 "$PID" 2>/dev/null; then
        kill -TERM "$PID" 2>/dev/null || true
        for _ in $(seq 1 50); do kill -0 "$PID" 2>/dev/null || break; sleep 0.1; done
        kill -KILL "$PID" 2>/dev/null || true
        wait "$PID" 2>/dev/null || true
    fi
    rm -rf -- "$TRAB" "$SOCKDIR"
}
trap limpar EXIT

cp -- "$COFRE" "$TRAB/test-vault.ward"
printf 'ward-estado 1\nvista 0\nqueimada 0\n' > "$TRAB/test-vault.state"
LOG="$TRAB/wardd.log"
env -i \
    HOME="$TRAB" \
    PATH=/usr/bin:/bin \
    XDG_RUNTIME_DIR="$RUNTIME" \
    LANG=C.UTF-8 \
    WARD_DEV_DIR="$TRAB" \
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

falhas=0
/usr/bin/python3 -I -B "$RAIZ/dev/tests/nm.py" "$RAIZ" "$TRAB" "$SOCKDIR" "$NM" "$SONDA" "$CHROME" "$LISTA" \
    || falhas=$((falhas + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; falhas=$((falhas + 1)); }
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
for f in "$LOG" "$TRAB/chrome.log"; do
    if [ -f "$f" ] && grep -qaF -f "$LISTA" -- "$f"; then
        echo "FALHA padrão de .dev/variations.txt em ${f##*/}" >&2
        falhas=$((falhas + 1))
    else
        echo "ok    ${f##*/} sem padrão de .dev/variations.txt"
    fi
done
if grep -qa "preencher acme-wp-dev$" -- "$LOG"; then
    echo "FALHA o daemon atendeu um preencher do papel agente" >&2
    falhas=$((falhas + 1))
fi

if [ "$falhas" -ne 0 ]; then
    echo "test-nm: $falhas parte(s) com falha" >&2
    exit 1
fi
echo "test-nm: tudo certo"
