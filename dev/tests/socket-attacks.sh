#!/bin/bash -p
# `make test-socket-attacks` (Fase 24a, scrutiny do M3): V3.3 e V3.4 pelo socket de um
# `wardd` de teste que só este script usa.
#
# - V3.4 (`socket_attack.py`): quadros de 2 MiB, tamanho negativo, JSON inválido,
#   operação desconhecida, quadro cortado e 10 000 conexões abertas sem falar; o daemon
#   recusa ou fecha cada uma e continua respondendo a um cliente normal.
# - V3.3 (`socket_matrix.py`): matriz operação × entrada × campo como agente; nenhuma
#   resposta com padrão de `.dev/variations.txt`, operações de app e extensão
#   respondem `nao_autorizado`.
# - O log do daemon (`RUST_LOG=trace`) sem padrão de `.dev/variations.txt`.
#
# Nunca derruba o daemon de teste de outro processo: sobe o seu com uma cópia do cofre
# de teste numa pasta própria em `.dev/` (`socket-ataques.XXXXXX`) e o socket numa pasta
# própria em `$XDG_RUNTIME_DIR` (`WARD_DEV_SOCKET_DIR`), e só mata o pid que ele mesmo
# criou. Precisa de `make dev-vault` e `make dev-variations`. Fica fora do `make check`
# (as 10 000 conexões); a versão curta (70 conexões) está no `cargo test` do `wardd`.
#
# Uso: dev/tests/socket-attacks.sh <wardd com test-mode> [conexões]

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$RAIZ/dev/env.sh"
cd "$RAIZ"

WARDD="${1:?uso: socket-attacks.sh <wardd> [conexões]}"
CONEXOES="${2:-10000}"
[[ "$CONEXOES" =~ ^[1-9][0-9]{0,5}$ ]] || { echo "conexões inválido" >&2; exit 2; }
case "$WARDD" in /*) ;; *) WARDD="$RAIZ/$WARDD" ;; esac
[ -x "$WARDD" ] || { echo "falta $WARDD" >&2; exit 1; }
COFRE="$RAIZ/.dev/test-vault.ward"
LISTA="$RAIZ/.dev/variations.txt"
[ -f "$COFRE" ] && [ ! -L "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$LISTA" ] && [ ! -L "$LISTA" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }

UID_ATUAL="$(id -u)"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$UID_ATUAL}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
TRAB="$(mktemp -d "$RAIZ/.dev/socket-attacks.XXXXXX")"
# A pasta do socket não pode existir antes: o daemon a cria 0700 (o nome é único).
SOCKDIR="$RUNTIME/ward-socket-ataques-$$-${TRAB##*.}"
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
# Estado novo (D39): geração 0, sobe no primeiro destravar (só no cofre de teste).
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
echo "==> V3.4: quadros ruins e $CONEXOES conexões caladas"
/usr/bin/python3 -I -B "$RAIZ/dev/tests/socket_attack.py" "$SOCKET" "$PID" "$CONEXOES" || falhas=$((falhas + 1))
echo "==> V3.3: matriz operação × entrada × campo (agente)"
/usr/bin/python3 -I -B "$RAIZ/dev/tests/socket_matrix.py" "$SOCKET" "$RAIZ" "$LISTA" || falhas=$((falhas + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; falhas=$((falhas + 1)); }
echo "==> log do daemon (RUST_LOG=trace) sem segredo"
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
if grep -qaF -f "$LISTA" -- "$LOG"; then
    echo "FALHA padrão de .dev/variations.txt no log do daemon" >&2
    falhas=$((falhas + 1))
else
    echo "ok    log do daemon ($(wc -l < "$LOG") linhas) sem padrão de .dev/variations.txt"
fi

if [ "$falhas" -ne 0 ]; then
    echo "test-socket-attacks: $falhas parte(s) com falha" >&2
    exit 1
fi
echo "test-socket-attacks: V3.3 e V3.4 ok"
