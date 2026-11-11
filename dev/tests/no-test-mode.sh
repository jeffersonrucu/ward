#!/bin/bash -p
# `make test-no-test-mode` (Phase 23a; D63, D7; V3.37). The exception of the test app
# (app role without the egid, `WARD_DEV_APP`; extension role without the egid,
# `WARD_DEV_EXTENSAO`, Phase 41) does not leak into the release:
#
# 1. no workspace member turns on `wardd/test-mode` or `ward-platform/test-mode`
#    through a normal or build dependency (`cargo tree -e normal,build`, without
#    `--all-features`); the check flags the case forced with `--features`, so it does
#    not pass by mistake;
# 2. the unit tests of `wardd` and `ward-platform` **without** the feature (the peer
#    without the egid opened by the launcher stays agent);
# 3. `strings` on the release `wardd` and on the release `ward-platform` library does
#    not find `WARD_DEV_APP` (nor `WARD_DEV_DIR`, nor the test app).
#
# Builds in target/no-test-mode (`make check` uses --all-features in target/).

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
cd "$ROOT"

CARGO="${CARGO:?o make passa o CARGO}"
TARGET_DIR="$ROOT/target/no-test-mode"
TMP="$(mktemp -d)"
trap 'rm -rf -- "$TMP"' EXIT
failed=0
# Lines `package vX (path)|features` of the Ward crates that have `test-mode` (or the
# old name `modo-teste`, m2:DT12: an undone rename is flagged too).
with_test_mode() {
    grep -E '^(wardd|ward-platform) v[^|]*\|.*(test-mode|modo-teste)' "$1" || true
}

echo "==> 1/3 cargo tree: ninguém liga test-mode em dependência normal"
"$CARGO" tree --workspace --locked -e normal,build -f '{p}|{f}' --prefix none > "$TMP/tree"
for package in ward wardd ward-nm ward-app-core ward-test-bridge; do
    "$CARGO" tree -p "$package" --locked -e normal,build -f '{p}|{f}' --prefix none >> "$TMP/tree"
done
finding="$(with_test_mode "$TMP/tree")"
if [ -n "$finding" ]; then
    echo "sem-modo-teste: test-mode ligado por dependência normal:" >&2
    echo "$finding" >&2
    failed=1
fi
# Self-test: with the feature forced, the check has to flag it.
"$CARGO" tree -p wardd --features test-mode --locked -e normal,build -f '{p}|{f}' --prefix none > "$TMP/forced"
if [ -z "$(with_test_mode "$TMP/forced")" ]; then
    echo "sem-modo-teste: a conferência do cargo tree não acusou o caso forçado" >&2
    failed=1
fi

echo "==> 2/3 testes de unidade sem a feature"
"$CARGO" test --locked --target-dir "$TARGET_DIR" -p wardd -p ward-platform --lib

echo "==> 3/3 strings no release"
"$CARGO" build --release --locked --target-dir "$TARGET_DIR" -p wardd --bin wardd
# Phase 40: the release extension host cannot have the test form either.
"$CARGO" build --release --locked --target-dir "$TARGET_DIR" -p ward-nm --bin ward-nm
"$CARGO" build --release --locked --target-dir "$TARGET_DIR" -p ward-platform --lib \
    --message-format=json > "$TMP/build.json"
RLIB="$(grep -o '"filenames":\["[^"]*/libward_platform[^"]*\.rlib"' "$TMP/build.json" \
    | sed -e 's/.*\["//' -e 's/"$//' | tail -n 1 || true)"
[ -f "$RLIB" ] || { echo "sem-modo-teste: rlib do ward-platform não achada" >&2; exit 1; }
# In the rlib, the metadata keeps the names of the items removed by `cfg` (for the
# "item configured out" diagnostic), without code: there only the texts are searched.
check_strings() {
    local binary="$1"
    shift
    # `strings` goes to a file before `grep -q` (SIGPIPE with `pipefail`).
    strings -a -n 4 -- "$binary" > "$TMP/strings.txt"
    for mark in "$@"; do
        if grep -qF -- "$mark" "$TMP/strings.txt"; then
            echo "sem-modo-teste: ${binary#"$ROOT"/} contém \"$mark\"" >&2
            failed=1
        fi
    done
}
# Phase 41: nor the exception of the test extension (`WARD_DEV_EXTENSAO`).
MARKS=(WARD_DEV_APP WARD_DEV_DIR WARD_DEV_SOCKET_DIR WARD_DEV_EXTENSAO wardd-app-teste wardd-aprovador-teste ward-ponte-teste)
check_strings "$RLIB" "${MARKS[@]}"
check_strings "$TARGET_DIR/release/ward-nm" "${MARKS[@]}"
check_strings "$TARGET_DIR/release/wardd" "${MARKS[@]}" classify_test_app classify_test_extension_host \
    TestBinary launch_for_test
# The checked binary really is the daemon (it has the system socket).
grep -qF -- '/run/ward' "$TMP/strings.txt" || {
    echo "sem-modo-teste: '/run/ward' não achado; o binário conferido é mesmo o wardd?" >&2
    failed=1
}

[ $failed -eq 0 ] && echo "sem-modo-teste: release sem WARD_DEV_APP nem a exceção do app de teste"
exit $failed
