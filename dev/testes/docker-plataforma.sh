#!/bin/bash -p
# `make teste-docker-plataforma` (Fase 21b; D46, D47, D49; V3.5, V3.23, V3.24).
#
# O que exige root (grupos de sistema, binários setgid, pasta /run/ward/<uid> do root,
# socket criado pelo root) roda num contêiner `ubuntu:24.04` descartável: mesma glibc do
# Pop!_OS 24.04 e o mesmo kernel do host. Nada de sudo, nada no /etc nem no /run do
# host. O contêiner não tem rede e só monta, somente leitura, uma pasta temporária em
# `.dev/` com o binário de teste (`ward-plataforma-conteiner`, feature `modo-teste`) e a
# biblioteca de ataque do LD_PRELOAD, mais o script de dentro. Fica fora do `make
# check` (exige Docker). Uso: dev/testes/docker-plataforma.sh <binário de teste>
# <app de teste> <ward de release> <wardd de release>. O app de teste (Fase 29a) é o
# `partida-conteiner` do `app/nucleo`: a partida do app real (D47, D57) sem o GTK. A CLI
# e o `wardd` de release (sem `modo-teste`) e o servidor falso não-dumpable (`falso.c`)
# são da 2ª parte do V4.2 (Fase 28, D65 e).

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/servicos/comum.sh
. "$RAIZ/dev/servicos/comum.sh"

# Imagem pela digest (a lista multiarquitetura de ubuntu:24.04 conferida na Fase 21b).
IMAGEM="ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852"
USO="uso: docker-plataforma.sh <binário de teste> <app de teste> <ward de release> <wardd de release>"
BIN="${1:?$USO}"
APP="${2:?$USO}"
CLI="${3:?$USO}"
WARDD="${4:?$USO}"
[ -x "$BIN" ] || { echo "binário de teste não encontrado: $BIN" >&2; exit 1; }
[ -x "$APP" ] || { echo "app de teste não encontrado: $APP" >&2; exit 1; }
[ -x "$CLI" ] || { echo "ward de release não encontrado: $CLI" >&2; exit 1; }
[ -x "$WARDD" ] || { echo "wardd de release não encontrado: $WARDD" >&2; exit 1; }
# V4.2 (2ª parte) exige o build sem `modo-teste`: as strings do daemon de teste não podem
# estar nos binários (o `ward` de teste aceitaria o socket de `WARD_DEV_SOCKET_DIR`).
for b in "$CLI" "$WARDD"; do
    if grep -q -a -F 'WARD_DEV_SOCKET_DIR' -- "$b"; then
        echo "$b foi compilado com modo-teste" >&2
        exit 1
    fi
done
for p in docker gcc; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$RAIZ/.dev"
TMP="$(mktemp -d "$RAIZ/.dev/conteiner-plataforma.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0755 "$BIN" "$TMP/ward-teste"
install -m 0755 "$APP" "$TMP/ward-app-teste"
install -m 0755 "$CLI" "$TMP/ward-release"
install -m 0755 "$WARDD" "$TMP/wardd-release"
gcc -shared -fPIC -O2 -Wall -Werror -o "$TMP/libmarca.so" "$RAIZ/dev/conteiner-plataforma/marca.c"
chmod 0644 "$TMP/libmarca.so"
gcc -O2 -Wall -Werror -o "$TMP/falso" "$RAIZ/dev/conteiner-plataforma/falso.c"
chmod 0755 "$TMP/falso"

docker image inspect "$IMAGEM" >/dev/null 2>&1 || docker pull --quiet "$IMAGEM" >/dev/null

# Sem rede; sem `no-new-privileges` (ele desligaria justamente o setgid que se testa).
# `memlock` infinito: o "daemon" do contêiner faz o `mlockall` de D62 (V3.34: o app que
# ele lança não herda o travamento).
docker run --rm --network none --pull never --ulimit memlock=-1:-1 \
    -v "$TMP:/opt/teste:ro" \
    -v "$RAIZ/dev/conteiner-plataforma/dentro.sh:/opt/dentro.sh:ro" \
    "$IMAGEM" /bin/bash -p /opt/dentro.sh
