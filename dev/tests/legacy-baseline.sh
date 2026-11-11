# shellcheck shell=bash
# The 0.1.4 side of the upgrade tests (Phase 57; m2:V5.7-V5.9), sourced by
# `dev/tests/install.sh` and `dev/tests/deb.sh` after `dev/services/common.sh` (git without
# the user's config, `dev/env.sh`). The includer defines ROOT.
#
# - Binaries and `.deb` of 0.1.4: out of git, in `.dev/baseline/` (Phase 3, m2:DT18), each
#   one checked against `.dev/baseline/SHA256SUMS` before use. Missing: the test stops and
#   says how to build it (the baseline section of docs/mission/handoffs/phase-57.md).
# - The 0.1.4 installer (`make install` of the tag): `git archive` of the tag v0.1.4.
# - The vault, the state and the protocol of 0.1.4 (fake values of D15) and the 0.1.4 names
#   of the install: `dev/fixtures/legacy/`, checked against its SHA256SUMS where listed.
#
# Every function exits the script (`exit 1`) on a problem; called inside `$(...)` the
# caller adds `|| exit 1`.

BASELINE="$ROOT/.dev/baseline"
LEGACY_FIXTURES="$ROOT/dev/fixtures/legacy"
LEGACY_TAG=v0.1.4

baseline_missing() {
    echo "linha de base 0.1.4: $*" >&2
    echo "  monte .dev/baseline/ como diz docs/mission/handoffs/phase-57.md (\"Linha de base\")" >&2
    exit 1
}

# baseline_file <path relative to .dev/baseline>: the absolute path, after the sha256.
baseline_file() {
    local rel="$1" sum
    [ -d "$BASELINE" ] && [ ! -L "$BASELINE" ] && [ -f "$BASELINE/SHA256SUMS" ] ||
        baseline_missing "falta $BASELINE/SHA256SUMS"
    sum="$(awk -v n="./$rel" '$2 == n {print $1; exit}' "$BASELINE/SHA256SUMS")"
    [[ "$sum" =~ ^[0-9a-f]{64}$ ]] || baseline_missing "$rel fora do $BASELINE/SHA256SUMS"
    [ -f "$BASELINE/$rel" ] && [ ! -L "$BASELINE/$rel" ] || baseline_missing "falta $BASELINE/$rel"
    [ "$(sha256sum -- "$BASELINE/$rel" | cut -d' ' -f1)" = "$sum" ] ||
        baseline_missing "$BASELINE/$rel não confere com o SHA256SUMS"
    printf '%s\n' "$BASELINE/$rel"
}

# legacy_name <key>: a 0.1.4 name of dev/fixtures/legacy/install-0.1.4.txt.
legacy_name() {
    local v
    v="$(awk -v k="$1" '$1 == k {print $2; exit}' "$LEGACY_FIXTURES/install-0.1.4.txt")"
    [[ "$v" =~ ^[A-Za-z0-9_./-]+$ ]] || { echo "falta '$1' em $LEGACY_FIXTURES/install-0.1.4.txt" >&2; exit 1; }
    printf '%s\n' "$v"
}

# stage_legacy_fixtures <dir>: copies (0644) the 0.1.4 vault, state and protocol, the 0.1.4
# names of the install and the v1 client into <dir>, for the read-only container mount.
stage_legacy_fixtures() {
    local dir="$1" f sum
    for f in vault-0.1.4.ward state-0.1.4 protocol-v1.jsonl; do
        sum="$(awk -v n="$f" '$2 == n {print $1; exit}' "$LEGACY_FIXTURES/SHA256SUMS")"
        [ "$(sha256sum -- "$LEGACY_FIXTURES/$f" | cut -d' ' -f1)" = "$sum" ] ||
            { echo "$LEGACY_FIXTURES/$f não confere com o SHA256SUMS da pasta" >&2; exit 1; }
    done
    install -d -m 0755 "$dir"
    install -m 0644 "$LEGACY_FIXTURES/vault-0.1.4.ward" "$LEGACY_FIXTURES/state-0.1.4" \
        "$LEGACY_FIXTURES/protocol-v1.jsonl" "$LEGACY_FIXTURES/install-0.1.4.txt" "$dir/"
    install -m 0644 "$ROOT/dev/tests/legacy_client.py" "$dir/"
}

# stage_legacy_installer <dir>: the 0.1.4 installer of the tag (its install dir and the icons
# its tree copies) in <dir>, read-only for the container.
stage_legacy_installer() {
    local dir="$1" script
    script="$(legacy_name script)" || exit 1
    git -C "$ROOT" rev-parse -q --verify "refs/tags/$LEGACY_TAG^{commit}" >/dev/null ||
        baseline_missing "falta a tag $LEGACY_TAG no repositório"
    install -d -m 0755 "$dir"
    git -C "$ROOT" archive --format=tar "$LEGACY_TAG" -- "${script%/*}" docs/design/logo | tar -x -C "$dir" -f -
    [ -f "$dir/$script" ] || { echo "o git archive de $LEGACY_TAG não trouxe $script" >&2; exit 1; }
    chmod -R u+rwX,go+rX,go-w -- "$dir"
}
