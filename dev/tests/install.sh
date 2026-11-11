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
#    root on the system. Every refusal exits ≠ 0 without leaving a Ward file.
# 2. `ubuntu:24.04` container with systemd and no network (`dev/container-install/`), where
#    root exists only inside it: real `make install`/`make uninstall`
#    (`dev/container-install/inside.sh`). Same permissions as `make test-units` (see the
#    comment of `dev/tests/units.sh`): CAP_SYS_ADMIN and no AppArmor only for systemd to
#    remount its cgroup subtree.
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
cmp -s "$CRX" "$D/usr/local/share/ward/extensao/ward.crx" || failed "pacote da extensão diferente"
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
    lacks "51c: core_pattern $c sem aviso" "$(dump "avisar_core_pattern $TMP/cp-$c")" "AVISO"
done
verify "51c: core_pattern de outro programa avisa" "$(dump "avisar_core_pattern $TMP/cp-other")" "AVISO: kernel.core_pattern entrega o core a '|/usr/libexec/abrt-hook-ccpp'"
verify "51c: core_pattern por outro socket avisa" "$(dump "avisar_core_pattern $TMP/cp-socket")" "AVISO: kernel.core_pattern"
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
assert e["update_url"] == "file:///usr/local/share/ward/extensao/update.xml", e
print("politica ok")
ns = "{http://www.google.com/update2/response}"
root_el = ET.parse(d + "/usr/local/share/ward/extensao/update.xml").getroot()
apps = root_el.findall(ns + "app")
assert len(apps) == 1 and apps[0].get("appid") == ident
uc = apps[0].findall(ns + "updatecheck")
assert len(uc) == 1 and uc[0].get("codebase") == "file:///usr/local/share/ward/extensao/ward.crx"
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
cp "$APP" "$TMP/app-marked" && printf 'ward-ponte-teste' >>"$TMP/app-marked"
new_dir; expect_refusal "V9.2: wardd com test-mode" "modo-teste" "$d" BIN_WARDD="$TEST_WARDD"
new_dir; expect_refusal "V9.2: ward com WARD_DEV_SOCKET_DIR" "modo-teste" "$d" BIN_WARD="$TMP/ward-marked"
new_dir; expect_refusal "V9.2: ward-nm com WARD_DEV_DIR" "modo-teste" "$d" BIN_NM="$TMP/nm-marked"
new_dir; expect_refusal "V9.2: ward-app com a ponte de teste" "modo-teste" "$d" BIN_APP="$TMP/app-marked"
# Phase 51a: the exception for the system D-Bus dev variable is gone (the release does not have the text).
cp "$WARDD" "$TMP/wardd-dbus" && printf "x WARD_DEV_DBUS_SISTEMA x" >>"$TMP/wardd-dbus"
new_dir; expect_refusal "V9.2: wardd com WARD_DEV_DBUS_SISTEMA (sem exceção)" "WARD_DEV_DBUS_SISTEMA" "$d" BIN_WARDD="$TMP/wardd-dbus"
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
install -m 0644 "$ROOT/install/groups.sh" "$ROOT/install/chrome.sh" "$ROOT/install/dump.sh" "$C/repo/install/"
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
/usr/bin/python3 -I -c 'import json,sys; sys.stdout.write(json.load(open(sys.argv[1]))["senha_mestra"])' \
    "$ROOT/dev/fixtures/entries.json" >"$C/senha"
chmod 0644 "$C/senha"

IMAGE="ward-teste/install-systemd:1"
docker build --quiet -t "$IMAGE" -f "$ROOT/dev/container-install/Dockerfile" "$ROOT/dev/container-units" >/dev/null
docker run -d --name "$CONTAINER_NAME" --network none --pull never \
    --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cgroupns=private --cap-add SYS_ADMIN --security-opt apparmor=unconfined \
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
