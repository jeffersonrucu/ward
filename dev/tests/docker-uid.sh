#!/bin/bash -p
# `make test-docker-uid` (Phase 22; D49; V3.2, S3).
#
# Another uid on the `wardd` socket needs root to exist (a uid 4242, and the same uid
# with CAP_DAC_OVERRIDE, which gets past the 0700/0600 permission of the directory and
# the socket). So it runs in a disposable `ubuntu:24.04` container without network: no
# sudo, nothing in the host's /etc or /run. The container only mounts, read-only, a
# temporary folder of `.dev/` with the test `wardd` (feature `test-mode`) and the minimal
# client (`wardd-test-client`), plus the inside script. Stays out of `make check`
# (requires Docker). Usage: dev/tests/docker-uid.sh <wardd> <wardd-test-client>

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"

# The same image (by digest) as `make test-docker-platform`.
IMAGE="ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852"
WARDD="${1:?uso: docker-uid.sh <wardd> <wardd-test-client>}"
CLIENT="${2:?uso: docker-uid.sh <wardd> <wardd-test-client>}"
for b in "$WARDD" "$CLIENT"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b" >&2; exit 1; }
done
command -v docker >/dev/null 2>&1 || { echo "falta o programa: docker" >&2; exit 1; }

mkdir -p -m 0700 "$ROOT/.dev"
TMP="$(mktemp -d "$ROOT/.dev/container-uid.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0755 "$WARDD" "$TMP/wardd"
install -m 0755 "$CLIENT" "$TMP/cliente"

docker image inspect "$IMAGE" >/dev/null 2>&1 || docker pull --quiet "$IMAGE" >/dev/null

docker run --rm --network none --pull never \
    -v "$TMP:/opt/teste:ro" \
    -v "$ROOT/dev/container-uid/inside.sh:/opt/inside.sh:ro" \
    "$IMAGE" /bin/bash -p /opt/inside.sh
