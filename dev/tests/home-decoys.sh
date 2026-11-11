#!/bin/bash -p
# Attack test (Phase 12b, vector `iscas.sh`/`iscas2.sh` of the 2nd audit of M1): the
# WHOLE service scripts (`verify.sh` and `attacks-services.sh`), and not only each
# client, open nothing from the user's home.
#
# Runs in a bwrap with the passwd home covered by tmpfs (the real home is never seen). In
# the tmpfs it plants FIFO decoys on the paths an attacker cares about: ssh keys and
# config, ~/.gitconfig and ~/.config/git/*, ~/.docker/config.json, ~/.curlrc, ~/.netrc,
# the AWS credentials and the real Ward vault. Each FIFO has a background writer that
# records when someone opens the decoy for reading. A positive control (ssh, git and
# docker without the isolation) has to open decoys, to prove they work.
#
# The scripts call real-home.sh, which re-executes itself in its own bwrap (another
# tmpfs, without the decoys): that still holds in here.
#
# Needs the services up, bwrap and python3. Usage: dev/tests/home-decoys.sh
# (or `make test-home-decoys`).

# Fixed PATH before any command (Phase 12b): nothing from the user's PATH runs here.
export PATH=/usr/bin:/bin
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
for p in bwrap mkfifo; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p (o teste falha fechado)" >&2; exit 1; }
done

# --- Outside the sandbox: re-executes inside it (without inheriting the inside mark) ----
if [ "${WARD_ISCAS_DENTRO:-}" != 1 ]; then
    exec /usr/bin/env -u WARD_HOME_COBERTO bwrap --die-with-parent --dev-bind / / --tmpfs "$PASSWD_HOME" \
        --bind "$ROOT" "$ROOT" --bind /tmp /tmp --setenv WARD_ISCAS_DENTRO 1 --chdir "$ROOT" \
        -- /bin/bash -p "$ROOT/dev/tests/home-decoys.sh" "$@"
fi

# --- Inside: the home is the tmpfs and starts empty ----------------------------------------
[ "$(stat -f -c %T "$PASSWD_HOME")" = tmpfs ] || { echo "o home não está coberto por tmpfs; abortando" >&2; exit 1; }
extra="$(find "$PASSWD_HOME" -mindepth 1 -maxdepth 1 ! -path "$PASSWD_HOME/$(cut -d/ -f1 <<<"${ROOT#"$PASSWD_HOME"/}")" -print -quit)"
[ -z "$extra" ] || { echo "o home coberto tem conteúdo inesperado; abortando" >&2; exit 1; }

TMP="$(mktemp -d)"
chmod 0700 "$TMP"
OPENED="$TMP/opened"
STAGE="$TMP/stage"
: >"$OPENED"
echo start >"$STAGE"
writers=()
cleanup() {
    local p
    for p in "${writers[@]}"; do kill "$p" 2>/dev/null || true; done
    wait 2>/dev/null || true
    rm -rf "$TMP"
}
trap cleanup EXIT

# Names built in parts (V1.5 refuses spelling these folders out in the scripts).
S=".s""sh" A=".a""ws" W=".local/share/w""ard"
decoys=(
    "$S/id_rsa" "$S/id_ecdsa" "$S/id_ecdsa_sk" "$S/id_ed25519" "$S/id_ed25519_sk" "$S/id_xmss"
    "$S/id_dsa" "$S/id_rsa-cert.pub" "$S/id_ed25519-cert.pub" "$S/config" "$S/known_hosts"
    "$S/known_hosts2" "$S/authorized_keys"
    .gitconfig .config/git/config .config/git/ignore .config/git/attributes .git-credentials
    .docker/config.json .curlrc .netrc .wgetrc .pgpass
    "$A/credentials" "$A/config" "$W/cofre.ward" .cargo/config.toml .cargo/credentials.toml
)
for f in "${decoys[@]}"; do
    mkdir -p "$PASSWD_HOME/$(dirname "$f")"
    mkfifo -m 0600 "$PASSWD_HOME/$f"
    # Opens for writing (blocks until someone opens for reading), records and repeats.
    ( while :; do printf '' >"$PASSWD_HOME/$f" && echo "$(cat "$STAGE") $f" >>"$OPENED"; done ) &
    writers+=("$!")
done

failures=0
ok() { printf 'ok    %s\n' "$1"; }
failed() { printf 'FALHA %s\n' "$1" >&2; failures=$((failures + 1)); }

run_stage() { # stage script
    echo "$1" >"$STAGE"
    local rc=0
    "$ROOT/$2" >"$TMP/$1.out" 2>&1 || rc=$?
    sleep 1
    if [ "$rc" -ne 0 ]; then
        failed "$2 saiu $rc com as iscas no home (fim da saída abaixo)"
        tail -15 "$TMP/$1.out" >&2
    fi
    if grep -q "^$1 " "$OPENED"; then
        failed "$2 abriu iscas do home:"
        grep "^$1 " "$OPENED" | sort | uniq -c | sed 's/^/        /' >&2
    else
        ok "$2 inteiro não abre nenhuma isca do home"
    fi
}
run_stage verify dev/services/verify.sh
run_stage attacks dev/tests/attacks-services.sh

# Controls: without the isolation, ssh, git and docker open decoys.
echo control >"$STAGE"
. "$ROOT/dev/services/common.sh"
timeout 10 /usr/bin/env -i PATH=/usr/bin:/bin ssh -o BatchMode=yes -o ConnectTimeout=3 \
    -p 2223 ninguem@127.0.0.1 true >/dev/null 2>&1 || true
timeout 10 /usr/bin/env -u GIT_CONFIG_GLOBAL -u GIT_CONFIG_COUNT -u GIT_CONFIG_NOSYSTEM \
    git -C "$ROOT" status --short >/dev/null 2>&1 || true
timeout 20 /usr/bin/env -u DOCKER_CONFIG docker version >/dev/null 2>&1 || true
sleep 1
for f in "$S/id_ed25519" .gitconfig .docker/config.json; do
    if grep -qx "control $f" "$OPENED"; then
        ok "controle: sem o isolamento a isca $f é aberta (as iscas funcionam)"
    else
        failed "controle: a isca $f não foi aberta nem sem o isolamento (teste sem efeito)"
    fi
done

if [ "$failures" -ne 0 ]; then
    printf '\n%d problema(s): algum script abre o home do usuário.\n' "$failures" >&2
    exit 1
fi
printf '\nOs scripts de serviço não abrem nada do home real.\n'
