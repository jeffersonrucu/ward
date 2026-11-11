#!/bin/bash -p
# `make test-units` (Phase 24; V3.21; D46, D57, D62).
#
# 1. Syntax: `systemd-analyze verify` of the units of `install/systemd/` on a copy in
#    `.dev/` (instance `wardd@1000`), without touching the host's systemd (nothing is
#    installed or reloaded; verify only reads the files it is given).
# 2. Container: an `ubuntu:24.04` image (same digest as Phases 21b/22) with systemd
#    (`dev/container-units/Dockerfile`; network only in the build), running without
#    network, with root only inside it. `inside.sh` installs the units and the `wardd`
#    (without `test-mode`), and verifies that the first connection to the socket starts
#    the daemon as uid 1000, with `LimitCORE=0`, `LimitMEMLOCK=infinity` and without
#    `no_new_privs`; with the test vault unlocked, `VmLck` covers `VmRSS` (mlockall,
#    D62), and the same binary with an 8 MiB `RLIMIT_MEMLOCK` exits ≠ 0 with a message
#    (V3.34).
# 3. V4.38 (Phase 26, D63): with the `ward-app`/`ward-nm` groups and setgid copies of
#    the minimal client, `ward run <free> -- ward-app …` (the release CLI) does not give
#    the app role to the daemon's child: the peer has the right group and parent, but it
#    is not the pid the launcher opened; the journal has the app role refusal line.
#
# systemd needs to write to the container's /sys/fs/cgroup (its subtree, with
# `--cgroupns=private`), which Docker mounts read-only: the container gets
# CAP_SYS_ADMIN and leaves the AppArmor profile only for `start.sh` to remount. It stays
# without network, without `--privileged`, mounting nothing from the host besides the
# read-only test folder, and is removed at the end. `--ulimit memlock=-1:-1` lets the
# systemd inside apply the unit's `LimitMEMLOCK=infinity` (Docker's limit is 8 MiB).
#
# Stays out of `make check` (requires Docker). Usage: dev/tests/units.sh <wardd> <client> <ward>

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"

WARDD="${1:?uso: units.sh <wardd> <wardd-test-client> <ward>}"
CLIENT="${2:?uso: units.sh <wardd> <wardd-test-client> <ward>}"
WARD="${3:?uso: units.sh <wardd> <wardd-test-client> <ward>}"
for b in "$WARDD" "$CLIENT" "$WARD"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b" >&2; exit 1; }
done
for p in docker systemd-analyze; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$ROOT/.dev"
TMP="$(mktemp -d "$ROOT/.dev/container-units.XXXXXX")"
CONTAINER_NAME="ward-units-$$"
trap 'docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true; rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0644 "$ROOT/install/systemd/wardd@.socket" "$ROOT/install/systemd/wardd@.service" "$TMP/"

echo "==> systemd-analyze verify"
# verify complains about the ExecStart that does not exist on the host; the rest has to come out clean.
output="$(systemd-analyze verify --man=no "$TMP/wardd@1000.socket" "$TMP/wardd@1000.service" 2>&1 || true)"
rest="$(printf '%s\n' "$output" | grep -v -e '/usr/local/bin/wardd' -e '^$' || true)"
if [ -n "$rest" ]; then
    printf '%s\n' "$output"
    echo "FALHOU: systemd-analyze verify"
    exit 1
fi
echo "ok: systemd-analyze verify"
# Control: the same verify flags an invalid directive (otherwise the "ok" above is worth nothing).
mkdir "$TMP/control"
install -m 0644 "$TMP/wardd@.socket" "$TMP/control/"
sed 's/^LimitCORE=0$/LimitCORE=not-a-number\nInventedDirective=1/' "$TMP/wardd@.service" >"$TMP/control/wardd@.service"
output="$(systemd-analyze verify --man=no "$TMP/control/wardd@1000.service" 2>&1 || true)"
if printf '%s\n' "$output" | grep -q -e 'InventedDirective' -e 'LimitCORE'; then
    echo "ok: o verify acusa diretiva inválida (controle)"
else
    printf '%s\n' "$output"
    echo "FALHOU: o verify não acusou a diretiva inválida do controle"
    exit 1
fi

install -m 0755 "$WARDD" "$TMP/wardd"
install -m 0755 "$CLIENT" "$TMP/cliente"
install -m 0755 "$WARD" "$TMP/ward"
# V3.34 (D62): the test vault (only fake values, from `make dev-vault`) for the
# container's `wardd` to unlock and show `VmLck`.
TEST_VAULT="$ROOT/.dev/test-vault.ward"
[ -f "$TEST_VAULT" ] || { echo "falta $TEST_VAULT; rode 'make dev-vault' antes" >&2; exit 1; }
install -m 0600 "$TEST_VAULT" "$TMP/cofre.ward"

echo "==> imagem com systemd"
IMAGE="ward-teste/units-systemd:1"
docker build --quiet -t "$IMAGE" "$ROOT/dev/container-units" >/dev/null

echo "==> contêiner"
docker run -d --name "$CONTAINER_NAME" --network none --pull never \
    --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cgroupns=private --cap-add SYS_ADMIN --security-opt apparmor=unconfined \
    --ulimit memlock=-1:-1 \
    -v "$TMP:/opt/teste:ro" \
    -v "$ROOT/dev/container-units/inside.sh:/opt/inside.sh:ro" \
    "$IMAGE" >/dev/null
docker exec "$CONTAINER_NAME" /bin/bash -p /opt/inside.sh
