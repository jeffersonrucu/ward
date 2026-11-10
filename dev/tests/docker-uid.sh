#!/bin/bash -p
# `make test-docker-uid` (Fase 22; D49; V3.2, S3).
#
# Outro uid no socket do `wardd` precisa de root para existir (um uid 4242, e o mesmo uid
# com CAP_DAC_OVERRIDE, que passa pela permissão 0700/0600 do diretório e do socket).
# Por isso roda num contêiner `ubuntu:24.04` descartável, sem rede: nada de sudo, nada no
# /etc nem no /run do host. O contêiner só monta, somente leitura, uma pasta temporária
# de `.dev/` com o `wardd` de teste (feature `test-mode`) e o cliente mínimo
# (`wardd-cliente-teste`), mais o script de dentro. Fica fora do `make check` (exige
# Docker). Uso: dev/tests/docker-uid.sh <wardd> <wardd-cliente-teste>

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$RAIZ/dev/services/common.sh"

# A mesma imagem (pela digest) do `make test-docker-platform`.
IMAGEM="ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852"
WARDD="${1:?uso: docker-uid.sh <wardd> <wardd-cliente-teste>}"
CLIENTE="${2:?uso: docker-uid.sh <wardd> <wardd-cliente-teste>}"
for b in "$WARDD" "$CLIENTE"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b" >&2; exit 1; }
done
command -v docker >/dev/null 2>&1 || { echo "falta o programa: docker" >&2; exit 1; }

mkdir -p -m 0700 "$RAIZ/.dev"
TMP="$(mktemp -d "$RAIZ/.dev/container-uid.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0755 "$WARDD" "$TMP/wardd"
install -m 0755 "$CLIENTE" "$TMP/cliente"

docker image inspect "$IMAGEM" >/dev/null 2>&1 || docker pull --quiet "$IMAGEM" >/dev/null

docker run --rm --network none --pull never \
    -v "$TMP:/opt/teste:ro" \
    -v "$RAIZ/dev/container-uid/inside.sh:/opt/inside.sh:ro" \
    "$IMAGEM" /bin/bash -p /opt/inside.sh
