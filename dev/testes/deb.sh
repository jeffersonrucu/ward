#!/bin/bash -p
# `make teste-deb` (Fase 52; V9.9, V9.8 no contêiner; D18, D47, D60, D63).
#
# Nunca instala nada no host. Duas partes:
#
# 1. Host, sem root: `make deb` com os artefatos sem `modo-teste` (debug,
#    `target/sem-modo-teste`), o `partida-conteiner` no lugar do `ward-app` e um pacote
#    CRX3 falso (`crx_falso.py`). Confere: a lista de `dpkg-deb -c` é a do staging e a
#    de `instalacao/arvore.txt` (V9.9); todo caminho root/root, nenhum bit setuid/setgid
#    no pacote (um 2755 root:root daria o gid do root), nada com escrita para grupo e
#    outros; conteúdo igual aos artefatos; control com Depends de sshpass; preinst com as
#    funções de grupos de `grupos.sh`. Recusas: artefato `modo-teste` (V9.2), id da
#    extensão diferente do pacote, DEB_DIR relativo ou por link; nenhuma deixa `.deb`.
# 2. Contêiner `ubuntu:24.04` com systemd e sem rede (`dev/conteiner-deb/`), root só
#    dentro dele: `dpkg -i` sem sshpass (falha), grupo fora de D60 (recusa sem arquivo),
#    instalação, reinstalação, `dpkg -r`/`-P` (`dev/conteiner-deb/dentro.sh`). Mesmas
#    permissões do `make teste-units`: CAP_SYS_ADMIN e sem AppArmor só para o systemd
#    remontar a subárvore de cgroup dele. CAP_SYS_PTRACE (Fase 51c) só para o apport,
#    chamado à mão como root, ler o /proc/<pid>/exe do processo "crashado" do uid 1000.
#    O apport e o systemd-coredump rodam com um core falso; o kernel.core_pattern (do
#    host) nunca é escrito: o /proc/sys do contêiner é só leitura, conferido antes.
#
# Fica fora do `make check` (exige Docker). Uso: dev/testes/deb.sh [--so-host]

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/servicos/comum.sh
. "$RAIZ/dev/servicos/comum.sh"
unset MAKEFLAGS MAKELEVEL MFLAGS
cd "$RAIZ"

SO_HOST=0
[ "${1:-}" = --so-host ] && SO_HOST=1

SEM="$RAIZ/target/sem-modo-teste/debug"
WARD="$SEM/ward"
WARDD="$SEM/wardd"
NM="$SEM/ward-nm"
APP="$SEM/examples/partida-conteiner"
CLIENTE="$SEM/examples/wardd-cliente-teste"
WARDD_TESTE="$RAIZ/target/debug/wardd"
for b in "$WARD" "$WARDD" "$NM" "$APP" "$CLIENTE" "$WARDD_TESTE"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b (rode pelo 'make teste-deb')" >&2; exit 1; }
done
for p in dpkg-deb make; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$RAIZ/.dev"
TMP="$(mktemp -d "$RAIZ/.dev/teste-deb.XXXXXX")"
NOME="ward-deb-$$"
trap 'docker rm -f "$NOME" >/dev/null 2>&1 || true; rm -rf -- "$TMP"' EXIT

falhas=0
ok() { echo "ok: $*"; }
falhou() { echo "FALHOU: $*"; falhas=$((falhas + 1)); }
confere() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else falhou "$1 (esperado '$3', veio '$2')"; fi
}

CRX="$TMP/ward.crx"
ID="$(/usr/bin/python3 -I "$RAIZ/dev/testes/crx_falso.py" "$CRX")"
[[ "$ID" =~ ^[a-p]{32}$ ]] || { echo "crx_falso.py não deu um id" >&2; exit 1; }

# `make deb`; argumentos a mais sobrepõem os padrões.
mk() {
    local d="$1"
    shift
    /usr/bin/make -s -C "$RAIZ" deb DEB_DIR="$d" EXTENSAO_ID="$ID" EXTENSAO_CRX="$CRX" \
        BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" "$@"
}

echo "==> 1. make deb (host, sem root)"
D="$TMP/deb"
r="$(mk "$D" 2>&1)" || { printf '%s\n' "$r"; falhou "make deb"; }
DEB="$(find "$D" -maxdepth 1 -name 'ward_*.deb' -print)"
confere "make deb gera ward_<versão>_<arq>.deb" "$DEB" "/ward_0.1.1_$(dpkg --print-architecture).deb"
[ -f "$DEB" ] || { echo "teste-deb: sem pacote, nada mais a conferir" >&2; exit 1; }
confere "make deb imprime o sha256" "$r" "$(sha256sum "$DEB" | cut -d' ' -f1)"

lista="$(dpkg-deb -c "$DEB")"
caminhos="$(awk '{print $6}' <<<"$lista" | sed -e 's#^\./##' -e 's#/$##' | grep -v '^$' | sort)"
staging="$(cd "$D/raiz" && find . -mindepth 1 -printf '%P\n' | sort)"
arvore="$(awk '!/^#/ && NF >= 5 {print $5}' "$RAIZ/instalacao/arvore.txt" | sort)"
[ "$caminhos" = "$staging" ] && ok "V9.9: dpkg -c tem a mesma lista do staging" || {
    diff <(printf '%s\n' "$staging") <(printf '%s\n' "$caminhos") || true
    falhou "V9.9: lista do pacote ≠ staging"
}
[ "$caminhos" = "$arvore" ] && ok "V9.9: e a mesma de instalacao/arvore.txt ($(wc -l <<<"$arvore") caminhos)" || falhou "V9.9: lista do pacote ≠ arvore.txt"
confere "todo caminho do pacote é root/root" "$(awk '$2 != "root/root"' <<<"$lista" | wc -l)" "0"
confere "nenhum bit setuid/setgid/sticky no pacote" "$(awk '$1 ~ /[sStT]/' <<<"$lista" | wc -l)" "0"
confere "nada com escrita para grupo e outros" "$(awk 'substr($1,6,1) == "w" || substr($1,9,1) == "w"' <<<"$lista" | wc -l)" "0"
confere "nenhum link no pacote" "$(awk 'substr($1,1,1) == "l"' <<<"$lista" | wc -l)" "0"
confere "ward-app vai 0755 no pacote (2755 só pelo statoverride)" "$(grep ' ./usr/local/bin/ward-app$' <<<"$lista" | cut -c1-10)" "-rwxr-xr-x"
confere "ward-nm vai 0755 no pacote" "$(grep ' ./usr/local/lib/ward/ward-nm$' <<<"$lista" | cut -c1-10)" "-rwxr-xr-x"
confere "staging tem ward-app 2755 (o modo da árvore)" "$(stat -c %a "$D/raiz/usr/local/bin/ward-app")" "2755"

dep="$(dpkg-deb -f "$DEB" Depends)"
confere "V9.9: Depends declara sshpass" ", $dep," ", sshpass,"
confere "Depends declara openssh-client e systemd" "$dep" "openssh-client, systemd"
confere "Package e Version" "$(dpkg-deb -f "$DEB" Package Version)" $'Package: ward\nVersion: 0.1.1'

X="$TMP/extraido"
dpkg-deb -x "$DEB" "$X"
cmp -s "$WARD" "$X/usr/local/bin/ward" && cmp -s "$WARDD" "$X/usr/local/bin/wardd" &&
    cmp -s "$APP" "$X/usr/local/bin/ward-app" && cmp -s "$NM" "$X/usr/local/lib/ward/ward-nm" &&
    ok "binários do pacote iguais aos artefatos" || falhou "binário do pacote diferente do artefato"
diff -r -q "$D/raiz" "$X" >/dev/null && ok "conteúdo do pacote igual ao staging" || falhou "conteúdo do pacote ≠ staging"
confere "manifesto de native messaging com o id" "$(cat "$X/etc/opt/chrome/native-messaging-hosts/br.ward.nm.json")" "chrome-extension://$ID/"

dpkg-deb -e "$DEB" "$TMP/control"
for s in preinst postinst prerm postrm; do
    [ -x "$TMP/control/$s" ] && bash -n "$TMP/control/$s" && ok "DEBIAN/$s executável e sem erro de sintaxe" || falhou "DEBIAN/$s"
done
grep -q '^garantir_grupo()' "$TMP/control/preinst" && grep -q '^conferir_grupo()' "$TMP/control/preinst" &&
    ok "preinst tem as funções de instalacao/grupos.sh" || falhou "preinst sem as funções de grupos"
grep -q '^conferir_politicas_chrome()' "$TMP/control/preinst" && grep -q '^        conferir_politicas_chrome /etc/opt/chrome/policies/managed$' "$TMP/control/preinst" &&
    ok "preinst confere as outras políticas do Chrome (Fase 51a)" || falhou "preinst sem conferir_politicas_chrome"
grep -q '^apagar_grupos_marcados()' "$TMP/control/postrm" && ok "postrm tem a marca dos grupos (Fase 51a)" || falhou "postrm sem apagar_grupos_marcados"
grep -q '^avisar_manifestos_de_usuario()' "$TMP/control/postinst" && ok "postinst avisa dos manifestos do perfil (Fase 51a)" || falhou "postinst sem o aviso"
grep -q '^avisar_core_pattern()' "$TMP/control/postinst" && grep -q '^        avisar_core_pattern$' "$TMP/control/postinst" &&
    ok "postinst avisa de core_pattern fora do apport/systemd-coredump (Fase 51c)" || falhou "postinst sem avisar_core_pattern"
confere "Fase 51c: pacote com a lista do apport (WebKitWebProcess)" "$(cat "$X/etc/apport/report-ignore/ward")" "/webkit2gtk-4.1/WebKitWebProcess"
cmp -s "$RAIZ/instalacao/coredump.conf.d/ward.conf" "$X/etc/systemd/coredump.conf.d/ward.conf" &&
    ok "Fase 51c: pacote com o drop-in do systemd-coredump" || falhou "Fase 51c: drop-in do systemd-coredump fora do pacote"
for s in preinst postinst prerm postrm; do
    confere "$s sem marca de modelo" "$(grep -c '#@' "$TMP/control/$s" || true)" "0"
done
confere "política do pacote com NativeMessagingUserLevelHosts false (Fase 51a)" "$(grep -c '^  "NativeMessagingUserLevelHosts": false,$' "$X/etc/opt/chrome/policies/managed/ward.json")" "1"
confere "política do pacote com PasswordManagerEnabled false (Fase 44c)" "$(grep -c '^  "PasswordManagerEnabled": false,$' "$X/etc/opt/chrome/policies/managed/ward.json")" "1"
confere "política do pacote com PasswordLeakDetectionEnabled false (Fase 44c)" "$(grep -c '^  "PasswordLeakDetectionEnabled": false,$' "$X/etc/opt/chrome/policies/managed/ward.json")" "1"
confere "preinst confere as pastas do sistema" "$(grep -c '^    usr/local/bin$' "$TMP/control/preinst")" "1"
[ ! -e "$TMP/control/conffiles" ] && ok "sem conffiles (a política e o manifesto saem com o pacote)" || falhou "conffiles inesperado"

echo "==> 1b. Recusas (nenhuma deixa .deb)"
sem_deb() { [ -z "$(find "$1" -maxdepth 1 -name '*.deb' -print 2>/dev/null)" ] && echo "sem deb" || echo "com deb"; }
recusa() {
    local rotulo="$1" msg="$2" d="$3"
    shift 3
    local codigo=0
    r="$(mk "$d" "$@" 2>&1)" || codigo=$?
    if [ "$codigo" -ne 0 ]; then ok "$rotulo: recusado (saiu $codigo)"; else falhou "$rotulo: gerou o pacote"; fi
    confere "$rotulo: mensagem" "$r" "$msg"
    confere "$rotulo: nenhum .deb" "$(sem_deb "$d")" "sem deb"
}
recusa "V9.2: wardd com modo-teste" "modo-teste" "$TMP/r1" BIN_WARDD="$WARDD_TESTE"
outro="$(/usr/bin/python3 -I "$RAIZ/dev/testes/crx_falso.py" "$TMP/outro.crx")"
recusa "EXTENSAO_ID de outra extensão" "não de EXTENSAO_ID" "$TMP/r2" EXTENSAO_ID="$outro"
recusa "EXTENSAO_ID vazio" "EXTENSAO_ID" "$TMP/r3" EXTENSAO_ID=
mkdir -p "$TMP/real-r4"
ln -s "$TMP/real-r4" "$TMP/r4"
recusa "DEB_DIR por link" "link simbólico" "$TMP/r4"
confere "DEB_DIR por link: nada no alvo" "$(ls -A "$TMP/real-r4")" ""
r="$(cd "$TMP" && /usr/bin/make -s -C "$RAIZ" deb DEB_DIR=relativo EXTENSAO_ID="$ID" EXTENSAO_CRX="$CRX" \
    BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" 2>&1; echo "saiu=$?")"
confere "DEB_DIR relativo: recusado" "$r" "caminho absoluto"
[[ "$r" != *"saiu=0"* ]] && ok "DEB_DIR relativo: não sai 0" || falhou "DEB_DIR relativo saiu 0"
[ ! -e "$RAIZ/relativo" ] || falhou "DEB_DIR relativo criou $RAIZ/relativo"

echo "==> 1c. make extensao-crx: chave errada, chave aberta e chave por link são recusadas"
if [ -x /opt/google/chrome/chrome ] && command -v openssl >/dev/null 2>&1; then
    E="$TMP/ext"
    mkdir -m 0700 "$E" "$E/dist"
    printf '{"manifest_version": 3, "name": "ward-teste", "version": "0.1.0"}\n' >"$E/dist/manifest.json"
    (umask 077 && openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$E/outra.pem" 2>/dev/null)
    crx() {
        env -i PATH=/usr/bin:/bin EXTENSAO_DIST="$E/dist" CRX_SAIDA="$E/saida/ward.crx" "$@" \
            "$RAIZ/instalacao/empacotar-extensao.sh" 2>&1
    }
    r="$(crx CHAVE_EXTENSAO="$E/outra.pem"; echo "saiu=$?")"
    confere "extensao-crx: outra chave (outro id) recusada" "$r" "não do ID_EXTENSAO"
    nao0() { [[ "$2" != *"saiu=0"* ]] && ok "$1" || falhou "$1 ($2)"; }
    nao0 "extensao-crx: outra chave não sai 0" "$r"
    [ ! -e "$E/saida/ward.crx" ] && ok "extensao-crx: nenhum pacote com a chave errada" || falhou "pacote gerado com a chave errada"
    cp "$E/outra.pem" "$E/aberta.pem" && chmod 0644 "$E/aberta.pem"
    r="$(crx CHAVE_EXTENSAO="$E/aberta.pem"; echo "saiu=$?")"
    confere "extensao-crx: chave 0644 recusada" "$r" "0600"
    ln -s "$E/outra.pem" "$E/link.pem"
    r="$(crx CHAVE_EXTENSAO="$E/link.pem"; echo "saiu=$?")"
    confere "extensao-crx: chave por link recusada" "$r" "link"
    ln -s "$E/outra.pem" "$E/dist/plantado"
    r="$(crx CHAVE_EXTENSAO="$E/outra.pem"; echo "saiu=$?")"
    confere "extensao-crx: link dentro do dist recusado" "$r" "link simbólico"
else
    echo "aviso: sem /opt/google/chrome/chrome ou openssl; seção 1c pulada"
fi

if [ "$SO_HOST" = 1 ]; then
    [ "$falhas" -eq 0 ] || { echo "teste-deb (host): $falhas falha(s)"; exit 1; }
    echo "teste-deb: parte do host ok (contêiner pulado: --so-host)"
    exit 0
fi

echo "==> 2. Contêiner com systemd (root só dentro dele)"
command -v docker >/dev/null 2>&1 || { echo "falta o programa: docker" >&2; exit 1; }
C="$TMP/conteiner"
mkdir -m 0755 "$C" "$C/bin"
install -m 0644 "$DEB" "$C/ward.deb"
install -m 0644 "$RAIZ/instalacao/arvore.txt" "$C/arvore.txt"
install -m 0755 "$CLIENTE" "$C/bin/cliente"
printf '%s\n' "$ID" >"$C/id"
chmod 0644 "$C/id"

IMAGEM="ward-teste/deb-systemd:1"
docker build --quiet -t "$IMAGEM" -f "$RAIZ/dev/conteiner-deb/Dockerfile" "$RAIZ/dev/conteiner-units" >/dev/null
docker run -d --name "$NOME" --network none --pull never \
    --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cgroupns=private --cap-add SYS_ADMIN --cap-add SYS_PTRACE --security-opt apparmor=unconfined \
    --ulimit memlock=-1:-1 \
    -v "$C:/opt/teste:ro" \
    -v "$RAIZ/dev/conteiner-deb/dentro.sh:/opt/dentro.sh:ro" \
    "$IMAGEM" >/dev/null
codigo=0
docker exec "$NOME" /bin/bash -p /opt/dentro.sh || codigo=$?
[ "$codigo" -eq 0 ] || falhou "contêiner: dentro.sh saiu $codigo"

if [ "$falhas" -ne 0 ]; then
    echo "teste-deb: $falhas falha(s)"
    exit 1
fi
echo "teste-deb: tudo ok"
