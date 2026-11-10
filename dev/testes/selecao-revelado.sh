#!/bin/bash -p
# `make teste-selecao-revelado` (Fase 39g; S2, S12; 2ª auditoria do M5).
#
# O valor revelado no `Campo secreto` não vai à PRIMARY nem à CLIPBOARD por seleção,
# Ctrl+A, Ctrl+C, arrasto ou clique triplo, no WebKitGTK de verdade: sobe um `wardd` de
# teste só deste teste (socket em WARD_DEV_SOCKET_DIR, padrão
# `/run/user/<uid>/ward-teste-selecao`; nunca o socket fixo de outro checkout) com
# WARD_DEV_APP = a ponte, pede `abrir_app` (dev/ponte.sh) e roda
# `dev/testes/selecao_revelado.py`, que abre a interface de teste num WebKitGTK 4.1 dentro
# de um Xwayland privado (a área de transferência da sessão do usuário não é lida nem
# escrita). No fim, a varredura de logs (V5.27) em .dev/logs e o cofre de teste volta
# ao que era.
#
# Precisa de: `make dev-cofre`, `make dev-variacoes`, `make app-front`, Xwayland, xauth,
# python3-gi com o WebKit2 4.1 e a sessão Wayland (uma janela do Xwayland aparece por
# uns 20 s). Nenhum daemon de teste deste repositório pode estar no ar
# (`make dev-daemon-parar`). Fora do `make check`.
# Uso: dev/testes/selecao-revelado.sh

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$RAIZ/dev/ambiente.sh"

DEV="$RAIZ/.dev"
for f in "$DEV/cofre-teste.ward" "$DEV/variacoes.txt" "$RAIZ/app/dist-teste/index.html" \
    "$RAIZ/target/debug/wardd" "$RAIZ/target/debug/ward-ponte-teste"; do
    [[ -f "$f" ]] || { echo "selecao-revelado: falta ${f#"$RAIZ"/} (make dev-cofre, dev-variacoes, app-front)" >&2; exit 1; }
done

UID_="$(id -u)"
SOCKDIR="${WARD_DEV_SOCKET_DIR:-/run/user/$UID_/ward-teste-selecao}"
[[ "$SOCKDIR" == /run/user/"$UID_"/* && "$SOCKDIR" != */../* ]] \
    || { echo "selecao-revelado: WARD_DEV_SOCKET_DIR fora de /run/user/$UID_/" >&2; exit 1; }
[[ "$SOCKDIR" != "/run/user/$UID_/ward-teste" ]] \
    || { echo "selecao-revelado: use uma pasta própria em WARD_DEV_SOCKET_DIR (a ward-teste é de todos os checkouts)" >&2; exit 1; }
if [[ -f "$DEV/wardd.pid" ]] && p="$(head -c 16 "$DEV/wardd.pid")" && [[ "$p" =~ ^[0-9]+$ ]] \
    && [[ "$(cat "/proc/$p/comm" 2> /dev/null)" == wardd ]]; then
    echo "selecao-revelado: o daemon de teste deste repositório já está no ar (pid $p); rode 'make dev-daemon-parar'" >&2
    exit 1
fi

umask 077
GUARDA="$DEV/selecao-guarda"
rm -rf "$GUARDA"
mkdir -m 0700 "$GUARDA"
cp -p "$DEV/cofre-teste.ward" "$GUARDA/"
[[ -f "$DEV/cofre-teste.estado" ]] && cp -p "$DEV/cofre-teste.estado" "$GUARDA/"

limpar() {
    WARD_DEV_SOCKET_DIR="$SOCKDIR" "$RAIZ/dev/daemon.sh" parar > /dev/null 2>&1 || true
    cp -p "$GUARDA/cofre-teste.ward" "$DEV/cofre-teste.ward"
    if [[ -f "$GUARDA/cofre-teste.estado" ]]; then
        cp -p "$GUARDA/cofre-teste.estado" "$DEV/cofre-teste.estado"
    else
        rm -f "$DEV/cofre-teste.estado"
    fi
    rm -rf "$GUARDA" "$DEV/selecao-revelado"
}
trap limpar EXIT

echo "==> daemon de teste em $SOCKDIR com a ponte (D63)"
WARD_DEV_SOCKET_DIR="$SOCKDIR" WARD_DEV_APP="$RAIZ/target/debug/ward-ponte-teste" "$RAIZ/dev/daemon.sh" subir
URL="$(WARD_DEV_SOCKET_DIR="$SOCKDIR" "$RAIZ/dev/ponte.sh")"

echo "==> valor revelado contra seleção e cópia (WebKitGTK num X privado)"
set +e
printf '%s\n' "$URL" | /usr/bin/python3 -I -B "$RAIZ/dev/testes/selecao_revelado.py"
qa=$?
set -e
unset URL

echo "==> varredura de logs (V5.27): .dev/logs com .dev/variacoes.txt"
WARD_DEV_SOCKET_DIR="$SOCKDIR" "$RAIZ/dev/daemon.sh" parar > /dev/null 2>&1 || true
/usr/bin/python3 -I -B "$RAIZ/dev/testes/varredura-logs.py" "$DEV/logs"
exit "$qa"
