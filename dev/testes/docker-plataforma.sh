#!/bin/bash -p
# `make teste-docker-plataforma` (Fase 21b; D46, D47, D49; V3.5, V3.23, V3.24).
#
# O que exige root (grupos de sistema, binários setgid, pasta /run/ward/<uid> do root,
# socket criado pelo root) roda num contêiner `ubuntu:24.04` descartável: mesma glibc do
# Pop!_OS 24.04 e o mesmo kernel do host. Nada de sudo, nada no /etc nem no /run do
# host. O contêiner não tem rede e só monta, somente leitura, uma pasta temporária em
# `.dev/` com o binário de teste (`ward-plataforma-conteiner`, feature `modo-teste`) e a
# biblioteca de ataque do LD_PRELOAD, mais o script de dentro. Fica fora do `make
# check` (exige Docker). Uso: dev/testes/docker-plataforma.sh <binário de teste>
# <app de teste>. O app de teste (Fase 29a) é o `partida-conteiner` do `app/nucleo`: a
# partida do app real (D47, D57) sem o GTK.

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/servicos/comum.sh
. "$RAIZ/dev/servicos/comum.sh"

# Uso: dev/testes/docker-plataforma.sh (ver $USO abaixo). Fase 40: o ward-nm setgid com o
# "Chrome" de pai fala com o wardd de teste real (D9, V6.1, V6.2).

# Imagem pela digest (a lista multiarquitetura de ubuntu:24.04 conferida na Fase 21b).
IMAGEM="ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852"
USO="uso: docker-plataforma.sh <binário de teste> <app de teste> <wardd modo-teste> <ward-nm sem modo-teste> <chrome-falso> <wardd-cliente-teste>"
BIN="${1:?$USO}"
APP="${2:?$USO}"
WARDD="${3:?$USO}"
NM="${4:?$USO}"
CHROME_FALSO="${5:?$USO}"
CLIENTE="${6:?$USO}"
for b in "$BIN" "$APP" "$WARDD" "$NM" "$CHROME_FALSO" "$CLIENTE"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b" >&2; exit 1; }
done
COFRE="$RAIZ/.dev/cofre-teste.ward"
[ -f "$COFRE" ] && [ ! -L "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-cofre'" >&2; exit 1; }
for p in docker gcc; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$RAIZ/.dev"
TMP="$(mktemp -d "$RAIZ/.dev/conteiner-plataforma.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0755 "$BIN" "$TMP/ward-teste"
install -m 0755 "$APP" "$TMP/ward-app-teste"
# Fase 40: o wardd de teste real, o ward-nm sem `modo-teste` (o do release: só aceita o
# socket do sistema com par uid 0), o "Chrome" falso e o cliente mínimo. Uma cópia do
# cofre de teste e a senha de teste (falsa, D15), lidas das fixtures.
install -m 0755 "$WARDD" "$TMP/wardd-real"
install -m 0755 "$NM" "$TMP/ward-nm-real"
install -m 0755 "$CHROME_FALSO" "$TMP/chrome-falso"
install -m 0755 "$CLIENTE" "$TMP/wardd-cliente-teste"
install -m 0644 "$COFRE" "$TMP/cofre-teste.ward"
/usr/bin/python3 -I -c 'import json,sys; sys.stdout.write(json.load(open(sys.argv[1]))["senha_mestra"])' \
    "$RAIZ/dev/fixtures/entradas.json" > "$TMP/senha"
chmod 0644 "$TMP/senha"
gcc -shared -fPIC -O2 -Wall -Werror -o "$TMP/libmarca.so" "$RAIZ/dev/conteiner-plataforma/marca.c"
chmod 0644 "$TMP/libmarca.so"

docker image inspect "$IMAGEM" >/dev/null 2>&1 || docker pull --quiet "$IMAGEM" >/dev/null

# Sem rede; sem `no-new-privileges` (ele desligaria justamente o setgid que se testa).
# `memlock` infinito: o "daemon" do contêiner faz o `mlockall` de D62 (V3.34: o app que
# ele lança não herda o travamento).
docker run --rm --network none --pull never --ulimit memlock=-1:-1 \
    -v "$TMP:/opt/teste:ro" \
    -v "$RAIZ/dev/conteiner-plataforma/dentro.sh:/opt/dentro.sh:ro" \
    "$IMAGEM" /bin/bash -p /opt/dentro.sh
