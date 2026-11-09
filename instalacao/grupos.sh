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
#   - fora de toda faixa do /etc/subgid (um namespace de usuário o mapearia);
#   - o NSS (getent) devolve esse mesmo grupo pelo nome e pelo gid.
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

garantir_grupo() {
    local nome="$1" criado=0 saida
    if ! grep -q -- "^${nome}:" /etc/group; then
        if getent group "$nome" >/dev/null; then
            erro "o grupo $nome vem do NSS (fora do /etc/group); a instalação não o aceita"
        fi
        groupadd --system "$nome"
        criado=1
    fi
    if ! saida="$(conferir_grupo "$nome")"; then
        if [ "$criado" = 1 ]; then
            groupdel "$nome" || true
        fi
        erro "grupo recusado (D60): $saida. Nada foi instalado. Corrija o grupo (ou apague-o com 'groupdel $nome') e rode de novo."
    fi
    [ "$criado" = 1 ] && passo "grupo $nome criado (gid $saida)" || passo "grupo $nome já existia e confere (gid $saida)"
}

