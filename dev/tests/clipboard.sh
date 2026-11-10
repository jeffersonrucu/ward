#!/bin/bash -p
# Fase 39f (S2, S12; V5.4, V5.5): o "copiar" do app na sessão gráfica de verdade, sem
# disco e sem `wl-copy`.
#
# Compila os testes do `app/src-tauri` (o mesmo `target/` do `make app-check`) e passa o
# binário a `dev/tests/clipboard.py`, que roda o teste ignorado
# `area_gtk::testes::sessao_real` (a área do GTK de verdade, com o `Copiador` do núcleo)
# sob `strace -f` e confere, só com valores falsos gerados na hora:
# - antes, o autoteste: o vigia de arquivos acha o valor que o `wl-copy` grava em
#   `/tmp/wl-copy-buffer-*/` (o ataque da auditoria do M5 é achado);
# - nenhum arquivo em `/tmp`, `/var/tmp` nem `$XDG_RUNTIME_DIR` recebe o valor
#   (inotify durante a rodada e varredura no fim), o processo do teste não executa
#   nenhum programa (nada de `wl-copy`) nem abre arquivo para escrita nessas pastas;
# - V5.4: o `wl-paste` devolve o valor copiado, com `x-kde-passwordManagerHint=secret`;
# - V5.5: some depois do prazo; o texto que o usuário copia depois fica;
# - a trava vinda de fora (limpar fora da thread principal) tira o valor na hora;
#   "copiar" fora da thread principal é recusado; ao sair do app o valor some.
#
# O compositor (COSMIC) só aceita a seleção de um cliente Wayland com foco de teclado,
# que um teste sem clique não tem: a rodada completa usa o backend X11 do GTK no
# Xwayland da sessão (o COSMIC repassa a seleção para o Wayland, e o `wl-paste` lê por
# lá). Uma 2ª rodada com o backend Wayland confere só o strace e o vigia (sem foco o
# compositor recusa a seleção; o handoff da Fase 39f explica). No fim a área de
# transferência é limpa.
#
# Exige `strace`, `wl-paste`/`wl-copy` (só o teste usa, para ler e para imitar o
# usuário), python3-gi (GTK 3), `libwebkit2gtk-4.1-dev` (ou PKG_CONFIG_PATH) e uma
# sessão com `WAYLAND_DISPLAY` e `DISPLAY`; fora do `make check`.
# Uso: dev/tests/clipboard.sh <cargo>   (ou `make test-clipboard`)

set -euo pipefail
export PATH=/usr/bin:/bin
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$RAIZ/dev/env.sh"

CARGO="${1:?uso: clipboard.sh <cargo>}"
[[ -x "$CARGO" ]] || { echo "area-transferencia: cargo não encontrado em $CARGO" >&2; exit 1; }
for f in strace wl-paste wl-copy python3; do
    command -v "$f" > /dev/null || { echo "area-transferencia: falta $f" >&2; exit 1; }
done

JSON="$(cd "$RAIZ/app/src-tauri" && "$CARGO" test --all-features --locked --no-run \
    --message-format=json --quiet)"
BIN="$(printf '%s\n' "$JSON" | /usr/bin/python3 -I -B -c '
import json, sys
for linha in sys.stdin:
    try:
        m = json.loads(linha)
    except ValueError:
        continue
    alvo = m.get("target", {})
    if m.get("reason") == "compiler-artifact" and m.get("executable") \
            and alvo.get("name") == "ward-app" and m.get("profile", {}).get("test"):
        print(m["executable"])
')"
[[ -n "$BIN" && -x "$BIN" ]] || { echo "area-transferencia: binário de teste do app não encontrado" >&2; exit 1; }

exec /usr/bin/python3 -I -B "$RAIZ/dev/tests/clipboard.py" "$BIN"
