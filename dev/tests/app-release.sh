#!/bin/bash -p
# V5.6 (S11, D7): the app's release binary contains neither the test bridge, nor the
# path of the test daemon, nor `WARD_DEV_DIR`. Usage: dev/tests/app-release.sh
# <binary> (or `make app-check`, which builds the release first).
#
# Devtools out of the release: guaranteed by the `configuracao_tauri` test (the
# `tauri/devtools` feature only comes in with `test-mode`) and by the window's
# `.devtools(cfg!(feature = "test-mode"))`.

set -euo pipefail
export PATH=/usr/bin:/bin

BINARY="${1:?uso: app-release.sh <binário de release>}"
[[ -f "$BINARY" && -x "$BINARY" ]] || { echo "app-release: $BINARY não é um executável" >&2; exit 1; }

# `strings` goes to a file before `grep -q`: with `pipefail`, the SIGPIPE of
# `strings` when `grep -q` finds early would make the finding look like an absence.
STRINGS_FILE="$(mktemp)"
trap 'rm -f "$STRINGS_FILE"' EXIT
strings -a -n 4 -- "$BINARY" > "$STRINGS_FILE"

failed=0
for mark in ward-ponte-teste ponte-teste test-bridge /ponte/ WARD_DEV_DIR WARD_DEV_APP ward-teste Vitrine Showcase 'token='; do
    if grep -qF -- "$mark" "$STRINGS_FILE"; then
        echo "app-release: o binário de release contém \"$mark\"" >&2
        failed=1
    fi
done
# The production socket path has to be there (the binary really is the app's).
grep -qF -- 'wardd.sock' "$STRINGS_FILE" || {
    echo "app-release: 'wardd.sock' não encontrado; o binário conferido é mesmo o ward-app?" >&2
    failed=1
}
[[ $failed -eq 0 ]] && echo "app-release: sem ponte, sem caminho de teste e sem WARD_DEV_DIR no binário de release"
exit $failed
