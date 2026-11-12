#!/bin/bash -p
# `make test-install` (Phase 51; V9.1, V9.2, V9.3, V9.4, V9.6, V9.7, V9.10, V9.11; D18,
# D46, D47, D57, D60, D62, D63).
#
# Never installs anything on the host. Two parts:
#
# 1. Host, without root: `make install DESTDIR=<dir in .dev/>` with the artifacts without
#    `test-mode` (debug, `target/no-test-mode`), the container start-up example in place of
#    `ward-app` (the real app's start-up without GTK) and a fake CRX3 package
#    (`fake_crx.py`: random key and signature). Verifies the exact tree of
#    `install/tree.txt` (V9.1), modes, contents, the Chrome JSON files (V9.6), the
#    `update.xml`, the `systemd-analyze verify` of the installed units, `make uninstall`
#    (V9.10: everything of Ward goes away, what is not its own stays, including a planted
#    "vault") and the refusals: `test-mode` artifact (V9.2), non-ELF, link, invalid
#    extension id or one different from the package's, tampered package, PREFIX, relative
#    DESTDIR or one with a link, a dir with a link in the path, and `make install` without
#    root on the system. Every refusal exits ≠ 0 without leaving a Ward file. Phase 57
#    (m2:V5.6, m2:DT15, m2:DT16): the new extension dir, no 0.1.x path, the 0.1.x extension
#    dir deleted by the install and by the uninstall (never through a link), the groups
#    marks (`groups.sh`) and the stop of the daemons (`upgrade.sh`) with fake commands.
# 2. `ubuntu:24.04` container with systemd and no network (`dev/container-install/`), where
#    root exists only inside it: real `make install`/`make uninstall`
#    (`dev/container-install/inside.sh`), and 0.2.0 over the 0.1.4 `make install` with
#    its wardd unlocked (m2:V5.7) and the 0.2.0 uninstall over a 0.1.4 install (m2:V5.8),
#    from `.dev/baseline/` (Phase 3) and the v0.1.4 tag. Same permissions as
#    `make test-units` (see the comment of `dev/tests/units.sh`): CAP_SYS_ADMIN and no
#    AppArmor only for systemd to remount its cgroup subtree.
#
# Stays out of `make check` (needs Docker). Usage: dev/tests/install.sh [--destdir-only]

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"
unset MAKEFLAGS MAKELEVEL MFLAGS
cd "$ROOT"

DESTDIR_ONLY=0
[ "${1:-}" = --destdir-only ] && DESTDIR_ONLY=1

NO_TEST_MODE="$ROOT/target/no-test-mode/debug"
WARD="$NO_TEST_MODE/ward"
WARDD="$NO_TEST_MODE/wardd"
NM="$NO_TEST_MODE/ward-nm"
APP="$NO_TEST_MODE/examples/startup-container"
CLIENT="$NO_TEST_MODE/examples/wardd-test-client"
TEST_WARDD="$ROOT/target/debug/wardd"
PLATFORM="$ROOT/target/debug/examples/ward-platform-container"
for b in "$WARD" "$WARDD" "$NM" "$APP" "$CLIENT" "$TEST_WARDD" "$PLATFORM"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b (rode pelo 'make test-install')" >&2; exit 1; }
done
for p in systemd-analyze make; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$ROOT/.dev"
TMP="$(mktemp -d "$ROOT/.dev/test-install.XXXXXX")"
CONTAINER_NAME="ward-install-$$"
trap 'docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true; rm -rf -- "$TMP"' EXIT

failures=0
ok() { echo "ok: $*"; }
failed() { echo "FALHOU: $*"; failures=$((failures + 1)); }
verify() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else failed "$1 (esperado '$3', veio '$2')"; fi
}
lacks() {
    if [[ "$2" != *"$3"* ]]; then ok "$1"; else failed "$1 (não devia ter '$3': $2)"; fi
}

# Fake package and its id.
CRX="$TMP/ward.crx"
ID="$(/usr/bin/python3 -I "$ROOT/dev/tests/fake_crx.py" "$CRX")"
[[ "$ID" =~ ^[a-p]{32}$ ]] || { echo "fake_crx.py não deu um id" >&2; exit 1; }

# `make install`/`uninstall` with DESTDIR; extra arguments override the defaults.
mk() {
    local action="$1" d="$2"
    shift 2
    /usr/bin/make -s -C "$ROOT" "$action" DESTDIR="$d" EXTENSION_ID="$ID" EXTENSION_CRX="$CRX" \
        BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" WARD_UID= "$@"
}

# 0.1.4 names (installer interface and installed paths, m2:DT15), recorded as baseline
# data in dev/fixtures/legacy/install-0.1.4.txt, and the 0.1.4 artifacts (Phase 57).
# shellcheck source=dev/tests/legacy-baseline.sh
. "$ROOT/dev/tests/legacy-baseline.sh"
OLD_EXT_DIR="$(legacy_name extension_dir)" || exit 1
OLD_MARK="$(legacy_name groups_mark)" || exit 1

# "type mode path" lines of the tree.
TREE="$TMP/tree"
awk '!/^#/ && NF >= 5 {print $1, $2, $5}' "$ROOT/install/tree.txt" >"$TREE"
FILES=()
while read -r t _ c; do [ "$t" = f ] && FILES+=("$c"); done <"$TREE"

# No Ward file in $1 (the refusal leaves nothing).
nothing_from_ward() {
    local d="$1" c
    for c in "${FILES[@]}"; do
        if [ -e "$d/$c" ] || [ -L "$d/$c" ]; then echo "$c"; return; fi
    done
    echo "nada"
}

# Refusal: exits ≠ 0, with the message, without a Ward file.
expect_refusal() {
    local label="$1" msg="$2" d="$3"
    shift 3
    local r code=0
    r="$(mk install "$d" "$@" 2>&1)" || code=$?
    if [ "$code" -ne 0 ]; then ok "$label: recusado (saiu $code)"; else failed "$label: instalou (saiu 0)"; fi
    verify "$label: mensagem" "$r" "$msg"
    verify "$label: nenhum arquivo do Ward" "$(nothing_from_ward "$d")" "nada"
}

echo "==> 1. DESTDIR (host, sem root)"
D="$TMP/root"
r="$(mk install "$D" 2>&1)" || { printf '%s\n' "$r"; failed "make install DESTDIR"; }
ok "make install DESTDIR saiu 0"

# V9.1: the tree is exactly the one of tree.txt.
expected="$(awk '{print $3}' "$TREE" | sort)"
real="$(cd "$D" && find . -mindepth 1 -printf '%P\n' | sort)"
if [ "$expected" = "$real" ]; then ok "V9.1: árvore exata (${#FILES[@]} arquivos)"; else
    failed "V9.1: árvore diferente"
    diff <(printf '%s\n' "$expected") <(printf '%s\n' "$real") || true
fi
# Modes (the root owner only in the container) and nothing writable by group and others.
while read -r t m c; do
    real_m="$(stat -c %a -- "$D/$c")"
    [ $((8#$real_m)) = $((8#$m)) ] || failed "modo de $c: $real_m (esperado $m)"
    [ $((8#$real_m & 8#022)) = 0 ] || failed "$c com escrita para grupo ou outros"
    if [ "$t" = f ]; then [ -f "$D/$c" ] && [ ! -L "$D/$c" ] || failed "$c não é arquivo regular"; fi
    if [ "$t" != f ]; then [ -d "$D/$c" ] && [ ! -L "$D/$c" ] || failed "$c não é pasta"; fi
done <"$TREE"
ok "V9.1: modos de tree.txt (ward-app e ward-nm 2755), sem escrita para grupo e outros"
# Contents.
cmp -s "$WARD" "$D/usr/local/bin/ward" && cmp -s "$WARDD" "$D/usr/local/bin/wardd" &&
    cmp -s "$APP" "$D/usr/local/bin/ward-app" && cmp -s "$NM" "$D/usr/local/lib/ward/ward-nm" &&
    ok "binários iguais aos artefatos" || failed "binário instalado diferente do artefato"
for u in wardd@.socket wardd@.service; do
    cmp -s "$ROOT/install/systemd/$u" "$D/usr/local/lib/systemd/system/$u" || failed "$u diferente"
done
cmp -s "$ROOT/install/tmpfiles.d/ward.conf" "$D/usr/local/lib/tmpfiles.d/ward.conf" || failed "tmpfiles diferente"
cmp -s "$CRX" "$D/usr/local/share/ward/extension/ward.crx" || failed "pacote da extensão diferente"
ok "units, tmpfiles.d e pacote iguais aos do repositório"

# Phase 51c (S2, S8): the apport list and systemd-coredump without storage.
cmp -s "$ROOT/install/coredump.conf.d/ward.conf" "$D/etc/systemd/coredump.conf.d/ward.conf" &&
    ok "51c: drop-in do systemd-coredump igual ao do repositório" || failed "51c: drop-in do systemd-coredump diferente"
verify "51c: drop-in com Storage=none e ProcessSizeMax=0" \
    "$(grep -v '^#' "$D/etc/systemd/coredump.conf.d/ward.conf" | tr '\n' ' ')" "[Coredump] Storage=none ProcessSizeMax=0"
cmp -s "$D/etc/apport/report-ignore/ward" "$D/etc/apport/blacklist.d/ward" &&
    ok "51c: report-ignore/ward e blacklist.d/ward iguais" || failed "51c: as duas listas do apport diferem"
APPORT_LIST="$(grep -v '^#' "$D/etc/apport/report-ignore/ward")"
for c in /usr/local/bin/ward /usr/local/bin/wardd /usr/local/bin/ward-app /usr/local/lib/ward/ward-nm; do
    grep -qxF -- "$c" <<<"$APPORT_LIST" || failed "51c: $c fora da lista do apport"
done
# Oracle independent of dump.sh: the helpers of the package installed on the host.
DPKG_ARCH="$(dpkg --print-architecture)"
PACKAGE_HELPERS="$(dpkg-query -L "libwebkit2gtk-4.1-0:$DPKG_ARCH" 2>/dev/null | grep -E '/webkit2gtk-4\.1/WebKit[A-Za-z]*Process$' || true)"
if [ -n "$PACKAGE_HELPERS" ]; then
    while IFS= read -r c; do
        grep -qxF -- "$c" <<<"$APPORT_LIST" || failed "51c: auxiliar $c do libwebkit2gtk-4.1-0 fora da lista do apport"
    done <<<"$PACKAGE_HELPERS"
    ok "51c: os $(wc -l <<<"$PACKAGE_HELPERS") auxiliares do libwebkit2gtk-4.1-0 do host e os 4 binários do Ward na lista"
else
    echo "aviso: libwebkit2gtk-4.1-0 fora do host; os auxiliares ficam para o test-deb"
fi
lacks "51c: nenhuma linha vazia ou com espaço na lista (o apport compara a linha inteira)" \
    "$(grep -n -E '^$|^[^#].*[[:space:]]' "$D/etc/apport/report-ignore/ward" || true)" ":"
# apport's own code (Report.check_ignored) reading the installed list, in both dirs;
# controls outside the list are not silenced.
if /usr/bin/python3 -I -c 'import apport.report' 2>/dev/null; then
    mapfile -t IGN <<<"$APPORT_LIST"
    for dir in report-ignore blacklist.d; do
        r="$(env -i PATH=/usr/bin:/bin HOME=/nonexistent /usr/bin/python3 -I "$ROOT/dev/tests/apport_list.py" \
            "$D/etc/apport/$dir" "${IGN[@]}" -- /usr/bin/sleep "${IGN[-1]}X" /usr/local/bin/ward-appX 2>&1; echo "saiu=$?")"
        verify "51c: o apport ($dir) cala os ${#IGN[@]} caminhos e não os controles" "$r" "saiu=0"
        lacks "51c: apport ($dir) sem FALHOU" "$r" "FALHOU"
    done
else
    echo "aviso: sem o módulo do apport no host; a leitura pelo apport fica para o test-deb"
fi

# dump.sh from the inside, with fake dpkg/dpkg-query (functions beat commands).
dump() {
    env -i PATH=/usr/bin:/bin /bin/bash -p -c '
        error() { printf "ERRO: %s\n" "$*" >&2; exit 1; }
        warning() { printf "AVISO: %s\n" "$*" >&2; }
        . "$1"; shift; eval "$1"' _ "$ROOT/install/dump.sh" "$1" 2>&1
    echo "saiu=$?"
}
r="$(dump 'dpkg() { echo amd64; }; dpkg-query() { printf "%s\n" /usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/WebKitWebProcess /usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/WebKitPrintProcess; }; apport_list')"
verify "51c ataque: auxiliar novo no pacote (WebKitPrintProcess) recusa a lista" "$r" "WebKitPrintProcess, fora da lista"
lacks "51c ataque: auxiliar novo não sai 0" "$r" "saiu=0"
r="$(dump 'dpkg() { echo sparc64; }; apport_list')"
verify "51c: arquitetura fora da tabela recusa" "$r" "fora da tabela"
lacks "51c: arquitetura fora da tabela não sai 0" "$r" "saiu=0"
r="$(dump 'dpkg() { echo arm64; }; dpkg-query() { return 1; }; apport_list')"
verify "51c: sem o libwebkit2gtk, avisa e usa o caminho do multiarch" "$r" "AVISO: libwebkit2gtk-4.1-0 (arm64) não está instalado"
verify "51c: arm64 -> aarch64-linux-gnu" "$r" "/usr/lib/aarch64-linux-gnu/webkit2gtk-4.1/WebKitWebProcess"
printf '%s' '|/usr/share/apport/apport -p%p -s%s -c%c -d%d -P%P -u%u -g%g -F%F -- %E' >"$TMP/cp-apport"
printf '%s' '|/usr/lib/systemd/systemd-coredump %P %u %g %s %t 9223372036854775808 %h %d' >"$TMP/cp-systemd"
printf '%s' 'core' >"$TMP/cp-file"
printf '%s' '|/usr/libexec/abrt-hook-ccpp %s %c %p %u %g %t %e' >"$TMP/cp-other"
printf '%s' '@/run/outro/coredump.socket' >"$TMP/cp-socket"
for c in apport systemd file; do
    lacks "51c: core_pattern $c sem aviso" "$(dump "warn_core_pattern $TMP/cp-$c")" "AVISO"
done
verify "51c: core_pattern de outro programa avisa" "$(dump "warn_core_pattern $TMP/cp-other")" "AVISO: kernel.core_pattern entrega o core a '|/usr/libexec/abrt-hook-ccpp'"
verify "51c: core_pattern por outro socket avisa" "$(dump "warn_core_pattern $TMP/cp-socket")" "AVISO: kernel.core_pattern"
# m2:DT15/DT16 from the inside: the groups marks of groups.sh and the stop of upgrade.sh,
# with the marks in $TMP/marks and fake mark_dir, groupdel and systemctl (functions beat
# commands: nothing reaches the host's groups nor its systemd; the host's /etc/group is
# only read).
M="$TMP/marks"
mkdir -m 0700 "$M"
upgrade_inside() {
    env -i PATH=/usr/bin:/bin /bin/bash -p -c '
        set -euo pipefail
        error() { printf "ERRO: %s\n" "$*" >&2; exit 1; }
        warning() { printf "AVISO: %s\n" "$*" >&2; }
        step() { printf "PASSO: %s\n" "$*"; }
        . "$1"; . "$2"; M="$3"; code="$4"
        GROUPS_MARK="$M/created-groups" LEGACY_GROUPS_MARK="$M/legacy-mark"
        UPGRADE_MARK="$M/upgrade-stopped" UPGRADE_MARK_OWNER="$(id -u)"
        mark_dir() { :; }
        groupdel() { printf "groupdel %s\n" "$*"; }
        # Fake systemd: active units in $M/active; a unit listed in $M/stuck does not stop.
        systemctl() {
            local last="${!#}"
            printf "%s\n" "$*" >>"$M/log"
            case "$1" in
                list-units) { grep -E "\.${last##*.}\$" "$M/active" || true; } | sed "s/\$/ loaded active running x/" ;;
                stop)
                    [ ! -e "$M/fail" ] || return 1
                    if ! grep -q -x -F -- "$2" "$M/stuck" 2>/dev/null; then
                        { grep -v -x -F -- "$2" "$M/active" || true; } >"$M/active.new"
                        mv -- "$M/active.new" "$M/active"
                    fi ;;
                *) ;;
            esac
        }
        eval "$code"' _ "$ROOT/install/groups.sh" "$ROOT/install/upgrade.sh" "$M" "$1" 2>&1
    echo "saiu=$?"
}
reset_marks() { rm -rf -- "${M:?}"/* "$M"/.[!.]* 2>/dev/null || true; }
reset_marks
printf 'ward-app 990\nward-nm 991\nward-x 5\nward-app 99999\n' >"$M/legacy-mark"
r="$(upgrade_inside 'migrate_legacy_mark; cat "$GROUPS_MARK"; [ -e "$LEGACY_GROUPS_MARK" ] || echo antiga-apagada')"
verify "m2:DT15: a marca da 0.1.x passa para created-groups com as mesmas linhas" "$r" $'ward-app 990\nward-nm 991\n'
verify "m2:DT15: a marca da 0.1.x é apagada depois" "$r" "antiga-apagada"
lacks "m2:DT15: linha inválida da marca antiga não passa" "$r" "ward-x"
lacks "m2:DT15: gid fora do sistema na marca antiga não passa" "$r" "99999"
verify "m2:DT15: migração sai 0" "$r" "saiu=0"
reset_marks
printf 'ward-app 990\n' >"$M/created-groups"
printf 'ward-app 990\nward-nm 991\n' >"$M/legacy-mark"
r="$(upgrade_inside 'migrate_legacy_mark; echo "nova=[$(cat "$GROUPS_MARK")]"; [ -e "$LEGACY_GROUPS_MARK" ] || echo antiga-apagada')"
verify "m2:DT15: com as duas marcas, a nova vence" "$r" "nova=[ward-app 990]"
verify "m2:DT15: com as duas marcas, a antiga é apagada" "$r" "antiga-apagada"
reset_marks
printf 'ward-nm 991\n' >"$M/legacy-mark"
verify "m2:V5.8: sem a nova, a marca lida é a da 0.1.x" "$(upgrade_inside 'marked_groups')" $'ward-nm 991\nsaiu=0'
reset_marks
printf 'ward-app 990\n' >"$TMP/planted-mark"
ln -s "$TMP/planted-mark" "$M/created-groups"
printf 'ward-nm 991\n' >"$M/legacy-mark"
verify "m2:DT15 ataque: marca nova por link não dá grupo nenhum (nem cai na antiga)" "$(upgrade_inside 'marked_groups')" "saiu=0"
lacks "m2:DT15 ataque: marca nova por link: nada lido" "$(upgrade_inside 'marked_groups')" "ward-"
reset_marks
ln -s "$TMP/planted-mark" "$M/legacy-mark"
r="$(upgrade_inside 'migrate_legacy_mark; [ -e "$GROUPS_MARK" ] && echo nova-criada; [ -L "$LEGACY_GROUPS_MARK" ] || echo link-apagado')"
lacks "m2:DT15 ataque: marca antiga por link não é seguida" "$r" "nova-criada"
verify "m2:DT15 ataque: o link da marca antiga é apagado" "$r" "link-apagado"
verify "m2:DT15 ataque: o alvo do link intacto" "$(cat "$TMP/planted-mark")" "ward-app 990"
reset_marks
printf 'ward-app 990\n' >"$M/created-groups"
printf 'ward-nm 991\n' >"$M/legacy-mark"
r="$(upgrade_inside 'delete_marked_groups')"
verify "m2:V5.8: delete_marked_groups sai 0" "$r" "saiu=0"
lacks "m2:V5.8: o uninstall apaga as duas marcas" "$(ls -A "$M")" "mark"
lacks "m2:V5.8: o uninstall apaga a created-groups" "$(ls -A "$M")" "created-groups"
# m2:DT16: sockets stop before the daemons, the daemons before any file; a daemon that
# stays up or a stop that fails refuses.
reset_marks
printf 'wardd@1000.socket\nwardd@1000.service\nwardd@1001.socket\n' >"$M/active"
r="$(upgrade_inside 'stop_running_daemons')"
verify "m2:DT16: stop_running_daemons sai 0" "$r" "saiu=0"
verify "m2:DT16: avisa que para o daemon antes de trocar" "$r" "PASSO: parando wardd@1000.service antes de trocar os binários"
verify "m2:DT16: os sockets param antes do daemon" "$(grep '^stop ' "$M/log" | tr '\n' ' ')" "stop wardd@1000.socket stop wardd@1001.socket stop wardd@1000.service "
verify "m2:DT16: nenhum wardd@ ativo no fim" "[$(cat "$M/active")]" "[]"
reset_marks
printf 'wardd@1000.service\n' >"$M/active"
printf 'wardd@1000.service\n' >"$M/stuck"
r="$(upgrade_inside 'stop_running_daemons; echo trocou')"
verify "m2:DT16 ataque: daemon que não para recusa" "$r" "continua no ar"
lacks "m2:DT16 ataque: daemon que não para: nada depois" "$r" "trocou"
lacks "m2:DT16 ataque: daemon que não para: não sai 0" "$r" "saiu=0"
reset_marks
printf 'wardd@1000.socket\n' >"$M/active"
: >"$M/fail"
r="$(upgrade_inside 'stop_running_daemons; echo trocou')"
verify "m2:DT16: stop que falha recusa" "$r" "não deu para parar wardd@1000.socket"
lacks "m2:DT16: stop que falha: nada depois" "$r" "trocou"
lacks "m2:DT16 (Fase 63a): stop que falha não deixa marca" "$(ls -A "$M")" "upgrade-stopped"
reset_marks
# Phase 63a (m2:DT16): the `.deb` preinst writes the stop mark after the stop; the
# abort-upgrade only starts the sockets in it and never restarts a daemon.
printf 'wardd@1000.socket\nwardd@1000.service\nwardd@1001.socket\n' >"$M/active"
r="$(upgrade_inside 'stop_running_daemons mark; stat -c "modo=%a" "$UPGRADE_MARK"; cat "$UPGRADE_MARK/sockets"')"
verify "m2:DT16 (Fase 63a): stop com marca sai 0" "$r" "saiu=0"
verify "m2:DT16 (Fase 63a): marca 0700" "$r" "modo=700"
verify "m2:DT16 (Fase 63a): marca com os sockets parados" "$r" $'wardd@1000.socket\nwardd@1001.socket\n'
lacks "m2:DT16 (Fase 63a): marca sem o serviço" "$(cat "$M/upgrade-stopped/sockets")" "service"
: >"$M/log"
r="$(upgrade_inside 'restart_stopped_sockets; [ -e "$UPGRADE_MARK" ] || echo marca-apagada')"
verify "m2:DT16 (Fase 63a): abort religa só os sockets da marca" "$(grep -E '^(start|try-restart|restart) ' "$M/log" | tr '\n' ' ')" "start wardd@1000.socket start wardd@1001.socket "
verify "m2:DT16 (Fase 63a): abort apaga a marca" "$r" "marca-apagada"
reset_marks
r="$(upgrade_inside 'stop_running_daemons mark; [ -e "$UPGRADE_MARK" ] || echo sem-marca')"
verify "m2:DT16 (Fase 63a): nada parado, nenhuma marca" "$r" "sem-marca"
reset_marks
printf 'wardd@1000.socket\nwardd@1000.service\n' >"$M/active"
r="$(upgrade_inside 'restart_stopped_sockets; echo fim')"
verify "m2:DT16 ataque (Fase 63a): sem marca (preinst recusado), o abort sai 0" "$r" "saiu=0"
verify "m2:DT16 ataque (Fase 63a): sem marca, o abort não religa nem reinicia nada" "[$(grep -E '^(start|stop|try-restart|restart|reload)' "$M/log" 2>/dev/null || true)]" "[]"
reset_marks
install -d -m 0700 "$TMP/planted-stop"
printf 'wardd@1000.socket\n' >"$TMP/planted-stop/sockets"
ln -s "$TMP/planted-stop" "$M/upgrade-stopped"
r="$(upgrade_inside 'restart_stopped_sockets; [ -L "$UPGRADE_MARK" ] || echo link-apagado')"
verify "m2:DT16 ataque (Fase 63a): marca por link não é seguida" "[$(grep -E '^(start|try-restart|restart) ' "$M/log" 2>/dev/null || true)]" "[]"
verify "m2:DT16 ataque (Fase 63a): o link da marca é apagado" "$r" "link-apagado"
verify "m2:DT16 ataque (Fase 63a): o alvo do link intacto" "$(cat "$TMP/planted-stop/sockets")" "wardd@1000.socket"
reset_marks
printf 'wardd@1000.socket\n' >"$M/active"
ln -s "$TMP/planted-stop" "$M/upgrade-stopped"
r="$(upgrade_inside 'stop_running_daemons mark; echo trocou')"
verify "m2:DT16 ataque (Fase 63a): link no lugar da marca recusa" "$r" "não deu para criar"
lacks "m2:DT16 ataque (Fase 63a): link no lugar da marca: nada depois" "$r" "trocou"
verify "m2:DT16 ataque (Fase 63a): link no lugar da marca: alvo intacto" "$(ls -A "$TMP/planted-stop")" "sockets"
rm -rf -- "$TMP/planted-stop"
reset_marks
install -d -m 0755 "$M/upgrade-stopped"
printf 'wardd@1000.socket\n' >"$M/upgrade-stopped/sockets"
r="$(upgrade_inside 'restart_stopped_sockets')"
verify "m2:DT16 ataque (Fase 63a): marca 0755 não é lida" "[$(grep -E '^start ' "$M/log" 2>/dev/null || true)]" "[]"
reset_marks
install -d -m 0700 "$M/upgrade-stopped"
printf 'wardd@1000.service\nwardd@x.socket\n../wardd@1.socket\nwardd@1001.socket; reboot\nwardd@1002.socket\n' >"$M/upgrade-stopped/sockets"
r="$(upgrade_inside 'restart_stopped_sockets')"
verify "m2:DT16 ataque (Fase 63a): da marca só sai wardd@<uid>.socket" "$(grep -E '^(start|try-restart|restart) ' "$M/log" | tr '\n' ' ')" "start wardd@1002.socket "
reset_marks
printf 'wardd@1000.socket\nwardd@1000.service\n' >"$M/active"
r="$(upgrade_inside 'start_enabled_sockets; [ -e "$UPGRADE_MARK" ] || echo marca-apagada')"
verify "m2:DT16 (Fase 63a): depois da troca (configure), o daemon ainda no ar vai para o binário novo" "$(grep -E '^try-restart ' "$M/log" | tr '\n' ' ')" "try-restart wardd@1000.service "
reset_marks

# V9.6, the policy and update.xml.
r="$(/usr/bin/python3 -I - "$D" "$ID" <<'EOF'
import json, sys, xml.etree.ElementTree as ET
d, ident = sys.argv[1], sys.argv[2]
nm = json.load(open(d + "/etc/opt/chrome/native-messaging-hosts/br.ward.nm.json"))
assert nm == {"name": "br.ward.nm", "description": nm["description"], "path": "/usr/local/lib/ward/ward-nm",
              "type": "stdio", "allowed_origins": [f"chrome-extension://{ident}/"]}, nm
print("nm ok")
pol = json.load(open(d + "/etc/opt/chrome/policies/managed/ward.json"))
# Phase 51a: NativeMessagingUserLevelHosts false (only root's manifest counts).
# Phase 44c: Chrome's password manager and leak detection turned off.
assert sorted(pol) == ["ExtensionSettings", "NativeMessagingUserLevelHosts", "PasswordLeakDetectionEnabled",
                       "PasswordManagerEnabled"], pol
assert pol["NativeMessagingUserLevelHosts"] is False, pol
assert pol["PasswordManagerEnabled"] is False, pol
assert pol["PasswordLeakDetectionEnabled"] is False, pol
assert list(pol["ExtensionSettings"]) == [ident], pol
e = pol["ExtensionSettings"][ident]
assert e["installation_mode"] == "force_installed", e
# m2:V5.6 (m2:DT15): the extension dir of 0.2.0.
assert e["update_url"] == "file:///usr/local/share/ward/extension/update.xml", e
print("politica ok")
ns = "{http://www.google.com/update2/response}"
root_el = ET.parse(d + "/usr/local/share/ward/extension/update.xml").getroot()
apps = root_el.findall(ns + "app")
assert len(apps) == 1 and apps[0].get("appid") == ident
uc = apps[0].findall(ns + "updatecheck")
assert len(uc) == 1 and uc[0].get("codebase") == "file:///usr/local/share/ward/extension/ward.crx"
assert uc[0].get("version") == "0.1.0"
print("update ok")
EOF
)" || true
verify "V9.6: manifesto de native messaging (allowed_origins só o id, path do ward-nm)" "$r" "nm ok"
verify "política do Chrome: NativeMessagingUserLevelHosts, PasswordManagerEnabled e PasswordLeakDetectionEnabled false, só a extensão do Ward, force_installed, update local" "$r" "politica ok"
verify "update.xml: id, pacote local e versão do pacote" "$r" "update ok"
# systemd-analyze verify of the installed units (a copy; ExecStart does not exist on the host).
mkdir "$TMP/verify"
cp "$D/usr/local/lib/systemd/system/wardd@.socket" "$D/usr/local/lib/systemd/system/wardd@.service" "$TMP/verify/"
output="$(systemd-analyze verify --man=no "$TMP/verify/wardd@1000.socket" "$TMP/verify/wardd@1000.service" 2>&1 || true)"
rest="$(printf '%s\n' "$output" | grep -v -e '/usr/local/bin/wardd' -e '^$' || true)"
[ -z "$rest" ] && ok "systemd-analyze verify das units instaladas" || { printf '%s\n' "$output"; failed "systemd-analyze verify"; }

# Reinstall on top (a link planted in place of a file: replaced, never followed).
printf 'intacto\n' >"$TMP/link-target"
rm -f "$D/usr/local/bin/wardd"
ln -s "$TMP/link-target" "$D/usr/local/bin/wardd"
r="$(mk install "$D" 2>&1)" || { printf '%s\n' "$r"; failed "reinstalar"; }
[ -f "$D/usr/local/bin/wardd" ] && [ ! -L "$D/usr/local/bin/wardd" ] && cmp -s "$WARDD" "$D/usr/local/bin/wardd" &&
    ok "reinstalar troca o link plantado pelo binário" || failed "o link plantado ficou"
verify "o alvo do link plantado não foi escrito" "$(cat "$TMP/link-target")" "intacto"

# m2:V5.6 (m2:DT15): the extension dir of 0.2.0 and no 0.1.x path; the mark of the created
# groups is not in the tree (only the install on the system writes it) and tree.txt says why.
[ -d "$D/usr/local/share/ward/extension" ] && [ -f "$D/usr/local/share/ward/extension/update.xml" ] &&
    ok "m2:V5.6: usr/local/share/ward/extension/ com o update.xml" || failed "m2:V5.6: sem usr/local/share/ward/extension/"
verify "m2:V5.6: nenhum caminho da 0.1.x na árvore" \
    "[$(cd "$D" && find . -mindepth 1 -printf '%P\n' | grep -x -F -e "$OLD_EXT_DIR" -e "$OLD_MARK" -e "$OLD_EXT_DIR/ward.crx" -e "$OLD_EXT_DIR/update.xml" || true)]" "[]"
lacks "m2:V5.6: tree.txt sem caminho da 0.1.x" "$(grep -v '^#' "$ROOT/install/tree.txt")" "$OLD_EXT_DIR"
verify "m2:V5.6: tree.txt explica a marca usr/local/lib/ward/created-groups" \
    "$(grep '^#' "$ROOT/install/tree.txt" | tr '\n' ' ')" "usr/local/lib/ward/created-groups, the mark of the groups the"
lacks "m2:V5.6: a marca não sai com DESTDIR" "$(ls -A "$D/usr/local/lib/ward")" "created-groups"

# m2:DT15: reinstall over a 0.1.x tree in the DESTDIR (the `.deb` staging case): the old
# extension dir goes, its link in place of a file is deleted without touching the target.
mkdir -p "$D/$OLD_EXT_DIR"
printf 'pacote 0.1.x\n' >"$D/$OLD_EXT_DIR/ward.crx"
printf 'alvo de fora\n' >"$TMP/legacy-target"
ln -s "$TMP/legacy-target" "$D/$OLD_EXT_DIR/update.xml"
r="$(mk install "$D" 2>&1)" || { printf '%s\n' "$r"; failed "m2:DT15: reinstalar por cima da 0.1.x"; }
[ ! -e "$D/$OLD_EXT_DIR" ] && [ ! -L "$D/$OLD_EXT_DIR" ] && ok "m2:DT15: a pasta da extensão da 0.1.x foi apagada" ||
    failed "m2:DT15: a pasta da extensão da 0.1.x ficou"
verify "m2:DT15: a instalação diz o que apagou" "$r" "removido /$OLD_EXT_DIR/ward.crx (0.1.x)"
verify "m2:DT15 ataque: o alvo do link no lugar do update.xml antigo intacto" "$(cat "$TMP/legacy-target")" "alvo de fora"
# The same on uninstall (a 0.1.x install that the 0.2.0 uninstall undoes).
mkdir -p "$D/$OLD_EXT_DIR"
printf 'pacote 0.1.x\n' >"$D/$OLD_EXT_DIR/ward.crx"
printf 'update 0.1.x\n' >"$D/$OLD_EXT_DIR/update.xml"
LEGACY_UNINSTALL="$TMP/legacy-uninstall"
cp -a "$D" "$LEGACY_UNINSTALL"
r="$(mk uninstall "$LEGACY_UNINSTALL" 2>&1)" || { printf '%s\n' "$r"; failed "m2:V5.8: uninstall com a pasta da 0.1.x"; }
[ ! -e "$LEGACY_UNINSTALL/usr/local/share/ward" ] && ok "m2:V5.8: o uninstall apaga a pasta da extensão da 0.1.x e usr/local/share/ward" ||
    failed "m2:V5.8: usr/local/share/ward ficou depois do uninstall"
rm -rf -- "$LEGACY_UNINSTALL"
r="$(mk install "$D" 2>&1)" || { printf '%s\n' "$r"; failed "reinstalar"; }

# V9.10: uninstall deletes what belongs to Ward and leaves the rest (a "vault" planted in the home).
# Fake home inside the DESTDIR; name built in pieces so the V1.5 scan
# (`dev/tests/attacks-foundation.sh`) does not mistake it for the real home.
VAULT_DIR="$D/home/u/.local/share/w""ard"
mkdir -p "$VAULT_DIR"
head -c 4096 /dev/urandom >"$VAULT_DIR/cofre.ward"
printf 'de outro pacote\n' >"$D/usr/local/bin/other"
before="$(sha256sum "$VAULT_DIR/cofre.ward" "$D/usr/local/bin/other")"
r="$(mk uninstall "$D" 2>&1)" || { printf '%s\n' "$r"; failed "make uninstall DESTDIR"; }
verify "V9.10: uninstall apaga todo arquivo do Ward" "$(nothing_from_ward "$D")" "nada"
for c in usr/local/lib/ward usr/local/share/ward; do
    [ ! -e "$D/$c" ] || failed "V9.10: a pasta $c ficou"
done
[ -d "$D/usr/local/bin" ] && [ -d "$D/etc/opt/chrome/policies/managed" ] && ok "V9.10: pastas comuns ficam" || failed "pasta comum apagada"
verify "V9.10: o cofre e o arquivo de outro pacote intactos" "$(sha256sum "$VAULT_DIR/cofre.ward" "$D/usr/local/bin/other")" "$before"
r="$(mk uninstall "$D" 2>&1)" && ok "uninstall de novo (nada instalado) sai 0" || { printf '%s\n' "$r"; failed "uninstall repetido"; }

echo "==> 1b. Recusas (cada uma numa pasta nova)"
# `new_dir` sets $d to a new dir (no subshell, so the counter moves).
n=0
new_dir() { n=$((n + 1)); d="$TMP/r$n"; }
# Artifacts with the test-mode mark: the real test wardd and copies with the mark.
cp "$WARD" "$TMP/ward-marked" && printf 'WARD_DEV_SOCKET_DIR' >>"$TMP/ward-marked"
cp "$NM" "$TMP/nm-marked" && printf 'x WARD_DEV_DIR x' >>"$TMP/nm-marked"
cp "$APP" "$TMP/app-marked" && printf 'ward-test-bridge' >>"$TMP/app-marked"
new_dir; expect_refusal "V9.2: wardd com test-mode" "compilado com test-mode" "$d" BIN_WARDD="$TEST_WARDD"
new_dir; expect_refusal "V9.2: ward com WARD_DEV_SOCKET_DIR" "compilado com test-mode" "$d" BIN_WARD="$TMP/ward-marked"
new_dir; expect_refusal "V9.2: ward-nm com WARD_DEV_DIR" "compilado com test-mode" "$d" BIN_NM="$TMP/nm-marked"
new_dir; expect_refusal "V9.2: ward-app com a ponte de teste" "compilado com test-mode" "$d" BIN_APP="$TMP/app-marked"
# Phase 51a: the exception for the system D-Bus dev variable is gone (the release does not have the text).
cp "$WARDD" "$TMP/wardd-dbus" && printf "x WARD_DEV_SYSTEM_DBUS x" >>"$TMP/wardd-dbus"
new_dir; expect_refusal "V9.2: wardd com WARD_DEV_SYSTEM_DBUS (sem exceção)" "WARD_DEV_SYSTEM_DBUS" "$d" BIN_WARDD="$TMP/wardd-dbus"
# m2:V5.4 (m2:DT12, m2:Phase 56): each test-mode mark, in the new form and in the 0.1.x
# one, is refused by itself in a release artifact, before any file.
for mark in WARD_DEV_EXTENSION WARD_DEV_EXTENSAO WARD_DEV_DBUS_SISTEMA ward-test-bridge \
    ward-ponte-teste wardd-test-app wardd-app-teste wardd-test-approver wardd-aprovador-teste \
    ward-test ward-teste; do
    cp "$WARDD" "$TMP/wardd-mark" && printf 'x %s x' "$mark" >>"$TMP/wardd-mark"
    new_dir; expect_refusal "m2:V5.4: wardd com $mark" "contém $mark" "$d" BIN_WARDD="$TMP/wardd-mark"
done
# m2:V5.4: the 0.1.4 test wardd (feature `modo-teste`, socket dir `ward-teste`) of the
# baseline (m2:Phase 3), when this checkout has it.
BASELINE_TEST_WARDD="$ROOT/.dev/baseline/bin-test-mode/wardd"
if [ -f "$BASELINE_TEST_WARDD" ]; then
    new_dir; expect_refusal "m2:V5.4: wardd de teste 0.1.4" "compilado com test-mode" "$d" BIN_WARDD="$BASELINE_TEST_WARDD"
else
    echo "aviso: sem $BASELINE_TEST_WARDD (make da linha de base, m2:Fase 3); caso do wardd 0.1.4 pulado" >&2
fi
verify "o wardd de release não tem nenhum WARD_DEV_" "$(grep -a -c -E "WARD_DEV_[A-Z0-9_]*" "$WARDD" || true)" "0"
# Phase 51a: another policy in policies/managed with one of the names of Ward's is refused
# before any file (Chrome does not merge the two; the attacker's/administrator's would win).
other_policy() {
    new_dir
    mkdir -p "$d/etc/opt/chrome/policies/managed"
    printf "%s" "$1" >"$d/etc/opt/chrome/policies/managed/$2"
}
other_policy "{\"NativeMessagingUserLevelHosts\": true}" other.json
expect_refusal "política: outro arquivo com NativeMessagingUserLevelHosts" "NativeMessagingUserLevelHosts" "$d"
other_policy "{\"ExtensionSettings\": {\"*\": {\"installation_mode\": \"blocked\"}}}" 00-company.json
expect_refusal "política: outro arquivo com ExtensionSettings" "ExtensionSettings" "$d"
other_policy "{\"\\u004eativeMessagingUserLevelHosts\": true}" zz.json
expect_refusal "política: nome escondido com escape \\u" "\\u" "$d"
other_policy "{\"NativeMessagingUserLevelHosts\": true}" .hidden
expect_refusal "política: arquivo oculto também conta" "NativeMessagingUserLevelHosts" "$d"
# Phase 44c: another file that turns the password manager or the leak detection back on.
other_policy "{\"PasswordManagerEnabled\": true}" passwords.json
expect_refusal "política: outro arquivo com PasswordManagerEnabled" "PasswordManagerEnabled" "$d"
other_policy "{\"PasswordLeakDetectionEnabled\": true}" leak.json
expect_refusal "política: outro arquivo com PasswordLeakDetectionEnabled" "PasswordLeakDetectionEnabled" "$d"
# A policy file without those names: installs and stays intact.
other_policy "{\"HomepageLocation\": \"https://exemplo.invalid/\"}" other.json
r="$(mk install "$d" 2>&1)" && ok "política: outro arquivo sem os nomes do Ward não impede" || { printf "%s\n" "$r"; failed "política sem conflito recusada"; }
verify "política: o outro arquivo ficou intacto" "$(cat "$d/etc/opt/chrome/policies/managed/other.json")" "HomepageLocation"
printf '#!/bin/sh\necho falso\n' >"$TMP/script"
chmod 0755 "$TMP/script"
new_dir; expect_refusal "wardd que não é ELF" "não é um executável ELF" "$d" BIN_WARDD="$TMP/script"
ln -s "$WARD" "$TMP/ward-link"
new_dir; expect_refusal "ward por link simbólico" "link simbólico" "$d" BIN_WARD="$TMP/ward-link"
new_dir; expect_refusal "artefato ausente" "não existe" "$d" BIN_NM="$TMP/does-not-exist"
new_dir; expect_refusal "EXTENSION_ID vazio" "EXTENSION_ID" "$d" EXTENSION_ID=
new_dir; expect_refusal "EXTENSION_ID em maiúsculas" "EXTENSION_ID" "$d" EXTENSION_ID="${ID^^}"
new_dir; expect_refusal "EXTENSION_ID com 31 letras" "EXTENSION_ID" "$d" EXTENSION_ID="${ID:1}"
new_dir; expect_refusal "EXTENSION_ID com aspas (injeção no JSON)" "EXTENSION_ID" "$d" EXTENSION_ID="${ID:0:28}\",\"x"
other_id="$(/usr/bin/python3 -I "$ROOT/dev/tests/fake_crx.py" "$TMP/other.crx")"
new_dir; expect_refusal "EXTENSION_ID de outra extensão" "não de EXTENSION_ID" "$d" EXTENSION_ID="$other_id"
/usr/bin/python3 -I "$ROOT/dev/tests/fake_crx.py" "$TMP/tampered.crx" --adulterado >"$TMP/tampered-id"
new_dir; expect_refusal "pacote com id que não é o da chave" "EXTENSION_CRX recusado" "$d" \
    EXTENSION_ID="$(cat "$TMP/tampered-id")" EXTENSION_CRX="$TMP/tampered.crx"
/usr/bin/python3 -I "$ROOT/dev/tests/fake_crx.py" "$TMP/version.crx" --versao "1.0'><x" >/dev/null
new_dir; expect_refusal "pacote com versão fora do formato" "EXTENSION_CRX recusado" "$d" EXTENSION_CRX="$TMP/version.crx"
new_dir; expect_refusal "pacote ausente" "EXTENSION_CRX" "$d" EXTENSION_CRX=
new_dir; expect_refusal "PREFIX=/opt/ward" "PREFIX" "$d" PREFIX=/opt/ward
new_dir; expect_refusal "PREFIX=/usr/local/../tmp" "PREFIX" "$d" PREFIX=/usr/local/../tmp
new_dir
mkdir -p "$TMP/real-$n"
ln -s "$TMP/real-$n" "$d"
expect_refusal "DESTDIR por link" "link simbólico" "$d"
verify "DESTDIR por link: nada no alvo do link" "$(nothing_from_ward "$TMP/real-$n")" "nada"
expect_refusal "DESTDIR com .." "'..'" "$TMP/r1/../r-dot-dot"
new_dir
mkdir -p "$d/usr/local" "$TMP/bin-target"
ln -s "$TMP/bin-target" "$d/usr/local/bin"
expect_refusal "usr/local/bin por link dentro do DESTDIR" "link simbólico" "$d"
verify "pasta-link: nada escrito no alvo" "$(ls -A "$TMP/bin-target")" ""
# Phase 51c: the apport list dir and the systemd-coredump one by link.
for p in etc/apport/report-ignore etc/systemd/coredump.conf.d; do
    new_dir
    mkdir -p "$d/${p%/*}" "$TMP/target-$n"
    ln -s "$TMP/target-$n" "$d/$p"
    expect_refusal "51c: $p por link" "link simbólico" "$d"
    verify "51c: $p por link: nada escrito no alvo" "$(ls -A "$TMP/target-$n")" ""
done
# m2:DT15 attacks: the 0.1.x extension dir (or the Ward dir above it) by link, and a dir in
# place of an old file, refuse before any change and never touch the link target.
for p in "$OLD_EXT_DIR" usr/local/share/ward; do
    new_dir
    mkdir -p "$d/${p%/*}" "$TMP/victim-$n/extensao"
    printf 'da vítima\n' >"$TMP/victim-$n/ward.crx"
    printf 'da vítima\n' >"$TMP/victim-$n/extensao/ward.crx"
    ln -s "$TMP/victim-$n" "$d/$p"
    expect_refusal "m2:DT15 ataque: $p por link" "link simbólico" "$d"
    verify "m2:DT15 ataque: $p por link: o alvo intacto" "$(cat "$TMP/victim-$n/ward.crx" "$TMP/victim-$n/extensao/ward.crx")" $'da vítima\nda vítima'
done
new_dir
mkdir -p "$d/$OLD_EXT_DIR/ward.crx"
expect_refusal "m2:DT15: pasta no lugar do ward.crx da 0.1.x" "não é um arquivo" "$d"
[ -d "$d/$OLD_EXT_DIR/ward.crx" ] && ok "m2:DT15: a pasta no lugar do ward.crx ficou" || failed "m2:DT15: a pasta no lugar do ward.crx sumiu"

# Relative DESTDIR (make passes it as it came).
r="$(cd "$TMP" && /usr/bin/make -s -C "$ROOT" install DESTDIR=relative EXTENSION_ID="$ID" EXTENSION_CRX="$CRX" \
    BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" 2>&1; echo "saiu=$?")"
verify "DESTDIR relativo: recusado" "$r" "caminho absoluto"
not_exit0() { [[ "$2" != *"saiu=0"* ]] && ok "$1" || failed "$1 ($2)"; }
not_exit0 "DESTDIR relativo: não sai 0" "$r"
[ ! -e "$ROOT/relative" ] || failed "DESTDIR relativo criou $ROOT/relative"
# On the system without root: refused before anything (the host is not touched).
for action in install uninstall; do
    r="$(/usr/bin/make -s -C "$ROOT" "$action" DESTDIR= EXTENSION_ID="$ID" EXTENSION_CRX="$CRX" \
        BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" WARD_UID=1000 2>&1; echo "saiu=$?")"
    verify "make $action no sistema sem root: recusa" "$r" "exige root"
    not_exit0 "make $action no sistema sem root: não sai 0" "$r"
done

if [ "$DESTDIR_ONLY" = 1 ]; then
    [ "$failures" -eq 0 ] || { echo "test-install (DESTDIR): $failures falha(s)"; exit 1; }
    echo "test-install: parte DESTDIR ok (contêiner pulado: --destdir-only)"
    exit 0
fi

echo "==> 2. Contêiner com systemd (root só dentro dele)"
for p in docker gcc; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done
TEST_VAULT="$ROOT/.dev/test-vault.ward"
[ -f "$TEST_VAULT" ] && [ ! -L "$TEST_VAULT" ] || { echo "falta $TEST_VAULT; rode 'make dev-vault'" >&2; exit 1; }
# The file names inside $C are the ones `dev/container-install/inside.sh` reads: they
# change only together with it.
C="$TMP/container"
mkdir -m 0755 "$C" "$C/repo" "$C/bin"
install -m 0644 "$ROOT/Makefile" "$C/repo/Makefile"
mkdir -m 0755 "$C/repo/install" "$C/repo/install/systemd" "$C/repo/install/tmpfiles.d" \
    "$C/repo/install/coredump.conf.d"
install -m 0644 "$ROOT/install/tree.txt" "$C/repo/install/"
install -m 0755 "$ROOT/install/install.sh" "$ROOT/install/crx.py" "$C/repo/install/"
install -m 0644 "$ROOT/install/groups.sh" "$ROOT/install/chrome.sh" "$ROOT/install/dump.sh" "$ROOT/install/upgrade.sh" \
    "$C/repo/install/"
install -m 0644 "$ROOT/install/coredump.conf.d/ward.conf" "$C/repo/install/coredump.conf.d/"
install -m 0644 "$ROOT/install/systemd/wardd@.socket" "$ROOT/install/systemd/wardd@.service" "$C/repo/install/systemd/"
install -m 0644 "$ROOT/install/tmpfiles.d/ward.conf" "$C/repo/install/tmpfiles.d/"
mkdir -m 0755 "$C/repo/install/desktop" "$C/repo/docs" "$C/repo/docs/design" "$C/repo/docs/design/logo"
install -m 0644 "$ROOT/install/desktop/local.ward.app.desktop" "$ROOT/install/desktop/local.ward.app.metainfo.xml" \
    "$C/repo/install/desktop/"
install -m 0644 "$ROOT"/docs/design/logo/ward-icone-app-{48,64,128,256,512}.png "$ROOT/docs/design/logo/ward-icone-app.svg" \
    "$C/repo/docs/design/logo/"
install -m 0755 "$WARD" "$C/bin/ward"
install -m 0755 "$WARDD" "$C/bin/wardd"
install -m 0755 "$NM" "$C/bin/ward-nm"
install -m 0755 "$APP" "$C/bin/ward-app"
install -m 0755 "$TEST_WARDD" "$C/bin/wardd-test-mode"
install -m 0755 "$CLIENT" "$C/bin/cliente"
install -m 0755 "$PLATFORM" "$C/bin/plataforma"
gcc -O2 -Wall -Werror -o "$C/bin/falso" "$ROOT/dev/container-platform/fake.c"
gcc -shared -fPIC -O2 -Wall -Werror -o "$C/bin/libmarca.so" "$ROOT/dev/container-platform/mark.c"
chmod 0755 "$C/bin/falso" "$C/bin/libmarca.so"
install -m 0644 "$CRX" "$C/ward.crx"
printf '%s\n' "$ID" >"$C/id"
install -m 0644 "$TEST_VAULT" "$C/cofre.ward"
/usr/bin/python3 -I -c 'import json,sys; sys.stdout.write(json.load(open(sys.argv[1]))["master_password"])' \
    "$ROOT/dev/fixtures/entries.json" >"$C/senha"
chmod 0644 "$C/senha"
# m2:V5.7/V5.8 (Phase 57): 0.1.4 installed by its own `make install` (the installer of the
# tag), with its release binaries (`ward-app` = the 0.1.4 container start-up) and its fake
# CRX3 package from .dev/baseline/, the 0.1.4 vault and state and the v1 client.
O="$C/old"
mkdir -m 0755 "$O" "$O/bin"
for pair in ward:bin-release/ward wardd:bin-release/wardd ward-nm:bin-release/ward-nm ward-app:bin-release/partida-conteiner; do
    f="$(baseline_file "${pair#*:}")" || exit 1
    install -m 0755 "$f" "$O/bin/${pair%%:*}"
done
f="$(baseline_file deb-0.1.4/ward-fake.crx)" || exit 1
install -m 0644 "$f" "$O/ward.crx"
f="$(baseline_file deb-0.1.4/extension-id.txt)" || exit 1
[[ "$(cat "$f")" =~ ^[a-p]{32}$ ]] || { echo "id da extensão 0.1.4 fora do formato em $f" >&2; exit 1; }
install -m 0644 "$f" "$O/id"
stage_legacy_installer "$O/repo"
stage_legacy_fixtures "$C/legacy"

IMAGE="ward-teste/install-systemd:1"
docker build --quiet -t "$IMAGE" -f "$ROOT/dev/container-install/Dockerfile" "$ROOT/dev/container-units" >/dev/null
# SYS_PTRACE (Phase 63a, like test-deb): the watcher of m2:V5.7 reads /proc/<pid>/exe of
# the wardd 0.1.4, which is not dumpable; without it the readlink fails and the watcher
# sees nothing (its control runs on a non-dumpable process).
docker run -d --name "$CONTAINER_NAME" --network none --pull never \
    --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cgroupns=private --cap-add SYS_ADMIN --cap-add SYS_PTRACE --security-opt apparmor=unconfined \
    --ulimit memlock=-1:-1 \
    -v "$C:/opt/teste:ro" \
    -v "$ROOT/dev/container-install/inside.sh:/opt/inside.sh:ro" \
    "$IMAGE" >/dev/null
code=0
docker exec "$CONTAINER_NAME" /bin/bash -p /opt/inside.sh || code=$?
[ "$code" -eq 0 ] || failed "contêiner: inside.sh saiu $code"

if [ "$failures" -ne 0 ]; then
    echo "test-install: $failures falha(s)"
    exit 1
fi
echo "test-install: tudo ok"
