#!/bin/bash -p
# Phase 63 (m2, Milestone 6, Battery C): the parts of V2.6, V2.7 and V4.4 that need the
# release daemon or root, by hand, in a disposable ubuntu:24.04 container with systemd
# (image `ward-teste/fase63-tools:tmp` = `ward-teste/units-systemd:1` + gdb, strace,
# inotify-tools, python3; deleted at the end of the phase). Root only inside the
# container; no network; the container gets CAP_SYS_ADMIN (systemd, as in test-units)
# and CAP_SYS_PTRACE (root dumps the daemon's memory and runs strace on it, V2.7/V4.4).
# Only the test vault, the 0.1.4 fixture vault and fake values (D15).
#
# Usage (from the repository root): manual-run.sh <release wardd> <release ward> <client>
export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(pwd -P)"
[ -f "$ROOT/Makefile" ] && [ -d "$ROOT/dev/services" ] || { echo "rode da raiz do repositório" >&2; exit 1; }
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"

WARDD="${1:?uso: manual-run.sh <wardd> <ward> <cliente>}"
WARD="${2:?uso}"
CLIENT="${3:?uso}"
HERE="$ROOT/docs/mission/evidence/M6/fase-63/scripts"
TMP="$(mktemp -d "$ROOT/.dev/fase63-manual.XXXXXX")"
NAME="ward-fase63-manual-$$"
trap 'docker rm -f "$NAME" >/dev/null 2>&1 || true; rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0755 "$WARDD" "$TMP/wardd"
install -m 0755 "$WARD" "$TMP/ward"
install -m 0755 "$CLIENT" "$TMP/cliente"
install -m 0644 "$ROOT/install/systemd/wardd@.socket" "$ROOT/install/systemd/wardd@.service" "$TMP/"
install -m 0644 "$ROOT/.dev/test-vault.ward" "$TMP/cofre.ward"
install -m 0644 "$ROOT/dev/fixtures/legacy/vault-0.1.4.ward" "$TMP/vault-0.1.4.ward"
install -m 0644 "$ROOT/dev/fixtures/legacy/state-0.1.4" "$TMP/state-0.1.4"
install -m 0644 "$ROOT/.dev/variations.txt" "$TMP/variations.txt"
install -m 0644 "$HERE/manual_dump.py" "$TMP/dump.py"
/usr/bin/python3 -I -c 'import json,sys; sys.stdout.write(json.load(open(sys.argv[1]))["master_password"])' \
    "$ROOT/dev/fixtures/entries.json" >"$TMP/senha"
chmod 0644 "$TMP/senha"

# Host side of "no core anywhere": the host's crash folder before and after (the kernel's
# core_pattern is the host's; a core of the container's wardd would land there).
crash_before="$(ls -1 /var/crash 2>/dev/null | sort || true)"

docker run -d --name "$NAME" --network none --pull never \
    --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cgroupns=private --cap-add SYS_ADMIN --cap-add SYS_PTRACE --security-opt apparmor=unconfined \
    --ulimit memlock=-1:-1 \
    -v "$TMP:/opt/teste:ro" \
    -v "$HERE/manual-inside.sh:/opt/inside.sh:ro" \
    ward-teste/fase63-tools:tmp >/dev/null
code=0
docker exec "$NAME" /bin/bash -p /opt/inside.sh || code=$?

crash_after="$(ls -1 /var/crash 2>/dev/null | sort || true)"
new_crash="$(comm -13 <(printf '%s\n' "$crash_before") <(printf '%s\n' "$crash_after") | grep -i ward || true)"
if [ -z "$new_crash" ]; then
    echo "ok: V2.6 host: nenhum arquivo novo do wardd em /var/crash"
else
    echo "FALHOU: V2.6 host: arquivo novo em /var/crash: $new_crash"
    code=1
fi
exit "$code"
