#!/bin/bash -p
# `make teste-install` (Fase 51; V9.1, V9.2, V9.3, V9.4, V9.6, V9.7, V9.10, V9.11; D18,
# D46, D47, D57, D60, D62, D63).
#
# Nunca instala nada no host. Duas partes:
#
# 1. Host, sem root: `make install DESTDIR=<pasta em .dev/>` com os artefatos sem
#    `modo-teste` (debug, `target/sem-modo-teste`), o `partida-conteiner` no lugar do
#    `ward-app` (a partida do app real sem GTK) e um pacote CRX3 falso
#    (`crx_falso.py`: chave e assinatura aleatórias). Confere a árvore exata de
#    `instalacao/arvore.txt` (V9.1), modos, conteúdo, os JSON do Chrome (V9.6), o
#    `update.xml`, o `systemd-analyze verify` das units instaladas, o `make uninstall`
#    (V9.10: some tudo do Ward, fica o que não é dele, inclusive um "cofre" plantado) e
#    as recusas: artefato `modo-teste` (V9.2), não-ELF, link, id da extensão inválido ou
#    diferente do pacote, pacote adulterado, PREFIX, DESTDIR relativo ou com link, pasta
#    com link no caminho, e `make install` sem root no sistema. Toda recusa sai ≠ 0 sem
#    deixar arquivo do Ward.
# 2. Contêiner `ubuntu:24.04` com systemd e sem rede (`dev/conteiner-install/`), onde o
#    root existe só dentro dele: `make install`/`make uninstall` de verdade
#    (`dev/conteiner-install/dentro.sh`). Mesmas permissões do `make teste-units` (ver
#    o comentário de `dev/testes/units.sh`): CAP_SYS_ADMIN e sem AppArmor só para o
#    systemd remontar a subárvore de cgroup dele.
#
# Fica fora do `make check` (exige Docker). Uso: dev/testes/install.sh [--so-destdir]

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/servicos/comum.sh
. "$RAIZ/dev/servicos/comum.sh"
unset MAKEFLAGS MAKELEVEL MFLAGS
cd "$RAIZ"

SO_DESTDIR=0
[ "${1:-}" = --so-destdir ] && SO_DESTDIR=1

SEM="$RAIZ/target/sem-modo-teste/debug"
WARD="$SEM/ward"
WARDD="$SEM/wardd"
NM="$SEM/ward-nm"
APP="$SEM/examples/partida-conteiner"
CLIENTE="$SEM/examples/wardd-cliente-teste"
WARDD_TESTE="$RAIZ/target/debug/wardd"
PLATAFORMA="$RAIZ/target/debug/examples/ward-plataforma-conteiner"
for b in "$WARD" "$WARDD" "$NM" "$APP" "$CLIENTE" "$WARDD_TESTE" "$PLATAFORMA"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b (rode pelo 'make teste-install')" >&2; exit 1; }
done
for p in systemd-analyze make; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$RAIZ/.dev"
TMP="$(mktemp -d "$RAIZ/.dev/teste-install.XXXXXX")"
NOME="ward-install-$$"
trap 'docker rm -f "$NOME" >/dev/null 2>&1 || true; rm -rf -- "$TMP"' EXIT

falhas=0
ok() { echo "ok: $*"; }
falhou() { echo "FALHOU: $*"; falhas=$((falhas + 1)); }
confere() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else falhou "$1 (esperado '$3', veio '$2')"; fi
}

# Pacote falso e o id dele.
CRX="$TMP/ward.crx"
ID="$(/usr/bin/python3 -I "$RAIZ/dev/testes/crx_falso.py" "$CRX")"
[[ "$ID" =~ ^[a-p]{32}$ ]] || { echo "crx_falso.py não deu um id" >&2; exit 1; }

# `make install`/`uninstall` com DESTDIR; argumentos a mais sobrepõem os padrões.
mk() {
    local acao="$1" d="$2"
    shift 2
    /usr/bin/make -s -C "$RAIZ" "$acao" DESTDIR="$d" EXTENSAO_ID="$ID" EXTENSAO_CRX="$CRX" \
        BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" WARD_UID= "$@"
}

# Linhas "tipo modo caminho" da árvore.
ARV="$TMP/arvore"
awk '!/^#/ && NF >= 5 {print $1, $2, $5}' "$RAIZ/instalacao/arvore.txt" >"$ARV"
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

# V9.1: a árvore é exatamente a de arvore.txt.
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
ok "V9.1: modos de arvore.txt (ward-app e ward-nm 2755), sem escrita para grupo e outros"
# Conteúdo.
cmp -s "$WARD" "$D/usr/local/bin/ward" && cmp -s "$WARDD" "$D/usr/local/bin/wardd" &&
    cmp -s "$APP" "$D/usr/local/bin/ward-app" && cmp -s "$NM" "$D/usr/local/lib/ward/ward-nm" &&
    ok "binários iguais aos artefatos" || falhou "binário instalado diferente do artefato"
for u in wardd@.socket wardd@.service; do
    cmp -s "$RAIZ/instalacao/systemd/$u" "$D/usr/local/lib/systemd/system/$u" || falhou "$u diferente"
done
cmp -s "$RAIZ/instalacao/tmpfiles.d/ward.conf" "$D/usr/local/lib/tmpfiles.d/ward.conf" || falhou "tmpfiles diferente"
cmp -s "$CRX" "$D/usr/local/share/ward/extensao/ward.crx" || falhou "pacote da extensão diferente"
ok "units, tmpfiles.d e pacote iguais aos do repositório"
# V9.6, a política e o update.xml.
r="$(/usr/bin/python3 -I - "$D" "$ID" <<'EOF'
import json, sys, xml.etree.ElementTree as ET
d, ident = sys.argv[1], sys.argv[2]
nm = json.load(open(d + "/etc/opt/chrome/native-messaging-hosts/br.ward.nm.json"))
assert nm == {"name": "br.ward.nm", "description": nm["description"], "path": "/usr/local/lib/ward/ward-nm",
              "type": "stdio", "allowed_origins": [f"chrome-extension://{ident}/"]}, nm
print("nm ok")
pol = json.load(open(d + "/etc/opt/chrome/policies/managed/ward.json"))
assert list(pol) == ["ExtensionSettings"] and list(pol["ExtensionSettings"]) == [ident], pol
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
confere "política do Chrome: só a extensão do Ward, force_installed, update local" "$r" "politica ok"
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
# (`dev/testes/ataques-fundacao.sh`) não confundir com o home real.
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
# Artefatos com marca de modo-teste: o wardd de teste de verdade e cópias com a marca.
cp "$WARD" "$TMP/ward-marcado" && printf 'WARD_DEV_SOCKET_DIR' >>"$TMP/ward-marcado"
cp "$NM" "$TMP/nm-marcado" && printf 'x WARD_DEV_DIR x' >>"$TMP/nm-marcado"
cp "$APP" "$TMP/app-marcado" && printf 'ward-ponte-teste' >>"$TMP/app-marcado"
nova; recusa "V9.2: wardd com modo-teste" "modo-teste" "$d" BIN_WARDD="$WARDD_TESTE"
nova; recusa "V9.2: ward com WARD_DEV_SOCKET_DIR" "modo-teste" "$d" BIN_WARD="$TMP/ward-marcado"
nova; recusa "V9.2: ward-nm com WARD_DEV_DIR" "modo-teste" "$d" BIN_NM="$TMP/nm-marcado"
nova; recusa "V9.2: ward-app com a ponte de teste" "modo-teste" "$d" BIN_APP="$TMP/app-marcado"
printf '#!/bin/sh\necho falso\n' >"$TMP/script"
chmod 0755 "$TMP/script"
nova; recusa "wardd que não é ELF" "não é um executável ELF" "$d" BIN_WARDD="$TMP/script"
ln -s "$WARD" "$TMP/ward-link"
nova; recusa "ward por link simbólico" "link simbólico" "$d" BIN_WARD="$TMP/ward-link"
nova; recusa "artefato ausente" "não existe" "$d" BIN_NM="$TMP/nao-existe"
nova; recusa "EXTENSAO_ID vazio" "EXTENSAO_ID" "$d" EXTENSAO_ID=
nova; recusa "EXTENSAO_ID em maiúsculas" "EXTENSAO_ID" "$d" EXTENSAO_ID="${ID^^}"
nova; recusa "EXTENSAO_ID com 31 letras" "EXTENSAO_ID" "$d" EXTENSAO_ID="${ID:1}"
nova; recusa "EXTENSAO_ID com aspas (injeção no JSON)" "EXTENSAO_ID" "$d" EXTENSAO_ID="${ID:0:28}\",\"x"
outro="$(/usr/bin/python3 -I "$RAIZ/dev/testes/crx_falso.py" "$TMP/outro.crx")"
nova; recusa "EXTENSAO_ID de outra extensão" "não de EXTENSAO_ID" "$d" EXTENSAO_ID="$outro"
/usr/bin/python3 -I "$RAIZ/dev/testes/crx_falso.py" "$TMP/adulterado.crx" --adulterado >"$TMP/id-adulterado"
nova; recusa "pacote com id que não é o da chave" "EXTENSAO_CRX recusado" "$d" \
    EXTENSAO_ID="$(cat "$TMP/id-adulterado")" EXTENSAO_CRX="$TMP/adulterado.crx"
/usr/bin/python3 -I "$RAIZ/dev/testes/crx_falso.py" "$TMP/versao.crx" --versao "1.0'><x" >/dev/null
nova; recusa "pacote com versão fora do formato" "EXTENSAO_CRX recusado" "$d" EXTENSAO_CRX="$TMP/versao.crx"
nova; recusa "pacote ausente" "EXTENSAO_CRX" "$d" EXTENSAO_CRX=
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
# DESTDIR relativo (o make passa como veio).
r="$(cd "$TMP" && /usr/bin/make -s -C "$RAIZ" install DESTDIR=relativo EXTENSAO_ID="$ID" EXTENSAO_CRX="$CRX" \
    BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" 2>&1; echo "saiu=$?")"
confere "DESTDIR relativo: recusado" "$r" "caminho absoluto"
nao_saiu0() { [[ "$2" != *"saiu=0"* ]] && ok "$1" || falhou "$1 ($2)"; }
nao_saiu0 "DESTDIR relativo: não sai 0" "$r"
[ ! -e "$RAIZ/relativo" ] || falhou "DESTDIR relativo criou $RAIZ/relativo"
# No sistema sem root: recusa antes de qualquer coisa (o host não é tocado).
for acao in install uninstall; do
    r="$(/usr/bin/make -s -C "$RAIZ" "$acao" DESTDIR= EXTENSAO_ID="$ID" EXTENSAO_CRX="$CRX" \
        BIN_WARD="$WARD" BIN_WARDD="$WARDD" BIN_NM="$NM" BIN_APP="$APP" WARD_UID=1000 2>&1; echo "saiu=$?")"
    confere "make $acao no sistema sem root: recusa" "$r" "exige root"
    nao_saiu0 "make $acao no sistema sem root: não sai 0" "$r"
done

if [ "$SO_DESTDIR" = 1 ]; then
    [ "$falhas" -eq 0 ] || { echo "teste-install (DESTDIR): $falhas falha(s)"; exit 1; }
    echo "teste-install: parte DESTDIR ok (contêiner pulado: --so-destdir)"
    exit 0
fi

echo "==> 2. Contêiner com systemd (root só dentro dele)"
for p in docker gcc; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done
COFRE_TESTE="$RAIZ/.dev/cofre-teste.ward"
[ -f "$COFRE_TESTE" ] && [ ! -L "$COFRE_TESTE" ] || { echo "falta $COFRE_TESTE; rode 'make dev-cofre'" >&2; exit 1; }
C="$TMP/conteiner"
mkdir -m 0755 "$C" "$C/repo" "$C/bin"
install -m 0644 "$RAIZ/Makefile" "$C/repo/Makefile"
mkdir -m 0755 "$C/repo/instalacao" "$C/repo/instalacao/systemd" "$C/repo/instalacao/tmpfiles.d"
install -m 0644 "$RAIZ/instalacao/arvore.txt" "$C/repo/instalacao/"
install -m 0755 "$RAIZ/instalacao/instalar.sh" "$RAIZ/instalacao/crx.py" "$C/repo/instalacao/"
install -m 0644 "$RAIZ/instalacao/grupos.sh" "$C/repo/instalacao/"
install -m 0644 "$RAIZ/instalacao/systemd/wardd@.socket" "$RAIZ/instalacao/systemd/wardd@.service" "$C/repo/instalacao/systemd/"
install -m 0644 "$RAIZ/instalacao/tmpfiles.d/ward.conf" "$C/repo/instalacao/tmpfiles.d/"
install -m 0755 "$WARD" "$C/bin/ward"
install -m 0755 "$WARDD" "$C/bin/wardd"
install -m 0755 "$NM" "$C/bin/ward-nm"
install -m 0755 "$APP" "$C/bin/ward-app"
install -m 0755 "$WARDD_TESTE" "$C/bin/wardd-modo-teste"
install -m 0755 "$CLIENTE" "$C/bin/cliente"
install -m 0755 "$PLATAFORMA" "$C/bin/plataforma"
gcc -O2 -Wall -Werror -o "$C/bin/falso" "$RAIZ/dev/conteiner-plataforma/falso.c"
gcc -shared -fPIC -O2 -Wall -Werror -o "$C/bin/libmarca.so" "$RAIZ/dev/conteiner-plataforma/marca.c"
chmod 0755 "$C/bin/falso" "$C/bin/libmarca.so"
install -m 0644 "$CRX" "$C/ward.crx"
printf '%s\n' "$ID" >"$C/id"
install -m 0644 "$COFRE_TESTE" "$C/cofre.ward"
/usr/bin/python3 -I -c 'import json,sys; sys.stdout.write(json.load(open(sys.argv[1]))["senha_mestra"])' \
    "$RAIZ/dev/fixtures/entradas.json" >"$C/senha"
chmod 0644 "$C/senha"

IMAGEM="ward-teste/install-systemd:1"
docker build --quiet -t "$IMAGEM" -f "$RAIZ/dev/conteiner-install/Dockerfile" "$RAIZ/dev/conteiner-units" >/dev/null
docker run -d --name "$NOME" --network none --pull never \
    --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cgroupns=private --cap-add SYS_ADMIN --security-opt apparmor=unconfined \
    --ulimit memlock=-1:-1 \
    -v "$C:/opt/teste:ro" \
    -v "$RAIZ/dev/conteiner-install/dentro.sh:/opt/dentro.sh:ro" \
    "$IMAGEM" >/dev/null
codigo=0
docker exec "$NOME" /bin/bash -p /opt/dentro.sh || codigo=$?
[ "$codigo" -eq 0 ] || falhou "contêiner: dentro.sh saiu $codigo"

if [ "$falhas" -ne 0 ]; then
    echo "teste-install: $falhas falha(s)"
    exit 1
fi
echo "teste-install: tudo ok"
