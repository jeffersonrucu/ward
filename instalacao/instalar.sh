#!/bin/bash -p
# Instalação do Ward (Fase 51; D18, D46, D47, D57, D60, D62, D63; V9.1–V9.4, V9.6, V9.7,
# V9.10, V9.11). Chamado pelo `make install` / `make uninstall`; nunca compila nada (o
# build é do usuário, sem root: `make release`).
#
#   instalar.sh instalar     instala a árvore de `arvore.txt`
#   instalar.sh desinstalar  desfaz a instalação; nunca toca no home (o cofre em
#                            ~/.local/share/ward/ fica)
#
# Variáveis (o Makefile exporta):
#   DESTDIR       vazio: instala no sistema (exige root: `sudo make install`). Com valor
#                 (caminho absoluto, sem link): só copia a árvore para lá, sem grupos,
#                 sem dono root e sem systemctl (staging do `.deb`, Fase 52, e testes).
#   PREFIX        só /usr/local: os binários procuram uns aos outros ali (D18).
#   WARD_UID      uid do usuário do cofre (padrão: SUDO_UID). Só no sistema.
#   EXTENSAO_ID   id da extensão (32 letras a–p), o mesmo do pacote.
#   EXTENSAO_CRX  pacote CRX3 da extensão.
#   BIN_WARD BIN_WARDD BIN_APP BIN_NM  artefatos de release.
#
# Ordem (falha fechada): confere tudo e copia os artefatos para uma pasta temporária do
# próprio processo (as conferências valem para a cópia, não para o caminho do usuário,
# que um processo do mesmo uid poderia trocar) -> grupos (D60) -> arquivos -> systemd.
# Nada é instalado se uma conferência falhar.

export PATH=/usr/sbin:/usr/bin:/sbin:/bin
export LC_ALL=C
set -euo pipefail
umask 077

AQUI="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
RAIZ="${AQUI%/*}"
ARVORE="$AQUI/arvore.txt"
PYTHON=(/usr/bin/python3 -I -B)

erro() { printf 'ward-instalar: %s\n' "$*" >&2; exit 1; }
aviso() { printf 'ward-instalar: aviso: %s\n' "$*" >&2; }
passo() { printf '==> %s\n' "$*"; }

ACAO="${1:-}"
[ $# -eq 1 ] || erro "uso: instalar.sh instalar|desinstalar"
case "$ACAO" in instalar | desinstalar) ;; *) erro "uso: instalar.sh instalar|desinstalar" ;; esac

# --- DESTDIR e PREFIX -----------------------------------------------------------------
DESTDIR="${DESTDIR-}"
PREFIX="${PREFIX-/usr/local}"
[ "$PREFIX" = /usr/local ] || erro "PREFIX tem de ser /usr/local (os binários, as units e o manifesto do Chrome apontam para lá; D18)"
if [ -n "$DESTDIR" ]; then
    DESTDIR="${DESTDIR%/}"
    [[ "$DESTDIR" == /* ]] || erro "DESTDIR tem de ser um caminho absoluto"
    [[ "$DESTDIR" != *//* && "$DESTDIR" != */./* && "$DESTDIR" != */../* && "$DESTDIR" != */. && "$DESTDIR" != */.. ]] ||
        erro "DESTDIR com '//', '.' ou '..'"
    [ -n "$DESTDIR" ] || erro "DESTDIR=/ : para instalar no sistema, deixe DESTDIR vazio"
    if [ "$ACAO" = instalar ] && [ ! -e "$DESTDIR" ]; then
        mkdir -p -m 0755 -- "$DESTDIR"
    fi
    [ -d "$DESTDIR" ] || erro "DESTDIR não é uma pasta"
    [ "$(cd -P -- "$DESTDIR" && pwd -P)" = "$DESTDIR" ] || erro "DESTDIR passa por link simbólico"
    SISTEMA=0
else
    SISTEMA=1
    [ "$(id -u)" = 0 ] || erro "instalar no sistema exige root: rode 'sudo make $([ "$ACAO" = instalar ] && echo install || echo uninstall)' (ou use DESTDIR=<pasta>)"
fi
RAIZ_ALVO="$DESTDIR"
ROOT=0
[ "$(id -u)" = 0 ] && ROOT=1

# --- arvore.txt -----------------------------------------------------------------------
TIPOS=() MODOS=() DONOS=() GRUPOS=() CAMINHOS=() ORIGENS=()
while read -r tipo modo dono grupo caminho origem resto; do
    [[ -z "$tipo" || "$tipo" == \#* ]] && continue
    [ -z "${resto:-}" ] || erro "arvore.txt: coluna a mais em $caminho"
    [[ "$tipo" =~ ^[Ddf]$ && "$modo" =~ ^[0-7]{4}$ && "$caminho" =~ ^[a-z][A-Za-z0-9@._/-]*$ ]] ||
        erro "arvore.txt: linha inválida ($tipo $modo $caminho)"
    [[ "$caminho" != *..* && "$caminho" != *//* && "$caminho" != */ ]] || erro "arvore.txt: caminho inválido $caminho"
    [ "$tipo" != f ] || [ -n "${origem:-}" ] || erro "arvore.txt: arquivo sem origem ($caminho)"
    TIPOS+=("$tipo") MODOS+=("$modo") DONOS+=("$dono") GRUPOS+=("$grupo")
    CAMINHOS+=("$caminho") ORIGENS+=("${origem:-}")
done <"$ARVORE"
[ "${#CAMINHOS[@]}" -gt 0 ] || erro "arvore.txt vazia"

# Marcas de artefato `modo-teste` (D7, D63; V9.2): as variáveis do daemon de teste, a
# ponte e o app de teste, e o nome da pasta do socket de teste. `WARD_DEV_DBUS_SISTEMA`
# é a única `WARD_DEV_*` que o release do `wardd` ainda carrega como texto (mensagem de
# erro de uma função que só o daemon de teste chama; ver handoff 51 › Problemas): ela é
# lida só no `modo-teste`. Qualquer outra `WARD_DEV_*` recusa.
conferir_sem_modo_teste() {
    local arq="$1" nome="$2" achado
    achado="$(grep -a -o -E 'WARD_DEV_[A-Z0-9_]*' -- "$arq" | grep -v -x -F WARD_DEV_DBUS_SISTEMA | head -n 1 || true)"
    [ -z "$achado" ] || erro "$nome foi compilado com modo-teste (contém $achado); use o build de release ('make release')"
    for marca in ward-ponte-teste wardd-app-teste ward-teste; do
        if grep -a -q -F -- "$marca" "$arq"; then
            erro "$nome foi compilado com modo-teste (contém $marca); use o build de release ('make release')"
        fi
    done
}

# Arquivo de origem: regular, sem link, não vazio, até `max` bytes.
conferir_origem() {
    local arq="$1" nome="$2" max="$3"
    [ -n "$arq" ] || erro "$nome não definido"
    [ ! -L "$arq" ] || erro "$nome ($arq) é um link simbólico"
    [ -f "$arq" ] || erro "$nome ($arq) não existe ou não é um arquivo"
    local tam
    tam="$(stat -c %s -- "$arq")"
    [ "$tam" -gt 0 ] && [ "$tam" -le "$max" ] || erro "$nome ($arq) vazio ou grande demais"
}

# Copia sem seguir link (`cp -P`) e confere que a cópia é um arquivo regular.
copiar_para() {
    local de="$1" para="$2" nome="$3"
    cp -P -- "$de" "$para"
    [ -f "$para" ] && [ ! -L "$para" ] || erro "$nome mudou durante a cópia (virou link?)"
}

conferir_elf() {
    local arq="$1" nome="$2"
    [ "$(head -c 4 -- "$arq" | od -An -tx1 | tr -d ' \n')" = 7f454c46 ] || erro "$nome não é um executável ELF"
}

# --- grupos (D47, D60; V9.11): conferir_grupo e garantir_grupo, os mesmos do `.deb` ---
# shellcheck source=instalacao/grupos.sh
. "$AQUI/grupos.sh"

# --- pastas e arquivos ----------------------------------------------------------------
# Pasta: sem link; no sistema, do root e sem escrita para grupo e outros.
garantir_pasta() {
    local p="$1"
    if [ -L "$p" ]; then
        erro "$p é um link simbólico; a instalação não segue links"
    elif [ -e "$p" ]; then
        [ -d "$p" ] || erro "$p existe e não é uma pasta"
        if [ "$SISTEMA" = 1 ]; then
            [ "$(stat -c %u -- "$p")" = 0 ] || erro "$p não é do root"
            local m
            m="$(stat -c %a -- "$p")"
            [ $((8#$m & 8#022)) = 0 ] || erro "$p tem escrita para grupo ou outros ($m)"
        fi
    else
        mkdir -m 0755 -- "$p"
        if [ "$ROOT" = 1 ]; then chown root:root -- "$p"; fi
    fi
}

# Arquivo: temporário na mesma pasta, dono e modo, e `rename` por cima (um link no
# destino é trocado, nunca seguido).
instalar_arquivo() {
    local de="$1" para="$2" modo="$3" dono="$4" grupo="$5" tmp
    [ ! -d "$para" ] || erro "$para é uma pasta"
    tmp="$(mktemp -- "${para%/*}/.ward-instalar.XXXXXXXX")"
    cat -- "$de" >"$tmp"
    if [ "$ROOT" = 1 ] && [ "$SISTEMA" = 1 ]; then chown -- "$dono:$grupo" "$tmp"; fi
    # Depois do chown (o chown tira o setgid).
    chmod -- "$modo" "$tmp"
    mv -f -T -- "$tmp" "$para"
}

# --- instalar -------------------------------------------------------------------------
instalar() {
    local i
    passo "conferindo os artefatos e a extensão"
    local id="${EXTENSAO_ID-}"
    [[ "$id" =~ ^[a-p]{32}$ ]] || erro "EXTENSAO_ID tem de ser o id da extensão (32 letras de a a p); veja o handoff da Fase 41"
    if [ "$SISTEMA" = 1 ]; then
        local uid="${WARD_UID-}"
        [[ "$uid" =~ ^[1-9][0-9]{3,9}$ ]] || erro "WARD_UID ($uid) tem de ser o uid do usuário do cofre (>= 1000); com sudo vem do SUDO_UID"
        [ "$uid" -ne 65534 ] || erro "WARD_UID não pode ser o nobody"
        getent passwd "$uid" >/dev/null || erro "WARD_UID $uid não existe"
    fi
    [ -x "${PYTHON[0]}" ] || erro "falta ${PYTHON[0]} (lê o pacote da extensão)"

    TMP="$(mktemp -d /tmp/ward-instalar.XXXXXXXX)"
    trap 'rm -rf -- "$TMP"' EXIT
    local nome var arq
    for par in ward:BIN_WARD wardd:BIN_WARDD app:BIN_APP nm:BIN_NM; do
        nome="${par%%:*}" var="${par#*:}"
        arq="${!var-}"
        conferir_origem "$arq" "$var" $((512 * 1024 * 1024))
        copiar_para "$arq" "$TMP/bin-$nome" "$var"
        conferir_elf "$TMP/bin-$nome" "$var ($arq)"
        conferir_sem_modo_teste "$TMP/bin-$nome" "$var ($arq)"
    done
    conferir_origem "${EXTENSAO_CRX-}" EXTENSAO_CRX $((64 * 1024 * 1024))
    copiar_para "$EXTENSAO_CRX" "$TMP/crx" EXTENSAO_CRX
    local lido id_crx versao
    lido="$("${PYTHON[@]}" "$AQUI/crx.py" "$TMP/crx")" || erro "EXTENSAO_CRX recusado"
    read -r id_crx versao <<<"$lido"
    [ "$id_crx" = "$id" ] || erro "o pacote EXTENSAO_CRX é da extensão $id_crx, não de EXTENSAO_ID=$id"
    [[ "$versao" =~ ^[0-9.]+$ ]] || erro "versão da extensão fora do formato"
    for i in "${!CAMINHOS[@]}"; do
        if [[ "${ORIGENS[$i]}" == src:* ]]; then
            local src="$RAIZ/${ORIGENS[$i]#src:}"
            conferir_origem "$src" "${ORIGENS[$i]}" $((1024 * 1024))
        fi
    done

    # Gerados (o id e a versão já passaram pelas expressões acima: sem aspas nem '<').
    local ext=/usr/local/share/ward/extensao
    cat >"$TMP/ger-nm" <<EOF
{
  "name": "br.ward.nm",
  "description": "Ward: host de native messaging da extensão",
  "path": "/usr/local/lib/ward/ward-nm",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$id/"]
}
EOF
    cat >"$TMP/ger-politica" <<EOF
{
  "ExtensionSettings": {
    "$id": {
      "installation_mode": "force_installed",
      "update_url": "file://$ext/update.xml",
      "toolbar_pin": "force_pinned"
    }
  }
}
EOF
    cat >"$TMP/ger-update" <<EOF
<?xml version='1.0' encoding='UTF-8'?>
<gupdate xmlns='http://www.google.com/update2/response' protocol='2.0'>
  <app appid='$id'>
    <updatecheck codebase='file://$ext/ward.crx' version='$versao' />
  </app>
</gupdate>
EOF

    if [ "$SISTEMA" = 1 ]; then
        passo "grupos de sistema (D47, D60)"
        garantir_grupo ward-app
        garantir_grupo ward-nm
        local ptrace suid
        ptrace="$(cat /proc/sys/kernel/yama/ptrace_scope 2>/dev/null || echo ?)"
        suid="$(cat /proc/sys/fs/suid_dumpable 2>/dev/null || echo ?)"
        [ "$ptrace" != 0 ] || aviso "kernel.yama.ptrace_scope = 0: o daemon recusa os papéis app e extensão (D47)"
        [ "$suid" != 1 ] || aviso "fs.suid_dumpable = 1: o daemon recusa os papéis app e extensão (D47)"
    fi

    passo "arquivos em ${DESTDIR:-/}"
    for i in "${!CAMINHOS[@]}"; do
        [ "${TIPOS[$i]}" = f ] && continue
        garantir_pasta "$RAIZ_ALVO/${CAMINHOS[$i]}"
    done
    local de
    for i in "${!CAMINHOS[@]}"; do
        [ "${TIPOS[$i]}" = f ] || continue
        case "${ORIGENS[$i]}" in
            bin:*) de="$TMP/bin-${ORIGENS[$i]#bin:}" ;;
            src:*) de="$TMP/src-$i"; copiar_para "$RAIZ/${ORIGENS[$i]#src:}" "$de" "${ORIGENS[$i]}" ;;
            crx) de="$TMP/crx" ;;
            ger:*) de="$TMP/ger-${ORIGENS[$i]#ger:}" ;;
            *) erro "arvore.txt: origem desconhecida ${ORIGENS[$i]}" ;;
        esac
        instalar_arquivo "$de" "$RAIZ_ALVO/${CAMINHOS[$i]}" "${MODOS[$i]}" "${DONOS[$i]}" "${GRUPOS[$i]}"
        printf '    %s %s:%s %s\n' "${MODOS[$i]}" "${DONOS[$i]}" "${GRUPOS[$i]}" "/${CAMINHOS[$i]}"
    done

    if [ "$SISTEMA" = 1 ]; then
        passo "systemd: /run/ward (tmpfiles.d) e wardd@$uid.socket (D46)"
        systemd-tmpfiles --create /usr/local/lib/tmpfiles.d/ward.conf
        systemctl daemon-reload
        systemctl enable --now "wardd@$uid.socket"
        # Reinstalação: o daemon que já roda passa ao binário novo (o cofre trava).
        systemctl try-restart "wardd@$uid.service"
        passo "pronto: o wardd sobe na primeira conexão; o uso diário não pede sudo"
    else
        passo "pronto (DESTDIR: sem grupos, sem dono root e sem systemctl; donos e modos de verdade em instalacao/arvore.txt)"
    fi
}

# --- desinstalar ----------------------------------------------------------------------
desinstalar() {
    local i p
    if [ "$SISTEMA" = 1 ]; then
        passo "systemd: parando e desligando o wardd@*"
        local unidades=() u
        while read -r u _; do
            [[ "$u" =~ ^wardd@[0-9]+\.(socket|service)$ ]] && unidades+=("$u")
        done < <(systemctl list-units --all --plain --no-legend --full 'wardd@*' 2>/dev/null || true)
        for p in /etc/systemd/system/*.wants/wardd@*.socket; do
            [ -e "$p" ] || [ -L "$p" ] || continue
            u="${p##*/}"
            [[ "$u" =~ ^wardd@[0-9]+\.socket$ ]] && unidades+=("$u")
        done
        for u in "${unidades[@]}"; do
            if [[ "$u" == *.socket ]]; then
                systemctl disable --now "$u" || aviso "não deu para desligar $u"
            else
                systemctl stop "$u" || aviso "não deu para parar $u"
            fi
        done
    fi

    passo "arquivos em ${DESTDIR:-/}"
    for i in "${!CAMINHOS[@]}"; do
        [ "${TIPOS[$i]}" = f ] || continue
        p="$RAIZ_ALVO/${CAMINHOS[$i]}"
        if [ -d "$p" ] && [ ! -L "$p" ]; then
            aviso "$p é uma pasta; ficou"
        elif [ -e "$p" ] || [ -L "$p" ]; then
            rm -f -- "$p"
            printf '    removido /%s\n' "${CAMINHOS[$i]}"
        fi
    done
    for ((i = ${#CAMINHOS[@]} - 1; i >= 0; i--)); do
        [ "${TIPOS[$i]}" = d ] || continue
        p="$RAIZ_ALVO/${CAMINHOS[$i]}"
        if [ -d "$p" ] && [ ! -L "$p" ]; then
            rmdir -- "$p" 2>/dev/null || aviso "$p não está vazia; ficou"
        fi
    done

    if [ "$SISTEMA" = 1 ]; then
        systemctl daemon-reload
        systemctl reset-failed 'wardd@*' 2>/dev/null || true
        # /run/ward/<uid>/ (o socket sai com o RemoveOnStop) e /run/ward, se vazias.
        if [ -d /run/ward ] && [ ! -L /run/ward ]; then
            for p in /run/ward/*; do
                [ -d "$p" ] && [ ! -L "$p" ] && { rmdir -- "$p" 2>/dev/null || aviso "$p não está vazia; ficou"; }
            done
            rmdir /run/ward 2>/dev/null || aviso "/run/ward não está vazia; ficou"
        fi
        passo "grupos"
        for p in ward-nm ward-app; do
            if grep -q -- "^${p}:" /etc/group; then
                groupdel "$p" || aviso "não deu para apagar o grupo $p"
            fi
        done
    fi
    passo "pronto: o cofre e os backups em ~/.local/share/ward/ não foram tocados"
}

"$ACAO"
