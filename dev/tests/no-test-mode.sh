#!/bin/bash -p
# `make test-no-test-mode` (Phase 23a; D63, D7; V3.37). The exception of the test app
# (app role without the egid, `WARD_DEV_APP`; extension role without the egid, through
# the extension variable, Phase 41) does not leak into the release:
#
# 1. no workspace member turns on `wardd/test-mode` or `ward-platform/test-mode`
#    through a normal or build dependency (`cargo tree -e normal,build`, without
#    `--all-features`); the check flags the case forced with `--features`, so it does
#    not pass by mistake;
# 2. the unit tests of `wardd` and `ward-platform` **without** the feature (the peer
#    without the egid opened by the launcher stays agent);
# 3. `strings` on the release `wardd` and on the release `ward-platform` library does
#    not find `WARD_DEV_APP` (nor `WARD_DEV_DIR`, nor the test app), nor any other test
#    name, new or from 0.1.x (m2:DT12); each mark planted in a copy is flagged.
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
# old pt-BR name of the feature, m2:DT12: an undone rename is flagged too).
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
    echo "no-test-mode: test-mode ligado por dependência normal:" >&2
    echo "$finding" >&2
    failed=1
fi
# Self-test: with the feature forced, the check has to flag it.
"$CARGO" tree -p wardd --features test-mode --locked -e normal,build -f '{p}|{f}' --prefix none > "$TMP/forced"
if [ -z "$(with_test_mode "$TMP/forced")" ]; then
    echo "no-test-mode: a conferência do cargo tree não acusou o caso forçado" >&2
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
[ -f "$RLIB" ] || { echo "no-test-mode: rlib do ward-platform não achada" >&2; exit 1; }
# In the rlib, the metadata keeps the names of the items removed by `cfg` (for the
# "item configured out" diagnostic), without code: there only the texts are searched.
# `strings` goes to a file before `grep -q` (SIGPIPE with `pipefail`).
found_marks() {
    local binary="$1"
    shift
    strings -a -n 4 -- "$binary" > "$TMP/strings.txt"
    for mark in "$@"; do
        if grep -qF -- "$mark" "$TMP/strings.txt"; then
            echo "$mark"
        fi
    done
}
check_strings() {
    local binary="$1" mark
    shift
    while read -r mark; do
        [ -n "$mark" ] || continue
        echo "no-test-mode: ${binary#"$ROOT"/} contém \"$mark\"" >&2
        failed=1
    done < <(found_marks "$binary" "$@")
}
# Phase 41: nor the exception of the test extension (its variable). m2:DT12 (m2:Phase
# 56, m2:V5.5): every test name in the new form and in the 0.1.x one, so an undone
# rename is flagged too. The rlib only gets the names: its metadata keeps public
# constants (the test socket dir name is one).
MARKS=(WARD_DEV_APP WARD_DEV_DIR WARD_DEV_SOCKET_DIR WARD_DEV_EXTENSION WARD_DEV_EXTENSAO
    WARD_DEV_SYSTEM_DBUS WARD_DEV_DBUS_SISTEMA wardd-test-app wardd-app-teste
    wardd-test-approver wardd-aprovador-teste ward-test-bridge ward-ponte-teste)
# In the release binaries, any `WARD_DEV_*` and the test socket dir (new and old) too.
BINARY_MARKS=("${MARKS[@]}" WARD_DEV_ ward-test ward-teste)
check_strings "$RLIB" "${MARKS[@]}"
check_strings "$TARGET_DIR/release/ward-nm" "${BINARY_MARKS[@]}"
check_strings "$TARGET_DIR/release/wardd" "${BINARY_MARKS[@]}" classify_test_app \
    classify_test_extension_host TestBinary launch_for_test
# The checked binary really is the daemon (it has the system socket).
grep -qF -- '/run/ward' "$TMP/strings.txt" || {
    echo "no-test-mode: '/run/ward' não achado; o binário conferido é mesmo o wardd?" >&2
    failed=1
}
# Self-test (m2:V5.5): each mark planted in a copy of the release wardd is flagged, so
# a mark misspelled in the list does not pass by mistake.
for mark in "${BINARY_MARKS[@]}"; do
    cp -- "$TARGET_DIR/release/wardd" "$TMP/planted"
    printf '\n%s\n' "$mark" >>"$TMP/planted"
    if ! found_marks "$TMP/planted" "$mark" | grep -qxF -- "$mark"; then
        echo "no-test-mode: a conferência não acusou \"$mark\" plantado no wardd" >&2
        failed=1
    fi
done
rm -f -- "$TMP/planted"

[ $failed -eq 0 ] && echo "no-test-mode: release sem WARD_DEV_*, sem os nomes de teste (novos e antigos) nem a exceção do app de teste"
exit $failed
