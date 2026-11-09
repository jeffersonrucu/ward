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

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/servicos/comum.sh
. "$RAIZ/dev/servicos/comum.sh"

# Imagem pela digest (a lista multiarquitetura de ubuntu:24.04 conferida na Fase 21b).
IMAGEM="ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852"
BIN="${1:?uso: docker-plataforma.sh <binário de teste>}"
[ -x "$BIN" ] || { echo "binário de teste não encontrado: $BIN" >&2; exit 1; }
for p in docker gcc; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$RAIZ/.dev"
TMP="$(mktemp -d "$RAIZ/.dev/conteiner-plataforma.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0755 "$BIN" "$TMP/ward-teste"
gcc -shared -fPIC -O2 -Wall -Werror -o "$TMP/libmarca.so" "$RAIZ/dev/conteiner-plataforma/marca.c"
chmod 0644 "$TMP/libmarca.so"

docker image inspect "$IMAGEM" >/dev/null 2>&1 || docker pull --quiet "$IMAGEM" >/dev/null

# Sem rede; sem `no-new-privileges` (ele desligaria justamente o setgid que se testa).
docker run --rm --network none --pull never \
    -v "$TMP:/opt/teste:ro" \
    -v "$RAIZ/dev/conteiner-plataforma/dentro.sh:/opt/dentro.sh:ro" \
    "$IMAGEM" /bin/bash -p /opt/dentro.sh
