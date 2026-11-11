#!/bin/bash -p
# `make test-deb` (Phase 52; V9.9, V9.8 in the container; D18, D47, D60, D63).
#
# Never installs anything on the host. Two parts:
#
# 1. Host, without root: `make deb` with the artifacts without `test-mode` (debug,
#    `target/no-test-mode`), the container startup example in place of `ward-app` and
#    a fake CRX3 package (`fake_crx.py`). Verifies: the `dpkg-deb -c` listing is the
#    staging one and the one of `install/tree.txt` (V9.9); every path root/root, no
#    setuid/setgid bit in the package (a 2755 root:root would give root's gid), nothing
#    writable by group and others; content equal to the artifacts; control with Depends
#    on sshpass; preinst with the group functions of `groups.sh`. Refusals: `test-mode`
#    artifact (V9.2), extension id different from the package, relative or symlinked
#    DEB_DIR; none leaves a `.deb`.
# 2. `ubuntu:24.04` container with systemd and no network (`dev/container-deb/`), root
#    only inside it: `dpkg -i` without sshpass (fails), group outside D60 (refused with
#    no file), install, reinstall, `dpkg -r`/`-P` (`dev/container-deb/inside.sh`). Same
#    permissions as `make test-units`: CAP_SYS_ADMIN and no AppArmor only for systemd to
#    remount its cgroup subtree. CAP_SYS_PTRACE (Phase 51c) only for apport, called by
#    hand as root, to read /proc/<pid>/exe of the "crashed" process of uid 1000. apport
#    and systemd-coredump run with a fake core; kernel.core_pattern (of the host) is
#    never written: the container /proc/sys is read-only, verified first.
#
# Stays out of `make check` (requires Docker). Usage: dev/tests/deb.sh [--host-only]

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"
unset MAKEFLAGS MAKELEVEL MFLAGS
cd "$ROOT"

HOST_ONLY=0
[ "${1:-}" = --host-only ] && HOST_ONLY=1

NO_TM="$ROOT/target/no-test-mode/debug"
WARD="$NO_TM/ward"
WARDD="$NO_TM/wardd"
NM="$NO_TM/ward-nm"
APP="$NO_TM/examples/startup-container"
CLIENT="$NO_TM/examples/wardd-test-client"
WARDD_TEST="$ROOT/target/debug/wardd"
for b in "$WARD" "$WARDD" "$NM" "$APP" "$CLIENT" "$WARDD_TEST"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b (rode pelo 'make test-deb')" >&2; exit 1; }
done
for p in dpkg-deb make; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$ROOT/.dev"
TMP="$(mktemp -d "$ROOT/.dev/test-deb.XXXXXX")"
CONTAINER_NAME="ward-deb-$$"
trap 'docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true; rm -rf -- "$TMP"' EXIT

failures=0
ok() { echo "ok: $*"; }
fail() { echo "FALHOU: $*"; failures=$((failures + 1)); }
verify() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else fail "$1 (esperado '$3', veio '$2')"; fi
}

CRX="$TMP/ward.crx"
ID="$(/usr/bin/python3 -I "$ROOT/dev/tests/fake_crx.py" "$CRX")"
[[ "$ID" =~ ^[a-p]{32}$ ]] || { echo "fake_crx.py não deu um id" >&2; exit 1; }

# `make deb`; extra arguments override the defaults.
mk() {
    local d="$1"
    shift
    /usr/bin/make -s -C "$ROOT" deb DEB_DIR="$d" EXTENSION_ID="$ID" EXTENSION_CRX="$CRX" \
        BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" "$@"
}

echo "==> 1. make deb (host, sem root)"
D="$TMP/deb"
r="$(mk "$D" 2>&1)" || { printf '%s\n' "$r"; fail "make deb"; }
DEB="$(find "$D" -maxdepth 1 -name 'ward_*.deb' -print)"
verify "make deb gera ward_<versão>_<arq>.deb" "$DEB" "/ward_0.1.4_$(dpkg --print-architecture).deb"
[ -f "$DEB" ] || { echo "test-deb: sem pacote, nada mais a conferir" >&2; exit 1; }
verify "make deb imprime o sha256" "$r" "$(sha256sum "$DEB" | cut -d' ' -f1)"

listing="$(dpkg-deb -c "$DEB")"
paths="$(awk '{print $6}' <<<"$listing" | sed -e 's#^\./##' -e 's#/$##' | grep -v '^$' | sort)"
staging="$(cd "$D/root" && find . -mindepth 1 -printf '%P\n' | sort)"
tree="$(awk '!/^#/ && NF >= 5 {print $5}' "$ROOT/install/tree.txt" | sort)"
[ "$paths" = "$staging" ] && ok "V9.9: dpkg -c tem a mesma lista do staging" || {
    diff <(printf '%s\n' "$staging") <(printf '%s\n' "$paths") || true
    fail "V9.9: lista do pacote ≠ staging"
}
[ "$paths" = "$tree" ] && ok "V9.9: e a mesma de install/tree.txt ($(wc -l <<<"$tree") caminhos)" || fail "V9.9: lista do pacote ≠ tree.txt"
verify "todo caminho do pacote é root/root" "$(awk '$2 != "root/root"' <<<"$listing" | wc -l)" "0"
verify "nenhum bit setuid/setgid/sticky no pacote" "$(awk '$1 ~ /[sStT]/' <<<"$listing" | wc -l)" "0"
verify "nada com escrita para grupo e outros" "$(awk 'substr($1,6,1) == "w" || substr($1,9,1) == "w"' <<<"$listing" | wc -l)" "0"
verify "nenhum link no pacote" "$(awk 'substr($1,1,1) == "l"' <<<"$listing" | wc -l)" "0"
verify "ward-app vai 0755 no pacote (2755 só pelo statoverride)" "$(grep ' ./usr/local/bin/ward-app$' <<<"$listing" | cut -c1-10)" "-rwxr-xr-x"
verify "ward-nm vai 0755 no pacote" "$(grep ' ./usr/local/lib/ward/ward-nm$' <<<"$listing" | cut -c1-10)" "-rwxr-xr-x"
verify "staging tem ward-app 2755 (o modo da árvore)" "$(stat -c %a "$D/root/usr/local/bin/ward-app")" "2755"

dep="$(dpkg-deb -f "$DEB" Depends)"
verify "V9.9: Depends declara sshpass" ", $dep," ", sshpass,"
verify "Depends declara openssh-client e systemd" "$dep" "openssh-client, systemd"
verify "Package e Version" "$(dpkg-deb -f "$DEB" Package Version)" $'Package: ward\nVersion: 0.1.4'

X="$TMP/extracted"
dpkg-deb -x "$DEB" "$X"
cmp -s "$WARD" "$X/usr/local/bin/ward" && cmp -s "$WARDD" "$X/usr/local/bin/wardd" &&
    cmp -s "$APP" "$X/usr/local/bin/ward-app" && cmp -s "$NM" "$X/usr/local/lib/ward/ward-nm" &&
    ok "binários do pacote iguais aos artefatos" || fail "binário do pacote diferente do artefato"
diff -r -q "$D/root" "$X" >/dev/null && ok "conteúdo do pacote igual ao staging" || fail "conteúdo do pacote ≠ staging"
verify "manifesto de native messaging com o id" "$(cat "$X/etc/opt/chrome/native-messaging-hosts/br.ward.nm.json")" "chrome-extension://$ID/"

dpkg-deb -e "$DEB" "$TMP/control"
for s in preinst postinst prerm postrm; do
    [ -x "$TMP/control/$s" ] && bash -n "$TMP/control/$s" && ok "DEBIAN/$s executável e sem erro de sintaxe" || fail "DEBIAN/$s"
done
grep -q '^garantir_grupo()' "$TMP/control/preinst" && grep -q '^conferir_grupo()' "$TMP/control/preinst" &&
    ok "preinst tem as funções de install/groups.sh" || fail "preinst sem as funções de grupos"
grep -q '^conferir_politicas_chrome()' "$TMP/control/preinst" && grep -q '^        conferir_politicas_chrome /etc/opt/chrome/policies/managed$' "$TMP/control/preinst" &&
    ok "preinst confere as outras políticas do Chrome (Fase 51a)" || fail "preinst sem conferir_politicas_chrome"
grep -q '^apagar_grupos_marcados()' "$TMP/control/postrm" && ok "postrm tem a marca dos grupos (Fase 51a)" || fail "postrm sem apagar_grupos_marcados"
grep -q '^avisar_manifestos_de_usuario()' "$TMP/control/postinst" && ok "postinst avisa dos manifestos do perfil (Fase 51a)" || fail "postinst sem o aviso"
grep -q '^avisar_core_pattern()' "$TMP/control/postinst" && grep -q '^        avisar_core_pattern$' "$TMP/control/postinst" &&
    ok "postinst avisa de core_pattern fora do apport/systemd-coredump (Fase 51c)" || fail "postinst sem avisar_core_pattern"
verify "Fase 51c: pacote com a lista do apport (WebKitWebProcess)" "$(cat "$X/etc/apport/report-ignore/ward")" "/webkit2gtk-4.1/WebKitWebProcess"
cmp -s "$ROOT/install/coredump.conf.d/ward.conf" "$X/etc/systemd/coredump.conf.d/ward.conf" &&
    ok "Fase 51c: pacote com o drop-in do systemd-coredump" || fail "Fase 51c: drop-in do systemd-coredump fora do pacote"
for s in preinst postinst prerm postrm; do
    verify "$s sem marca de modelo" "$(grep -c '#@' "$TMP/control/$s" || true)" "0"
done
verify "política do pacote com NativeMessagingUserLevelHosts false (Fase 51a)" "$(grep -c '^  "NativeMessagingUserLevelHosts": false,$' "$X/etc/opt/chrome/policies/managed/ward.json")" "1"
verify "política do pacote com PasswordManagerEnabled false (Fase 44c)" "$(grep -c '^  "PasswordManagerEnabled": false,$' "$X/etc/opt/chrome/policies/managed/ward.json")" "1"
verify "política do pacote com PasswordLeakDetectionEnabled false (Fase 44c)" "$(grep -c '^  "PasswordLeakDetectionEnabled": false,$' "$X/etc/opt/chrome/policies/managed/ward.json")" "1"
verify "preinst confere as pastas do sistema" "$(grep -c '^    usr/local/bin$' "$TMP/control/preinst")" "1"
[ ! -e "$TMP/control/conffiles" ] && ok "sem conffiles (a política e o manifesto saem com o pacote)" || fail "conffiles inesperado"

echo "==> 1b. Recusas (nenhuma deixa .deb)"
no_deb() { [ -z "$(find "$1" -maxdepth 1 -name '*.deb' -print 2>/dev/null)" ] && echo "sem deb" || echo "com deb"; }
refusal() {
    local label="$1" msg="$2" d="$3"
    shift 3
    local code=0
    r="$(mk "$d" "$@" 2>&1)" || code=$?
    if [ "$code" -ne 0 ]; then ok "$label: recusado (saiu $code)"; else fail "$label: gerou o pacote"; fi
    verify "$label: mensagem" "$r" "$msg"
    verify "$label: nenhum .deb" "$(no_deb "$d")" "sem deb"
}
refusal "V9.2: wardd com test-mode" "modo-teste" "$TMP/r1" BIN_WARDD="$WARDD_TEST"
other_id="$(/usr/bin/python3 -I "$ROOT/dev/tests/fake_crx.py" "$TMP/other.crx")"
refusal "EXTENSION_ID de outra extensão" "não de EXTENSION_ID" "$TMP/r2" EXTENSION_ID="$other_id"
refusal "EXTENSION_ID vazio" "EXTENSION_ID" "$TMP/r3" EXTENSION_ID=
mkdir -p "$TMP/real-r4"
ln -s "$TMP/real-r4" "$TMP/r4"
refusal "DEB_DIR por link" "link simbólico" "$TMP/r4"
verify "DEB_DIR por link: nada no alvo" "$(ls -A "$TMP/real-r4")" ""
r="$(cd "$TMP" && /usr/bin/make -s -C "$ROOT" deb DEB_DIR=relativo EXTENSION_ID="$ID" EXTENSION_CRX="$CRX" \
    BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" 2>&1; echo "saiu=$?")"
verify "DEB_DIR relativo: recusado" "$r" "caminho absoluto"
[[ "$r" != *"saiu=0"* ]] && ok "DEB_DIR relativo: não sai 0" || fail "DEB_DIR relativo saiu 0"
[ ! -e "$ROOT/relativo" ] || fail "DEB_DIR relativo criou $ROOT/relativo"

echo "==> 1c. make extension-crx: chave errada, chave aberta e chave por link são recusadas"
if [ -x /opt/google/chrome/chrome ] && command -v openssl >/dev/null 2>&1; then
    E="$TMP/ext"
    mkdir -m 0700 "$E" "$E/dist"
    printf '{"manifest_version": 3, "name": "ward-teste", "version": "0.1.0"}\n' >"$E/dist/manifest.json"
    (umask 077 && openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$E/other.pem" 2>/dev/null)
    crx() {
        env -i PATH=/usr/bin:/bin EXTENSAO_DIST="$E/dist" CRX_SAIDA="$E/out/ward.crx" "$@" \
            "$ROOT/install/pack-extension.sh" 2>&1
    }
    r="$(crx EXTENSION_KEY="$E/other.pem"; echo "saiu=$?")"
    verify "extension-crx: outra chave (outro id) recusada" "$r" "não do EXTENSION_ID"
    nonzero() { [[ "$2" != *"saiu=0"* ]] && ok "$1" || fail "$1 ($2)"; }
    nonzero "extension-crx: outra chave não sai 0" "$r"
    [ ! -e "$E/out/ward.crx" ] && ok "extension-crx: nenhum pacote com a chave errada" || fail "pacote gerado com a chave errada"
    cp "$E/other.pem" "$E/open.pem" && chmod 0644 "$E/open.pem"
    r="$(crx EXTENSION_KEY="$E/open.pem"; echo "saiu=$?")"
    verify "extension-crx: chave 0644 recusada" "$r" "0600"
    ln -s "$E/other.pem" "$E/link.pem"
    r="$(crx EXTENSION_KEY="$E/link.pem"; echo "saiu=$?")"
    verify "extension-crx: chave por link recusada" "$r" "link"
    ln -s "$E/other.pem" "$E/dist/planted"
    r="$(crx EXTENSION_KEY="$E/other.pem"; echo "saiu=$?")"
    verify "extension-crx: link dentro do dist recusado" "$r" "link simbólico"
else
    echo "aviso: sem /opt/google/chrome/chrome ou openssl; seção 1c pulada"
fi

if [ "$HOST_ONLY" = 1 ]; then
    [ "$failures" -eq 0 ] || { echo "test-deb (host): $failures falha(s)"; exit 1; }
    echo "test-deb: parte do host ok (contêiner pulado: --host-only)"
    exit 0
fi

echo "==> 2. Contêiner com systemd (root só dentro dele)"
command -v docker >/dev/null 2>&1 || { echo "falta o programa: docker" >&2; exit 1; }
C="$TMP/container"
mkdir -m 0755 "$C" "$C/bin"
install -m 0644 "$DEB" "$C/ward.deb"
install -m 0644 "$ROOT/install/tree.txt" "$C/tree.txt"
install -m 0755 "$CLIENT" "$C/bin/cliente"
printf '%s\n' "$ID" >"$C/id"
chmod 0644 "$C/id"

IMAGE="ward-teste/deb-systemd:1"
docker build --quiet -t "$IMAGE" -f "$ROOT/dev/container-deb/Dockerfile" "$ROOT/dev/container-units" >/dev/null
docker run -d --name "$CONTAINER_NAME" --network none --pull never \
    --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cgroupns=private --cap-add SYS_ADMIN --cap-add SYS_PTRACE --security-opt apparmor=unconfined \
    --ulimit memlock=-1:-1 \
    -v "$C:/opt/teste:ro" \
    -v "$ROOT/dev/container-deb/inside.sh:/opt/inside.sh:ro" \
    "$IMAGE" >/dev/null
code=0
docker exec "$CONTAINER_NAME" /bin/bash -p /opt/inside.sh || code=$?
[ "$code" -eq 0 ] || fail "contêiner: inside.sh saiu $code"

if [ "$failures" -ne 0 ]; then
    echo "test-deb: $failures falha(s)"
    exit 1
fi
echo "test-deb: tudo ok"
