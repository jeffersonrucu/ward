#!/bin/bash -p
# `make extension-crx` (Fase 52; D5, D18): empacota `extension/dist/` (do `make extension`)
# como CRX3 assinado com a chave privada da extensão, pelo próprio Chrome
# (`--pack-extension`; nada de assinatura feita à mão). Roda como o usuário.
#
# O Chrome sobe com HOME e perfil numa pasta temporária em target/ (nunca o perfil do
# usuário) e só lê a chave pelo caminho. O pacote sai em target/extension/ward.crx e é
# conferido pelo `crx.py` da instalação: o id assinado tem de ser o `ID_EXTENSAO` do
# `ward-nm` (outra chave dá outro id e é recusada).
#
# Variáveis: EXTENSION_KEY (padrão .dev/extension/key.pem, 0600, do usuário),
# EXTENSAO_DIST (padrão extension/dist), CHROME (padrão /opt/google/chrome/chrome),
# CRX_SAIDA (padrão target/extension/ward.crx).

export PATH=/usr/bin:/bin
export LC_ALL=C
set -euo pipefail
umask 077

AQUI="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
RAIZ="${AQUI%/*}"
erro() { printf 'ward-extensao: %s\n' "$*" >&2; exit 1; }

[ $# -eq 0 ] || erro "uso: pack-extension.sh (as variáveis vêm do ambiente)"
[ "$(id -u)" != 0 ] || erro "rode como o usuário, sem sudo"

CHAVE="${EXTENSION_KEY:-$RAIZ/.dev/extension/key.pem}"
DIST="${EXTENSAO_DIST:-$RAIZ/extension/dist}"
CHROME="${CHROME:-/opt/google/chrome/chrome}"
SAIDA="${CRX_SAIDA:-$RAIZ/target/extension/ward.crx}"
for v in "$CHAVE" "$DIST" "$CHROME" "$SAIDA"; do
    [[ "$v" == /* ]] || erro "caminho relativo: $v"
done

[ ! -L "$CHAVE" ] && [ -f "$CHAVE" ] || erro "a chave $CHAVE não existe ou é link (veja o handoff da Fase 41)"
[ "$(stat -c '%u %a' -- "$CHAVE")" = "$(id -u) 600" ] || erro "a chave $CHAVE tem de ser do usuário e 0600"
[ ! -L "$DIST" ] && [ -f "$DIST/manifest.json" ] || erro "$DIST/manifest.json não existe (rode 'make extension')"
[ -z "$(find "$DIST" -type l -print -quit)" ] || erro "$DIST tem link simbólico"
[ -x "$CHROME" ] || erro "Chrome não encontrado em $CHROME"
ESPERADO="$(sed -n -E 's/^pub const ID_EXTENSAO: &str = "([a-p]{32})";$/\1/p' "$RAIZ/crates/ward-nm/src/lib.rs")"
[[ "$ESPERADO" =~ ^[a-p]{32}$ ]] || erro "ID_EXTENSAO não achado em crates/ward-nm/src/lib.rs"

mkdir -p -m 0755 -- "$RAIZ/target"
TMP="$(mktemp -d "$RAIZ/target/extension-crx.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
mkdir -m 0700 -- "$TMP/home" "$TMP/perfil"
cp -r -- "$DIST" "$TMP/ward"
env -i HOME="$TMP/home" PATH=/usr/bin:/bin LANG=C.UTF-8 /usr/bin/timeout 120 "$CHROME" \
    --headless=new --no-first-run --no-default-browser-check --disable-sync \
    --user-data-dir="$TMP/perfil" \
    --pack-extension="$TMP/ward" --pack-extension-key="$CHAVE" >/dev/null 2>"$TMP/chrome.log" ||
    { cat "$TMP/chrome.log" >&2; erro "o Chrome não empacotou a extensão"; }
[ -f "$TMP/ward.crx" ] && [ ! -L "$TMP/ward.crx" ] || erro "o Chrome não gerou o pacote"
[ ! -e "$TMP/ward.pem" ] || erro "o Chrome gerou outra chave (a chave não foi usada)"

lido="$(/usr/bin/python3 -I -B "$AQUI/crx.py" "$TMP/ward.crx")" || erro "pacote recusado pelo crx.py"
read -r id versao <<<"$lido"
[ "$id" = "$ESPERADO" ] || erro "o pacote é da extensão $id, não do ID_EXTENSAO $ESPERADO (chave errada?)"

mkdir -p -m 0755 -- "${SAIDA%/*}"
install -m 0644 -- "$TMP/ward.crx" "$SAIDA"
printf 'pacote: %s (versão %s)\n' "$SAIDA" "$versao"
printf 'EXTENSION_ID=%s EXTENSION_CRX=%s\n' "$id" "$SAIDA"
