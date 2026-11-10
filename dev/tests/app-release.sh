#!/bin/bash -p
# V5.6 (S11, D7): o binário de release do app não contém a ponte de teste, nem o
# caminho do daemon de teste, nem `WARD_DEV_DIR`. Uso: dev/tests/app-release.sh
# <binário> (ou `make app-check`, que compila o release antes).
#
# Devtools fora do release: garantido pelo teste `configuracao_tauri` (a feature
# `tauri/devtools` só entra com `test-mode`) e pelo `.devtools(cfg!(feature = "test-mode"))`
# da janela.

set -euo pipefail
export PATH=/usr/bin:/bin

BINARIO="${1:?uso: app-release.sh <binário de release>}"
[[ -f "$BINARIO" && -x "$BINARIO" ]] || { echo "app-release: $BINARIO não é um executável" >&2; exit 1; }

# `strings` vai para um arquivo antes do `grep -q`: com `pipefail`, o SIGPIPE do
# `strings` quando o `grep -q` acha cedo faria o achado parecer ausência.
TEXTOS="$(mktemp)"
trap 'rm -f "$TEXTOS"' EXIT
strings -a -n 4 -- "$BINARIO" > "$TEXTOS"

falhou=0
for marca in ward-ponte-teste ponte-teste test-bridge /ponte/ WARD_DEV_DIR WARD_DEV_APP ward-teste Vitrine Showcase 'token='; do
    if grep -qF -- "$marca" "$TEXTOS"; then
        echo "app-release: o binário de release contém \"$marca\"" >&2
        falhou=1
    fi
done
# O caminho do socket de produção tem de estar lá (o binário é mesmo o do app).
grep -qF -- 'wardd.sock' "$TEXTOS" || {
    echo "app-release: 'wardd.sock' não encontrado; o binário conferido é mesmo o ward-app?" >&2
    falhou=1
}
[[ $falhou -eq 0 ]] && echo "app-release: sem ponte, sem caminho de teste e sem WARD_DEV_DIR no binário de release"
exit $falhou
