#!/bin/bash -p
# `make test-docker-platform` (Phase 21b; D46, D47, D49; V3.5, V3.23, V3.24).
#
# What requires root (system groups, setgid binaries, root's /run/ward/<uid> folder,
# socket created by root) runs in a disposable `ubuntu:24.04` container: the same glibc
# as Pop!_OS 24.04 and the same kernel as the host. No sudo, nothing in the host's /etc
# or /run. The container has no network and only mounts, read-only, a temporary folder
# in `.dev/` with the test binary (`ward-platform-container`, feature `test-mode`) and
# the LD_PRELOAD attack library, plus the inside script. Stays out of `make check`
# (requires Docker). Usage: dev/tests/docker-platform.sh <test binary> <test app>
# <release ward> <release wardd>. The test app (Phase 29a) is the `startup-container`
# of `app/core`: the startup of the real app (D47, D57) without GTK. The release CLI
# and `wardd` (without `test-mode`) and the non-dumpable fake server (`fake.c`) belong
# to the 2nd part of V4.2 (Phase 28, D65 e).

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"

# Usage: dev/tests/docker-platform.sh (see $USAGE below). Phase 40: the setgid ward-nm
# with the "Chrome" as parent talks to the real test wardd (D9, V6.1, V6.2).

# Image by digest (the multi-arch list of ubuntu:24.04 verified in Phase 21b).
IMAGE="ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852"
USAGE="uso: docker-platform.sh <binário de teste> <app de teste> <ward de release> <wardd de release> <wardd test-mode> <ward-nm de release> <fake-chrome> <wardd-test-client>"
BIN="${1:?$USAGE}"
APP="${2:?$USAGE}"
CLI="${3:?$USAGE}"
WARDD="${4:?$USAGE}"
WARDD_TEST="${5:?$USAGE}"
NM="${6:?$USAGE}"
FAKE_CHROME="${7:?$USAGE}"
CLIENT="${8:?$USAGE}"
for b in "$BIN" "$APP" "$CLI" "$WARDD" "$WARDD_TEST" "$NM" "$FAKE_CHROME" "$CLIENT"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b" >&2; exit 1; }
done
# V4.2 (2nd part) and Phase 40 require the build without `test-mode`: the strings of
# the test daemon cannot be in the binaries (the test `ward` or `ward-nm` would accept
# the socket of `WARD_DEV_SOCKET_DIR`).
for b in "$CLI" "$WARDD" "$NM"; do
    if grep -q -a -F 'WARD_DEV_SOCKET_DIR' -- "$b"; then
        echo "$b foi compilado com test-mode" >&2
        exit 1
    fi
done
VAULT="$ROOT/.dev/test-vault.ward"
[ -f "$VAULT" ] && [ ! -L "$VAULT" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
for p in docker gcc; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$ROOT/.dev"
TMP="$(mktemp -d "$ROOT/.dev/container-platform.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0755 "$BIN" "$TMP/ward-teste"
install -m 0755 "$APP" "$TMP/ward-app-teste"
install -m 0755 "$CLI" "$TMP/ward-release"
install -m 0755 "$WARDD" "$TMP/wardd-release"
# Phase 40: the real test wardd, the ward-nm without `test-mode` (the release one: it
# only accepts the system socket with a uid 0 peer), the fake "Chrome" and the minimal
# client. A copy of the test vault and the test password (fake, D15), read from the
# fixtures.
install -m 0755 "$WARDD_TEST" "$TMP/wardd-real"
install -m 0755 "$NM" "$TMP/ward-nm-real"
install -m 0755 "$FAKE_CHROME" "$TMP/fake-chrome"
install -m 0755 "$CLIENT" "$TMP/wardd-test-client"
install -m 0644 "$VAULT" "$TMP/test-vault.ward"
/usr/bin/python3 -I -c 'import json,sys; sys.stdout.write(json.load(open(sys.argv[1]))["master_password"])' \
    "$ROOT/dev/fixtures/entries.json" > "$TMP/senha"
chmod 0644 "$TMP/senha"
gcc -shared -fPIC -O2 -Wall -Werror -o "$TMP/libmarca.so" "$ROOT/dev/container-platform/mark.c"
chmod 0644 "$TMP/libmarca.so"
gcc -O2 -Wall -Werror -o "$TMP/falso" "$ROOT/dev/container-platform/fake.c"
chmod 0755 "$TMP/falso"

docker image inspect "$IMAGE" >/dev/null 2>&1 || docker pull --quiet "$IMAGE" >/dev/null

# No network; no `no-new-privileges` (it would turn off exactly the setgid under test).
# Infinite `memlock`: the container's "daemon" does the `mlockall` of D62 (V3.34: the
# app it launches does not inherit the lock).
docker run --rm --network none --pull never --ulimit memlock=-1:-1 \
    -v "$TMP:/opt/teste:ro" \
    -v "$ROOT/dev/container-platform/inside.sh:/opt/inside.sh:ro" \
    "$IMAGE" /bin/bash -p /opt/inside.sh
