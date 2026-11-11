# shellcheck shell=bash
# Core dump of the WebKitGTK helpers (Phase 51c; S2, S8). Read by `install.sh`
# (`make install`, which generates the apport list) and copied whole into the
# `postinst` of the `.deb` by `deb.sh` (only the `core_pattern` warning). The includer
# defines `error` and `warning`.
#
# `ward`, `wardd`, `ward-app` and `ward-nm` are non-dumpable (PR_SET_DUMPABLE=0) and
# have RLIMIT_CORE 0/0: the kernel does not even call the `core_pattern` program. The
# WebKitGTK helpers the app opens (WebKitWebProcess, WebKitNetworkProcess,
# WebKitGPUProcess) are ordinary programs: `exec` leaves them dumpable, and a `kill -SEGV`
# from another process of the same uid produces the core. The inherited RLIMIT_CORE is
# not enough:
#   - apport (core_pattern with a pipe) writes the report to /var/crash, readable by the
#     user, with the process memory, even with the limit at 0;
#   - Ubuntu's systemd-coredump gets 9223372036854775808 in place of the limit
#     (/usr/lib/sysctl.d/50-coredump.conf) and keeps the core in /var/lib/systemd/coredump;
#   - limit 1 (the kernel does not call the pipe) does not hold against the attacker: the
#     process of the same uid lowers the helper's current limit to 0 with prlimit(2).
# That is why the install puts these paths on the apport list and turns off the storage
# of systemd-coredump (`install/coredump.conf.d/ward.conf`).

# Dir of the WebKitGTK 4.1 helpers on Debian/Ubuntu (multiarch of the dpkg
# architecture). An architecture outside the table refuses: the list would get the wrong
# path.
webkit_dir() {
    local arch ma
    arch="$(dpkg --print-architecture 2>/dev/null)" || error "falta o dpkg (a lista do apport usa a arquitetura do sistema)"
    case "$arch" in
        amd64) ma=x86_64-linux-gnu ;;
        arm64) ma=aarch64-linux-gnu ;;
        armhf) ma=arm-linux-gnueabihf ;;
        i386) ma=i386-linux-gnu ;;
        ppc64el) ma=powerpc64le-linux-gnu ;;
        riscv64) ma=riscv64-linux-gnu ;;
        s390x) ma=s390x-linux-gnu ;;
        *) error "arquitetura $arch fora da tabela dos auxiliares do WebKitGTK (install/dump.sh)" ;;
    esac
    printf '/usr/lib/%s/webkit2gtk-4.1\n' "$ma"
}

# Helpers Ward knows. A new helper in the package (another WebKitGTK version) refuses
# the install until it is added here (fail closed).
WEBKIT_HELPERS=(WebKitGPUProcess WebKitNetworkProcess WebKitWebProcess)

# apport_list: the content of /etc/apport/report-ignore/ward (and of the legacy
# /etc/apport/blacklist.d/ward). apport compares the process ExecutablePath with each
# whole line; the comment line never matches a path.
apport_list() {
    local dir arch package c
    # `|| exit 1`: the `error` inside $(...) only leaves the subshell.
    dir="$(webkit_dir)" || exit 1
    arch="$(dpkg --print-architecture)" || error "falta o dpkg"
    # Verifies against the installed package: every helper of it must be on the list.
    if package="$(dpkg-query -L "libwebkit2gtk-4.1-0:$arch" 2>/dev/null)"; then
        while IFS= read -r c; do
            [[ "$c" =~ ^"$dir"/WebKit[A-Za-z]*Process$ ]] || continue
            local known=0 a
            for a in "${WEBKIT_HELPERS[@]}"; do
                [ "$c" = "$dir/$a" ] && known=1
            done
            [ "$known" = 1 ] ||
                error "o libwebkit2gtk-4.1-0 tem o auxiliar ${c##*/}, fora da lista do apport (WEBKIT_HELPERS em install/dump.sh); nada foi instalado"
        done <<<"$package"
    else
        warning "libwebkit2gtk-4.1-0 ($arch) não está instalado: a lista do apport usa os caminhos padrão de $dir"
    fi
    printf '# Ward (Fase 51c): o apport não grava relatório de crash destes programas.\n'
    printf '# Os auxiliares do WebKitGTK guardam na memória o que o app mostra (S2, S8).\n'
    printf '%s\n' /usr/local/bin/ward /usr/local/bin/wardd /usr/local/bin/ward-app /usr/local/lib/ward/ward-nm
    for a in "${WEBKIT_HELPERS[@]}"; do
        printf '%s/%s\n' "$dir" "$a"
    done
}

# Core pattern warning (optional argument: a file): Ward only covers apport and
# systemd-coredump. Another program in `core_pattern` (pipe or socket) can write the core
# of the WebKitGTK helpers. A file `core_pattern` honors the RLIMIT_CORE 0/0 the app
# gives the helpers. The argument (default: the kernel's) is for the test.
avisar_core_pattern() {
    local cp
    cp="$(head -c 4096 -- "${1:-/proc/sys/kernel/core_pattern}" 2>/dev/null || true)"
    case "$cp" in
        '|/usr/share/apport/apport '* | '|/usr/share/apport/apport') ;;
        '|/usr/lib/systemd/systemd-coredump '* | '|/usr/lib/systemd/systemd-coredump') ;;
        '@@/run/systemd/coredump.socket' | '@/run/systemd/coredump.socket') ;;
        '|'* | '@'*)
            warning "kernel.core_pattern entrega o core a '${cp%% *}', que o Ward não configura: um crash dos auxiliares do WebKitGTK pode ir para o disco com a memória do app (docs/instalacao.md, 'Core dump')"
            ;;
        *) ;;
    esac
}
