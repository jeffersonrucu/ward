# shellcheck shell=bash
# Grupos do Ward (D47, D60; V9.11). Lido pelo `instalar.sh` (`make install`) e copiado
# inteiro para o `preinst` do `.deb` pelo `deb.sh` (Fase 52): as duas instalações
# conferem os grupos pelas mesmas regras. Quem inclui define `erro` (sai ≠ 0) e `passo`.
#
# Um grupo do Ward só é aceito se ninguém além do binário setgid consegue o gid dele:
#   - uma linha só no /etc/group, gid de 1 a 999 (de sistema) e nenhum outro grupo com
#     o mesmo gid (um `groupadd -o` daria o gid aos membros do outro);
#   - sem membros no /etc/group nem no /etc/gshadow, sem administradores, e senha
#     travada no /etc/gshadow (começa por '!'; grupo com senha entra por `newgrp`);
#   - nenhum usuário com ele de grupo primário (/etc/passwd e NSS);
#   - o NSS (getent) devolve esse mesmo grupo pelo nome e pelo gid;
#   - fora de toda faixa do /etc/subgid (um namespace de usuário o mapearia), e sem
#     `subid:` no /etc/nsswitch.conf com fonte que não seja `files` (faixas vindas do
#     NSS, que a instalação não consegue ler; Fase 51a);
#   - nenhuma linha do pam_group (/etc/security/group.conf e group.conf.d) dá o grupo
#     no login, e nenhum sudoers (/etc/sudoers, /etc/sudoers.d e os includes deles) cita
#     o nome nem `#<gid>` (`sudo -g ward-app` daria o gid); com `sudoers:` no
#     nsswitch.conf vindo de fora de `files`, recusa (Fase 51a);
#   - `group:` e `initgroups:` no /etc/nsswitch.conf só com `files` e `systemd` (Fase
#     51b): outra fonte (sss, ldap, nis, compat…) pode pôr o usuário no grupo sem passar
#     pelo /etc/group que a instalação confere.
#
# Marca (Fase 51a): o grupo que a instalação cria vai para MARCA_GRUPOS com o gid; o
# uninstall e o `dpkg -r` só apagam grupo que está na marca com o mesmo gid. Um grupo que
# o administrador já tinha (e confere com D60) fica.
conferir_grupo() {
    local nome="$1" linhas n senha gid membros admins
    linhas="$(grep -c -- "^${nome}:" /etc/group || true)"
    [ "$linhas" = 1 ] || { echo "o grupo $nome aparece $linhas vez(es) no /etc/group"; return 1; }
    IFS=: read -r n senha gid membros <<<"$(grep -- "^${nome}:" /etc/group)"
    [[ "$gid" =~ ^[1-9][0-9]{0,2}$ ]] || { echo "o grupo $nome tem gid $gid (precisa ser de sistema, de 1 a 999)"; return 1; }
    [ -z "$membros" ] || { echo "o grupo $nome tem membros ($membros)"; return 1; }
    [ "$(awk -F: -v g="$gid" '$3 == g' /etc/group | wc -l)" = 1 ] ||
        { echo "outro grupo do /etc/group usa o gid $gid de $nome"; return 1; }
    [ "$(getent group "$nome" | cut -d: -f3,4)" = "$gid:" ] || { echo "o NSS devolve outro $nome (getent group)"; return 1; }
    [ "$(getent group "$gid" | cut -d: -f1)" = "$nome" ] || { echo "o NSS devolve outro grupo para o gid $gid"; return 1; }
    if awk -F: -v g="$gid" '$4 == g {achou = 1} END {exit !achou}' /etc/passwd ||
        getent passwd | awk -F: -v g="$gid" '$4 == g {achou = 1} END {exit !achou}'; then
        echo "um usuário tem $nome (gid $gid) como grupo primário"
        return 1
    fi
    [ -f /etc/gshadow ] || { echo "/etc/gshadow não existe"; return 1; }
    linhas="$(grep -c -- "^${nome}:" /etc/gshadow || true)"
    [ "$linhas" = 1 ] || { echo "o grupo $nome aparece $linhas vez(es) no /etc/gshadow"; return 1; }
    IFS=: read -r n senha admins membros <<<"$(grep -- "^${nome}:" /etc/gshadow)"
    [[ "$senha" == '!'* ]] || { echo "o grupo $nome tem senha no /etc/gshadow (precisa ser '!')"; return 1; }
    [ -z "$admins" ] || { echo "o grupo $nome tem administradores no /etc/gshadow"; return 1; }
    [ -z "$membros" ] || { echo "o grupo $nome tem membros no /etc/gshadow"; return 1; }
    # nsswitch.conf: faixas de subgid, regras de sudo e grupos vindos de fora dos
    # arquivos. `group:`/`initgroups:` aceitam também o `systemd` (usuários dinâmicos e
    # userdb do root; o padrão do Pop!_OS e do Ubuntu é `files systemd`). O nome da base
    # é casado como o glibc o lê (espaço antes e depois, `:` colado à fonte, sem caixa);
    # na dúvida (ação `[...]` com espaço dentro, por exemplo), recusa.
    local fonte aceitas
    for fonte in subid sudoers group initgroups; do
        case "$fonte" in
            group | initgroups) aceitas="files systemd" ;;
            *) aceitas="files" ;;
        esac
        if [ -f /etc/nsswitch.conf ] && awk -v k="$fonte" -v aceitas=" $aceitas " '
            {
                sub(/#.*/, "")
                linha = $0
                sub(/^[ \t]+/, "", linha)
                if (tolower(substr(linha, 1, length(k))) != k) next
                linha = substr(linha, length(k) + 1)
                if (linha !~ /^[ \t]*:/) next
                sub(/^[ \t]*:/, "", linha)
                n = split(linha, f, /[ \t]+/)
                for (i = 1; i <= n; i++) {
                    if (f[i] == "" || f[i] ~ /^\[[^] \t]*\]$/) continue
                    if (index(aceitas, " " f[i] " ") == 0) achou = 1
                }
            }
            END { exit !achou }' /etc/nsswitch.conf; then
            echo "o /etc/nsswitch.conf busca '$fonte' fora de ($aceitas) (a instalação não consegue conferir $nome ali)"
            return 1
        fi
    done
    # pam_group: `servicos;ttys;usuarios;horarios;grupos` dá os grupos no login.
    local f
    for f in /etc/security/group.conf /etc/security/group.conf.d/*; do
        [ -e "$f" ] || continue
        if sed -e 's/#.*//' -- "$f" | grep -q -E -- "(^|[^A-Za-z0-9_.-])${nome}([^A-Za-z0-9_.-]|\$)"; then
            echo "$f dá o grupo $nome no login (pam_group)"
            return 1
        fi
    done
    # sudoers: o nome ou `#<gid>` em qualquer lugar (comentário também: na dúvida,
    # recusa), seguindo @include/@includedir e as formas antigas com '#'.
    local sudoers=() i=0 dir alvo g
    [ -e /etc/sudoers ] && sudoers+=(/etc/sudoers)
    for f in /etc/sudoers.d/*; do [ -e "$f" ] && sudoers+=("$f"); done
    while [ "$i" -lt "${#sudoers[@]}" ] && [ "$i" -lt 512 ]; do
        f="${sudoers[$i]}"
        i=$((i + 1))
        [ -d "$f" ] && continue
        if grep -a -q -E -- "(^|[^A-Za-z0-9_.-])(${nome}|#${gid})([^A-Za-z0-9_.-]|\$)" "$f"; then
            echo "o sudoers $f cita $nome (ou #$gid): 'sudo -g' daria o gid do grupo"
            return 1
        fi
        while read -r dir alvo _; do
            alvo="${alvo%\"}"
            alvo="${alvo#\"}"
            [ -n "$alvo" ] || continue
            [[ "$alvo" == /* ]] || alvo="${f%/*}/$alvo"
            case "$dir" in
                @include | '#include') [ -e "$alvo" ] && sudoers+=("$alvo") ;;
                @includedir | '#includedir')
                    for g in "$alvo"/*; do [ -e "$g" ] && sudoers+=("$g"); done
                    ;;
            esac
        done < <(grep -a -E '^[[:space:]]*[@#]include(dir)?[[:space:]]' -- "$f" || true)
    done
    [ "$i" -ge "${#sudoers[@]}" ] || { echo "sudoers com includes demais (laço?); não deu para conferir $nome"; return 1; }
    if [ -e /etc/subgid ]; then
        local dono inicio qtd
        while IFS=: read -r dono inicio qtd; do
            [[ -z "$dono" || "$dono" == \#* ]] && continue
            [[ "$inicio" =~ ^[0-9]+$ && "$qtd" =~ ^[0-9]+$ ]] || { echo "/etc/subgid com linha fora do formato"; return 1; }
            if [ "$gid" -ge "$inicio" ] && [ "$gid" -lt $((inicio + qtd)) ]; then
                echo "o gid $gid de $nome cai na faixa $inicio+$qtd do /etc/subgid ($dono)"
                return 1
            fi
        done </etc/subgid
    fi
    printf '%s\n' "$gid"
}

# --- marca dos grupos criados (Fase 51a) -----------------------------------------------
MARCA_GRUPOS=/usr/local/lib/ward/grupos-criados

# Linhas "nome gid" válidas da marca (só ward-app/ward-nm, gid de sistema).
grupos_marcados() {
    [ -f "$MARCA_GRUPOS" ] && [ ! -L "$MARCA_GRUPOS" ] || return 0
    grep -a -x -E 'ward-(app|nm) [1-9][0-9]{0,2}' -- "$MARCA_GRUPOS" || true
}

# Grava a marca inteira (temporário na mesma pasta e `rename`); vazia, apaga. A pasta do
# Ward é criada 0755 root:root se faltar; a de cima (/usr/local/lib) quem chama conferiu.
pasta_da_marca() {
    local pasta="${MARCA_GRUPOS%/*}"
    if [ ! -e "$pasta" ] && [ ! -L "$pasta" ]; then
        mkdir -m 0755 -- "$pasta" || erro "não deu para criar $pasta (a marca dos grupos)"
    fi
    [ -d "$pasta" ] && [ ! -L "$pasta" ] && [ "$(stat -c %u -- "$pasta")" = 0 ] ||
        erro "$pasta não é uma pasta do root"
}

gravar_marca() {
    local conteudo="$1" pasta="${MARCA_GRUPOS%/*}" tmp
    pasta_da_marca
    if [ -z "$conteudo" ]; then
        rm -f -- "$MARCA_GRUPOS"
        return 0
    fi
    tmp="$(mktemp -- "$pasta/.grupos.XXXXXXXX")"
    printf '%s\n' "$conteudo" >"$tmp"
    chmod 0644 -- "$tmp"
    mv -f -T -- "$tmp" "$MARCA_GRUPOS"
}

# Marca de `nome`: criado agora (`criado=1`) grava "nome gid"; já existente, fica a
# linha só se o gid for o mesmo (outro grupo com o nome não herda a marca).
marcar_grupo() {
    local nome="$1" gid="$2" criado="$3" antes linhas
    antes="$(grupos_marcados)"
    linhas="$(grep -v -- "^$nome " <<<"$antes" || true)"
    if [ "$criado" = 1 ] || grep -q -x -F -- "$nome $gid" <<<"$antes"; then
        linhas="$(printf '%s\n%s\n' "$linhas" "$nome $gid" | grep -v '^$' || true)"
    fi
    [ "$linhas" = "$antes" ] || gravar_marca "$linhas"
}

# Apaga os grupos da marca que ainda têm o gid marcado; os outros ficam (com aviso).
apagar_grupos_marcados() {
    local g gid marcados
    marcados="$(grupos_marcados)"
    for g in ward-nm ward-app; do
        grep -q -- "^${g}:" /etc/group || continue
        gid="$(awk -v g="$g" '$1 == g {print $2; exit}' <<<"$marcados")"
        if [ -n "$gid" ] && [ "$(grep -- "^${g}:" /etc/group | cut -d: -f3)" = "$gid" ]; then
            groupdel "$g" || printf 'ward: aviso: não deu para apagar o grupo %s\n' "$g" >&2
        else
            printf 'ward: aviso: o grupo %s não foi criado pelo Ward (sem marca em %s); ficou\n' "$g" "$MARCA_GRUPOS" >&2
        fi
    done
    if [ -e "$MARCA_GRUPOS" ] || [ -L "$MARCA_GRUPOS" ]; then
        rm -f -- "$MARCA_GRUPOS"
    fi
}

garantir_grupo() {
    local nome="$1" criado=0 saida
    if ! grep -q -- "^${nome}:" /etc/group; then
        if getent group "$nome" >/dev/null; then
            erro "o grupo $nome vem do NSS (fora do /etc/group); a instalação não o aceita"
        fi
        # A pasta da marca antes do groupadd: um grupo criado fica sempre marcado.
        pasta_da_marca
        groupadd --system "$nome"
        criado=1
    fi
    if ! saida="$(conferir_grupo "$nome")"; then
        if [ "$criado" = 1 ]; then
            groupdel "$nome" || true
        fi
        erro "grupo recusado (D60): $saida. Nada foi instalado. Corrija o grupo (ou apague-o com 'groupdel $nome') e rode de novo."
    fi
    marcar_grupo "$nome" "$saida" "$criado"
    [ "$criado" = 1 ] && passo "grupo $nome criado (gid $saida)" || passo "grupo $nome já existia e confere (gid $saida)"
}
