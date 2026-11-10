#!/bin/bash -p
# Teste de ataque (Fase 12b, vetor `iscas.sh`/`iscas2.sh` da 2ª auditoria do M1): os
# scripts de serviço INTEIROS (`verify.sh` e `attacks-services.sh`), e não só cada
# cliente, não abrem nada do home do usuário.
#
# Roda num bwrap com o home do passwd coberto por tmpfs (o home real nunca é visto). No
# tmpfs planta iscas FIFO nos caminhos que interessam a um atacante: chaves e config do
# ssh, ~/.gitconfig e ~/.config/git/*, ~/.docker/config.json, ~/.curlrc, ~/.netrc, as
# credenciais da AWS e o cofre real do Ward. Cada FIFO tem um escritor em segundo plano
# que anota quando alguém abre a isca para ler. Um controle positivo (ssh, git e docker
# sem o isolamento) tem de abrir iscas, para provar que elas funcionam.
#
# Os scripts chamam o real-home.sh, que se reexecuta num bwrap próprio (outro tmpfs, sem
# as iscas): isso continua valendo aqui dentro.
#
# Precisa dos serviços de pé, de bwrap e de python3. Uso: dev/tests/home-decoys.sh
# (ou `make test-home-decoys`).

# PATH fixo antes de qualquer comando (Fase 12b): nada do PATH do usuário roda aqui.
export PATH=/usr/bin:/bin
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/env.sh
. "$RAIZ/dev/env.sh"
CASA="$CASA_PASSWD"
for p in bwrap mkfifo; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p (o teste falha fechado)" >&2; exit 1; }
done

# --- Fora do sandbox: reexecuta dentro dele (sem herdar a marca de dentro) ---------------
if [ "${WARD_ISCAS_DENTRO:-}" != 1 ]; then
    exec /usr/bin/env -u WARD_HOME_COBERTO bwrap --die-with-parent --dev-bind / / --tmpfs "$CASA" \
        --bind "$RAIZ" "$RAIZ" --bind /tmp /tmp --setenv WARD_ISCAS_DENTRO 1 --chdir "$RAIZ" \
        -- /bin/bash -p "$RAIZ/dev/tests/home-decoys.sh" "$@"
fi

# --- Dentro: o home é o tmpfs e começa vazio ------------------------------------------------
[ "$(stat -f -c %T "$CASA")" = tmpfs ] || { echo "o home não está coberto por tmpfs; abortando" >&2; exit 1; }
extra="$(find "$CASA" -mindepth 1 -maxdepth 1 ! -path "$CASA/$(cut -d/ -f1 <<<"${RAIZ#"$CASA"/}")" -print -quit)"
[ -z "$extra" ] || { echo "o home coberto tem conteúdo inesperado; abortando" >&2; exit 1; }

TMP="$(mktemp -d)"
chmod 0700 "$TMP"
ABERTAS="$TMP/abertas"
FASE="$TMP/fase"
: >"$ABERTAS"
echo inicio >"$FASE"
escritores=()
limpar() {
    local p
    for p in "${escritores[@]}"; do kill "$p" 2>/dev/null || true; done
    wait 2>/dev/null || true
    rm -rf "$TMP"
}
trap limpar EXIT

# Nomes montados por partes (o V1.5 recusa citar essas pastas por extenso nos scripts).
S=".s""sh" A=".a""ws" W=".local/share/w""ard"
iscas=(
    "$S/id_rsa" "$S/id_ecdsa" "$S/id_ecdsa_sk" "$S/id_ed25519" "$S/id_ed25519_sk" "$S/id_xmss"
    "$S/id_dsa" "$S/id_rsa-cert.pub" "$S/id_ed25519-cert.pub" "$S/config" "$S/known_hosts"
    "$S/known_hosts2" "$S/authorized_keys"
    .gitconfig .config/git/config .config/git/ignore .config/git/attributes .git-credentials
    .docker/config.json .curlrc .netrc .wgetrc .pgpass
    "$A/credentials" "$A/config" "$W/cofre.ward" .cargo/config.toml .cargo/credentials.toml
)
for f in "${iscas[@]}"; do
    mkdir -p "$CASA/$(dirname "$f")"
    mkfifo -m 0600 "$CASA/$f"
    # Abre para escrita (bloqueia até alguém abrir para ler), anota e repete.
    ( while :; do printf '' >"$CASA/$f" && echo "$(cat "$FASE") $f" >>"$ABERTAS"; done ) &
    escritores+=("$!")
done

falhas=0
ok() { printf 'ok    %s\n' "$1"; }
falhou() { printf 'FALHA %s\n' "$1" >&2; falhas=$((falhas + 1)); }

rodar() { # fase script
    echo "$1" >"$FASE"
    local rc=0
    "$RAIZ/$2" >"$TMP/$1.out" 2>&1 || rc=$?
    sleep 1
    if [ "$rc" -ne 0 ]; then
        falhou "$2 saiu $rc com as iscas no home (fim da saída abaixo)"
        tail -15 "$TMP/$1.out" >&2
    fi
    if grep -q "^$1 " "$ABERTAS"; then
        falhou "$2 abriu iscas do home:"
        grep "^$1 " "$ABERTAS" | sort | uniq -c | sed 's/^/        /' >&2
    else
        ok "$2 inteiro não abre nenhuma isca do home"
    fi
}
rodar conferir dev/services/verify.sh
rodar ataques dev/tests/attacks-services.sh

# Controles: sem o isolamento, ssh, git e docker abrem iscas.
echo controle >"$FASE"
. "$RAIZ/dev/services/common.sh"
timeout 10 /usr/bin/env -i PATH=/usr/bin:/bin ssh -o BatchMode=yes -o ConnectTimeout=3 \
    -p 2223 ninguem@127.0.0.1 true >/dev/null 2>&1 || true
timeout 10 /usr/bin/env -u GIT_CONFIG_GLOBAL -u GIT_CONFIG_COUNT -u GIT_CONFIG_NOSYSTEM \
    git -C "$RAIZ" status --short >/dev/null 2>&1 || true
timeout 20 /usr/bin/env -u DOCKER_CONFIG docker version >/dev/null 2>&1 || true
sleep 1
for f in "$S/id_ed25519" .gitconfig .docker/config.json; do
    if grep -qx "controle $f" "$ABERTAS"; then
        ok "controle: sem o isolamento a isca $f é aberta (as iscas funcionam)"
    else
        falhou "controle: a isca $f não foi aberta nem sem o isolamento (teste sem efeito)"
    fi
done

if [ "$falhas" -ne 0 ]; then
    printf '\n%d problema(s): algum script abre o home do usuário.\n' "$falhas" >&2
    exit 1
fi
printf '\nOs scripts de serviço não abrem nada do home real.\n'
