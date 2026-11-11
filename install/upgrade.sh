# shellcheck shell=bash
# Upgrade over 0.1.x (m2:DT15, m2:DT16). Read by `install.sh` (`make install` and
# `make uninstall`) and copied whole into the `preinst` and the `postinst` of the `.deb` by
# `deb.sh`, like `groups.sh`. The includer defines `error` (exits != 0), `warning` and
# `step`.
#
# m2:DT16: the running `wardd@<uid>.service` stops before the binaries are swapped, so no
# 0.1.x daemon stays up with 0.2.0 clients. A stopped daemon locks the vault (S8); the
# sockets stop with it during the swap and come back after it, and the next connection
# starts the new binary.
#
# m2:DT15: 0.1.x installed the extension in another dir. The paths below are the only
# ones this file deletes: fixed, relative to the root, never through a link (a linked dir
# refuses the install before any change, `verify_legacy_paths`; on uninstall it only
# warns). The groups mark of 0.1.x is moved by `groups.sh` (`migrate_legacy_mark`).

# Files and dir of 0.1.x, relative to the root ("/" or DESTDIR).
LEGACY_FILES=(usr/local/share/ward/extensao/ward.crx usr/local/share/ward/extensao/update.xml)
LEGACY_DIR=usr/local/share/ward/extensao
# Dirs from the root down to LEGACY_DIR: none may be a link.
LEGACY_CHAIN=(usr usr/local usr/local/share usr/local/share/ward)

# Prints the first problem with the 0.1.x paths under `$1` (the root, without a trailing
# slash: empty is "/"), or nothing.
legacy_paths_problem() {
    local root="$1" p f
    for p in "${LEGACY_CHAIN[@]}" "$LEGACY_DIR"; do
        if [ -L "$root/$p" ]; then
            echo "/$p é um link simbólico; a instalação não segue links"
            return 0
        fi
        if [ -e "$root/$p" ] && [ ! -d "$root/$p" ]; then
            echo "/$p existe e não é uma pasta"
            return 0
        fi
    done
    for f in "${LEGACY_FILES[@]}"; do
        if [ -e "$root/$f" ] && [ ! -L "$root/$f" ] && [ ! -f "$root/$f" ]; then
            echo "/$f existe e não é um arquivo"
            return 0
        fi
    done
}

# Before any change (install): refuses a 0.1.x path in an unexpected shape.
verify_legacy_paths() {
    local problem
    problem="$(legacy_paths_problem "$1")"
    [ -z "$problem" ] || error "caminho da 0.1.x fora do esperado: $problem. Nada foi instalado."
}

# Deletes the 0.1.x files and the dir, if empty (`rm -f` deletes a link in place of a
# file, never what it points to). With a problem in the path, deletes nothing and warns.
remove_legacy_paths() {
    local root="$1" problem f
    problem="$(legacy_paths_problem "$root")"
    if [ -n "$problem" ]; then
        warning "caminhos da 0.1.x não apagados: $problem"
        return 0
    fi
    for f in "${LEGACY_FILES[@]}"; do
        if [ -e "$root/$f" ] || [ -L "$root/$f" ]; then
            rm -f -- "$root/$f"
            printf '    removido /%s (0.1.x)\n' "$f"
        fi
    done
    if [ -d "$root/$LEGACY_DIR" ]; then
        if rmdir -- "$root/$LEGACY_DIR" 2>/dev/null; then
            printf '    removido /%s/ (0.1.x)\n' "$LEGACY_DIR"
        else
            warning "/$LEGACY_DIR não está vazia; ficou"
        fi
    fi
}

# Prints the 0.1.x paths still under `$1`, one per line (the `.deb` postinst verifies).
legacy_paths_left() {
    local root="$1" f
    for f in "${LEGACY_FILES[@]}" "$LEGACY_DIR"; do
        if [ -e "$root/$f" ] || [ -L "$root/$f" ]; then printf '/%s\n' "$f"; fi
    done
}

# Active `wardd@<uid>.socket`/`.service` units (`$1`: socket or service), one per line.
active_units() {
    local kind="$1" u
    while read -r u _; do
        [[ "$u" =~ ^wardd@[0-9]+\.${kind}$ ]] && printf '%s\n' "$u"
    done < <(systemctl list-units --plain --no-legend --full --state=active "wardd@*.$kind" 2>/dev/null || true)
}

# m2:DT16: before the swap, stops the sockets (no connection starts a daemon in the
# middle of the swap) and then every running daemon. A unit that does not stop refuses
# the install before any file (fail closed: an old daemon would stay up with the new
# clients). The sockets come back by `start_enabled_sockets`.
stop_running_daemons() {
    local u
    while read -r u; do
        [ -n "$u" ] || continue
        systemctl stop "$u" || error "não deu para parar $u; nada foi trocado"
    done < <(active_units socket)
    while read -r u; do
        [ -n "$u" ] || continue
        step "parando $u antes de trocar os binários (o cofre trava; o socket sobe o novo na próxima conexão)"
        systemctl stop "$u" || error "não deu para parar $u; nada foi trocado"
    done < <(active_units service)
    [ -z "$(active_units service)" ] || error "um wardd@ continua no ar depois do stop; nada foi trocado"
}

# After the swap: starts the enabled sockets again (the `wants` links of every uid; the
# install itself only enables the one of its uid). A daemon still up (none should be)
# moves to the new binary.
start_enabled_sockets() {
    local p u
    for p in /etc/systemd/system/*.wants/wardd@*.socket; do
        [ -e "$p" ] || [ -L "$p" ] || continue
        u="${p##*/}"
        [[ "$u" =~ ^wardd@[0-9]+\.socket$ ]] || continue
        systemctl start "$u" || warning "não deu para religar $u"
    done
    while read -r u; do
        [ -n "$u" ] || continue
        systemctl try-restart "$u" || warning "não deu para reiniciar $u"
    done < <(active_units service)
}
