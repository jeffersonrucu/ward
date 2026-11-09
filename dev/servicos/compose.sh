#!/bin/bash -p
# `docker compose -f dev/compose.yml …` com o ambiente isolado de comum.sh (Fase 12b):
# PATH fixo, DOCKER_CONFIG vazio em .dev/docker/, sem DOCKER_HOST/DOCKER_CONTEXT nem
# COMPOSE_* herdados. É por aqui que o Makefile chama o compose.
# Uso: dev/servicos/compose.sh up -d --build
export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/servicos/comum.sh
. "$RAIZ/dev/servicos/comum.sh"
exec "${COMPOSE[@]}" "$@"
