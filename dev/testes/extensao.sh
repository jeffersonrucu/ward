#!/bin/bash -p
# `make teste-extensao` (Fase 41; D5, D9, D47; V6.2, V6.4–V6.7): a extensão MV3 de
# `extensao/dist/` no Chrome real (headless, perfil temporário em `.dev/extensao.*`),
# pelo `ward-nm` de teste, contra um `wardd` de teste só deste script com
# `WARD_DEV_EXTENSAO=chrome` (o filho do Chrome instalado ganha o papel extensão sem o
# egid do grupo `ward-nm`; só no `modo-teste`) e os serviços de teste (8081–8084).
#
# - V6.7: popup em `localhost:8081` com `acme-wp-dev` e `acme-wp-prod`; clicar em
#   `acme-wp-prod` preenche usuário e senha; em `localhost:8083`, `acme-totp-dev`
#   preenche usuário, senha e TOTP e o login entra. Medidas do popup conferidas com os
#   nodes do FigCraft (17332, 17392) e screenshots em docs/mission/evidence/fase-41/.
# - V6.2: em `127.0.0.1:8081`, nada sugerido; preencher forçado: o daemon recusa.
# - V6.4: página da origem certa com iframe do phishing: só o documento principal.
# - V6.5/V6.6: nada guardado, página que manda mensagem/postMessage/lê o popup não
#   consegue nada; DOM clobbering; campo de senha escondido; phishing (8082) e redir
#   (8084): nada sugerido, `posts.log` sem mudança.
# - Logs do daemon, do Chrome (stderr do `ward-nm`) e o console da extensão sem padrão de
#   `.dev/variacoes.txt` (V6.16 parcial).
#
# Nunca toca o perfil real do Chrome, `~/.config/google-chrome` nem o daemon de
# `make dev-daemon`. Precisa de `make dev-cofre`, `make dev-variacoes`,
# `make dev-servicos-subir`, `make app-front` (fontes) e do Chrome em
# /opt/google/chrome/chrome; fora do `make check`.
#
# Uso: dev/testes/extensao.sh <wardd modo-teste> <ward-nm modo-teste> <chrome-falso>

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/ambiente.sh
. "$RAIZ/dev/ambiente.sh"
cd "$RAIZ"

abs() { case "$1" in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$RAIZ" "$1" ;; esac; }
USO="uso: extensao.sh <wardd> <ward-nm> <chrome-falso>"
WARDD="$(abs "${1:?$USO}")"
NM="$(abs "${2:?$USO}")"
FALSO="$(abs "${3:?$USO}")"
for b in "$WARDD" "$NM" "$FALSO"; do [ -x "$b" ] || { echo "falta $b" >&2; exit 1; }; done
CHROME=/opt/google/chrome/chrome
[ -x "$CHROME" ] || { echo "falta o Chrome em $CHROME" >&2; exit 1; }
COFRE="$RAIZ/.dev/cofre-teste.ward"
LISTA="$RAIZ/.dev/variacoes.txt"
EXT="$RAIZ/extensao/dist"
[ -f "$COFRE" ] && [ ! -L "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-cofre'" >&2; exit 1; }
[ -s "$LISTA" ] && [ ! -L "$LISTA" ] || { echo "falta a lista: rode 'make dev-variacoes'" >&2; exit 1; }
[ -f "$EXT/manifest.json" ] || { echo "falta extensao/dist: rode 'make extensao'" >&2; exit 1; }
for porta in 8081 8082 8083 8084; do
    /usr/bin/python3 -I -c "import socket,sys; socket.create_connection(('127.0.0.1', int(sys.argv[1])), 2).close()" "$porta" 2>/dev/null \
        || { echo "serviço de teste na porta $porta fora do ar: rode 'make dev-servicos-subir'" >&2; exit 1; }
done

UID_ATUAL="$(id -u)"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$UID_ATUAL}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
TRAB="$(mktemp -d "$RAIZ/.dev/extensao.XXXXXX")"
SOCKDIR="$RUNTIME/ward-extensao-teste-$$-${TRAB##*.}"
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

cp -- "$COFRE" "$TRAB/cofre-teste.ward"
printf 'ward-estado 1\nvista 0\nqueimada 0\n' > "$TRAB/cofre-teste.estado"
LOG="$TRAB/wardd.log"
env -i \
    HOME="$TRAB" \
    PATH=/usr/bin:/bin \
    XDG_RUNTIME_DIR="$RUNTIME" \
    LANG=C.UTF-8 \
    WARD_DEV_DIR="$TRAB" \
    WARD_DEV_SOCKET_DIR="$SOCKDIR" \
    WARD_DEV_EXTENSAO=chrome \
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

falhas=0
/usr/bin/python3 -I -B "$RAIZ/dev/testes/extensao.py" "$RAIZ" "$TRAB" "$SOCKDIR" "$NM" "$FALSO" "$CHROME" "$LISTA" "$EXT" \
    || falhas=$((falhas + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; falhas=$((falhas + 1)); }
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
for f in "$LOG" "$TRAB/chrome.log" "$TRAB/console.log"; do
    if [ ! -f "$f" ]; then
        echo "FALHA ${f##*/} não existe" >&2
        falhas=$((falhas + 1))
    elif grep -qaF -f "$LISTA" -- "$f"; then
        echo "FALHA padrão de .dev/variacoes.txt em ${f##*/}" >&2
        falhas=$((falhas + 1))
    else
        echo "ok    ${f##*/} sem padrão de .dev/variacoes.txt"
    fi
done
# O log do daemon não leva a origem pedida (S13): nenhuma das origens usadas no teste.
if grep -qaE 'localhost:808[1-4]|127\.0\.0\.1:808[1-4]' -- "$LOG"; then
    echo "FALHA o log do daemon tem uma origem pedida" >&2
    falhas=$((falhas + 1))
else
    echo "ok    log do daemon sem as origens pedidas"
fi
grep -qa 'papel extensão de teste' -- "$LOG" \
    && echo "ok    o daemon de teste deu o papel extensão ao ward-nm filho do Chrome" \
    || { echo "FALHA o daemon não registrou o papel extensão de teste" >&2; falhas=$((falhas + 1)); }

if [ "$falhas" -ne 0 ]; then
    echo "teste-extensao: $falhas parte(s) com falha" >&2
    exit 1
fi
echo "teste-extensao: tudo certo"
