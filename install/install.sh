#!/bin/bash -p
# Ward install (Phase 51; D18, D46, D47, D57, D60, D62, D63; V9.1–V9.4, V9.6, V9.7,
# V9.10, V9.11). Called by `make install` / `make uninstall`; never compiles anything
# (the build is the user's, without root: `make release`).
#
#   install.sh install    installs the tree of `tree.txt`
#   install.sh uninstall  undoes the install; never touches the home (the vault in
#                         ~/.local/share/ward/ stays)
#
# Core dump (Phase 51c; S2, S8): the apport list (/etc/apport/report-ignore/ward and the
# legacy blacklist.d/ward) with Ward and the WebKitGTK helpers, and systemd-coredump
# without storage (/etc/systemd/coredump.conf.d/ward.conf). See dump.sh.
#
# Variables (the Makefile exports them):
#   DESTDIR        empty: installs on the system (needs root: `sudo make install`). With
#                  a value (absolute path, no link): only copies the tree there, without
#                  groups, root owner or systemctl (staging of the `.deb`, Phase 52, and
#                  tests).
#   PREFIX         only /usr/local: the binaries look for each other there (D18).
#   WARD_UID       uid of the vault user (default: SUDO_UID). System only.
#   EXTENSION_ID   extension id (32 letters a–p), the same as the package's.
#   EXTENSION_CRX  CRX3 package of the extension.
#   BIN_WARD BIN_WARDD BIN_APP BIN_NM  release artifacts.
#
# Order (fail closed): verifies everything and copies the artifacts to a temporary dir
# of this process (the checks hold for the copy, not for the user's path, which a
# process of the same uid could swap) -> groups (D60) -> files -> systemd.
# Nothing is installed if a check fails.

export PATH=/usr/sbin:/usr/bin:/sbin:/bin
export LC_ALL=C
set -euo pipefail
umask 077

HERE="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
REPO_ROOT="${HERE%/*}"
TREE="$HERE/tree.txt"
PYTHON=(/usr/bin/python3 -I -B)

error() { printf 'ward-instalar: %s\n' "$*" >&2; exit 1; }
warning() { printf 'ward-instalar: aviso: %s\n' "$*" >&2; }
step() { printf '==> %s\n' "$*"; }

ACTION="${1:-}"
[ $# -eq 1 ] || error "uso: install.sh install|uninstall"
case "$ACTION" in install | uninstall) ;; *) error "uso: install.sh install|uninstall" ;; esac

# --- DESTDIR and PREFIX ---------------------------------------------------------------
DESTDIR="${DESTDIR-}"
PREFIX="${PREFIX-/usr/local}"
[ "$PREFIX" = /usr/local ] || error "PREFIX tem de ser /usr/local (os binários, as units e o manifesto do Chrome apontam para lá; D18)"
if [ -n "$DESTDIR" ]; then
    DESTDIR="${DESTDIR%/}"
    [[ "$DESTDIR" == /* ]] || error "DESTDIR tem de ser um caminho absoluto"
    [[ "$DESTDIR" != *//* && "$DESTDIR" != */./* && "$DESTDIR" != */../* && "$DESTDIR" != */. && "$DESTDIR" != */.. ]] ||
        error "DESTDIR com '//', '.' ou '..'"
    [ -n "$DESTDIR" ] || error "DESTDIR=/ : para instalar no sistema, deixe DESTDIR vazio"
    if [ "$ACTION" = install ] && [ ! -e "$DESTDIR" ]; then
        mkdir -p -m 0755 -- "$DESTDIR"
    fi
    [ -d "$DESTDIR" ] || error "DESTDIR não é uma pasta"
    [ "$(cd -P -- "$DESTDIR" && pwd -P)" = "$DESTDIR" ] || error "DESTDIR passa por link simbólico"
    SYSTEM=0
else
    SYSTEM=1
    [ "$(id -u)" = 0 ] || error "instalar no sistema exige root: rode 'sudo make $([ "$ACTION" = install ] && echo install || echo uninstall)' (ou use DESTDIR=<pasta>)"
fi
TARGET_ROOT="$DESTDIR"
ROOT=0
[ "$(id -u)" = 0 ] && ROOT=1

# --- tree.txt -------------------------------------------------------------------------
# OWNER_GROUPS, not GROUPS: bash ignores assignments to its GROUPS array.
KINDS=() MODES=() OWNERS=() OWNER_GROUPS=() PATHS=() ORIGINS=()
while read -r kind mode owner group path origin rest; do
    [[ -z "$kind" || "$kind" == \#* ]] && continue
    [ -z "${rest:-}" ] || error "tree.txt: coluna a mais em $path"
    [[ "$kind" =~ ^[Ddf]$ && "$mode" =~ ^[0-7]{4}$ && "$path" =~ ^[a-z][A-Za-z0-9@._/-]*$ ]] ||
        error "tree.txt: linha inválida ($kind $mode $path)"
    [[ "$path" != *..* && "$path" != *//* && "$path" != */ ]] || error "tree.txt: caminho inválido $path"
    [ "$kind" != f ] || [ -n "${origin:-}" ] || error "tree.txt: arquivo sem origem ($path)"
    KINDS+=("$kind") MODES+=("$mode") OWNERS+=("$owner") OWNER_GROUPS+=("$group")
    PATHS+=("$path") ORIGINS+=("${origin:-}")
done <"$TREE"
[ "${#PATHS[@]}" -gt 0 ] || error "tree.txt vazia"

# Marks of a `test-mode` artifact (D7, D63; V9.2): the variables of the test daemon, the
# bridge and the test app, and the name of the test socket dir. Any `WARD_DEV_*`
# refuses, no exception (Phase 51a: the release carries none of them as text).
verify_no_test_mode() {
    local file="$1" name="$2" found
    found="$(grep -a -o -E 'WARD_DEV_[A-Z0-9_]*' -- "$file" | head -n 1 || true)"
    [ -z "$found" ] || error "$name foi compilado com modo-teste (contém $found); use o build de release ('make release')"
    for mark in ward-ponte-teste wardd-app-teste ward-teste; do
        if grep -a -q -F -- "$mark" "$file"; then
            error "$name foi compilado com modo-teste (contém $mark); use o build de release ('make release')"
        fi
    done
}

# Source file: regular, no link, not empty, up to `max` bytes.
verify_source() {
    local file="$1" name="$2" max="$3"
    [ -n "$file" ] || error "$name não definido"
    [ ! -L "$file" ] || error "$name ($file) é um link simbólico"
    [ -f "$file" ] || error "$name ($file) não existe ou não é um arquivo"
    local size
    size="$(stat -c %s -- "$file")"
    [ "$size" -gt 0 ] && [ "$size" -le "$max" ] || error "$name ($file) vazio ou grande demais"
}

# Copies without following links (`cp -P`) and verifies the copy is a regular file.
copy_to() {
    local from="$1" to="$2" name="$3"
    cp -P -- "$from" "$to"
    [ -f "$to" ] && [ ! -L "$to" ] || error "$name mudou durante a cópia (virou link?)"
}

verify_elf() {
    local file="$1" name="$2"
    [ "$(head -c 4 -- "$file" | od -An -tx1 | tr -d ' \n')" = 7f454c46 ] || error "$name não é um executável ELF"
}

# --- groups (D47, D60; V9.11): group check and creation, the same as the `.deb`'s -----
# shellcheck source=install/groups.sh
. "$HERE/groups.sh"
# --- Chrome policies (Phase 51a): the other-policies check, the same as the `.deb`'s ---
# shellcheck source=install/chrome.sh
. "$HERE/chrome.sh"
# --- core dump (Phase 51c): webkit_dir, apport_list and the core_pattern warning -------
# shellcheck source=install/dump.sh
. "$HERE/dump.sh"

# --- dirs and files -------------------------------------------------------------------
# Dir: no link; on the system, owned by root and not writable by group and others.
ensure_dir() {
    local p="$1"
    if [ -L "$p" ]; then
        error "$p é um link simbólico; a instalação não segue links"
    elif [ -e "$p" ]; then
        [ -d "$p" ] || error "$p existe e não é uma pasta"
        if [ "$SYSTEM" = 1 ]; then
            [ "$(stat -c %u -- "$p")" = 0 ] || error "$p não é do root"
            local m
            m="$(stat -c %a -- "$p")"
            [ $((8#$m & 8#022)) = 0 ] || error "$p tem escrita para grupo ou outros ($m)"
        fi
    else
        mkdir -m 0755 -- "$p"
        if [ "$ROOT" = 1 ]; then chown root:root -- "$p"; fi
    fi
}

# File: temporary file in the same dir, owner and mode, and `rename` over it (a link at
# the destination is replaced, never followed).
install_file() {
    local from="$1" to="$2" mode="$3" owner="$4" group="$5" tmp
    [ ! -d "$to" ] || error "$to é uma pasta"
    tmp="$(mktemp -- "${to%/*}/.ward-instalar.XXXXXXXX")"
    cat -- "$from" >"$tmp"
    if [ "$ROOT" = 1 ] && [ "$SYSTEM" = 1 ]; then chown -- "$owner:$group" "$tmp"; fi
    # After the chown (chown clears the setgid bit).
    chmod -- "$mode" "$tmp"
    mv -f -T -- "$tmp" "$to"
}

# --- install --------------------------------------------------------------------------
do_install() {
    local i
    step "conferindo os artefatos e a extensão"
    local id="${EXTENSION_ID-}"
    [[ "$id" =~ ^[a-p]{32}$ ]] || error "EXTENSION_ID tem de ser o id da extensão (32 letras de a a p); veja o handoff da Fase 41"
    if [ "$SYSTEM" = 1 ]; then
        local uid="${WARD_UID-}"
        [[ "$uid" =~ ^[1-9][0-9]{3,9}$ ]] || error "WARD_UID ($uid) tem de ser o uid do usuário do cofre (>= 1000); com sudo vem do SUDO_UID"
        [ "$uid" -ne 65534 ] || error "WARD_UID não pode ser o nobody"
        getent passwd "$uid" >/dev/null || error "WARD_UID $uid não existe"
    fi
    [ -x "${PYTHON[0]}" ] || error "falta ${PYTHON[0]} (lê o pacote da extensão)"

    TMP="$(mktemp -d /tmp/ward-instalar.XXXXXXXX)"
    trap 'rm -rf -- "$TMP"' EXIT
    local name var file
    for pair in ward:BIN_WARD wardd:BIN_WARDD app:BIN_APP nm:BIN_NM; do
        name="${pair%%:*}" var="${pair#*:}"
        file="${!var-}"
        verify_source "$file" "$var" $((512 * 1024 * 1024))
        copy_to "$file" "$TMP/bin-$name" "$var"
        verify_elf "$TMP/bin-$name" "$var ($file)"
        verify_no_test_mode "$TMP/bin-$name" "$var ($file)"
    done
    verify_source "${EXTENSION_CRX-}" EXTENSION_CRX $((64 * 1024 * 1024))
    copy_to "$EXTENSION_CRX" "$TMP/crx" EXTENSION_CRX
    local parsed crx_id version
    parsed="$("${PYTHON[@]}" "$HERE/crx.py" "$TMP/crx")" || error "EXTENSION_CRX recusado"
    read -r crx_id version <<<"$parsed"
    [ "$crx_id" = "$id" ] || error "o pacote EXTENSION_CRX é da extensão $crx_id, não de EXTENSION_ID=$id"
    [[ "$version" =~ ^[0-9.]+$ ]] || error "versão da extensão fora do formato"
    for i in "${!PATHS[@]}"; do
        if [[ "${ORIGINS[$i]}" == src:* ]]; then
            local src="$REPO_ROOT/${ORIGINS[$i]#src:}"
            verify_source "$src" "${ORIGINS[$i]}" $((1024 * 1024))
        fi
    done
    conferir_politicas_chrome "$TARGET_ROOT/etc/opt/chrome/policies/managed"

    # Generated (the id and the version already went through the expressions above: no
    # quotes nor '<').
    local ext=/usr/local/share/ward/extensao
    cat >"$TMP/gen-nm" <<EOF
{
  "name": "br.ward.nm",
  "description": "Ward: host de native messaging da extensão",
  "path": "/usr/local/lib/ward/ward-nm",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$id/"]
}
EOF
    # NativeMessagingUserLevelHosts: false (Phase 51a): Chrome only opens native
    # messaging hosts from /etc/opt/chrome/native-messaging-hosts/ (root's). Without it, a
    # br.ward.nm.json planted in the user's profile beats the system one, and the
    # attacker's script execs the real ward-nm with stdin/stdout in hand (M9 audit).
    # PasswordManagerEnabled and PasswordLeakDetectionEnabled: false (Phase 44c, M6
    # audit): Chrome's password manager does not offer to save the password the
    # extension fills (it would go to the user's profile) nor sends a hash of it to
    # Google for the leak check.
    cat >"$TMP/gen-policy" <<EOF
{
  "NativeMessagingUserLevelHosts": false,
  "PasswordManagerEnabled": false,
  "PasswordLeakDetectionEnabled": false,
  "ExtensionSettings": {
    "$id": {
      "installation_mode": "force_installed",
      "update_url": "file://$ext/update.xml",
      "toolbar_pin": "force_pinned"
    }
  }
}
EOF
    cat >"$TMP/gen-update" <<EOF
<?xml version='1.0' encoding='UTF-8'?>
<gupdate xmlns='http://www.google.com/update2/response' protocol='2.0'>
  <app appid='$id'>
    <updatecheck codebase='file://$ext/ward.crx' version='$version' />
  </app>
</gupdate>
EOF
    # Phase 51c: the apport list (Ward and the WebKitGTK helpers), verified against the
    # installed libwebkit2gtk-4.1-0 before any change.
    apport_list >"$TMP/gen-apport"

    if [ "$SYSTEM" = 1 ]; then
        step "grupos de sistema (D47, D60)"
        # The mark of the created groups lives in /usr/local/lib/ward/ (Phase 51a): the
        # dirs above it are verified first.
            local p
        for p in usr usr/local usr/local/lib; do ensure_dir "/$p"; done
        garantir_grupo ward-app
        garantir_grupo ward-nm
        local ptrace suid
        ptrace="$(cat /proc/sys/kernel/yama/ptrace_scope 2>/dev/null || echo ?)"
        suid="$(cat /proc/sys/fs/suid_dumpable 2>/dev/null || echo ?)"
        [ "$ptrace" != 0 ] || warning "kernel.yama.ptrace_scope = 0: o daemon recusa os papéis app e extensão (D47)"
        [ "$suid" != 1 ] || warning "fs.suid_dumpable = 1: o daemon recusa os papéis app e extensão (D47)"
        avisar_core_pattern
    fi

    step "arquivos em ${DESTDIR:-/}"
    for i in "${!PATHS[@]}"; do
        [ "${KINDS[$i]}" = f ] && continue
        ensure_dir "$TARGET_ROOT/${PATHS[$i]}"
    done
    local from
    for i in "${!PATHS[@]}"; do
        [ "${KINDS[$i]}" = f ] || continue
        case "${ORIGINS[$i]}" in
            bin:*) from="$TMP/bin-${ORIGINS[$i]#bin:}" ;;
            src:*) from="$TMP/src-$i"; copy_to "$REPO_ROOT/${ORIGINS[$i]#src:}" "$from" "${ORIGINS[$i]}" ;;
            crx) from="$TMP/crx" ;;
            gen:*) from="$TMP/gen-${ORIGINS[$i]#gen:}" ;;
            *) error "tree.txt: origem desconhecida ${ORIGINS[$i]}" ;;
        esac
        install_file "$from" "$TARGET_ROOT/${PATHS[$i]}" "${MODES[$i]}" "${OWNERS[$i]}" "${OWNER_GROUPS[$i]}"
        printf '    %s %s:%s %s\n' "${MODES[$i]}" "${OWNERS[$i]}" "${OWNER_GROUPS[$i]}" "/${PATHS[$i]}"
    done

    if [ "$SYSTEM" = 1 ]; then
        step "systemd: /run/ward (tmpfiles.d) e wardd@$uid.socket (D46)"
        systemd-tmpfiles --create /usr/local/lib/tmpfiles.d/ward.conf
        systemctl daemon-reload
        systemctl enable --now "wardd@$uid.socket"
        # Reinstall: the daemon already running moves to the new binary (the vault locks).
        systemctl try-restart "wardd@$uid.service"
        avisar_manifestos_de_usuario "$uid"
        step "pronto: o wardd sobe na primeira conexão; o uso diário não pede sudo"
    else
        step "pronto (DESTDIR: sem grupos, sem dono root e sem systemctl; donos e modos de verdade em install/tree.txt)"
    fi
}

# --- uninstall ------------------------------------------------------------------------
do_uninstall() {
    local i p
    if [ "$SYSTEM" = 1 ]; then
        step "systemd: parando e desligando o wardd@*"
        local units=() u
        while read -r u _; do
            [[ "$u" =~ ^wardd@[0-9]+\.(socket|service)$ ]] && units+=("$u")
        done < <(systemctl list-units --all --plain --no-legend --full 'wardd@*' 2>/dev/null || true)
        for p in /etc/systemd/system/*.wants/wardd@*.socket; do
            [ -e "$p" ] || [ -L "$p" ] || continue
            u="${p##*/}"
            [[ "$u" =~ ^wardd@[0-9]+\.socket$ ]] && units+=("$u")
        done
        for u in "${units[@]}"; do
            if [[ "$u" == *.socket ]]; then
                systemctl disable --now "$u" || warning "não deu para desligar $u"
            else
                systemctl stop "$u" || warning "não deu para parar $u"
            fi
        done
    fi

    step "arquivos em ${DESTDIR:-/}"
    for i in "${!PATHS[@]}"; do
        [ "${KINDS[$i]}" = f ] || continue
        p="$TARGET_ROOT/${PATHS[$i]}"
        if [ -d "$p" ] && [ ! -L "$p" ]; then
            warning "$p é uma pasta; ficou"
        elif [ -e "$p" ] || [ -L "$p" ]; then
            rm -f -- "$p"
            printf '    removido /%s\n' "${PATHS[$i]}"
        fi
    done
    if [ "$SYSTEM" = 1 ]; then
        # Only the groups the install created (mark in /usr/local/lib/ward/, Phase 51a);
        # before the dirs, so the mark goes with them.
        step "grupos"
        apagar_grupos_marcados
    fi
    for ((i = ${#PATHS[@]} - 1; i >= 0; i--)); do
        [ "${KINDS[$i]}" = d ] || continue
        p="$TARGET_ROOT/${PATHS[$i]}"
        if [ -d "$p" ] && [ ! -L "$p" ]; then
            rmdir -- "$p" 2>/dev/null || warning "$p não está vazia; ficou"
        fi
    done

    if [ "$SYSTEM" = 1 ]; then
        systemctl daemon-reload
        systemctl reset-failed 'wardd@*' 2>/dev/null || true
        # /run/ward/<uid>/ (the socket goes with RemoveOnStop) and /run/ward, if empty.
        if [ -d /run/ward ] && [ ! -L /run/ward ]; then
            for p in /run/ward/*; do
                [ -d "$p" ] && [ ! -L "$p" ] && { rmdir -- "$p" 2>/dev/null || warning "$p não está vazia; ficou"; }
            done
            rmdir /run/ward 2>/dev/null || warning "/run/ward não está vazia; ficou"
        fi
    fi
    step "pronto: o cofre e os backups em ~/.local/share/ward/ não foram tocados"
}

# Explicit dispatch: a function named `install` would shadow /usr/bin/install.
case "$ACTION" in
    install) do_install ;;
    uninstall) do_uninstall ;;
esac
