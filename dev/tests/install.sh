#!/bin/bash -p
# `make test-install` (Fase 51; V9.1, V9.2, V9.3, V9.4, V9.6, V9.7, V9.10, V9.11; D18,
# D46, D47, D57, D60, D62, D63).
#
# Nunca instala nada no host. Duas partes:
#
# 1. Host, sem root: `make install DESTDIR=<pasta em .dev/>` com os artefatos sem
#    `test-mode` (debug, `target/no-test-mode`), o `startup-container` no lugar do
#    `ward-app` (a partida do app real sem GTK) e um pacote CRX3 falso
#    (`fake_crx.py`: chave e assinatura aleatórias). Confere a árvore exata de
#    `install/tree.txt` (V9.1), modos, conteúdo, os JSON do Chrome (V9.6), o
#    `update.xml`, o `systemd-analyze verify` das units instaladas, o `make uninstall`
#    (V9.10: some tudo do Ward, fica o que não é dele, inclusive um "cofre" plantado) e
#    as recusas: artefato `test-mode` (V9.2), não-ELF, link, id da extensão inválido ou
#    diferente do pacote, pacote adulterado, PREFIX, DESTDIR relativo ou com link, pasta
#    com link no caminho, e `make install` sem root no sistema. Toda recusa sai ≠ 0 sem
#    deixar arquivo do Ward.
# 2. Contêiner `ubuntu:24.04` com systemd e sem rede (`dev/container-install/`), onde o
#    root existe só dentro dele: `make install`/`make uninstall` de verdade
#    (`dev/container-install/inside.sh`). Mesmas permissões do `make test-units` (ver
#    o comentário de `dev/tests/units.sh`): CAP_SYS_ADMIN e sem AppArmor só para o
#    systemd remontar a subárvore de cgroup dele.
#
# Fica fora do `make check` (exige Docker). Uso: dev/tests/install.sh [--so-destdir]

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$RAIZ/dev/services/common.sh"
unset MAKEFLAGS MAKELEVEL MFLAGS
cd "$RAIZ"

SO_DESTDIR=0
[ "${1:-}" = --so-destdir ] && SO_DESTDIR=1

SEM="$RAIZ/target/no-test-mode/debug"
WARD="$SEM/ward"
WARDD="$SEM/wardd"
NM="$SEM/ward-nm"
APP="$SEM/examples/startup-container"
CLIENTE="$SEM/examples/wardd-test-client"
WARDD_TESTE="$RAIZ/target/debug/wardd"
PLATAFORMA="$RAIZ/target/debug/examples/ward-platform-container"
for b in "$WARD" "$WARDD" "$NM" "$APP" "$CLIENTE" "$WARDD_TESTE" "$PLATAFORMA"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b (rode pelo 'make test-install')" >&2; exit 1; }
done
for p in systemd-analyze make; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$RAIZ/.dev"
TMP="$(mktemp -d "$RAIZ/.dev/test-install.XXXXXX")"
NOME="ward-install-$$"
trap 'docker rm -f "$NOME" >/dev/null 2>&1 || true; rm -rf -- "$TMP"' EXIT

falhas=0
ok() { echo "ok: $*"; }
falhou() { echo "FALHOU: $*"; falhas=$((falhas + 1)); }
confere() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else falhou "$1 (esperado '$3', veio '$2')"; fi
}
nao_tem() {
    if [[ "$2" != *"$3"* ]]; then ok "$1"; else falhou "$1 (não devia ter '$3': $2)"; fi
}

# Pacote falso e o id dele.
CRX="$TMP/ward.crx"
ID="$(/usr/bin/python3 -I "$RAIZ/dev/tests/fake_crx.py" "$CRX")"
[[ "$ID" =~ ^[a-p]{32}$ ]] || { echo "fake_crx.py não deu um id" >&2; exit 1; }

# `make install`/`uninstall` com DESTDIR; argumentos a mais sobrepõem os padrões.
mk() {
    local acao="$1" d="$2"
    shift 2
    /usr/bin/make -s -C "$RAIZ" "$acao" DESTDIR="$d" EXTENSION_ID="$ID" EXTENSION_CRX="$CRX" \
        BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" WARD_UID= "$@"
}

# Linhas "tipo modo caminho" da árvore.
ARV="$TMP/arvore"
awk '!/^#/ && NF >= 5 {print $1, $2, $5}' "$RAIZ/install/tree.txt" >"$ARV"
ARQUIVOS=()
while read -r t _ c; do [ "$t" = f ] && ARQUIVOS+=("$c"); done <"$ARV"

# Nenhum arquivo do Ward em $1 (a recusa não deixa nada).
nada_do_ward() {
    local d="$1" c
    for c in "${ARQUIVOS[@]}"; do
        if [ -e "$d/$c" ] || [ -L "$d/$c" ]; then echo "$c"; return; fi
    done
    echo "nada"
}

# Recusa: sai ≠ 0, com a mensagem, sem arquivo do Ward.
recusa() {
    local rotulo="$1" msg="$2" d="$3"
    shift 3
    local r codigo=0
    r="$(mk install "$d" "$@" 2>&1)" || codigo=$?
    if [ "$codigo" -ne 0 ]; then ok "$rotulo: recusado (saiu $codigo)"; else falhou "$rotulo: instalou (saiu 0)"; fi
    confere "$rotulo: mensagem" "$r" "$msg"
    confere "$rotulo: nenhum arquivo do Ward" "$(nada_do_ward "$d")" "nada"
}

echo "==> 1. DESTDIR (host, sem root)"
D="$TMP/raiz"
r="$(mk install "$D" 2>&1)" || { printf '%s\n' "$r"; falhou "make install DESTDIR"; }
ok "make install DESTDIR saiu 0"

# V9.1: a árvore é exatamente a de tree.txt.
esperado="$(awk '{print $3}' "$ARV" | sort)"
real="$(cd "$D" && find . -mindepth 1 -printf '%P\n' | sort)"
if [ "$esperado" = "$real" ]; then ok "V9.1: árvore exata (${#ARQUIVOS[@]} arquivos)"; else
    falhou "V9.1: árvore diferente"
    diff <(printf '%s\n' "$esperado") <(printf '%s\n' "$real") || true
fi
# Modos (o dono root só no contêiner) e nada com escrita para grupo e outros.
while read -r t m c; do
    real_m="$(stat -c %a -- "$D/$c")"
    [ $((8#$real_m)) = $((8#$m)) ] || falhou "modo de $c: $real_m (esperado $m)"
    [ $((8#$real_m & 8#022)) = 0 ] || falhou "$c com escrita para grupo ou outros"
    if [ "$t" = f ]; then [ -f "$D/$c" ] && [ ! -L "$D/$c" ] || falhou "$c não é arquivo regular"; fi
    if [ "$t" != f ]; then [ -d "$D/$c" ] && [ ! -L "$D/$c" ] || falhou "$c não é pasta"; fi
done <"$ARV"
ok "V9.1: modos de tree.txt (ward-app e ward-nm 2755), sem escrita para grupo e outros"
# Conteúdo.
cmp -s "$WARD" "$D/usr/local/bin/ward" && cmp -s "$WARDD" "$D/usr/local/bin/wardd" &&
    cmp -s "$APP" "$D/usr/local/bin/ward-app" && cmp -s "$NM" "$D/usr/local/lib/ward/ward-nm" &&
    ok "binários iguais aos artefatos" || falhou "binário instalado diferente do artefato"
for u in wardd@.socket wardd@.service; do
    cmp -s "$RAIZ/install/systemd/$u" "$D/usr/local/lib/systemd/system/$u" || falhou "$u diferente"
done
cmp -s "$RAIZ/install/tmpfiles.d/ward.conf" "$D/usr/local/lib/tmpfiles.d/ward.conf" || falhou "tmpfiles diferente"
cmp -s "$CRX" "$D/usr/local/share/ward/extensao/ward.crx" || falhou "pacote da extensão diferente"
ok "units, tmpfiles.d e pacote iguais aos do repositório"

# Fase 51c (S2, S8): lista do apport e systemd-coredump sem armazenamento.
cmp -s "$RAIZ/install/coredump.conf.d/ward.conf" "$D/etc/systemd/coredump.conf.d/ward.conf" &&
    ok "51c: drop-in do systemd-coredump igual ao do repositório" || falhou "51c: drop-in do systemd-coredump diferente"
confere "51c: drop-in com Storage=none e ProcessSizeMax=0" \
    "$(grep -v '^#' "$D/etc/systemd/coredump.conf.d/ward.conf" | tr '\n' ' ')" "[Coredump] Storage=none ProcessSizeMax=0"
cmp -s "$D/etc/apport/report-ignore/ward" "$D/etc/apport/blacklist.d/ward" &&
    ok "51c: report-ignore/ward e blacklist.d/ward iguais" || falhou "51c: as duas listas do apport diferem"
LISTA_APPORT="$(grep -v '^#' "$D/etc/apport/report-ignore/ward")"
for c in /usr/local/bin/ward /usr/local/bin/wardd /usr/local/bin/ward-app /usr/local/lib/ward/ward-nm; do
    grep -qxF -- "$c" <<<"$LISTA_APPORT" || falhou "51c: $c fora da lista do apport"
done
# Oráculo independente do dump.sh: os auxiliares do pacote instalado no host.
ARQ_DPKG="$(dpkg --print-architecture)"
AUX_PACOTE="$(dpkg-query -L "libwebkit2gtk-4.1-0:$ARQ_DPKG" 2>/dev/null | grep -E '/webkit2gtk-4\.1/WebKit[A-Za-z]*Process$' || true)"
if [ -n "$AUX_PACOTE" ]; then
    while IFS= read -r c; do
        grep -qxF -- "$c" <<<"$LISTA_APPORT" || falhou "51c: auxiliar $c do libwebkit2gtk-4.1-0 fora da lista do apport"
    done <<<"$AUX_PACOTE"
    ok "51c: os $(wc -l <<<"$AUX_PACOTE") auxiliares do libwebkit2gtk-4.1-0 do host e os 4 binários do Ward na lista"
else
    echo "aviso: libwebkit2gtk-4.1-0 fora do host; os auxiliares ficam para o test-deb"
fi
nao_tem "51c: nenhuma linha vazia ou com espaço na lista (o apport compara a linha inteira)" \
    "$(grep -n -E '^$|^[^#].*[[:space:]]' "$D/etc/apport/report-ignore/ward" || true)" ":"
# O próprio código do apport (Report.check_ignored) lendo a lista instalada, nas duas
# pastas; controles fora da lista não são calados.
if /usr/bin/python3 -I -c 'import apport.report' 2>/dev/null; then
    mapfile -t IGN <<<"$LISTA_APPORT"
    for pasta in report-ignore blacklist.d; do
        r="$(env -i PATH=/usr/bin:/bin HOME=/nonexistent /usr/bin/python3 -I "$RAIZ/dev/tests/apport_list.py" \
            "$D/etc/apport/$pasta" "${IGN[@]}" -- /usr/bin/sleep "${IGN[-1]}X" /usr/local/bin/ward-appX 2>&1; echo "saiu=$?")"
        confere "51c: o apport ($pasta) cala os ${#IGN[@]} caminhos e não os controles" "$r" "saiu=0"
        nao_tem "51c: apport ($pasta) sem FALHOU" "$r" "FALHOU"
    done
else
    echo "aviso: sem o módulo do apport no host; a leitura pelo apport fica para o test-deb"
fi

# dump.sh por dentro, com dpkg/dpkg-query falsos (funções vencem comandos).
despejo() {
    env -i PATH=/usr/bin:/bin /bin/bash -p -c '
        erro() { printf "ERRO: %s\n" "$*" >&2; exit 1; }
        aviso() { printf "AVISO: %s\n" "$*" >&2; }
        . "$1"; shift; eval "$1"' _ "$RAIZ/install/dump.sh" "$1" 2>&1
    echo "saiu=$?"
}
r="$(despejo 'dpkg() { echo amd64; }; dpkg-query() { printf "%s\n" /usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/WebKitWebProcess /usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/WebKitPrintProcess; }; lista_apport')"
confere "51c ataque: auxiliar novo no pacote (WebKitPrintProcess) recusa a lista" "$r" "WebKitPrintProcess, fora da lista"
nao_tem "51c ataque: auxiliar novo não sai 0" "$r" "saiu=0"
r="$(despejo 'dpkg() { echo sparc64; }; lista_apport')"
confere "51c: arquitetura fora da tabela recusa" "$r" "fora da tabela"
nao_tem "51c: arquitetura fora da tabela não sai 0" "$r" "saiu=0"
r="$(despejo 'dpkg() { echo arm64; }; dpkg-query() { return 1; }; lista_apport')"
confere "51c: sem o libwebkit2gtk, avisa e usa o caminho do multiarch" "$r" "AVISO: libwebkit2gtk-4.1-0 (arm64) não está instalado"
confere "51c: arm64 -> aarch64-linux-gnu" "$r" "/usr/lib/aarch64-linux-gnu/webkit2gtk-4.1/WebKitWebProcess"
printf '%s' '|/usr/share/apport/apport -p%p -s%s -c%c -d%d -P%P -u%u -g%g -F%F -- %E' >"$TMP/cp-apport"
printf '%s' '|/usr/lib/systemd/systemd-coredump %P %u %g %s %t 9223372036854775808 %h %d' >"$TMP/cp-systemd"
printf '%s' 'core' >"$TMP/cp-arquivo"
printf '%s' '|/usr/libexec/abrt-hook-ccpp %s %c %p %u %g %t %e' >"$TMP/cp-outro"
printf '%s' '@/run/outro/coredump.socket' >"$TMP/cp-socket"
for c in apport systemd arquivo; do
    nao_tem "51c: core_pattern $c sem aviso" "$(despejo "avisar_core_pattern $TMP/cp-$c")" "AVISO"
done
confere "51c: core_pattern de outro programa avisa" "$(despejo "avisar_core_pattern $TMP/cp-outro")" "AVISO: kernel.core_pattern entrega o core a '|/usr/libexec/abrt-hook-ccpp'"
confere "51c: core_pattern por outro socket avisa" "$(despejo "avisar_core_pattern $TMP/cp-socket")" "AVISO: kernel.core_pattern"
# V9.6, a política e o update.xml.
r="$(/usr/bin/python3 -I - "$D" "$ID" <<'EOF'
import json, sys, xml.etree.ElementTree as ET
d, ident = sys.argv[1], sys.argv[2]
nm = json.load(open(d + "/etc/opt/chrome/native-messaging-hosts/br.ward.nm.json"))
assert nm == {"name": "br.ward.nm", "description": nm["description"], "path": "/usr/local/lib/ward/ward-nm",
              "type": "stdio", "allowed_origins": [f"chrome-extension://{ident}/"]}, nm
print("nm ok")
pol = json.load(open(d + "/etc/opt/chrome/policies/managed/ward.json"))
# Fase 51a: NativeMessagingUserLevelHosts false (só o manifesto do root vale).
# Fase 44c: o gerenciador de senhas do Chrome e a checagem de vazamento desligados.
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
raiz = ET.parse(d + "/usr/local/share/ward/extensao/update.xml").getroot()
apps = raiz.findall(ns + "app")
assert len(apps) == 1 and apps[0].get("appid") == ident
uc = apps[0].findall(ns + "updatecheck")
assert len(uc) == 1 and uc[0].get("codebase") == "file:///usr/local/share/ward/extensao/ward.crx"
assert uc[0].get("version") == "0.1.0"
print("update ok")
EOF
)" || true
confere "V9.6: manifesto de native messaging (allowed_origins só o id, path do ward-nm)" "$r" "nm ok"
confere "política do Chrome: NativeMessagingUserLevelHosts, PasswordManagerEnabled e PasswordLeakDetectionEnabled false, só a extensão do Ward, force_installed, update local" "$r" "politica ok"
confere "update.xml: id, pacote local e versão do pacote" "$r" "update ok"
# systemd-analyze verify das units instaladas (cópia; o ExecStart não existe no host).
mkdir "$TMP/verify"
cp "$D/usr/local/lib/systemd/system/wardd@.socket" "$D/usr/local/lib/systemd/system/wardd@.service" "$TMP/verify/"
saida="$(systemd-analyze verify --man=no "$TMP/verify/wardd@1000.socket" "$TMP/verify/wardd@1000.service" 2>&1 || true)"
resto="$(printf '%s\n' "$saida" | grep -v -e '/usr/local/bin/wardd' -e '^$' || true)"
[ -z "$resto" ] && ok "systemd-analyze verify das units instaladas" || { printf '%s\n' "$saida"; falhou "systemd-analyze verify"; }

# Reinstalar por cima (link plantado no lugar de um arquivo: trocado, nunca seguido).
printf 'intacto\n' >"$TMP/alvo-do-link"
rm -f "$D/usr/local/bin/wardd"
ln -s "$TMP/alvo-do-link" "$D/usr/local/bin/wardd"
r="$(mk install "$D" 2>&1)" || { printf '%s\n' "$r"; falhou "reinstalar"; }
[ -f "$D/usr/local/bin/wardd" ] && [ ! -L "$D/usr/local/bin/wardd" ] && cmp -s "$WARDD" "$D/usr/local/bin/wardd" &&
    ok "reinstalar troca o link plantado pelo binário" || falhou "o link plantado ficou"
confere "o alvo do link plantado não foi escrito" "$(cat "$TMP/alvo-do-link")" "intacto"

# V9.10: uninstall apaga o que é do Ward e deixa o resto (um "cofre" plantado no home).
# Home falso dentro do DESTDIR; nome montado por partes para a varredura do V1.5
# (`dev/tests/attacks-foundation.sh`) não confundir com o home real.
COFRE_D="$D/home/u/.local/share/w""ard"
mkdir -p "$COFRE_D"
head -c 4096 /dev/urandom >"$COFRE_D/cofre.ward"
printf 'de outro pacote\n' >"$D/usr/local/bin/outro"
antes="$(sha256sum "$COFRE_D/cofre.ward" "$D/usr/local/bin/outro")"
r="$(mk uninstall "$D" 2>&1)" || { printf '%s\n' "$r"; falhou "make uninstall DESTDIR"; }
confere "V9.10: uninstall apaga todo arquivo do Ward" "$(nada_do_ward "$D")" "nada"
for c in usr/local/lib/ward usr/local/share/ward; do
    [ ! -e "$D/$c" ] || falhou "V9.10: a pasta $c ficou"
done
[ -d "$D/usr/local/bin" ] && [ -d "$D/etc/opt/chrome/policies/managed" ] && ok "V9.10: pastas comuns ficam" || falhou "pasta comum apagada"
confere "V9.10: o cofre e o arquivo de outro pacote intactos" "$(sha256sum "$COFRE_D/cofre.ward" "$D/usr/local/bin/outro")" "$antes"
r="$(mk uninstall "$D" 2>&1)" && ok "uninstall de novo (nada instalado) sai 0" || { printf '%s\n' "$r"; falhou "uninstall repetido"; }

echo "==> 1b. Recusas (cada uma numa pasta nova)"
# `nova` define $d com uma pasta nova (sem subshell, para o contador andar).
n=0
nova() { n=$((n + 1)); d="$TMP/r$n"; }
# Artefatos com marca de test-mode: o wardd de teste de verdade e cópias com a marca.
cp "$WARD" "$TMP/ward-marcado" && printf 'WARD_DEV_SOCKET_DIR' >>"$TMP/ward-marcado"
cp "$NM" "$TMP/nm-marcado" && printf 'x WARD_DEV_DIR x' >>"$TMP/nm-marcado"
cp "$APP" "$TMP/app-marcado" && printf 'ward-ponte-teste' >>"$TMP/app-marcado"
nova; recusa "V9.2: wardd com test-mode" "modo-teste" "$d" BIN_WARDD="$WARDD_TESTE"
nova; recusa "V9.2: ward com WARD_DEV_SOCKET_DIR" "modo-teste" "$d" BIN_WARD="$TMP/ward-marcado"
nova; recusa "V9.2: ward-nm com WARD_DEV_DIR" "modo-teste" "$d" BIN_NM="$TMP/nm-marcado"
nova; recusa "V9.2: ward-app com a ponte de teste" "modo-teste" "$d" BIN_APP="$TMP/app-marcado"
# Fase 51a: a exceção do WARD_DEV_DBUS_SISTEMA saiu (o release não tem o texto).
cp "$WARDD" "$TMP/wardd-dbus" && printf "x WARD_DEV_DBUS_SISTEMA x" >>"$TMP/wardd-dbus"
nova; recusa "V9.2: wardd com WARD_DEV_DBUS_SISTEMA (sem exceção)" "WARD_DEV_DBUS_SISTEMA" "$d" BIN_WARDD="$TMP/wardd-dbus"
confere "o wardd de release não tem nenhum WARD_DEV_" "$(grep -a -c -E "WARD_DEV_[A-Z0-9_]*" "$WARDD" || true)" "0"
# Fase 51a: outra política em policies/managed com um nome da do Ward recusa antes de
# qualquer arquivo (o Chrome não junta os dois; a do atacante/administrador venceria).
politica_outra() {
    nova
    mkdir -p "$d/etc/opt/chrome/policies/managed"
    printf "%s" "$1" >"$d/etc/opt/chrome/policies/managed/$2"
}
politica_outra "{\"NativeMessagingUserLevelHosts\": true}" outra.json
recusa "política: outro arquivo com NativeMessagingUserLevelHosts" "NativeMessagingUserLevelHosts" "$d"
politica_outra "{\"ExtensionSettings\": {\"*\": {\"installation_mode\": \"blocked\"}}}" 00-empresa.json
recusa "política: outro arquivo com ExtensionSettings" "ExtensionSettings" "$d"
politica_outra "{\"\\u004eativeMessagingUserLevelHosts\": true}" zz.json
recusa "política: nome escondido com escape \\u" "\\u" "$d"
politica_outra "{\"NativeMessagingUserLevelHosts\": true}" .escondido
recusa "política: arquivo oculto também conta" "NativeMessagingUserLevelHosts" "$d"
# Fase 44c: outro arquivo que religue o gerenciador de senhas ou a checagem de vazamento.
politica_outra "{\"PasswordManagerEnabled\": true}" senhas.json
recusa "política: outro arquivo com PasswordManagerEnabled" "PasswordManagerEnabled" "$d"
politica_outra "{\"PasswordLeakDetectionEnabled\": true}" vazamento.json
recusa "política: outro arquivo com PasswordLeakDetectionEnabled" "PasswordLeakDetectionEnabled" "$d"
# Arquivo de política sem esses nomes: instala e fica intacto.
politica_outra "{\"HomepageLocation\": \"https://exemplo.invalid/\"}" outra.json
r="$(mk install "$d" 2>&1)" && ok "política: outro arquivo sem os nomes do Ward não impede" || { printf "%s\n" "$r"; falhou "política sem conflito recusada"; }
confere "política: o outro arquivo ficou intacto" "$(cat "$d/etc/opt/chrome/policies/managed/outra.json")" "HomepageLocation"
printf '#!/bin/sh\necho falso\n' >"$TMP/script"
chmod 0755 "$TMP/script"
nova; recusa "wardd que não é ELF" "não é um executável ELF" "$d" BIN_WARDD="$TMP/script"
ln -s "$WARD" "$TMP/ward-link"
nova; recusa "ward por link simbólico" "link simbólico" "$d" BIN_WARD="$TMP/ward-link"
nova; recusa "artefato ausente" "não existe" "$d" BIN_NM="$TMP/nao-existe"
nova; recusa "EXTENSION_ID vazio" "EXTENSION_ID" "$d" EXTENSION_ID=
nova; recusa "EXTENSION_ID em maiúsculas" "EXTENSION_ID" "$d" EXTENSION_ID="${ID^^}"
nova; recusa "EXTENSION_ID com 31 letras" "EXTENSION_ID" "$d" EXTENSION_ID="${ID:1}"
nova; recusa "EXTENSION_ID com aspas (injeção no JSON)" "EXTENSION_ID" "$d" EXTENSION_ID="${ID:0:28}\",\"x"
outro="$(/usr/bin/python3 -I "$RAIZ/dev/tests/fake_crx.py" "$TMP/outro.crx")"
nova; recusa "EXTENSION_ID de outra extensão" "não de EXTENSION_ID" "$d" EXTENSION_ID="$outro"
/usr/bin/python3 -I "$RAIZ/dev/tests/fake_crx.py" "$TMP/adulterado.crx" --adulterado >"$TMP/id-adulterado"
nova; recusa "pacote com id que não é o da chave" "EXTENSION_CRX recusado" "$d" \
    EXTENSION_ID="$(cat "$TMP/id-adulterado")" EXTENSION_CRX="$TMP/adulterado.crx"
/usr/bin/python3 -I "$RAIZ/dev/tests/fake_crx.py" "$TMP/versao.crx" --versao "1.0'><x" >/dev/null
nova; recusa "pacote com versão fora do formato" "EXTENSION_CRX recusado" "$d" EXTENSION_CRX="$TMP/versao.crx"
nova; recusa "pacote ausente" "EXTENSION_CRX" "$d" EXTENSION_CRX=
nova; recusa "PREFIX=/opt/ward" "PREFIX" "$d" PREFIX=/opt/ward
nova; recusa "PREFIX=/usr/local/../tmp" "PREFIX" "$d" PREFIX=/usr/local/../tmp
nova
mkdir -p "$TMP/real-$n"
ln -s "$TMP/real-$n" "$d"
recusa "DESTDIR por link" "link simbólico" "$d"
confere "DESTDIR por link: nada no alvo do link" "$(nada_do_ward "$TMP/real-$n")" "nada"
recusa "DESTDIR com .." "'..'" "$TMP/r1/../r-ponto-ponto"
nova
mkdir -p "$d/usr/local" "$TMP/alvo-bin"
ln -s "$TMP/alvo-bin" "$d/usr/local/bin"
recusa "usr/local/bin por link dentro do DESTDIR" "link simbólico" "$d"
confere "pasta-link: nada escrito no alvo" "$(ls -A "$TMP/alvo-bin")" ""
# Fase 51c: a pasta da lista do apport e a do systemd-coredump por link.
for p in etc/apport/report-ignore etc/systemd/coredump.conf.d; do
    nova
    mkdir -p "$d/${p%/*}" "$TMP/alvo-$n"
    ln -s "$TMP/alvo-$n" "$d/$p"
    recusa "51c: $p por link" "link simbólico" "$d"
    confere "51c: $p por link: nada escrito no alvo" "$(ls -A "$TMP/alvo-$n")" ""
done
# DESTDIR relativo (o make passa como veio).
r="$(cd "$TMP" && /usr/bin/make -s -C "$RAIZ" install DESTDIR=relativo EXTENSION_ID="$ID" EXTENSION_CRX="$CRX" \
    BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" 2>&1; echo "saiu=$?")"
confere "DESTDIR relativo: recusado" "$r" "caminho absoluto"
nao_saiu0() { [[ "$2" != *"saiu=0"* ]] && ok "$1" || falhou "$1 ($2)"; }
nao_saiu0 "DESTDIR relativo: não sai 0" "$r"
[ ! -e "$RAIZ/relativo" ] || falhou "DESTDIR relativo criou $RAIZ/relativo"
# No sistema sem root: recusa antes de qualquer coisa (o host não é tocado).
for acao in install uninstall; do
    r="$(/usr/bin/make -s -C "$RAIZ" "$acao" DESTDIR= EXTENSION_ID="$ID" EXTENSION_CRX="$CRX" \
        BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" WARD_UID=1000 2>&1; echo "saiu=$?")"
    confere "make $acao no sistema sem root: recusa" "$r" "exige root"
    nao_saiu0 "make $acao no sistema sem root: não sai 0" "$r"
done

if [ "$SO_DESTDIR" = 1 ]; then
    [ "$falhas" -eq 0 ] || { echo "test-install (DESTDIR): $falhas falha(s)"; exit 1; }
    echo "test-install: parte DESTDIR ok (contêiner pulado: --so-destdir)"
    exit 0
fi

echo "==> 2. Contêiner com systemd (root só dentro dele)"
for p in docker gcc; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done
COFRE_TESTE="$RAIZ/.dev/test-vault.ward"
[ -f "$COFRE_TESTE" ] && [ ! -L "$COFRE_TESTE" ] || { echo "falta $COFRE_TESTE; rode 'make dev-vault'" >&2; exit 1; }
C="$TMP/conteiner"
mkdir -m 0755 "$C" "$C/repo" "$C/bin"
install -m 0644 "$RAIZ/Makefile" "$C/repo/Makefile"
mkdir -m 0755 "$C/repo/install" "$C/repo/install/systemd" "$C/repo/install/tmpfiles.d" \
    "$C/repo/install/coredump.conf.d"
install -m 0644 "$RAIZ/install/tree.txt" "$C/repo/install/"
install -m 0755 "$RAIZ/install/install.sh" "$RAIZ/install/crx.py" "$C/repo/install/"
install -m 0644 "$RAIZ/install/groups.sh" "$RAIZ/install/chrome.sh" "$RAIZ/install/dump.sh" "$C/repo/install/"
install -m 0644 "$RAIZ/install/coredump.conf.d/ward.conf" "$C/repo/install/coredump.conf.d/"
install -m 0644 "$RAIZ/install/systemd/wardd@.socket" "$RAIZ/install/systemd/wardd@.service" "$C/repo/install/systemd/"
install -m 0644 "$RAIZ/install/tmpfiles.d/ward.conf" "$C/repo/install/tmpfiles.d/"
mkdir -m 0755 "$C/repo/install/desktop" "$C/repo/docs" "$C/repo/docs/design" "$C/repo/docs/design/logo"
install -m 0644 "$RAIZ/install/desktop/local.ward.app.desktop" "$RAIZ/install/desktop/local.ward.app.metainfo.xml" \
    "$C/repo/install/desktop/"
install -m 0644 "$RAIZ"/docs/design/logo/ward-icone-app-{48,64,128,256,512}.png "$RAIZ/docs/design/logo/ward-icone-app.svg" \
    "$C/repo/docs/design/logo/"
install -m 0755 "$WARD" "$C/bin/ward"
install -m 0755 "$WARDD" "$C/bin/wardd"
install -m 0755 "$NM" "$C/bin/ward-nm"
install -m 0755 "$APP" "$C/bin/ward-app"
install -m 0755 "$WARDD_TESTE" "$C/bin/wardd-test-mode"
install -m 0755 "$CLIENTE" "$C/bin/cliente"
install -m 0755 "$PLATAFORMA" "$C/bin/plataforma"
gcc -O2 -Wall -Werror -o "$C/bin/falso" "$RAIZ/dev/container-platform/fake.c"
gcc -shared -fPIC -O2 -Wall -Werror -o "$C/bin/libmarca.so" "$RAIZ/dev/container-platform/mark.c"
chmod 0755 "$C/bin/falso" "$C/bin/libmarca.so"
install -m 0644 "$CRX" "$C/ward.crx"
printf '%s\n' "$ID" >"$C/id"
install -m 0644 "$COFRE_TESTE" "$C/cofre.ward"
/usr/bin/python3 -I -c 'import json,sys; sys.stdout.write(json.load(open(sys.argv[1]))["senha_mestra"])' \
    "$RAIZ/dev/fixtures/entries.json" >"$C/senha"
chmod 0644 "$C/senha"

IMAGEM="ward-teste/install-systemd:1"
docker build --quiet -t "$IMAGEM" -f "$RAIZ/dev/container-install/Dockerfile" "$RAIZ/dev/container-units" >/dev/null
docker run -d --name "$NOME" --network none --pull never \
    --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cgroupns=private --cap-add SYS_ADMIN --security-opt apparmor=unconfined \
    --ulimit memlock=-1:-1 \
    -v "$C:/opt/teste:ro" \
    -v "$RAIZ/dev/container-install/inside.sh:/opt/inside.sh:ro" \
    "$IMAGEM" >/dev/null
codigo=0
docker exec "$NOME" /bin/bash -p /opt/inside.sh || codigo=$?
[ "$codigo" -eq 0 ] || falhou "contêiner: inside.sh saiu $codigo"

if [ "$falhas" -ne 0 ]; then
    echo "test-install: $falhas falha(s)"
    exit 1
fi
echo "test-install: tudo ok"
