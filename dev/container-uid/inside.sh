#!/bin/bash -p
# Parte de dentro do `make test-docker-uid` (Fase 22; D49; V3.2). Roda como root **só
# dentro** do contêiner `ubuntu:24.04` descartável; do host só vêm /opt/teste (somente
# leitura: `wardd` de teste e cliente mínimo) e este script.
#
# O `wardd` de teste sobe como uid 1000 (sem grupo nenhum) com o socket em
# /run/user/1000/ward-teste/. Clientes:
# - uid 4242: o `connect` falha com EACCES (13) pelo diretório 0700;
# - uid 4242 com CAP_DAC_OVERRIDE (passa pelo 0700/0600): conecta, o daemon fecha sem
#   mandar um byte e o log tem "UidRecusado" com o uid 4242;
# - root (uid 0, com CAP_DAC_OVERRIDE do Docker): igual, com o uid 0;
# - uid 1000: atendido.
# Cada conferência imprime "ok:" ou "FALHOU:"; qualquer falha sai ≠ 0.

export PATH=/usr/sbin:/usr/bin:/bin
set -euo pipefail

WARDD=/opt/teste/wardd
CLIENTE=/opt/teste/cliente
DIR=/run/user/1000/ward-teste
SOCK=$DIR/wardd.sock
LOG=/tmp/wardd.log
falhas=0

ok() { echo "ok: $*"; }
falhou() { echo "FALHOU: $*"; falhas=$((falhas + 1)); }
confere() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else falhou "$1 (esperado '$3', veio '$2')"; fi
}

[ "$(id -u)" = 0 ] || { echo "inside.sh roda como root, só no contêiner" >&2; exit 1; }
[ -f /.dockerenv ] || { echo "inside.sh só roda dentro do contêiner" >&2; exit 1; }

# Diretórios do usuário de teste (como o systemd-logind e o home dariam no host).
install -d -m 0700 -o 1000 -g 1000 /run/user/1000
install -d -m 0700 -o 1000 -g 1000 /tmp/dev

setpriv --reuid=1000 --regid=1000 --clear-groups -- \
    env -i PATH=/usr/bin:/bin WARD_DEV_DIR=/tmp/dev WARD_DEV_SOCKET_DIR="$DIR" RUST_LOG=debug \
    "$WARDD" </dev/null >/dev/null 2>"$LOG" &
PID=$!
for _ in $(seq 1 100); do
    [ -S "$SOCK" ] && break
    kill -0 "$PID" 2>/dev/null || break
    sleep 0.1
done
[ -S "$SOCK" ] || { echo "o wardd não subiu:"; cat "$LOG"; exit 1; }
ok "wardd de teste no ar como uid 1000"

# uid 4242 sem capacidade: barrado pelo diretório 0700.
r="$(setpriv --reuid=4242 --regid=4242 --clear-groups -- "$CLIENTE" "$SOCK" estado)"
confere "uid 4242: connect recusado (EACCES)" "$r" "connect: 13"

# uid 4242 com CAP_DAC_OVERRIDE (ambiente): passa pelo 0700/0600; o SO_PEERCRED barra.
r="$(setpriv --reuid=4242 --regid=4242 --clear-groups \
    --inh-caps=+dac_override --ambient-caps=+dac_override -- "$CLIENTE" "$SOCK" lista)"
confere "uid 4242 com CAP_DAC_OVERRIDE: fechado sem nenhum byte" "$r" "bytes: 0"

# root: igual.
r="$("$CLIENTE" "$SOCK" lista)"
confere "root: fechado sem nenhum byte" "$r" "bytes: 0"

# O dono é atendido.
r="$(setpriv --reuid=1000 --regid=1000 --clear-groups -- "$CLIENTE" "$SOCK" estado)"
confere "uid 1000: atendido" "$r" "resposta: estado"

sleep 0.3
log="$(cat "$LOG")"
confere "log: uid 4242 recusado por SO_PEERCRED" "$log" "conexão recusada (UidRecusado): uid 4242"
confere "log: root recusado por SO_PEERCRED" "$log" "conexão recusada (UidRecusado): uid 0"

kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true

if [ "$falhas" -ne 0 ]; then
    echo "--- log do wardd ---"
    cat "$LOG"
    echo "$falhas conferência(s) falharam"
    exit 1
fi
echo "test-docker-uid: tudo ok"
