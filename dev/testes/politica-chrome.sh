#!/bin/bash -p
# `make teste-politica-chrome` (Fase 51a; V9.5, V9.6; achado crítico da auditoria do M9):
# a política do Chrome que o `instalar.sh` gera (a mesma do `.deb`, que usa o mesmo
# staging) desliga os manifestos de native messaging do perfil do usuário
# (`NativeMessagingUserLevelHosts: false`), com o Chrome real.
#
# O Chrome lê /etc/opt/chrome; aqui ele roda num bwrap em que /etc/opt é um tmpfs e
# /etc/opt/chrome é uma pasta deste teste em `.dev/politica-chrome.*` (nada do sistema
# muda, nenhum sudo). Perfil, HOME e manifestos do usuário também ficam nela; o perfil
# real do Chrome e `~/.config/google-chrome/` nunca são tocados.
#
# Quatro rodadas (`dev/testes/politica_chrome.py`, a partir dos scripts da auditoria):
#   1. precedência sem política (controle): o host do perfil é chamado (o ataque vale;
#      prova que o teste enxerga o ataque);
#   2. precedência com o `ward.json` gerado: o host do perfil NÃO é chamado e o do
#      sistema é;
#   3. o ataque inteiro da auditoria sem política (controle, fora do bwrap: o host não
#      tem /etc/opt/chrome; com ela, a rodada é pulada): o host do perfil faz exec do
#      `ward-nm` de teste e tira senhas do `wardd` de teste (`WARD_DEV_EXTENSAO=chrome`);
#      os logs do daemon e do Chrome sem padrão de `.dev/variacoes.txt`;
#   4. o mesmo ataque com o `ward.json` gerado, no bwrap: o host do atacante nem roda,
#      o do sistema roda e nada sai.
#
# Uso: politica-chrome.sh <wardd modo-teste> <ward-nm modo-teste> <ward> <wardd> <ward-nm> <ward-app>
#      (os quatro últimos sem `modo-teste`, para o `instalar.sh` gerar a política).
# Precisa de bwrap, openssl, `make dev-cofre`, `make dev-variacoes` e do Chrome em
# /opt/google/chrome; fora do `make check`.

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/ambiente.sh
. "$RAIZ/dev/ambiente.sh"
unset MAKEFLAGS MAKELEVEL MFLAGS
cd "$RAIZ"

uso="uso: politica-chrome.sh <wardd-teste> <ward-nm-teste> <ward> <wardd> <ward-nm> <ward-app>"
[ $# -eq 6 ] || { echo "$uso" >&2; exit 2; }
abs() { case "$1" in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$RAIZ" "$1" ;; esac; }
WARDD_T="$(abs "$1")" NM_T="$(abs "$2")"
B_WARD="$(abs "$3")" B_WARDD="$(abs "$4")" B_NM="$(abs "$5")" B_APP="$(abs "$6")"
for b in "$WARDD_T" "$NM_T" "$B_WARD" "$B_WARDD" "$B_NM" "$B_APP"; do
    [ -x "$b" ] || { echo "falta $b" >&2; exit 1; }
done
CHROME=/opt/google/chrome/chrome
[ -x "$CHROME" ] || { echo "falta o Chrome em $CHROME" >&2; exit 1; }
for p in bwrap openssl; do command -v "$p" >/dev/null || { echo "falta o programa: $p" >&2; exit 1; }; done
COFRE="$RAIZ/.dev/cofre-teste.ward"
LISTA="$RAIZ/.dev/variacoes.txt"
[ -f "$COFRE" ] && [ ! -L "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-cofre'" >&2; exit 1; }
[ -s "$LISTA" ] && [ ! -L "$LISTA" ] || { echo "falta a lista: rode 'make dev-variacoes'" >&2; exit 1; }
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
mkdir -p -m 0700 "$RAIZ/.dev"
TRAB="$(mktemp -d "$RAIZ/.dev/politica-chrome.XXXXXX")"
trap 'rm -rf -- "$TRAB"' EXIT
mkdir -m 0700 "$TRAB/etc-chrome"

falhas=0
ok() { echo "ok: $*"; }
falhou() { echo "FALHOU: $*"; falhas=$((falhas + 1)); }

# A política de verdade: o `instalar.sh` com DESTDIR (staging, como o `make deb`).
CRX="$TRAB/ward.crx"
ID="$(/usr/bin/python3 -I "$RAIZ/dev/testes/crx_falso.py" "$CRX")"
/usr/bin/make -s -C "$RAIZ" install DESTDIR="$TRAB/raiz" EXTENSAO_ID="$ID" EXTENSAO_CRX="$CRX" \
    BIN_WARD="$B_WARD" BIN_WARDD="$B_WARDD" BIN_NM="$B_NM" BIN_APP="$B_APP" WARD_UID= >/dev/null
POLITICA="$TRAB/raiz/etc/opt/chrome/policies/managed/ward.json"
r="$(/usr/bin/python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("NativeMessagingUserLevelHosts"))' "$POLITICA")"
[ "$r" = False ] && ok "a política gerada pelo instalar.sh tem NativeMessagingUserLevelHosts false" ||
    falhou "a política gerada não desliga os manifestos do usuário ($r)"
# Chave da extensão de teste (só para o id; descartada no fim).
openssl genrsa -out "$TRAB/k.pem" 2048 2>/dev/null

rodada() {
    env -i HOME="$TRAB" PATH=/usr/bin:/bin LANG=C.UTF-8 XDG_RUNTIME_DIR="$RUNTIME" \
        bwrap --dev-bind / / --tmpfs /etc/opt --bind "$TRAB/etc-chrome" /etc/opt/chrome \
        --die-with-parent -- /usr/bin/python3 -I -B "$RAIZ/dev/testes/politica_chrome.py" "$RAIZ" "$TRAB" "$@"
}
campo() { /usr/bin/python3 -I -c 'import json,sys; print(json.loads(sys.argv[1]).get(sys.argv[2]))' "$1" "$2"; }

echo "==> 1. precedência sem política (controle)"
r="$(rodada precedencia -)" || falhou "rodada 1 saiu ≠ 0"
echo "    $r"
[ "$(campo "$r" usuario)" = True ] && ok "sem a política, o manifesto plantado no perfil vence (o ataque da auditoria)" ||
    falhou "controle: o host do perfil não foi chamado; o teste não enxerga o ataque"

echo "==> 2. precedência com a política do Ward"
r="$(rodada precedencia "$POLITICA")" || falhou "rodada 2 saiu ≠ 0"
echo "    $r"
[ "$(campo "$r" usuario)" = False ] && ok "com a política, o manifesto plantado no perfil não é chamado" ||
    falhou "com a política, o host do perfil foi chamado"
[ "$(campo "$r" sistema)" = True ] && ok "com a política, o manifesto do sistema é chamado" ||
    falhou "com a política, o host do sistema não foi chamado"

echo "==> 3. ataque da auditoria sem política (controle, fora do bwrap)"
r="$(env -i HOME="$TRAB" PATH=/usr/bin:/bin LANG=C.UTF-8 XDG_RUNTIME_DIR="$RUNTIME" \
    /usr/bin/python3 -I -B "$RAIZ/dev/testes/politica_chrome.py" "$RAIZ" "$TRAB" roubo - "$WARDD_T" "$NM_T")" ||
    falhou "rodada 3 saiu ≠ 0"
echo "    $r"
if [ "$(campo "$r" pulado)" != None ]; then
    echo "pulado: rodada 3 ($(campo "$r" pulado))"
else
    [ "$(campo "$r" atacante_rodou)" = True ] && [ "$(campo "$r" com_segredo)" != 0 ] &&
        ok "sem a política, o host do atacante faz exec do ward-nm e tira senhas (controle)" ||
        falhou "controle: o ataque da auditoria não reproduziu"
    for f in "$TRAB/roubo-sem-politica/wardd.log" "$TRAB/roubo-sem-politica/chrome.log"; do
        if [ -f "$f" ] && grep -qaF -f "$LISTA" -- "$f"; then
            falhou "padrão de .dev/variacoes.txt em ${f#"$TRAB"/}"
        else
            ok "${f#"$TRAB"/} sem padrão de .dev/variacoes.txt"
        fi
    done
fi

echo "==> 4. ataque da auditoria com a política do Ward"
r="$(rodada roubo "$POLITICA" "$WARDD_T" "$NM_T")" || falhou "rodada 4 saiu ≠ 0"
echo "    $r"
[ "$(campo "$r" atacante_rodou)" = False ] && ok "com a política, o host do atacante (no perfil) nem roda" ||
    falhou "com a política, o host do atacante rodou"
[ "$(campo "$r" sistema_rodou)" = True ] && ok "com a política, o Chrome abre o host do sistema" ||
    falhou "com a política, o host do sistema não rodou"
[ "$(campo "$r" com_segredo)" = 0 ] && ok "com a política, nenhuma senha sai" || falhou "com a política, saiu senha"

if [ "$falhas" -ne 0 ]; then
    echo "teste-politica-chrome: $falhas falha(s)"
    exit 1
fi
echo "teste-politica-chrome: tudo ok"
