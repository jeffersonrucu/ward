#!/bin/bash -p
# `make test-docker-platform` (Fase 21b; D46, D47, D49; V3.5, V3.23, V3.24).
#
# O que exige root (grupos de sistema, binários setgid, pasta /run/ward/<uid> do root,
# socket criado pelo root) roda num contêiner `ubuntu:24.04` descartável: mesma glibc do
# Pop!_OS 24.04 e o mesmo kernel do host. Nada de sudo, nada no /etc nem no /run do
# host. O contêiner não tem rede e só monta, somente leitura, uma pasta temporária em
# `.dev/` com o binário de teste (`ward-platform-container`, feature `test-mode`) e a
# biblioteca de ataque do LD_PRELOAD, mais o script de dentro. Fica fora do `make
# check` (exige Docker). Uso: dev/tests/docker-platform.sh <binário de teste>
# <app de teste> <ward de release> <wardd de release>. O app de teste (Fase 29a) é o
# `startup-container` do `app/core`: a partida do app real (D47, D57) sem o GTK. A CLI
# e o `wardd` de release (sem `test-mode`) e o servidor falso não-dumpable (`fake.c`)
# são da 2ª parte do V4.2 (Fase 28, D65 e).

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$RAIZ/dev/services/common.sh"

# Uso: dev/tests/docker-platform.sh (ver $USO abaixo). Fase 40: o ward-nm setgid com o
# "Chrome" de pai fala com o wardd de teste real (D9, V6.1, V6.2).

# Imagem pela digest (a lista multiarquitetura de ubuntu:24.04 conferida na Fase 21b).
IMAGEM="ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852"
USO="uso: docker-platform.sh <binário de teste> <app de teste> <ward de release> <wardd de release> <wardd test-mode> <ward-nm de release> <fake-chrome> <wardd-test-client>"
BIN="${1:?$USO}"
APP="${2:?$USO}"
CLI="${3:?$USO}"
WARDD="${4:?$USO}"
WARDD_TESTE="${5:?$USO}"
NM="${6:?$USO}"
CHROME_FALSO="${7:?$USO}"
CLIENTE="${8:?$USO}"
for b in "$BIN" "$APP" "$CLI" "$WARDD" "$WARDD_TESTE" "$NM" "$CHROME_FALSO" "$CLIENTE"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b" >&2; exit 1; }
done
# V4.2 (2ª parte) e a Fase 40 exigem o build sem `test-mode`: as strings do daemon de
# teste não podem estar nos binários (o `ward` ou o `ward-nm` de teste aceitariam o
# socket de `WARD_DEV_SOCKET_DIR`).
for b in "$CLI" "$WARDD" "$NM"; do
    if grep -q -a -F 'WARD_DEV_SOCKET_DIR' -- "$b"; then
        echo "$b foi compilado com test-mode" >&2
        exit 1
    fi
done
COFRE="$RAIZ/.dev/test-vault.ward"
[ -f "$COFRE" ] && [ ! -L "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
for p in docker gcc; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$RAIZ/.dev"
TMP="$(mktemp -d "$RAIZ/.dev/container-platform.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0755 "$BIN" "$TMP/ward-teste"
install -m 0755 "$APP" "$TMP/ward-app-teste"
install -m 0755 "$CLI" "$TMP/ward-release"
install -m 0755 "$WARDD" "$TMP/wardd-release"
# Fase 40: o wardd de teste real, o ward-nm sem `test-mode` (o do release: só aceita o
# socket do sistema com par uid 0), o "Chrome" falso e o cliente mínimo. Uma cópia do
# cofre de teste e a senha de teste (falsa, D15), lidas das fixtures.
install -m 0755 "$WARDD_TESTE" "$TMP/wardd-real"
install -m 0755 "$NM" "$TMP/ward-nm-real"
install -m 0755 "$CHROME_FALSO" "$TMP/fake-chrome"
install -m 0755 "$CLIENTE" "$TMP/wardd-test-client"
install -m 0644 "$COFRE" "$TMP/test-vault.ward"
/usr/bin/python3 -I -c 'import json,sys; sys.stdout.write(json.load(open(sys.argv[1]))["senha_mestra"])' \
    "$RAIZ/dev/fixtures/entries.json" > "$TMP/senha"
chmod 0644 "$TMP/senha"
gcc -shared -fPIC -O2 -Wall -Werror -o "$TMP/libmarca.so" "$RAIZ/dev/container-platform/mark.c"
chmod 0644 "$TMP/libmarca.so"
gcc -O2 -Wall -Werror -o "$TMP/falso" "$RAIZ/dev/container-platform/fake.c"
chmod 0755 "$TMP/falso"

docker image inspect "$IMAGEM" >/dev/null 2>&1 || docker pull --quiet "$IMAGEM" >/dev/null

# Sem rede; sem `no-new-privileges` (ele desligaria justamente o setgid que se testa).
# `memlock` infinito: o "daemon" do contêiner faz o `mlockall` de D62 (V3.34: o app que
# ele lança não herda o travamento).
docker run --rm --network none --pull never --ulimit memlock=-1:-1 \
    -v "$TMP:/opt/teste:ro" \
    -v "$RAIZ/dev/container-platform/inside.sh:/opt/inside.sh:ro" \
    "$IMAGEM" /bin/bash -p /opt/inside.sh
