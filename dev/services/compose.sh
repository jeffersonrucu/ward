#!/bin/bash -p
# `docker compose -f dev/compose.yml …` com o ambiente isolado de common.sh (Fase 12b):
# PATH fixo, DOCKER_CONFIG vazio em .dev/docker/, sem DOCKER_HOST/DOCKER_CONTEXT nem
# COMPOSE_* herdados. É por aqui que o Makefile chama o compose.
# Uso: dev/services/compose.sh up -d --build
export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/services/common.sh
. "$RAIZ/dev/services/common.sh"
exec "${COMPOSE[@]}" "$@"
