#!/bin/bash -p
# `make teste-app-e2e` (Fase 39c; V5.10, V5.16, V5.24; D2, D3, D63, S10).
#
# O app de ponta a ponta com o daemon de teste e a ponte, sem a janela nativa: sobe um
# `wardd` de teste só deste teste (socket em WARD_DEV_SOCKET_DIR, padrão
# `/run/user/<uid>/ward-teste-e2e`; nunca o socket fixo de outro checkout) com
# WARD_DEV_APP = a ponte, pede `abrir_app` (dev/ponte.sh) e roda o
# `dev/testes/app_e2e.py` (Playwright do Python, Chrome headless) com a CLI de teste de
# verdade: `ward run` com a janela de aprovação (negar, fechar, uma vez, 1 hora com
# travar e destravar, sempre) e o "Conectar" com `ward ssh --token-fd` contra o
# `ssh-salto` (2224), mais os ataques ao token. No fim, a varredura de logs (V5.27) em
# .dev/logs e o cofre de teste volta ao que era antes (o "sempre" o muda).
#
# Precisa de: `make dev-cofre`, `make dev-variacoes`, `make app-front`, os serviços de
# teste de pé (`make dev-servicos-subir`), o Chrome em /opt/google/chrome e o Playwright
# do Python em PW_PYTHONPATH. Nenhum daemon de teste deste repositório pode estar no ar
# (`make dev-daemon-parar`).
# Uso: PW_PYTHONPATH=<site-packages> dev/testes/app-e2e.sh

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$RAIZ/dev/ambiente.sh"

PW="${PW_PYTHONPATH:-}"
[[ "$PW" == /* && -d "$PW/playwright" ]] || { echo "app-e2e: PW_PYTHONPATH=<site-packages com o playwright> (absoluto)" >&2; exit 1; }
[[ -x /opt/google/chrome/chrome ]] || { echo "app-e2e: falta o Chrome em /opt/google/chrome" >&2; exit 1; }
DEV="$RAIZ/.dev"
for f in "$DEV/cofre-teste.ward" "$DEV/variacoes.txt" "$RAIZ/app/dist-teste/index.html" \
    "$RAIZ/target/debug/wardd" "$RAIZ/target/debug/ward" "$RAIZ/target/debug/ward-ponte-teste"; do
    [[ -f "$f" ]] || { echo "app-e2e: falta ${f#"$RAIZ"/} (make dev-cofre, dev-variacoes, app-front)" >&2; exit 1; }
done
(exec 3<>/dev/tcp/127.0.0.1/2224) 2> /dev/null \
    || { echo "app-e2e: o ssh-salto (2224) não responde; rode 'make dev-servicos-subir'" >&2; exit 1; }

UID_="$(id -u)"
SOCKDIR="${WARD_DEV_SOCKET_DIR:-/run/user/$UID_/ward-teste-e2e}"
[[ "$SOCKDIR" == /run/user/"$UID_"/* && "$SOCKDIR" != */../* ]] \
    || { echo "app-e2e: WARD_DEV_SOCKET_DIR fora de /run/user/$UID_/" >&2; exit 1; }
[[ "$SOCKDIR" != "/run/user/$UID_/ward-teste" ]] \
    || { echo "app-e2e: use uma pasta própria em WARD_DEV_SOCKET_DIR (a ward-teste é de todos os checkouts)" >&2; exit 1; }
if [[ -f "$DEV/wardd.pid" ]] && p="$(head -c 16 "$DEV/wardd.pid")" && [[ "$p" =~ ^[0-9]+$ ]] \
    && [[ "$(cat "/proc/$p/comm" 2> /dev/null)" == wardd ]]; then
    echo "app-e2e: o daemon de teste deste repositório já está no ar (pid $p); rode 'make dev-daemon-parar'" >&2
    exit 1
fi

# O cofre de teste volta ao que era (o "Permitir sempre" muda o nível de uma entrada).
umask 077
GUARDA="$DEV/e2e-guarda"
rm -rf "$GUARDA"
mkdir -m 0700 "$GUARDA"
cp -p "$DEV/cofre-teste.ward" "$GUARDA/"
[[ -f "$DEV/cofre-teste.estado" ]] && cp -p "$DEV/cofre-teste.estado" "$GUARDA/"
rm -f "$DEV/logs/terminal-teste.log" "$DEV/e2e-marca" "$DEV/terminal-teste.modo"

limpar() {
    WARD_DEV_SOCKET_DIR="$SOCKDIR" "$RAIZ/dev/daemon.sh" parar > /dev/null 2>&1 || true
    cp -p "$GUARDA/cofre-teste.ward" "$DEV/cofre-teste.ward"
    if [[ -f "$GUARDA/cofre-teste.estado" ]]; then
        cp -p "$GUARDA/cofre-teste.estado" "$DEV/cofre-teste.estado"
    else
        rm -f "$DEV/cofre-teste.estado"
    fi
    rm -rf "$GUARDA"
    rm -f "$DEV/e2e-marca" "$DEV/terminal-teste.modo"
}
trap limpar EXIT

echo "==> daemon de teste em $SOCKDIR com a ponte (D63)"
WARD_DEV_SOCKET_DIR="$SOCKDIR" WARD_DEV_APP="$RAIZ/target/debug/ward-ponte-teste" "$RAIZ/dev/daemon.sh" subir
URL="$(WARD_DEV_SOCKET_DIR="$SOCKDIR" "$RAIZ/dev/ponte.sh")"

echo "==> app de ponta a ponta (Chrome headless)"
set +e
printf '%s\n' "$URL" | /usr/bin/python3 -I -B "$RAIZ/dev/testes/app_e2e.py" "$RAIZ" "$SOCKDIR" "$PW"
qa=$?
set -e
unset URL

echo "==> varredura de logs (V5.27): .dev/logs com .dev/variacoes.txt"
WARD_DEV_SOCKET_DIR="$SOCKDIR" "$RAIZ/dev/daemon.sh" parar > /dev/null 2>&1 || true
/usr/bin/python3 -I -B "$RAIZ/dev/testes/varredura-logs.py" "$DEV/logs"
exit "$qa"
