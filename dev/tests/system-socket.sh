#!/bin/bash -p
# `make test-system-socket` (m2:Phase 54a; S1, S5, D46, D63). No test reaches the
# socket of the installed daemon (`/run/ward/<uid>/wardd.sock`).
#
# The finding of the Phase 51: `cargo test -p ward -p ward-nm -p ward-test-bridge`
# built the `ward-nm` of `crates/ward-nm/tests/binary.rs` without `test-mode` (feature
# unification only turned on `ward-platform/test-mode`), and that release host talked
# to the `wardd` installed on the machine. Here:
#
# 1. static: every test file that runs a binary of the workspace (`CARGO_BIN_EXE_`) has
#    the `test-mode` guard (`compile_error!` without the feature); self-test: a copy of
#    `binary.rs` without the guard is flagged;
# 2. without `test-mode`, the test targets that talk to a daemon do not build (the
#    combination of the finding included) and the error is the guard's (or, for the
#    `wardd` tests named with `--test`, cargo's `required-features` refusal);
# 3. builds every test binary that does build: the whole workspace with
#    `--all-features` (the set of `make check`) and the packages without the feature
#    (`ward-core`, `ward-platform`, `ward-app-core`, the libraries of `wardd` and
#    `ward-nm`, the set of `make test-no-test-mode`), and the release `ward-nm` (control);
# 4. in a disposable `ubuntu:24.04` container without network (the host's `/run` is not
#    there), root plants a fake **system socket**: `/run/ward/1000/` of root 0755,
#    socket 0600 of uid 1000 listened by a non-dumpable process of uid 0
#    (`dev/container-platform/fake.c`), so it passes every check of the release client
#    (D46). Each test binary runs as uid 1000 with a clean environment; the fake's record
#    has to stay empty. Then the control: the release `ward-nm` connects to the fake (the
#    plant is reachable, the record catches it).
#
# The tests that need what the container does not have (Chrome, dbus, ssh, the
# services) fail there: they are listed, but only the record decides. Never the real
# vault, the real daemon nor the host's `/run/ward`. Evidence (without any secret) in
# `.dev/logs/system-socket/`. Needs Docker and gcc; out of `make check`. `PER_BINARY=<s>`
# changes the time limit of each test binary (default 600).
# Usage: dev/tests/system-socket.sh (the make passes CARGO)

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"
cd "$ROOT"

CARGO="${CARGO:?o make passa o CARGO}"
# The same image (by digest) as `make test-docker-uid` and `make test-docker-platform`.
IMAGE="ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852"
PER_BINARY="${PER_BINARY:-600}"
case "$PER_BINARY" in '' | *[!0-9]*) echo "PER_BINARY não é um número" >&2; exit 1 ;; esac
# The text of the guard's `compile_error!` (the same in every guarded file).
MARK='este teste exige a feature test-mode'
# Not `require` of `common.sh`: this test does not need the services (`make dev-services`).
for p in docker gcc python3; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done
[ "$(id -u)" = 1000 ] || { echo "socket-do-sistema: o contêiner usa o uid 1000; rode como o usuário 1000" >&2; exit 1; }

mkdir -p -m 0700 "$ROOT/.dev/logs"
TMP="$(mktemp -d "$ROOT/.dev/system-socket.XXXXXX")"
trap 'rm -rf -- "$TMP"' EXIT
EVIDENCE="$ROOT/.dev/logs/system-socket"
rm -rf -- "$EVIDENCE"
mkdir -m 0700 -- "$EVIDENCE"
failed=0
ok() { echo "ok: $*"; }
fail() { echo "FALHOU: $*" >&2; failed=1; }

# Files (under the given folders) that run a workspace binary without the guard.
unguarded() {
    local f
    while IFS= read -r -d '' f; do
        grep -qF 'CARGO_BIN_EXE_' "$f" || continue
        if ! grep -qF '#[cfg(not(feature = "test-mode"))]' "$f" || ! grep -qF "$MARK" "$f"; then
            echo "${f#"$ROOT"/}"
        fi
    done < <(find "$@" -type f -name '*.rs' -print0)
}

echo "==> 1/4 guarda do test-mode em todo teste que roda binário do Ward"
finding="$(unguarded "$ROOT/crates" "$ROOT/app/core")"
if [ -n "$finding" ]; then
    fail "teste que roda binário do Ward sem a guarda do test-mode:"
    echo "$finding" >&2
else
    ok "todo arquivo com CARGO_BIN_EXE_ tem a guarda"
fi
# Self-test: the file of the finding without the guard has to be flagged.
mkdir -p "$TMP/self-test/tests"
grep -vF -e 'compile_error!' -e '#[cfg(not(feature = "test-mode"))]' -e "$MARK" \
    "$ROOT/crates/ward-nm/tests/binary.rs" > "$TMP/self-test/tests/binary.rs"
if [ -n "$(unguarded "$TMP/self-test")" ]; then
    ok "autoteste: a cópia sem a guarda é acusada"
else
    fail "autoteste: a cópia de binary.rs sem a guarda passou"
fi

echo "==> 2/4 sem test-mode, os testes que falam com um daemon não compilam"
# The combination of the Phase 51 finding first.
while IFS= read -r combo; do
    # shellcheck disable=SC2086
    if "$CARGO" test --locked --no-run $combo > "$TMP/build.log" 2>&1; then
        fail "'cargo test $combo' compilou sem test-mode"
    elif grep -qF "$MARK" "$TMP/build.log"; then
        ok "'cargo test $combo' sem test-mode para na guarda"
    elif [[ "$combo" == *"--test "* ]] \
        && grep -qF 'requires the features: `test-mode`' "$TMP/build.log"; then
        # The wardd tests that start the daemon also have `required-features` in
        # `crates/wardd/Cargo.toml`: cargo refuses them by name before compiling (and a
        # plain `cargo test -p wardd` skips them); the guard is the second layer.
        ok "'cargo test $combo' sem test-mode recusado pelo required-features"
    else
        fail "'cargo test $combo' falhou sem a mensagem da guarda (log em $EVIDENCE/guard.log)"
        cp -- "$TMP/build.log" "$EVIDENCE/guard.log"
    fi
done <<'EOF'
-p ward -p ward-nm -p ward-test-bridge
-p ward-nm
-p ward
-p ward-test-bridge
-p wardd --test socket
-p wardd --test lock
-p wardd --test run
-p wardd --test test_app
-p wardd --test app_value_binary
EOF

echo "==> 3/4 binários de teste (com --all-features e os que compilam sem a feature)"
"$CARGO" test --workspace --all-features --locked --no-run \
    --message-format=json-render-diagnostics > "$TMP/all.json"
"$CARGO" test --locked --no-run --message-format=json-render-diagnostics \
    -p ward-core -p ward-platform -p ward-app-core > "$TMP/plain.json"
"$CARGO" test --locked --no-run --message-format=json-render-diagnostics \
    -p wardd -p ward-nm --lib > "$TMP/plain-lib.json"
"$CARGO" build --quiet --locked -p ward-nm --bin ward-nm --target-dir target/no-test-mode
mkdir -m 0755 "$TMP/opt"
install -m 0755 target/no-test-mode/debug/ward-nm "$TMP/opt/ward-nm"
gcc -O2 -Wall -Werror -o "$TMP/opt/fake" "$ROOT/dev/container-platform/fake.c"
chmod 0755 "$TMP/opt/fake"
# "<package folder>\t<binary>" of each test binary (profile test), without repeats.
python3 -I - "$TMP/all.json" "$TMP/plain.json" "$TMP/plain-lib.json" > "$TMP/opt/binaries" <<'EOF'
import json
import os
import sys

seen = set()
for name in sys.argv[1:]:
    with open(name, encoding="utf-8") as f:
        for line in f:
            if not line.startswith("{"):
                continue
            m = json.loads(line)
            exe = m.get("executable")
            if m.get("reason") != "compiler-artifact" or not exe or not m["profile"]["test"]:
                continue
            if exe in seen:
                continue
            seen.add(exe)
            print(f"{os.path.dirname(m['manifest_path'])}\t{exe}")
EOF
chmod 0644 "$TMP/opt/binaries"
count="$(wc -l < "$TMP/opt/binaries")"
[ "$count" -gt 10 ] || fail "poucos binários de teste ($count)"
ok "$count binários de teste"

echo "==> 4/4 no contêiner, com um socket do sistema falso plantado"
mkdir -m 0755 "$TMP/out" "$TMP/out/logs"
docker image inspect "$IMAGE" >/dev/null 2>&1 || docker pull --quiet "$IMAGE" >/dev/null
# The worktree at its own path (the binaries have absolute paths in them); nothing of
# the host's home, `/run` or `/etc`. Limits: the tests run in a burst of processes.
rc=0
docker run --rm --network none --pull never \
    --pids-limit 2048 --memory 5g --cpus 4 \
    -e PER_BINARY="$PER_BINARY" \
    -v "$ROOT:$ROOT" \
    -v "$TMP/opt:/opt/teste:ro" \
    -v "$TMP/out:/opt/out" \
    -v "$ROOT/dev/container-system-socket/inside.sh:/opt/inside.sh:ro" \
    "$IMAGE" /bin/bash -p /opt/inside.sh | tee "$EVIDENCE/inside.log" || rc=$?
cp -r -- "$TMP/out/logs" "$EVIDENCE/tests" 2>/dev/null || true
[ "$rc" -eq 0 ] || fail "parte de dentro do contêiner (saída $rc)"

if [ $failed -eq 0 ]; then
    echo "socket-do-sistema: nenhum teste tocou no socket do sistema falso"
fi
exit $failed
