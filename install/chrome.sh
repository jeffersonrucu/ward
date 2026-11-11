# shellcheck shell=bash
# Chrome policies (Phase 51a; V9.5). Read by `install.sh` (`make install`) and copied
# whole into the `preinst` of the `.deb` by `deb.sh`, like `groups.sh`. The includer
# defines `error` (exits != 0).
#
# The Ward policy (`ward.json`) turns on `ExtensionSettings` (force-installed extension),
# `NativeMessagingUserLevelHosts: false` (Chrome only opens native messaging hosts from
# /etc/opt/chrome/native-messaging-hosts/; a `br.ward.nm.json` manifest planted in the
# user's profile by a process of the same uid no longer counts) and, since Phase 44c,
# `PasswordManagerEnabled: false` and `PasswordLeakDetectionEnabled: false` (Chrome does
# not keep the password the extension fills nor sends a hash of it to Google). Chrome
# reads every file of policies/managed/ and does not merge the same name from two files:
# the one from another file can beat Ward's. So another file that names one of these
# (or uses a `\u` escape, which would hide the name) refuses the install, before any
# change.

# Names the Ward policy defines and that no other file may name.
WARD_POLICIES='ExtensionSettings|NativeMessagingUserLevelHosts|PasswordManagerEnabled|PasswordLeakDetectionEnabled'

# Verifies a policies/managed dir: exits through `error` if another file of the dir
# defines a name of the Ward policy. Missing dir: nothing to verify.
conferir_politicas_chrome() {
    local dir="$1" f base found
    [ -e "$dir" ] || [ -L "$dir" ] || return 0
    [ -d "$dir" ] && [ ! -L "$dir" ] || error "$dir não é uma pasta (link?)"
    for f in "$dir"/* "$dir"/.[!.]* "$dir"/..?*; do
        [ -e "$f" ] || [ -L "$f" ] || continue
        base="${f##*/}"
        [ "$base" = ward.json ] && continue
        [ -d "$f" ] && [ ! -L "$f" ] && continue
        [ -f "$f" ] && [ -r "$f" ] ||
            error "$f não é um arquivo legível; o Chrome pode lê-lo como política. Tire-o e rode de novo."
        found="$(grep -a -o -E -- "$WARD_POLICIES|\\\\u" "$f" | head -n 1 || true)"
        if [ -n "$found" ]; then
            error "$f também cita política do Chrome que o Ward define ($found). O Chrome não junta o mesmo nome de dois arquivos de $dir e um deles vence. Tire esse arquivo de $dir (veja docs/instalacao.md, 'Outras políticas do Chrome') e rode de novo. Nada foi instalado."
        fi
    done
}

# Warns about the user's manifests (argument: uid): with
# `NativeMessagingUserLevelHosts: false`, the native messaging hosts of the user's
# profile (1Password, Claude in Chrome...) stop. Only lists the names (reads and follows
# nothing); copying them to /etc/opt/chrome/native-messaging-hosts/ is up to the user
# (docs/instalacao.md, the section on other native messaging apps).
avisar_manifestos_de_usuario() {
    local home f base names=()
    home="$(getent passwd "$1" | cut -d: -f6)"
    [[ "$home" == /* ]] || return 0
    for f in "$home/.config/google-chrome/NativeMessagingHosts"/*.json; do
        [ -e "$f" ] || [ -L "$f" ] || continue
        base="${f##*/}"
        [[ "$base" =~ ^[A-Za-z0-9._-]{1,128}$ ]] || base="(nome fora do padrão)"
        [ "$base" = br.ward.nm.json ] && base="br.ward.nm.json (o Ward nunca grava no perfil: apague-o e confira quem o pôs)"
        names+=("$base")
    done
    [ "${#names[@]}" -gt 0 ] || return 0
    printf 'ward: aviso: o Chrome agora só abre hosts de native messaging do sistema (NativeMessagingUserLevelHosts: false). Estes do seu perfil param até serem copiados para /etc/opt/chrome/native-messaging-hosts/ (docs/instalacao.md, "Outros apps com native messaging"):\n' >&2
    printf '    %s\n' "${names[@]}" >&2
}
