#!/bin/bash -p
# `make deb` (Fase 52; D18, D47, D60, D63; V9.9): o `.deb` do Ward por `dpkg-deb`, a
# partir da mesma árvore do `make install`. Roda como o usuário, nunca como root, e
# nunca chama o cargo (os artefatos vêm do `make release`).
#
#   1. `instalar.sh instalar` com DESTDIR=<DEB_DIR>/raiz: o staging, com as mesmas
#      conferências do `make install` (artefato `modo-teste` recusado, ELF, sem link,
#      pacote da extensão com o id de EXTENSAO_ID, PREFIX).
#   2. Cópia do staging para <DEB_DIR>/pacote sem os bits setuid/setgid: no pacote todo
#      arquivo é root:root (`--root-owner-group`) e um 2755 root:root daria o gid do
#      root a quem rodasse o binário. O 2755 com o grupo certo entra pelo
#      `dpkg-statoverride` do preinst.
#   3. DEBIAN/: control (Depends com sshpass), preinst (grupos de `grupos.sh`, políticas
#      do Chrome de `chrome.sh` e as pastas comuns de `arvore.txt`), postinst (aviso de
#      `chrome.sh`), prerm e postrm (marca dos grupos de `grupos.sh`) de `instalacao/deb/`.
#   4. `dpkg-deb --build`, e a conferência do resultado: a lista de `dpkg-deb -c` é a do
#      staging (V9.9), todo caminho é root/root, nenhum bit setuid/setgid nem escrita
#      para grupo e outros, Depends com sshpass.
#
# Variáveis (o Makefile exporta): DEB_DIR (padrão: target/deb; caminho absoluto),
# EXTENSAO_ID, EXTENSAO_CRX, BIN_WARD, BIN_WARDD, BIN_APP, BIN_NM.

export PATH=/usr/bin:/bin
export LC_ALL=C
set -euo pipefail
umask 022

AQUI="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
RAIZ="${AQUI%/*}"

erro() { printf 'ward-deb: %s\n' "$*" >&2; exit 1; }
passo() { printf '==> %s\n' "$*"; }

[ $# -eq 0 ] || erro "uso: deb.sh (sem argumentos; as variáveis vêm do ambiente)"
[ "$(id -u)" != 0 ] || erro "rode como o usuário, sem sudo: o pacote é montado sem root ('make deb')"
command -v dpkg-deb >/dev/null || erro "falta o dpkg-deb"

DEB_DIR="${DEB_DIR:-$RAIZ/target/deb}"
DEB_DIR="${DEB_DIR%/}"
[[ "$DEB_DIR" == /* ]] || erro "DEB_DIR tem de ser um caminho absoluto"
[[ "$DEB_DIR" != *//* && "$DEB_DIR" != */./* && "$DEB_DIR" != */../* && "$DEB_DIR" != */. && "$DEB_DIR" != */.. ]] ||
    erro "DEB_DIR com '//', '.' ou '..'"
mkdir -p -m 0755 -- "$DEB_DIR"
[ "$(cd -P -- "$DEB_DIR" && pwd -P)" = "$DEB_DIR" ] || erro "DEB_DIR passa por link simbólico"

# Versão do workspace e arquitetura.
VERSAO="$(awk '/^\[workspace.package\]/{s=1; next} /^\[/{s=0} s && /^version *=/{gsub(/[" ]/, "", $0); sub(/^version=/, "", $0); print; exit}' "$RAIZ/Cargo.toml")"
[[ "$VERSAO" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || erro "versão do workspace fora do formato ($VERSAO)"
ARQ="$(dpkg --print-architecture)"
[[ "$ARQ" =~ ^[a-z0-9-]+$ ]] || erro "arquitetura fora do formato"

STAGING="$DEB_DIR/raiz"
PACOTE="$DEB_DIR/pacote"
SAIDA="$DEB_DIR/ward_${VERSAO}_${ARQ}.deb"
rm -rf -- "$STAGING" "$PACOTE" "$SAIDA"

passo "staging: make install DESTDIR=$STAGING"
DESTDIR="$STAGING" PREFIX=/usr/local "$AQUI/instalar.sh" instalar

passo "pacote em $PACOTE (sem setuid/setgid; o 2755 vem do dpkg-statoverride)"
cp -a -- "$STAGING" "$PACOTE"
find "$PACOTE" -perm /6000 -exec chmod ug-s -- {} +
find "$PACOTE" -type d -exec chmod 0755 -- {} +

# Scripts do mantenedor. As marcas `#@GRUPOS@` (grupos.sh), `#@CHROME@` (chrome.sh),
# `#@DESPEJO@` (despejo.sh, Fase 51c) e `#@PASTAS@` (pastas comuns de arvore.txt) viram
# o conteúdo; cada marca que um modelo
# usa aparece uma vez só, e nenhuma sobra.
install -d -m 0755 -- "$PACOTE/DEBIAN"
/usr/bin/python3 -I -B - "$AQUI/deb" "$AQUI/grupos.sh" "$AQUI/chrome.sh" "$AQUI/despejo.sh" "$AQUI/arvore.txt" "$PACOTE/DEBIAN" <<'EOF'
import re, sys
modelos, grupos, chrome, despejo, arvore, saida = sys.argv[1:]
pastas = []
for linha in open(arvore, encoding="utf-8"):
    campos = linha.split()
    if not campos or campos[0].startswith("#"):
        continue
    if campos[0] == "D":
        if not re.fullmatch(r"[a-z][a-z0-9._/-]*", campos[4]) or ".." in campos[4]:
            sys.exit(f"pasta fora do formato: {campos[4]}")
        pastas.append(f"    {campos[4]}")
trocas = {
    "#@GRUPOS@\n": open(grupos, encoding="utf-8").read(),
    "#@CHROME@\n": open(chrome, encoding="utf-8").read(),
    "#@DESPEJO@\n": open(despejo, encoding="utf-8").read(),
    "#@PASTAS@\n": "\n".join(pastas) + "\n",
}
for s in ("preinst", "postinst", "prerm", "postrm"):
    texto = open(f"{modelos}/{s}", encoding="utf-8").read()
    for marca, conteudo in trocas.items():
        n = texto.count(marca)
        if n > 1:
            sys.exit(f"marca {marca.strip()} aparece {n} vezes no {s}")
        texto = texto.replace(marca, conteudo)
    if "#@" in texto:
        sys.exit(f"marca de modelo desconhecida no {s}")
    open(f"{saida}/{s}", "w", encoding="utf-8").write(texto)
EOF
for s in preinst postinst prerm postrm; do
    chmod 0755 -- "$PACOTE/DEBIAN/$s"
    bash -n -- "$PACOTE/DEBIAN/$s" || erro "DEBIAN/$s com erro de sintaxe"
done
TAM="$(du -sk --exclude=DEBIAN -- "$PACOTE" | cut -f1)"
cat >"$PACOTE/DEBIAN/control" <<EOF
Package: ward
Version: $VERSAO
Architecture: $ARQ
Maintainer: Ward <ward@localhost>
Installed-Size: $TAM
Depends: sshpass, openssh-client, systemd, libwebkit2gtk-4.1-0
Section: admin
Priority: optional
Description: cofre de credenciais offline, só de trabalho
 Daemon wardd (socket do sistema por usuário), CLI ward, app desktop ward-app,
 host de native messaging ward-nm e a extensão do Chrome (instalada pela
 política). Entrega credenciais ao agente sem que ele veja o segredo.
EOF
chmod 0644 -- "$PACOTE/DEBIAN/control"

passo "dpkg-deb --build"
dpkg-deb --root-owner-group -Zxz --build "$PACOTE" "$SAIDA" >/dev/null

passo "conferindo o pacote"
# V9.9: a mesma lista de caminhos do staging.
esperado="$(cd "$STAGING" && find . -mindepth 1 -printf '%P\n' | sort)"
lista="$(dpkg-deb -c "$SAIDA")"
real="$(awk '{print $6}' <<<"$lista" | sed -e 's#^\./##' -e 's#/$##' | grep -v '^$' | sort)"
[ "$esperado" = "$real" ] || { diff <(printf '%s\n' "$esperado") <(printf '%s\n' "$real") >&2 || true; erro "a lista do pacote difere do staging"; }
# Donos, bits especiais e escrita para grupo e outros; nenhum link.
awk '$2 != "root/root" {print "dono: " $0; ruim = 1}
     substr($1, 1, 1) == "l" {print "link: " $0; ruim = 1}
     $1 ~ /[sStT]/ {print "bit especial: " $0; ruim = 1}
     substr($1, 6, 1) == "w" || substr($1, 9, 1) == "w" {print "escrita: " $0; ruim = 1}
     END {exit ruim}' <<<"$lista" >&2 || erro "pacote com dono, link, bit especial ou escrita fora do esperado"
dep="$(dpkg-deb -f "$SAIDA" Depends)"
[[ ", $dep," == *", sshpass,"* ]] || erro "o pacote não declara a dependência de sshpass"
for s in preinst postinst prerm postrm; do
    dpkg-deb -I "$SAIDA" "$s" >/dev/null || erro "o pacote não tem $s"
done

passo "pronto: $SAIDA"
sha256sum -- "$SAIDA"
