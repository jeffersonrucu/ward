#!/bin/bash -p
# `make teste-playwright` (Fase 44): `ward playwright` de ponta a ponta, com o Chrome
# instalado e os serviços de teste (WordPress 8081, phish 8082, redir 8084).
#
# - V6.12: `ward playwright acme-wp-prod` imprime `<rt>playwright/acme-wp-prod.json`
#   (0600) e um teste Playwright em Python com esse `storage_state` abre `/wp-admin/`
#   logado (o Playwright do Python vem de PW_PYTHONPATH, fora do repositório; sem ele o
#   teste falha, a não ser com SEM_PLAYWRIGHT_PYTHON=1, que pula só essa conferência).
# - V6.13: nenhum padrão de `.dev/variacoes.txt` no storage state nem no perfil
#   temporário (lido a cada 50 ms enquanto o Chrome roda); o perfil some ao fim e
#   `ward travar` apaga os storage states.
# - V6.14: enquanto o Chrome roda, nenhum processo dele escuta em porta TCP
#   (`/proc/net/tcp*` e `ss -ltnp`), o argv tem `--remote-debugging-pipe` e nenhum
#   segredo, e o pipe de depuração só está no Chrome principal.
# - V6.15 (parte do `ward playwright`): `delta-redir` (8084 → 8082) recusa por origem,
#   e o `posts.log` do phish não ganha linha nem padrão do cofre.
# - V6.16 (parte do daemon): o log do daemon (`RUST_LOG=trace`) sem padrão de
#   `.dev/variacoes.txt`.
#
# Nunca derruba o daemon de teste de outro processo: sobe o seu com uma cópia do cofre de
# teste numa pasta própria em `.dev/` e o socket numa pasta própria em
# `$XDG_RUNTIME_DIR` (`WARD_DEV_SOCKET_DIR`), e só mata o pid que ele mesmo criou.
# Precisa de `make dev-cofre`, `make dev-variacoes` e dos serviços de pé.
#
# Uso: dev/testes/playwright.sh <wardd com modo-teste> <ward com modo-teste>

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/ambiente.sh
. "$RAIZ/dev/ambiente.sh"
cd "$RAIZ"

WARDD="${1:?uso: playwright.sh <wardd> <ward>}"
WARD="${2:?uso: playwright.sh <wardd> <ward>}"
case "$WARDD" in /*) ;; *) WARDD="$RAIZ/$WARDD" ;; esac
case "$WARD" in /*) ;; *) WARD="$RAIZ/$WARD" ;; esac
[ -x "$WARDD" ] && [ -x "$WARD" ] || { echo "falta $WARDD ou $WARD" >&2; exit 1; }
COFRE="$RAIZ/.dev/cofre-teste.ward"
LISTA="$RAIZ/.dev/variacoes.txt"
[ -f "$COFRE" ] && [ ! -L "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-cofre'" >&2; exit 1; }
[ -s "$LISTA" ] && [ ! -L "$LISTA" ] || { echo "falta a lista: rode 'make dev-variacoes'" >&2; exit 1; }
[ -x /opt/google/chrome/chrome ] || { echo "falta o Chrome em /opt/google/chrome/chrome" >&2; exit 1; }

UID_ATUAL="$(id -u)"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$UID_ATUAL}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
TRAB="$(mktemp -d "$RAIZ/.dev/playwright.XXXXXX")"
# Curto: o Chrome cria um socket Unix dentro do perfil (`<rt>pw-*/t/…`), com no máximo
# 107 bytes de caminho.
SOCKDIR="$RUNTIME/ward-pw-$$"
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
/usr/bin/python3 -I -B "$RAIZ/dev/testes/playwright.py" "$WARD" "$RAIZ" "$SOCKDIR" "$TRAB" \
    || falhas=$((falhas + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; falhas=$((falhas + 1)); }
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
if grep -qaF -f "$LISTA" -- "$LOG"; then
    echo "FALHA padrão de .dev/variacoes.txt no log do daemon" >&2
    falhas=$((falhas + 1))
else
    echo "ok    V6.16 log do daemon ($(wc -l < "$LOG") linhas) sem padrão de .dev/variacoes.txt"
fi
if [ "$falhas" -ne 0 ]; then
    # O log já foi conferido sem segredo acima; o fim dele ajuda a achar a falha.
    tail -n 30 -- "$LOG" >&2
    echo "teste-playwright: falhou" >&2
    exit 1
fi
echo "teste-playwright: ok"
