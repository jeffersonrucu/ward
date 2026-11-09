#!/bin/bash -p
# `make app-ponte` (Fase 23a, D63): a ponte de teste aberta pelo daemon de teste.
#
# O `make` já subiu o daemon de teste com WARD_DEV_APP = a ponte (dev/daemon.sh). Aqui:
# pede `abrir_app` (dev/abrir-app.py, sem campo) e espera a URL que a ponte imprime no
# log dela (.dev/logs/ward-ponte-teste.log, 0600). A ponte é filha do daemon, recebe o
# papel app (o pid do lançador), confere o papel na primeira resposta e sai quando o
# daemon para. Executada à mão, ela recebe o papel agente e sai.
#
# A URL tem o token da ponte: vai só para a saída deste comando. Qualquer processo do
# uid que leia o log também a vê (resíduo de D63, só com o cofre de teste).

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
. "$RAIZ/dev/ambiente.sh"

LOG="$RAIZ/.dev/logs/ward-ponte-teste.log"
SOCKET="/run/user/$(id -u)/ward-teste/wardd.sock"
URL_RE='^http://127\.0\.0\.1:[0-9]{1,5}/#token=[0-9a-f]{64}$'

antes=0
[ -f "$LOG" ] && antes="$(stat -c %s -- "$LOG")"

/usr/bin/python3 -I -B "$RAIZ/dev/abrir-app.py" "$SOCKET" >/dev/null

novo() { tail -c "+$((antes + 1))" -- "$LOG" 2>/dev/null || true; }

for _ in $(seq 1 150); do
    url="$(novo | grep -E -- "$URL_RE" | tail -n 1 || true)"
    if [ -n "$url" ]; then
        echo "$url"
        exit 0
    fi
    if novo | grep -q -- '^ward-ponte-teste:'; then
        echo "a ponte não subiu; saída dela:" >&2
        novo | grep -- '^ward-ponte-teste:' >&2
        exit 1
    fi
    sleep 0.1
done

# Sem URL nova: a ponte já estava aberta (uma por vez, D57); vale a última do log.
url="$(grep -E -- "$URL_RE" "$LOG" 2>/dev/null | tail -n 1 || true)"
if [ -n "$url" ]; then
    echo "$url"
    echo "(a ponte já estava aberta)" >&2
    exit 0
fi
echo "a ponte não imprimiu a URL; veja .dev/logs/wardd.log e $LOG" >&2
exit 1
