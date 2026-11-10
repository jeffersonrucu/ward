# shellcheck shell=bash
# Core dump dos auxiliares do WebKitGTK (Fase 51c; S2, S8). Lido pelo `install.sh`
# (`make install`, que gera a lista do apport) e copiado inteiro para o `postinst` do
# `.deb` pelo `deb.sh` (só o aviso do `core_pattern`). Quem inclui define `erro` e
# `aviso`.
#
# O `ward`, o `wardd`, o `ward-app` e o `ward-nm` ficam não-dumpable (PR_SET_DUMPABLE=0)
# e com RLIMIT_CORE 0/0: o kernel nem chama o programa do `core_pattern`. Os auxiliares
# do WebKitGTK que o app abre (WebKitWebProcess, WebKitNetworkProcess, WebKitGPUProcess)
# são programas comuns: o `exec` os deixa dumpable, e um `kill -SEGV` de outro processo
# do mesmo uid gera o core. O RLIMIT_CORE herdado não basta:
#   - o apport (core_pattern com pipe) grava o relatório em /var/crash, legível pelo
#     usuário, com a memória do processo, mesmo com o limite em 0;
#   - o systemd-coredump do Ubuntu recebe 9223372036854775808 no lugar do limite
#     (/usr/lib/sysctl.d/50-coredump.conf) e guarda o core em /var/lib/systemd/coredump;
#   - o limite 1 (o kernel não chama o pipe) não vale contra o atacante: o processo do
#     mesmo uid baixa o limite atual do auxiliar para 0 com prlimit(2).
# Por isso a instalação põe esses caminhos na lista do apport e desliga o armazenamento
# do systemd-coredump (`install/coredump.conf.d/ward.conf`).

# Pasta dos auxiliares do WebKitGTK 4.1 no Debian/Ubuntu (multiarch da arquitetura do
# dpkg). Arquitetura fora da tabela recusa: a lista sairia com o caminho errado.
pasta_webkit() {
    local arq ma
    arq="$(dpkg --print-architecture 2>/dev/null)" || erro "falta o dpkg (a lista do apport usa a arquitetura do sistema)"
    case "$arq" in
        amd64) ma=x86_64-linux-gnu ;;
        arm64) ma=aarch64-linux-gnu ;;
        armhf) ma=arm-linux-gnueabihf ;;
        i386) ma=i386-linux-gnu ;;
        ppc64el) ma=powerpc64le-linux-gnu ;;
        riscv64) ma=riscv64-linux-gnu ;;
        s390x) ma=s390x-linux-gnu ;;
        *) erro "arquitetura $arq fora da tabela dos auxiliares do WebKitGTK (install/dump.sh)" ;;
    esac
    printf '/usr/lib/%s/webkit2gtk-4.1\n' "$ma"
}

# Auxiliares que o Ward conhece. Um auxiliar novo do pacote (outra versão do WebKitGTK)
# recusa a instalação até entrar aqui (falha fechada).
AUXILIARES_WEBKIT=(WebKitGPUProcess WebKitNetworkProcess WebKitWebProcess)

# lista_apport : o conteúdo de /etc/apport/report-ignore/ward (e do legado
# /etc/apport/blacklist.d/ward). O apport compara o ExecutablePath do processo com cada
# linha inteira; a linha de comentário nunca casa com um caminho.
lista_apport() {
    local pasta arq pacote c
    # `|| exit 1`: o `erro` de dentro do $(…) só sai do subshell.
    pasta="$(pasta_webkit)" || exit 1
    arq="$(dpkg --print-architecture)" || erro "falta o dpkg"
    # Confere contra o pacote instalado: todo auxiliar dele tem de estar na lista.
    if pacote="$(dpkg-query -L "libwebkit2gtk-4.1-0:$arq" 2>/dev/null)"; then
        while IFS= read -r c; do
            [[ "$c" =~ ^"$pasta"/WebKit[A-Za-z]*Process$ ]] || continue
            local conhecido=0 a
            for a in "${AUXILIARES_WEBKIT[@]}"; do
                [ "$c" = "$pasta/$a" ] && conhecido=1
            done
            [ "$conhecido" = 1 ] ||
                erro "o libwebkit2gtk-4.1-0 tem o auxiliar ${c##*/}, fora da lista do apport (AUXILIARES_WEBKIT em install/dump.sh); nada foi instalado"
        done <<<"$pacote"
    else
        aviso "libwebkit2gtk-4.1-0 ($arq) não está instalado: a lista do apport usa os caminhos padrão de $pasta"
    fi
    printf '# Ward (Fase 51c): o apport não grava relatório de crash destes programas.\n'
    printf '# Os auxiliares do WebKitGTK guardam na memória o que o app mostra (S2, S8).\n'
    printf '%s\n' /usr/local/bin/ward /usr/local/bin/wardd /usr/local/bin/ward-app /usr/local/lib/ward/ward-nm
    for a in "${AUXILIARES_WEBKIT[@]}"; do
        printf '%s/%s\n' "$pasta" "$a"
    done
}

# avisar_core_pattern [arquivo] : o Ward só cobre o apport e o systemd-coredump. Outro
# programa no `core_pattern` (pipe ou socket) pode gravar o core dos auxiliares do
# WebKitGTK. Um `core_pattern` de arquivo respeita o RLIMIT_CORE 0/0 que o app passa aos
# auxiliares. O argumento (padrão: o do kernel) é para o teste.
avisar_core_pattern() {
    local cp
    cp="$(head -c 4096 -- "${1:-/proc/sys/kernel/core_pattern}" 2>/dev/null || true)"
    case "$cp" in
        '|/usr/share/apport/apport '* | '|/usr/share/apport/apport') ;;
        '|/usr/lib/systemd/systemd-coredump '* | '|/usr/lib/systemd/systemd-coredump') ;;
        '@@/run/systemd/coredump.socket' | '@/run/systemd/coredump.socket') ;;
        '|'* | '@'*)
            aviso "kernel.core_pattern entrega o core a '${cp%% *}', que o Ward não configura: um crash dos auxiliares do WebKitGTK pode ir para o disco com a memória do app (docs/instalacao.md, 'Core dump')"
            ;;
        *) ;;
    esac
}
