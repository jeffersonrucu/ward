#!/bin/bash -p
# `make extension-crx` (Phase 52; D5, D18): packs `extension/dist/` (from
# `make extension`) as a CRX3 signed with the extension's private key, by Chrome itself
# (`--pack-extension`; no hand-made signature). Runs as the user.
#
# Chrome starts with HOME and profile in a temporary dir in target/ (never the user's
# profile) and only reads the key by its path. The package goes to
# target/extension/ward.crx and is verified by the install's `crx.py`: the signed id must
# be the extension id constant of `ward-nm` (another key gives another id and is refused).
#
# Variables: EXTENSION_KEY (default .dev/extension/key.pem, 0600, the user's),
# EXTENSION_DIST (default extension/dist), CHROME (default /opt/google/chrome/chrome),
# CRX_OUTPUT (default target/extension/ward.crx).

export PATH=/usr/bin:/bin
export LC_ALL=C
set -euo pipefail
umask 077

HERE="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
ROOT="${HERE%/*}"
error() { printf 'ward-extensao: %s\n' "$*" >&2; exit 1; }

[ $# -eq 0 ] || error "uso: pack-extension.sh (as variáveis vêm do ambiente)"
[ "$(id -u)" != 0 ] || error "rode como o usuário, sem sudo"

KEY="${EXTENSION_KEY:-$ROOT/.dev/extension/key.pem}"
DIST="${EXTENSION_DIST:-$ROOT/extension/dist}"
CHROME="${CHROME:-/opt/google/chrome/chrome}"
OUTPUT="${CRX_OUTPUT:-$ROOT/target/extension/ward.crx}"
for v in "$KEY" "$DIST" "$CHROME" "$OUTPUT"; do
    [[ "$v" == /* ]] || error "caminho relativo: $v"
done

[ ! -L "$KEY" ] && [ -f "$KEY" ] || error "a chave $KEY não existe ou é link (veja o handoff da Fase 41)"
[ "$(stat -c '%u %a' -- "$KEY")" = "$(id -u) 600" ] || error "a chave $KEY tem de ser do usuário e 0600"
[ ! -L "$DIST" ] && [ -f "$DIST/manifest.json" ] || error "$DIST/manifest.json não existe (rode 'make extension')"
[ -z "$(find "$DIST" -type l -print -quit)" ] || error "$DIST tem link simbólico"
[ -x "$CHROME" ] || error "Chrome não encontrado em $CHROME"
EXPECTED="$(sed -n -E 's/^pub const EXTENSION_ID: &str = "([a-p]{32})";$/\1/p' "$ROOT/crates/ward-nm/src/lib.rs")"
[[ "$EXPECTED" =~ ^[a-p]{32}$ ]] || error "EXTENSION_ID não achado em crates/ward-nm/src/lib.rs"

mkdir -p -m 0755 -- "$ROOT/target"
TMP="$(mktemp -d "$ROOT/target/extension-crx.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
mkdir -m 0700 -- "$TMP/home" "$TMP/perfil"
cp -r -- "$DIST" "$TMP/ward"
env -i HOME="$TMP/home" PATH=/usr/bin:/bin LANG=C.UTF-8 /usr/bin/timeout 120 "$CHROME" \
    --headless=new --no-first-run --no-default-browser-check --disable-sync \
    --user-data-dir="$TMP/perfil" \
    --pack-extension="$TMP/ward" --pack-extension-key="$KEY" >/dev/null 2>"$TMP/chrome.log" ||
    { cat "$TMP/chrome.log" >&2; error "o Chrome não empacotou a extensão"; }
[ -f "$TMP/ward.crx" ] && [ ! -L "$TMP/ward.crx" ] || error "o Chrome não gerou o pacote"
[ ! -e "$TMP/ward.pem" ] || error "o Chrome gerou outra chave (a chave não foi usada)"

parsed="$(/usr/bin/python3 -I -B "$HERE/crx.py" "$TMP/ward.crx")" || error "pacote recusado pelo crx.py"
read -r id version <<<"$parsed"
[ "$id" = "$EXPECTED" ] || error "o pacote é da extensão $id, não do EXTENSION_ID $EXPECTED (chave errada?)"

mkdir -p -m 0755 -- "${OUTPUT%/*}"
install -m 0644 -- "$TMP/ward.crx" "$OUTPUT"
printf 'pacote: %s (versão %s)\n' "$OUTPUT" "$version"
printf 'EXTENSION_ID=%s EXTENSION_CRX=%s\n' "$id" "$OUTPUT"
