# shellcheck shell=bash
# Ward groups (D47, D60; V9.11). Read by `install.sh` (`make install`) and copied whole
# into the `preinst` of the `.deb` by `deb.sh` (Phase 52): both installs verify the
# groups by the same rules. The includer defines `error` (exits != 0) and `step`.
#
# A Ward group is only accepted if nobody but the setgid binary can get its gid:
#   - a single line in /etc/group, gid from 1 to 999 (system) and no other group with
#     the same gid (a `groupadd -o` would give the gid to the members of the other one);
#   - no members in /etc/group nor in /etc/gshadow, no administrators, and a locked
#     password in /etc/gshadow (starts with '!'; a group with a password joins by `newgrp`);
#   - no user has it as primary group (/etc/passwd and NSS);
#   - NSS (getent) returns this same group by name and by gid;
#   - outside every range of /etc/subgid (a user namespace would map it), and no
#     `subid:` in /etc/nsswitch.conf with a source other than `files` (ranges coming from
#     NSS, which the install cannot read; Phase 51a);
#   - no pam_group line (/etc/security/group.conf and group.conf.d) gives the group at
#     login, and no sudoers (/etc/sudoers, /etc/sudoers.d and their includes) names it
#     nor `#<gid>` (`sudo -g ward-app` would give the gid); with `sudoers:` in
#     nsswitch.conf coming from outside `files`, refuses (Phase 51a);
#   - `group:` and `initgroups:` in /etc/nsswitch.conf only with `files` and `systemd`
#     (Phase 51b): another source (sss, ldap, nis, compat...) can put the user in the
#     group without going through the /etc/group the install verifies.
#
# Mark (Phase 51a): the group the install creates goes into GROUPS_MARK with its gid;
# uninstall and `dpkg -r` only delete a group that is in the mark with the same gid. A
# group the administrator already had (and that passes D60) stays.
conferir_grupo() {
    local name="$1" lines n password gid members admins
    lines="$(grep -c -- "^${name}:" /etc/group || true)"
    [ "$lines" = 1 ] || { echo "o grupo $name aparece $lines vez(es) no /etc/group"; return 1; }
    IFS=: read -r n password gid members <<<"$(grep -- "^${name}:" /etc/group)"
    [[ "$gid" =~ ^[1-9][0-9]{0,2}$ ]] || { echo "o grupo $name tem gid $gid (precisa ser de sistema, de 1 a 999)"; return 1; }
    [ -z "$members" ] || { echo "o grupo $name tem membros ($members)"; return 1; }
    [ "$(awk -F: -v g="$gid" '$3 == g' /etc/group | wc -l)" = 1 ] ||
        { echo "outro grupo do /etc/group usa o gid $gid de $name"; return 1; }
    [ "$(getent group "$name" | cut -d: -f3,4)" = "$gid:" ] || { echo "o NSS devolve outro $name (getent group)"; return 1; }
    [ "$(getent group "$gid" | cut -d: -f1)" = "$name" ] || { echo "o NSS devolve outro grupo para o gid $gid"; return 1; }
    if awk -F: -v g="$gid" '$4 == g {found = 1} END {exit !found}' /etc/passwd ||
        getent passwd | awk -F: -v g="$gid" '$4 == g {found = 1} END {exit !found}'; then
        echo "um usuário tem $name (gid $gid) como grupo primário"
        return 1
    fi
    [ -f /etc/gshadow ] || { echo "/etc/gshadow não existe"; return 1; }
    lines="$(grep -c -- "^${name}:" /etc/gshadow || true)"
    [ "$lines" = 1 ] || { echo "o grupo $name aparece $lines vez(es) no /etc/gshadow"; return 1; }
    IFS=: read -r n password admins members <<<"$(grep -- "^${name}:" /etc/gshadow)"
    [[ "$password" == '!'* ]] || { echo "o grupo $name tem senha no /etc/gshadow (precisa ser '!')"; return 1; }
    [ -z "$admins" ] || { echo "o grupo $name tem administradores no /etc/gshadow"; return 1; }
    [ -z "$members" ] || { echo "o grupo $name tem membros no /etc/gshadow"; return 1; }
    # nsswitch.conf: subgid ranges, sudo rules and groups coming from outside the
    # files. `group:`/`initgroups:` also accept `systemd` (dynamic users and the root's
    # userdb; the default of Pop!_OS and Ubuntu is `files systemd`). The database name
    # is matched the way glibc reads it (space before and after, `:` stuck to the source,
    # case-insensitive); when in doubt (an `[...]` action with a space inside, for
    # example), refuses.
    local database accepted
    for database in subid sudoers group initgroups; do
        case "$database" in
            group | initgroups) accepted="files systemd" ;;
            *) accepted="files" ;;
        esac
        if [ -f /etc/nsswitch.conf ] && awk -v k="$database" -v accepted=" $accepted " '
            {
                sub(/#.*/, "")
                line = $0
                sub(/^[ \t]+/, "", line)
                if (tolower(substr(line, 1, length(k))) != k) next
                line = substr(line, length(k) + 1)
                if (line !~ /^[ \t]*:/) next
                sub(/^[ \t]*:/, "", line)
                n = split(line, f, /[ \t]+/)
                for (i = 1; i <= n; i++) {
                    if (f[i] == "" || f[i] ~ /^\[[^] \t]*\]$/) continue
                    if (index(accepted, " " f[i] " ") == 0) found = 1
                }
            }
            END { exit !found }' /etc/nsswitch.conf; then
            echo "o /etc/nsswitch.conf busca '$database' fora de ($accepted) (a instalação não consegue conferir $name ali)"
            return 1
        fi
    done
    # pam_group: `services;ttys;users;times;groups` gives the groups at login.
    local f
    for f in /etc/security/group.conf /etc/security/group.conf.d/*; do
        [ -e "$f" ] || continue
        if sed -e 's/#.*//' -- "$f" | grep -q -E -- "(^|[^A-Za-z0-9_.-])${name}([^A-Za-z0-9_.-]|\$)"; then
            echo "$f dá o grupo $name no login (pam_group)"
            return 1
        fi
    done
    # sudoers: the name or `#<gid>` anywhere (comments too: when in doubt, refuses),
    # following @include/@includedir and the old forms with '#'.
    local sudoers=() i=0 dir target g
    [ -e /etc/sudoers ] && sudoers+=(/etc/sudoers)
    for f in /etc/sudoers.d/*; do [ -e "$f" ] && sudoers+=("$f"); done
    while [ "$i" -lt "${#sudoers[@]}" ] && [ "$i" -lt 512 ]; do
        f="${sudoers[$i]}"
        i=$((i + 1))
        [ -d "$f" ] && continue
        if grep -a -q -E -- "(^|[^A-Za-z0-9_.-])(${name}|#${gid})([^A-Za-z0-9_.-]|\$)" "$f"; then
            echo "o sudoers $f cita $name (ou #$gid): 'sudo -g' daria o gid do grupo"
            return 1
        fi
        while read -r dir target _; do
            target="${target%\"}"
            target="${target#\"}"
            [ -n "$target" ] || continue
            [[ "$target" == /* ]] || target="${f%/*}/$target"
            case "$dir" in
                @include | '#include') [ -e "$target" ] && sudoers+=("$target") ;;
                @includedir | '#includedir')
                    for g in "$target"/*; do [ -e "$g" ] && sudoers+=("$g"); done
                    ;;
            esac
        done < <(grep -a -E '^[[:space:]]*[@#]include(dir)?[[:space:]]' -- "$f" || true)
    done
    [ "$i" -ge "${#sudoers[@]}" ] || { echo "sudoers com includes demais (laço?); não deu para conferir $name"; return 1; }
    if [ -e /etc/subgid ]; then
        local owner start count
        while IFS=: read -r owner start count; do
            [[ -z "$owner" || "$owner" == \#* ]] && continue
            [[ "$start" =~ ^[0-9]+$ && "$count" =~ ^[0-9]+$ ]] || { echo "/etc/subgid com linha fora do formato"; return 1; }
            if [ "$gid" -ge "$start" ] && [ "$gid" -lt $((start + count)) ]; then
                echo "o gid $gid de $name cai na faixa $start+$count do /etc/subgid ($owner)"
                return 1
            fi
        done </etc/subgid
    fi
    printf '%s\n' "$gid"
}

# --- mark of the created groups (Phase 51a) --------------------------------------------
GROUPS_MARK=/usr/local/lib/ward/grupos-criados

# Valid "name gid" lines of the mark (only ward-app/ward-nm, system gid).
marked_groups() {
    [ -f "$GROUPS_MARK" ] && [ ! -L "$GROUPS_MARK" ] || return 0
    grep -a -x -E 'ward-(app|nm) [1-9][0-9]{0,2}' -- "$GROUPS_MARK" || true
}

# Writes the whole mark (temporary file in the same dir and `rename`); empty, deletes it.
# The Ward dir is created 0755 root:root if missing; the caller verified the one above
# (/usr/local/lib).
mark_dir() {
    local dir="${GROUPS_MARK%/*}"
    if [ ! -e "$dir" ] && [ ! -L "$dir" ]; then
        mkdir -m 0755 -- "$dir" || error "não deu para criar $dir (a marca dos grupos)"
    fi
    [ -d "$dir" ] && [ ! -L "$dir" ] && [ "$(stat -c %u -- "$dir")" = 0 ] ||
        error "$dir não é uma pasta do root"
}

write_mark() {
    local content="$1" dir="${GROUPS_MARK%/*}" tmp
    mark_dir
    if [ -z "$content" ]; then
        rm -f -- "$GROUPS_MARK"
        return 0
    fi
    tmp="$(mktemp -- "$dir/.grupos.XXXXXXXX")"
    printf '%s\n' "$content" >"$tmp"
    chmod 0644 -- "$tmp"
    mv -f -T -- "$tmp" "$GROUPS_MARK"
}

# Mark of `name`: created now (`created=1`) writes "name gid"; already existing, the
# line stays only if the gid is the same (another group with the name does not inherit
# the mark).
mark_group() {
    local name="$1" gid="$2" created="$3" before lines
    before="$(marked_groups)"
    lines="$(grep -v -- "^$name " <<<"$before" || true)"
    if [ "$created" = 1 ] || grep -q -x -F -- "$name $gid" <<<"$before"; then
        lines="$(printf '%s\n%s\n' "$lines" "$name $gid" | grep -v '^$' || true)"
    fi
    [ "$lines" = "$before" ] || write_mark "$lines"
}

# Deletes the groups of the mark that still have the marked gid; the others stay (with a
# warning).
apagar_grupos_marcados() {
    local g gid marked
    marked="$(marked_groups)"
    for g in ward-nm ward-app; do
        grep -q -- "^${g}:" /etc/group || continue
        gid="$(awk -v g="$g" '$1 == g {print $2; exit}' <<<"$marked")"
        if [ -n "$gid" ] && [ "$(grep -- "^${g}:" /etc/group | cut -d: -f3)" = "$gid" ]; then
            groupdel "$g" || printf 'ward: aviso: não deu para apagar o grupo %s\n' "$g" >&2
        else
            printf 'ward: aviso: o grupo %s não foi criado pelo Ward (sem marca em %s); ficou\n' "$g" "$GROUPS_MARK" >&2
        fi
    done
    if [ -e "$GROUPS_MARK" ] || [ -L "$GROUPS_MARK" ]; then
        rm -f -- "$GROUPS_MARK"
    fi
}

garantir_grupo() {
    local name="$1" created=0 output
    if ! grep -q -- "^${name}:" /etc/group; then
        if getent group "$name" >/dev/null; then
            error "o grupo $name vem do NSS (fora do /etc/group); a instalação não o aceita"
        fi
        # The mark dir before the groupadd: a created group is always marked.
        mark_dir
        groupadd --system "$name"
        created=1
    fi
    if ! output="$(conferir_grupo "$name")"; then
        if [ "$created" = 1 ]; then
            groupdel "$name" || true
        fi
        error "grupo recusado (D60): $output. Nada foi instalado. Corrija o grupo (ou apague-o com 'groupdel $name') e rode de novo."
    fi
    mark_group "$name" "$output" "$created"
    [ "$created" = 1 ] && step "grupo $name criado (gid $output)" || step "grupo $name já existia e confere (gid $output)"
}
