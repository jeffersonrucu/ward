#!/bin/bash -p
# The test bundle (the test Vite build in app/) has to be the one of the current
# sources (Phase 65c for `make test-app-e2e`; Phase 66 for `make app-bridge` and
# `make test-revealed-selection`). An older one (the 0.1.x bundle calls the old bridge
# path, which the bridge no longer serves) stops on the unlock screen with an invalid
# bridge reply error, and the suites only see a timeout.
# Exits 1 with a pt-BR message when the bundle is missing or older than any source.
# Usage: dev/check-test-bundle.sh <label for the message>

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
. "$ROOT/dev/env.sh"

LABEL="${1:-test-bundle}"
[[ "$LABEL" =~ ^[a-z0-9-]{1,40}$ ]] || { echo "check-test-bundle: rótulo inválido" >&2; exit 2; }
INDEX="$ROOT/app/dist-teste/index.html"
[[ -f "$INDEX" && ! -L "$INDEX" ]] \
    || { echo "$LABEL: falta app/dist-teste/index.html; rode 'make app-front'" >&2; exit 1; }
STALE="$(find "$ROOT/app/src" "$ROOT/app/index.html" "$ROOT/app/vite.config.js" \
    "$ROOT/app/package.json" "$ROOT/app/package-lock.json" \
    -newer "$INDEX" -print -quit)"
[[ -z "$STALE" ]] \
    || { echo "$LABEL: app/dist-teste é mais velho que ${STALE#"$ROOT"/}; rode 'make app-front' (refaz o pacote)" >&2; exit 1; }
