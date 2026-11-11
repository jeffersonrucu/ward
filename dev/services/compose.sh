#!/bin/bash -p
# `docker compose -f dev/compose.yml …` with the isolated environment of common.sh (Phase 12b):
# fixed PATH, empty DOCKER_CONFIG in .dev/docker/, no inherited DOCKER_HOST/DOCKER_CONTEXT
# nor COMPOSE_*. This is how the Makefile calls compose.
# Usage: dev/services/compose.sh up -d --build
export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"
exec "${COMPOSE[@]}" "$@"
