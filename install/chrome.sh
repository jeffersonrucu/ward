# shellcheck shell=bash
# Políticas do Chrome (Fase 51a; V9.5). Lido pelo `install.sh` (`make install`) e
# copiado inteiro para o `preinst` do `.deb` pelo `deb.sh`, como o `groups.sh`. Quem
# inclui define `erro` (sai ≠ 0).
#
# A política do Ward (`ward.json`) liga `ExtensionSettings` (extensão force-installed),
# `NativeMessagingUserLevelHosts: false` (o Chrome só abre host de native messaging de
# /etc/opt/chrome/native-messaging-hosts/; um manifesto `br.ward.nm.json` plantado no
# perfil do usuário por um processo do mesmo uid não vale mais) e, desde a Fase 44c,
# `PasswordManagerEnabled: false` e `PasswordLeakDetectionEnabled: false` (o Chrome não
# guarda a senha que a extensão preenche nem manda um hash dela ao Google). O Chrome lê todo arquivo
# de policies/managed/ e não junta o mesmo nome de dois arquivos: o de outro arquivo pode
# vencer o do Ward. Por isso, outro arquivo que cite um desses nomes (ou use escape
# `\u`, que esconderia o nome) recusa a instalação, antes de qualquer mudança.

# Nomes que a política do Ward define e que nenhum outro arquivo pode citar.
POLITICAS_DO_WARD='ExtensionSettings|NativeMessagingUserLevelHosts|PasswordManagerEnabled|PasswordLeakDetectionEnabled'

# conferir_politicas_chrome <pasta policies/managed> : sai pelo `erro` se outro arquivo
# da pasta define um nome da política do Ward. Pasta ausente: nada a conferir.
conferir_politicas_chrome() {
    local pasta="$1" f base achado
    [ -e "$pasta" ] || [ -L "$pasta" ] || return 0
    [ -d "$pasta" ] && [ ! -L "$pasta" ] || erro "$pasta não é uma pasta (link?)"
    for f in "$pasta"/* "$pasta"/.[!.]* "$pasta"/..?*; do
        [ -e "$f" ] || [ -L "$f" ] || continue
        base="${f##*/}"
        [ "$base" = ward.json ] && continue
        [ -d "$f" ] && [ ! -L "$f" ] && continue
        [ -f "$f" ] && [ -r "$f" ] ||
            erro "$f não é um arquivo legível; o Chrome pode lê-lo como política. Tire-o e rode de novo."
        achado="$(grep -a -o -E -- "$POLITICAS_DO_WARD|\\\\u" "$f" | head -n 1 || true)"
        if [ -n "$achado" ]; then
            erro "$f também cita política do Chrome que o Ward define ($achado). O Chrome não junta o mesmo nome de dois arquivos de $pasta e um deles vence. Tire esse arquivo de $pasta (veja docs/instalacao.md, 'Outras políticas do Chrome') e rode de novo. Nada foi instalado."
        fi
    done
}

# avisar_manifestos_de_usuario <uid> : com `NativeMessagingUserLevelHosts: false`, os
# hosts de native messaging do perfil do usuário (1Password, Claude in Chrome…) param.
# Só lista os nomes (não lê nem segue nada); copiar para
# /etc/opt/chrome/native-messaging-hosts/ é do usuário (docs/instalacao.md, "Outros
# apps com native messaging").
avisar_manifestos_de_usuario() {
    local home f base nomes=()
    home="$(getent passwd "$1" | cut -d: -f6)"
    [[ "$home" == /* ]] || return 0
    for f in "$home/.config/google-chrome/NativeMessagingHosts"/*.json; do
        [ -e "$f" ] || [ -L "$f" ] || continue
        base="${f##*/}"
        [[ "$base" =~ ^[A-Za-z0-9._-]{1,128}$ ]] || base="(nome fora do padrão)"
        [ "$base" = br.ward.nm.json ] && base="br.ward.nm.json (o Ward nunca grava no perfil: apague-o e confira quem o pôs)"
        nomes+=("$base")
    done
    [ "${#nomes[@]}" -gt 0 ] || return 0
    printf 'ward: aviso: o Chrome agora só abre hosts de native messaging do sistema (NativeMessagingUserLevelHosts: false). Estes do seu perfil param até serem copiados para /etc/opt/chrome/native-messaging-hosts/ (docs/instalacao.md, "Outros apps com native messaging"):\n' >&2
    printf '    %s\n' "${nomes[@]}" >&2
}
