#!/bin/bash -p
# `make deb` (Phase 52; D18, D47, D60, D63; V9.9): Ward's `.deb` by `dpkg-deb`, from
# the same tree as `make install`. Runs as the user, never as root, and never calls
# cargo (the artifacts come from `make release`).
#
#   1. `install.sh install` with DESTDIR=<DEB_DIR>/root: the staging, with the same
#      checks as `make install` (`test-mode` artifact refused, ELF, no link, extension
#      package with the id of EXTENSION_ID, PREFIX).
#   2. Copy of the staging to <DEB_DIR>/package without the setuid/setgid bits: in the
#      package every file is root:root (`--root-owner-group`) and a 2755 root:root would
#      give root's gid to whoever ran the binary. The 2755 with the right group comes
#      from the `dpkg-statoverride` of the preinst.
#   3. DEBIAN/: control (Depends with sshpass), preinst (groups from `groups.sh`, Chrome
#      policies from `chrome.sh`, the common dirs of `tree.txt` and the stop of the
#      running `wardd@*` from `upgrade.sh`, m2:DT16), postinst (warning from `chrome.sh`,
#      0.1.x paths and sockets from `upgrade.sh`, m2:DT15), prerm and postrm (mark of the
#      groups from `groups.sh`) from `install/deb/`.
#   4. `dpkg-deb --build`, and the check of the result: the list of `dpkg-deb -c` is the
#      staging's (V9.9), every path is root/root, no setuid/setgid bit nor write for
#      group and others, Depends with sshpass.
#
# Variables (the Makefile exports them): DEB_DIR (default: target/deb; absolute path),
# EXTENSION_ID, EXTENSION_CRX, BIN_WARD, BIN_WARDD, BIN_APP, BIN_NM.

export PATH=/usr/bin:/bin
export LC_ALL=C
set -euo pipefail
umask 022

HERE="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
ROOT="${HERE%/*}"

error() { printf 'ward-deb: %s\n' "$*" >&2; exit 1; }
step() { printf '==> %s\n' "$*"; }

[ $# -eq 0 ] || error "uso: deb.sh (sem argumentos; as variáveis vêm do ambiente)"
[ "$(id -u)" != 0 ] || error "rode como o usuário, sem sudo: o pacote é montado sem root ('make deb')"
command -v dpkg-deb >/dev/null || error "falta o dpkg-deb"

DEB_DIR="${DEB_DIR:-$ROOT/target/deb}"
DEB_DIR="${DEB_DIR%/}"
[[ "$DEB_DIR" == /* ]] || error "DEB_DIR tem de ser um caminho absoluto"
[[ "$DEB_DIR" != *//* && "$DEB_DIR" != */./* && "$DEB_DIR" != */../* && "$DEB_DIR" != */. && "$DEB_DIR" != */.. ]] ||
    error "DEB_DIR com '//', '.' ou '..'"
mkdir -p -m 0755 -- "$DEB_DIR"
[ "$(cd -P -- "$DEB_DIR" && pwd -P)" = "$DEB_DIR" ] || error "DEB_DIR passa por link simbólico"

# Workspace version and architecture.
VERSION="$(awk '/^\[workspace.package\]/{s=1; next} /^\[/{s=0} s && /^version *=/{gsub(/[" ]/, "", $0); sub(/^version=/, "", $0); print; exit}' "$ROOT/Cargo.toml")"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || error "versão do workspace fora do formato ($VERSION)"
ARCH="$(dpkg --print-architecture)"
[[ "$ARCH" =~ ^[a-z0-9-]+$ ]] || error "arquitetura fora do formato"

STAGING="$DEB_DIR/root"
PACKAGE="$DEB_DIR/package"
OUTPUT="$DEB_DIR/ward_${VERSION}_${ARCH}.deb"
rm -rf -- "$STAGING" "$PACKAGE" "$OUTPUT"

step "staging: make install DESTDIR=$STAGING"
DESTDIR="$STAGING" PREFIX=/usr/local "$HERE/install.sh" install

step "pacote em $PACKAGE (sem setuid/setgid; o 2755 vem do dpkg-statoverride)"
cp -a -- "$STAGING" "$PACKAGE"
find "$PACKAGE" -perm /6000 -exec chmod ug-s -- {} +
find "$PACKAGE" -type d -exec chmod 0755 -- {} +

# Maintainer scripts. The markers `#@GROUPS@` (groups.sh), `#@CHROME@` (chrome.sh),
# `#@DUMP@` (dump.sh, Phase 51c), `#@UPGRADE@` (upgrade.sh, m2:DT15/DT16) and `#@DIRS@`
# (common dirs of tree.txt) become the content; each marker a template uses appears only
# once, and none is left over.
install -d -m 0755 -- "$PACKAGE/DEBIAN"
/usr/bin/python3 -I -B - "$HERE/deb" "$HERE/groups.sh" "$HERE/chrome.sh" "$HERE/dump.sh" "$HERE/upgrade.sh" "$HERE/tree.txt" "$PACKAGE/DEBIAN" <<'EOF'
import re, sys
templates, groups, chrome, dump, upgrade, tree, out_dir = sys.argv[1:]
dirs = []
for line in open(tree, encoding="utf-8"):
    fields = line.split()
    if not fields or fields[0].startswith("#"):
        continue
    if fields[0] == "D":
        if not re.fullmatch(r"[a-z][a-z0-9._/-]*", fields[4]) or ".." in fields[4]:
            sys.exit(f"pasta fora do formato: {fields[4]}")
        dirs.append(f"    {fields[4]}")
replacements = {
    "#@GROUPS@\n": open(groups, encoding="utf-8").read(),
    "#@CHROME@\n": open(chrome, encoding="utf-8").read(),
    "#@DUMP@\n": open(dump, encoding="utf-8").read(),
    "#@UPGRADE@\n": open(upgrade, encoding="utf-8").read(),
    "#@DIRS@\n": "\n".join(dirs) + "\n",
}
for s in ("preinst", "postinst", "prerm", "postrm"):
    text = open(f"{templates}/{s}", encoding="utf-8").read()
    for marker, content in replacements.items():
        n = text.count(marker)
        if n > 1:
            sys.exit(f"marca {marker.strip()} aparece {n} vezes no {s}")
        text = text.replace(marker, content)
    if "#@" in text:
        sys.exit(f"marca de modelo desconhecida no {s}")
    open(f"{out_dir}/{s}", "w", encoding="utf-8").write(text)
EOF
for s in preinst postinst prerm postrm; do
    chmod 0755 -- "$PACKAGE/DEBIAN/$s"
    bash -n -- "$PACKAGE/DEBIAN/$s" || error "DEBIAN/$s com erro de sintaxe"
done
SIZE="$(du -sk --exclude=DEBIAN -- "$PACKAGE" | cut -f1)"
cat >"$PACKAGE/DEBIAN/control" <<EOF
Package: ward
Version: $VERSION
Architecture: $ARCH
Maintainer: Jefferson Oliveira <jefferoidtex7@gmail.com>
Installed-Size: $SIZE
Depends: sshpass, openssh-client, systemd, libwebkit2gtk-4.1-0
Section: admin
Priority: optional
Homepage: https://github.com/jeffersonrucu/ward
Description: offline work-credential vault that lets AI agents use secrets unseen
 Ward keeps work credentials (databases, SSH servers, web logins, API tokens,
 AWS) in an encrypted offline vault and hands them to Claude Code and other
 agents without ever showing the secret: the wardd daemon runs the command with
 the credential in the child's environment and masks it in the output.
 .
 Includes the ward CLI, the wardd daemon (systemd socket per user), the
 ward-app desktop app, the ward-nm native messaging host and the Chrome
 extension, installed by policy. Each use can require approval with a PIN.
EOF
chmod 0644 -- "$PACKAGE/DEBIAN/control"

step "dpkg-deb --build"
dpkg-deb --root-owner-group -Zxz --build "$PACKAGE" "$OUTPUT" >/dev/null

step "conferindo o pacote"
# V9.9: the same list of paths as the staging.
expected="$(cd "$STAGING" && find . -mindepth 1 -printf '%P\n' | sort)"
listing="$(dpkg-deb -c "$OUTPUT")"
actual="$(awk '{print $6}' <<<"$listing" | sed -e 's#^\./##' -e 's#/$##' | grep -v '^$' | sort)"
[ "$expected" = "$actual" ] || { diff <(printf '%s\n' "$expected") <(printf '%s\n' "$actual") >&2 || true; error "a lista do pacote difere do staging"; }
# Owners, special bits and write for group and others; no link.
awk '$2 != "root/root" {print "dono: " $0; bad = 1}
     substr($1, 1, 1) == "l" {print "link: " $0; bad = 1}
     $1 ~ /[sStT]/ {print "bit especial: " $0; bad = 1}
     substr($1, 6, 1) == "w" || substr($1, 9, 1) == "w" {print "escrita: " $0; bad = 1}
     END {exit bad}' <<<"$listing" >&2 || error "pacote com dono, link, bit especial ou escrita fora do esperado"
# m2:DT15: no 0.1.x path in the package (dpkg deletes them on the upgrade because the new
# package does not list them).
if grep -E '^usr/local/share/ward/extensao(/|$)|^usr/local/lib/ward/grupos-criados$' <<<"$actual" >&2; then
    error "o pacote tem caminho da 0.1.x"
fi
dep="$(dpkg-deb -f "$OUTPUT" Depends)"
[[ ", $dep," == *", sshpass,"* ]] || error "o pacote não declara a dependência de sshpass"
for s in preinst postinst prerm postrm; do
    dpkg-deb -I "$OUTPUT" "$s" >/dev/null || error "o pacote não tem $s"
done

step "pronto: $OUTPUT"
sha256sum -- "$OUTPUT"
