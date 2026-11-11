#!/bin/bash -p
# Attack tests of the Foundation (Phase 11). Every attack has to FAIL, that is, be caught
# by `make check`. Covers V1.2 (S15), V1.3 (S14) and V1.5 (S15), plus the findings of the
# M1 audit (Phase 12a): extra scan without a path allowlist, cargo origins
# (source/patch/registries/path outside) and the tools' binary/variables; and of the
# 2nd audit (Phase 12b): origins as an allowlist and pre-check (alias, rustc-wrapper,
# linker, rustflags, build.rustc, runner, [env], RUSTC*/CARGO_* variables), the
# CARGO_HOME config, toolchain, the user's PATH/MAKEFLAGS/BASH_ENV and git without the
# user's config.
#
# Only uses material generated on the spot in a temporary directory: a new ed25519 key, a
# random secret, a test crate. Nothing of the user is read. What is created in the
# repository (`leak.txt`) is removed on exit, even on error.
#
# Usage: dev/tests/attacks-foundation.sh   (or `make test-attacks-foundation`)

set -euo pipefail

# Fixed PATH before any command (Phase 12b): nothing from the user's PATH runs here.
export PATH=/usr/bin:/bin
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
# The rustup cargo (there is no cargo in /usr/bin), at the end of the PATH.
export PATH="$PATH:$PASSWD_HOME/.cargo/bin"
# gitleaks is always the one in `.dev/` (not an inherited variable), verified by sha256.
GITLEAKS="$ROOT/.dev/tools/bin/gitleaks"
(cd "$ROOT" && make -s gitleaks-binary)
# Phase 28d: the copies left `/tmp`. The `cargo test` of the copy starts the test daemon
# with `WARD_DEV_APP`, and `TestBinary` refuses (on purpose) an app under a dir writable by
# others, such as `/tmp` (1777): in a copy under `/tmp` that test always fails.
# Phase 28f: and they also left the repository. In `.dev/`, the `.cargo/config*` planted in
# the copies were found by the root's `dev/origins.py` (which scans the whole repository,
# on purpose): during the run every `make` of the checkout failed in `origins`, and V1.2
# itself (`make check` at the root) exited 2 without naming `leak.txt`. `origins.py` still
# has no exception for `.dev/`; the copies go to `~/.cache/` (passwd home), in a 0700
# `mktemp` dir, and the chain of dirs up to `/` is verified first (only the user's or
# root's, no group/other write, no link).
private_dir() {
    local p="$1" owner mode
    [ "$(realpath -e -- "$p")" = "$p" ] || { echo "pasta das cópias: $p é link ou não existe" >&2; exit 1; }
    while :; do
        read -r owner mode < <(stat -c '%u %a' -- "$p")
        if { [ "$owner" != "$(id -u)" ] && [ "$owner" != 0 ]; } || (( 8#$mode & 8#022 )); then
            echo "pasta das cópias: $p é de outro dono ou gravável por outros" >&2
            exit 1
        fi
        [ "$p" = / ] && break
        p="$(dirname -- "$p")"
    done
}
COPIES_CACHE="$PASSWD_HOME/.cache"
[ -L "$COPIES_CACHE" ] && { echo "pasta das cópias: $COPIES_CACHE é link" >&2; exit 1; }
mkdir -p -m 0700 "$COPIES_CACHE"
private_dir "$COPIES_CACHE"
TMP="$(mktemp -d "$COPIES_CACHE/ward-attacks-foundation.XXXXXX")"
private_dir "$TMP"
case "$TMP/" in
    "$ROOT"/*) echo "pasta das cópias dentro do repositório: $TMP" >&2; exit 1 ;;
esac
LEAK="$ROOT/leak.txt"
cleanup() { rm -rf "$TMP"; rm -f "$LEAK"; }
trap cleanup EXIT

failures=0
ok() { printf 'ok    %s\n' "$1"; }
failed() { printf 'FALHA %s\n' "$1" >&2; failures=$((failures + 1)); }

# Copies the files git tracks (`git ls-files`, with the working tree content), without
# `.dev/`, `target/` or `node_modules/`, which git ignores. Phase 28d: it used to copy only
# `crates/` and a few loose files, but the workspace has a member in `app/` (`app/core`)
# and the tests read `dev/fixtures/` through `include_str!`; the copy broke in clippy before
# the attacks. With git's list, whatever the workspace references comes along.
# gitleaks goes into the copy's `.dev/`: the Makefile only uses its own (`GITLEAKS :=`)
# and verifies the binary's sha256.
copy_workspace() {
    local dest="$1"
    mkdir -p "$dest/.dev/tools/bin"
    (cd "$ROOT" && git ls-files -z --cached) \
        | (cd "$ROOT" && tar --null --no-recursion -T - -cf -) \
        | tar -C "$dest" -xf -
    # An empty git repository in the copy cuts the discovery: the copy's `git`/`gitleaks git`
    # never sees a repository above (the home may be one; before 28f it was Ward itself).
    (cd "$dest" && git init -q)
    local essential
    for essential in Cargo.toml Cargo.lock Makefile dev/origins.py app/core/Cargo.toml \
        dev/fixtures/entries.json; do
        [ -f "$dest/$essential" ] || { echo "copy_workspace: falta $essential" >&2; exit 1; }
    done
    cp "$GITLEAKS" "$dest/.dev/tools/bin/gitleaks"
}

# Adds a test crate `crates/attack` with the given dependency.
add_attack_crate() {
    local dest="$1" dependency="$2"
    mkdir -p "$dest/crates/attack/src"
    cat >"$dest/crates/attack/Cargo.toml" <<EOF
[package]
name = "attack"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
$dependency
EOF
    printf '//! Test crate of the dependency attack.\n' >"$dest/crates/attack/src/lib.rs"
    sed -i 's|^    "crates/ward-test-bridge",$|    "crates/ward-test-bridge",\n    "crates/attack",|' \
        "$dest/Cargo.toml"
    grep -q '"crates/attack"' "$dest/Cargo.toml"
    (cd "$dest" && cargo generate-lockfile --quiet)
}

# --- V1.2 (S15): OpenSSH private key and AWS secret in a new file --------------------
v1_2() {
    ssh-keygen -q -t ed25519 -N '' -C 'ward-test-leak' -f "$TMP/key" </dev/null
    local secret
    secret="$(head -c 96 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 40)"
    [ "${#secret}" -eq 40 ]
    {
        cat "$TMP/key"
        printf 'aws_secret_access_key = %s\n' "$secret"
    } >"$LEAK"
    rm -f "$TMP/key" "$TMP/key.pub"

    local output rc=0
    output="$(cd "$ROOT" && make check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q 'leak.txt' <<<"$output"; then
        ok "V1.2 make check sai $rc e aponta leak.txt"
    else
        failed "V1.2 make check não pegou leak.txt (exit $rc)"
    fi
    # The report must not repeat the secret (--redact).
    if grep -qF "$secret" <<<"$output"; then
        failed "V1.2 a saída do make check repete o segredo"
    else
        ok "V1.2 a saída do make check não repete o segredo"
    fi

    rm -f "$LEAK"
    if (cd "$ROOT" && make check >"$TMP/v1_2-clean.log" 2>&1); then
        ok "V1.2 sem o arquivo, make check volta a 0"
    else
        failed "V1.2 sem o arquivo, make check continua falhando (ver $TMP/v1_2-clean.log)"
        tail -20 "$TMP/v1_2-clean.log" >&2
    fi
}

# --- the D15 allowlist matches the whole value: "falso-…" + a real secret is found ----
anchored_allowlist() {
    local dir="$TMP/anchor"
    copy_workspace "$dir"
    printf 'api_secret = "falso-WP-senha-01%s"\n' \
        "$(head -c 96 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24)" >"$dir/bypass.txt"
    local rc=0
    (cd "$dir" && "$GITLEAKS" dir --config dev/gitleaks-tree.toml --redact --no-banner \
        --exit-code 1 . >/dev/null 2>&1) || rc=$?
    if [ "$rc" -ne 0 ]; then
        ok "allowlist: valor falso com sufixo ainda é achado"
    else
        failed "allowlist: 'falso-WP-senha-01<segredo>' passou pelo gitleaks"
    fi
}

# --- a `.dev/` file forced into the index (git add -f) is found -----------------------
dev_forced_into_index() {
    local dir="$TMP/index"
    copy_workspace "$dir"
    mkdir -p "$dir/.dev"
    ssh-keygen -q -t ed25519 -N '' -C 'ward-test-index' -f "$dir/.dev/key" </dev/null
    (cd "$dir" && git init -q && git add -f .dev/key)
    local output rc=0
    output="$(cd "$dir" && make gitleaks 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '.dev/key' <<<"$output"; then
        ok "índice: chave em .dev/ forçada com git add -f é achada"
    else
        failed "índice: chave forçada em .dev/ passou pelo gitleaks (exit $rc)"
    fi
}

# --- V1.3 (S14): dependency with a known advisory -------------------------------------
v1_3_advisory() {
    local dir="$TMP/advisory"
    copy_workspace "$dir"
    add_attack_crate "$dir" 'time = "=0.1.43"'
    local output rc=0
    output="$(cd "$dir" && make check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '==> 4/6 cargo audit' <<<"$output" \
        && grep -q 'RUSTSEC-2020-0071' <<<"$output"; then
        ok "V1.3 make check falha em cargo audit com RUSTSEC-2020-0071"
    else
        failed "V1.3 make check não falhou em cargo audit pelo advisory (exit $rc)"
        tail -30 <<<"$output" >&2
    fi
    # Second barrier: cargo deny alone also refuses.
    rc=0
    output="$(cd "$dir" && make deny 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q 'RUSTSEC-2020-0071' <<<"$output"; then
        ok "V1.3 cargo deny também recusa RUSTSEC-2020-0071"
    else
        failed "V1.3 cargo deny aceitou o advisory (exit $rc)"
    fi
}

# --- V1.3 (S14): dependency from a git source that is not allowed ---------------------
v1_3_git() {
    local repo="$TMP/repo-git"
    mkdir -p "$repo/src"
    cat >"$repo/Cargo.toml" <<'EOF'
[package]
name = "fake-git"
version = "0.1.0"
edition = "2024"
license = "MIT"
EOF
    printf '//! Fake crate served over git.\n' >"$repo/src/lib.rs"
    (cd "$repo" && git init -q && git add -A \
        && git -c user.name=test -c user.email=test@test.invalid commit -q -m test)

    local dir="$TMP/git"
    copy_workspace "$dir"
    add_attack_crate "$dir" "fake-git = { git = \"file://$repo\" }"
    local output rc=0
    output="$(cd "$dir" && make check 2>&1)" || rc=$?
    # Phase 12b: the git origin is already refused in the pre-check (before clippy compiles
    # the crate, with build.rs, in step 2); cargo deny keeps refusing it too.
    if [ "$rc" -ne 0 ] && grep -q '==> 0/6 origens' <<<"$output" && ! grep -q '==> 1/6' <<<"$output" \
        && grep -q "origens: pacote fake-git: fonte fora do crates.io" <<<"$output"; then
        ok "V1.3 make check recusa a origem git na pré-checagem, antes do passo 1"
    else
        failed "V1.3 make check não recusou a dependência de git antes do passo 1 (exit $rc)"
        tail -30 <<<"$output" >&2
    fi
    rc=0
    output="$(cd "$dir" && "$PASSWD_HOME/.cargo/bin/cargo-deny" --all-features check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q "source-not-allowed" <<<"$output" && grep -q "fake-git" <<<"$output"; then
        ok "V1.3 cargo deny também recusa a origem git (source-not-allowed)"
    else
        failed "V1.3 cargo deny aceitou a dependência de git (exit $rc)"
    fi
}

# --- V1.3 (S14): Cargo.lock exists and is not ignored by git --------------------------
v1_3_lockfile() {
    if [ -f "$ROOT/Cargo.lock" ] && ! (cd "$ROOT" && git check-ignore -q Cargo.lock); then
        ok "V1.3 Cargo.lock existe e não é ignorado"
    else
        failed "V1.3 Cargo.lock ausente ou ignorado pelo git"
    fi
}

# --- V1.5 (S15): .dev/ ignored; Makefile and scripts do not name the user's dirs -------
v1_5() {
    local target
    for target in .dev/anything .dev/test-vault.ward .dev/tools/bin/gitleaks; do
        if (cd "$ROOT" && git check-ignore -q "$target"); then
            ok "V1.5 $target é ignorado"
        else
            failed "V1.5 $target NÃO é ignorado pelo git"
        fi
    done

    if forbidden_dirs_scan "$ROOT" >&2; then
        ok "V1.5 Makefile e dev/ não citam pasta proibida"
    else
        failed "V1.5 Makefile ou dev/ cita pasta proibida (linhas acima)"
    fi
    v1_5_baseline_controls
}

# Patterns built in pieces so that this file does not match itself.
FORBIDDEN_DIR_PATTERNS=(
    '\.local/share/w''ard'
    '\.s''sh(/|\b)'
    '\.a''ws(/|\b)'
    'keepass''xc'
    'ssh_con''fig'
    'SSH_AUTH''_SOCK'
)

# Phase 47a: the 0.1.4 baseline (Phase 3) in `dev/fixtures/legacy/` holds literals of the old
# binary (`strings` and `--help`) that name the real vault path, the SSH agent variable and
# the ssh dir. They are recorded output, not access, so they are skipped; but only exactly:
# a file directly listed in `dev/fixtures/legacy/SHA256SUMS` whose content still has that
# sha256. Anything else in that dir (new file, edited file, file in a subdir not listed), a
# sibling like `dev/fixtures/legacy-x/` and the rest of `dev/` are still scanned.
# Prints the findings (`grep -n`) and returns 1 if there is any.
forbidden_dirs_scan() {
    local root="$1" legacy="$1/dev/fixtures/legacy"
    local -A baseline=()
    local sum name
    if [ -f "$legacy/SHA256SUMS" ] && [ ! -L "$legacy" ] && [ ! -L "$legacy/SHA256SUMS" ]; then
        while read -r sum name; do
            # Only a plain relative name: no `..`, no absolute path, no empty part.
            [[ "$sum" =~ ^[0-9a-f]{64}$ ]] || continue
            [[ "$name" =~ ^[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*$ ]] || continue
            [[ "/$name/" == */../* || "/$name/" == */./* ]] && continue
            baseline["$legacy/$name"]="$sum"
        done <"$legacy/SHA256SUMS"
    fi
    local files=("$root/Makefile") f
    while IFS= read -r -d '' f; do
        if [ -n "${baseline[$f]:-}" ] && [ ! -L "$f" ] \
            && [ "$(sha256sum -- "$f" | cut -d' ' -f1)" = "${baseline[$f]}" ]; then
            continue
        fi
        files+=("$f")
    done < <(find "$root/dev" -type f -print0)
    local p found=0
    for p in "${FORBIDDEN_DIR_PATTERNS[@]}"; do
        if grep -nEi -- "$p" "${files[@]}"; then
            found=1
        fi
    done
    [ "$found" -eq 0 ]
}

# Phase 47a: the exclusion above does not hide the same pattern anywhere else. In a copy of
# the tracked files (no build), one plant per case; every one has to be found.
v1_5_baseline_controls() {
    local base="$TMP/v1_5-base" dir rc
    copy_workspace "$base"
    # The line planted: the real vault path, built in pieces like the patterns.
    local line='vault = ~/.local/share/w''ard/vault.ward'

    # Control: the baseline really has the patterns (otherwise the exclusion proves nothing).
    rc=0
    grep -qEi -- "${FORBIDDEN_DIR_PATTERNS[0]}|${FORBIDDEN_DIR_PATTERNS[5]}" \
        "$base/dev/fixtures/legacy/strings-0.1.4.txt" || rc=$?
    if [ "$rc" -eq 0 ]; then
        ok "47a controle: a linha de base 0.1.4 tem os padrões (a exclusão tem efeito)"
    else
        failed "47a controle: a linha de base 0.1.4 não tem os padrões (exclusão sem efeito)"
    fi
    # Control: the untouched copy is clean, like the repository.
    if forbidden_dirs_scan "$base" >/dev/null; then
        ok "47a controle: cópia sem mudança limpa (só a linha de base é pulada)"
    else
        failed "47a controle: cópia sem mudança acusada"
    fi

    # Each plant on a fresh copy of the base; the scan has to find it.
    plant_found() {
        local what="$1" rel="$2" mode="$3"
        dir="$TMP/v1_5-plant"; rm -rf "$dir"; cp -a "$base" "$dir"
        mkdir -p "$(dirname -- "$dir/$rel")"
        case "$mode" in
            new) printf '%s\n' "$line" >"$dir/$rel" ;;
            append) printf '%s\n' "$line" >>"$dir/$rel" ;;
        esac
        if forbidden_dirs_scan "$dir" >/dev/null; then
            failed "47a padrão plantado em $what não foi acusado"
        else
            ok "47a padrão plantado em $what é acusado"
        fi
    }
    plant_found "dev/ fora da linha de base" dev/tests/plant-47a.sh new
    plant_found "dev/fixtures/ (vizinho de legacy/)" dev/fixtures/plant-47a.txt new
    plant_found "pasta com prefixo legacy (dev/fixtures/legacy-x/)" dev/fixtures/legacy-x/help-0.1.4.txt new
    plant_found "pasta legacy/ aninhada (dev/tests/fixtures/legacy/)" dev/tests/fixtures/legacy/strings-0.1.4.txt new
    plant_found "arquivo novo em legacy/ (fora do SHA256SUMS)" dev/fixtures/legacy/notes.txt new
    plant_found "subpasta de legacy/ fora do SHA256SUMS" dev/fixtures/legacy/backup-0.1.4/notes.txt new
    plant_found "arquivo da linha de base alterado (sha256 não bate)" dev/fixtures/legacy/help-0.1.4.txt append
    plant_found "Makefile" Makefile append
    # A changed baseline file with its SHA256SUMS line updated too would be skipped here;
    # catching that is V1.6 (SHA256SUMS against the Phase 3 hashes), not this scan. What this
    # scan guarantees is that SHA256SUMS never reaches outside legacy/: a `../` name is ignored.
    dir="$TMP/v1_5-plant"; rm -rf "$dir"; cp -a "$base" "$dir"
    printf '%s\n' "$line" >"$dir/dev/tests/plant-47a.sh"
    printf '%s  ../../tests/plant-47a.sh\n' "$(sha256sum -- "$dir/dev/tests/plant-47a.sh" | cut -d' ' -f1)" \
        >>"$dir/dev/fixtures/legacy/SHA256SUMS"
    if forbidden_dirs_scan "$dir" >/dev/null; then
        failed "47a SHA256SUMS com ../ tirou da varredura um arquivo fora de legacy/"
    else
        ok "47a SHA256SUMS com ../ não tira da varredura um arquivo fora de legacy/"
    fi
    rm -rf "$base" "$TMP/v1_5-plant"
}

# --- Phase 12a (S15): key/AWS in a path the global gitleaks allowlist skips -------------
# The default gitleaks rules ignore lockfiles, images, svg, pdf, fonts, vendor and
# `gitleaks.toml`, and do not open compressed files. The extra scan has to find it in all
# of them, in the tree and in the index.
extra_without_path_allowlist() {
    local key secret akid
    ssh-keygen -q -t ed25519 -N '' -C 'ward-test-extra' -f "$TMP/key-extra" </dev/null
    key="$(cat "$TMP/key-extra")"
    rm -f "$TMP/key-extra" "$TMP/key-extra.pub"
    secret="$(head -c 96 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 40)"
    akid="AKIA$(head -c 96 /dev/urandom | base64 | tr -dc 'A-Z2-7' | head -c 16)"
    local base="$TMP/extra-base"
    copy_workspace "$base"
    local f kind content dir rc output
    for kind in chave aws id; do
        case "$kind" in
            chave) content="$key" ;;
            aws) content="aws_secret_access_key = $secret" ;;
            id) content="aws_access_key_id = $akid" ;;
        esac
        for f in app/package-lock.json app/pnpm-lock.yaml Cargo.lock.bak go.sum gitleaks.toml \
            docs/.gitleaks.toml docs/icon.svg docs/photo.png docs/manual.pdf docs/font.woff2 \
            vendor/x/README docs/package.txt.gz; do
            dir="$TMP/extra"
            rm -rf "$dir"
            cp -a "$base" "$dir"
            mkdir -p "$(dirname "$dir/$f")"
            if [[ "$f" == *.gz ]]; then
                printf '%s\n' "$content" | gzip >"$dir/$f"
            else
                printf '%s\n' "$content" >"$dir/$f"
            fi
            rc=0
            output="$(cd "$dir" && make gitleaks 2>&1)" || rc=$?
            if [ "$rc" -eq 0 ] || ! grep -q "$f" <<<"$output"; then
                failed "extra: $kind em $f passou pela árvore (exit $rc)"
            elif grep -qF -e "$secret" -e "$akid" <<<"$output"; then
                failed "extra: a saída repete o segredo ($f)"
            else
                ok "extra: $kind em $f é achado na árvore"
            fi
            # Index: the same file forced with git add -f.
            (cd "$dir" && git init -q && git add -f "$f")
            rc=0
            (cd "$dir" && "$GITLEAKS" git --staged --pre-commit --config dev/gitleaks-extra.toml \
                --redact --no-banner --max-archive-depth 3 --exit-code 1 . >/dev/null 2>&1) || rc=$?
            if [ "$rc" -ne 0 ]; then ok "extra: $kind em $f é achado no índice"; else failed "extra: $kind em $f passou pelo índice"; fi
        done
    done
    rm -rf "$TMP/extra" "$base"
}

# --- Phase 12a (S14): cargo origins that cargo deny does not see ------------------------
# refused_origin <description> <copy dir>: `make origins` has to fail.
refused_origin() {
    local rc=0 output
    output="$(cd "$2" && env "${@:3}" make origins 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '^origens: ' <<<"$output"; then
        ok "origens: $1 é recusado"
    else
        failed "origens: $1 passou (exit $rc)"
    fi
}
cargo_origins() {
    local dir
    # Source replacement by a vendored directory (the audit's attack). Here through the
    # whole `make check`: it has to fail in the pre-check, before step 1.
    dir="$TMP/vendor"
    copy_workspace "$dir"
    mkdir -p "$dir/.cargo" "$dir/vendor"
    printf '[source.crates-io]\nreplace-with = "v"\n\n[source.v]\ndirectory = "vendor"\n' >"$dir/.cargo/config.toml"
    local output rc=0
    output="$(cd "$dir" && make check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '==> 0/6 origens' <<<"$output" && ! grep -q '==> 1/6' <<<"$output" \
        && grep -q '.cargo/config.toml: .cargo/config\* no repositório é proibido' <<<"$output"; then
        ok "origens: make check falha na pré-checagem (antes do passo 1) com replace-with para vendor/"
    else
        failed "origens: make check aceitou a substituição de fonte (exit $rc)"
        tail -20 <<<"$output" >&2
    fi
    # Dotted key (no `[source` table header).
    dir="$TMP/dotted"; copy_workspace "$dir"; mkdir -p "$dir/.cargo"
    printf 'source.crates-io.replace-with = "v"\nsource.v.directory = "vendor"\n' >"$dir/.cargo/config.toml"
    refused_origin "source em chave pontilhada" "$dir"
    # `.cargo/config` without extension, with [patch].
    dir="$TMP/old-config"; copy_workspace "$dir"; mkdir -p "$dir/.cargo"
    printf '[patch.crates-io]\nitoa = { path = "x" }\n' >"$dir/.cargo/config"
    refused_origin ".cargo/config com [patch]" "$dir"
    # Alternative registry.
    dir="$TMP/registry"; copy_workspace "$dir"; mkdir -p "$dir/.cargo"
    printf '[registries.mau]\nindex = "sparse+https://example.invalid/index/"\n' >"$dir/.cargo/config.toml"
    refused_origin "[registries]" "$dir"
    # Config in a parent directory of the root (cargo reads it too).
    mkdir -p "$TMP/parent/.cargo"
    printf '[source.crates-io]\nreplace-with = "v"\n[source.v]\ndirectory = "/does/not/exist"\n' >"$TMP/parent/.cargo/config.toml"
    dir="$TMP/parent/child"; copy_workspace "$dir"
    refused_origin "source num .cargo/config.toml de diretório pai" "$dir"
    # Invalid TOML does not pass silently.
    dir="$TMP/invalid"; copy_workspace "$dir"; mkdir -p "$dir/.cargo"
    printf '[source\n' >"$dir/.cargo/config.toml"
    refused_origin ".cargo/config.toml inválido" "$dir"
    # Environment variable.
    dir="$TMP/environment"; copy_workspace "$dir"
    refused_origin "CARGO_REGISTRIES_MAU_INDEX no ambiente" "$dir" CARGO_REGISTRIES_MAU_INDEX=sparse+https://example.invalid/
    # [patch] and [replace] in the workspace Cargo.toml.
    dir="$TMP/patch"; copy_workspace "$dir"
    printf '\n[patch.crates-io]\nward-fake = { path = "crates/ward-core" }\n' >>"$dir/Cargo.toml"
    refused_origin "[patch] no Cargo.toml" "$dir"
    dir="$TMP/replace"; copy_workspace "$dir"
    printf '\n[replace]\n"itoa:1.0.15" = { path = "crates/ward-core" }\n' >>"$dir/Cargo.toml"
    refused_origin "[replace] no Cargo.toml" "$dir"
    # Path dependency outside the repository.
    mkdir -p "$TMP/outside/src"
    printf '[package]\nname = "outside"\nversion = "0.1.0"\nedition = "2024"\nlicense = "MIT"\n' >"$TMP/outside/Cargo.toml"
    printf '//! Crate outside the repository.\n' >"$TMP/outside/src/lib.rs"
    dir="$TMP/path-outside"; copy_workspace "$dir"
    add_attack_crate "$dir" "outside = { path = \"$TMP/outside\" }"
    refused_origin "dependência por path fora do repositório" "$dir"
    # Relative path that leaves the root through `..`.
    dir="$TMP/path-rel"; copy_workspace "$dir"
    add_attack_crate "$dir" 'outside = { path = "../../../outside" }'
    refused_origin "path relativo com .. para fora do repositório" "$dir"
    # Control: the clean copy passes.
    dir="$TMP/clean"; copy_workspace "$dir"
    if (cd "$dir" && make origins >/dev/null 2>&1); then ok "origens: cópia limpa passa"; else failed "origens: cópia limpa recusada"; fi
}

# --- Phase 12a: swapped tool (gitleaks binary and environment variables) -----------------
fixed_tools() {
    local dir="$TMP/tools" rc output
    copy_workspace "$dir"
    ssh-keygen -q -t ed25519 -N '' -C 'ward-test-tools' -f "$dir/leaked" </dev/null
    rm -f "$dir/leaked.pub"
    # GITLEAKS in the environment does not swap the Makefile's gitleaks: the leak is still found.
    rc=0
    (cd "$dir" && GITLEAKS=/bin/true make gitleaks >/dev/null 2>&1) || rc=$?
    if [ "$rc" -ne 0 ]; then ok "GITLEAKS=/bin/true no ambiente não desliga o gitleaks"; else failed "GITLEAKS do ambiente trocou o gitleaks"; fi
    # CARGO in the environment does not swap cargo: with CARGO=false fmt-check still runs and passes.
    if (cd "$dir" && CARGO=false make fmt-check >/dev/null 2>&1); then ok "CARGO=false no ambiente não troca o cargo"; else failed "CARGO do ambiente trocou o cargo"; fi
    # Tampered gitleaks binary in .dev/: refused before scanning.
    rm -f "$dir/leaked"
    printf '#!/bin/sh\nexit 0\n' >"$dir/.dev/tools/bin/gitleaks"
    rc=0
    output="$(cd "$dir" && make gitleaks 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q 'não é o gitleaks' <<<"$output"; then
        ok "gitleaks trocado em .dev/ é recusado pelo sha256"
    else
        failed "gitleaks trocado em .dev/ rodou (exit $rc)"
    fi
    cp "$GITLEAKS" "$dir/.dev/tools/bin/gitleaks"
    printf 'x' >>"$dir/.dev/tools/bin/gitleaks"
    rc=0
    (cd "$dir" && make gitleaks >/dev/null 2>&1) || rc=$?
    if [ "$rc" -ne 0 ]; then ok "gitleaks com um byte a mais é recusado"; else failed "gitleaks com um byte a mais rodou"; fi
}

# --- Phase 12b (S14): origins as an allowlist, before step 1 ---------------------------------
# The vectors of the 2nd audit (scratchpad/r2: v13-alias.sh, ataque-config.sh, wrap.sh): a
# `.cargo/config.toml` or an environment variable that swaps what cargo runs. Each one
# runs the whole `make check` in a copy and has to fail in the `0/6` pre-check, before
# step 1, without the planted program running (the mark must not show up).
MARKS="$TMP/marks"
prepare_cargo_decoys() {
    mkdir -p "$TMP/decoys"
    # The audit's wrap.sh: swaps the ward-core source that rustc compiles.
    cat >"$TMP/decoys/wrap.sh" <<EOF
#!/bin/bash
echo "wrapper \$*" | head -c 80 >>"$MARKS"; echo >>"$MARKS"
args=()
for a in "\$@"; do
    if [[ "\$a" == *crates/ward-core/src/lib.rs ]]; then
        printf '//! tampered\npub fn back_door() -> u8 { 42 }\n' >"$TMP/decoys/lib.rs"; a="$TMP/decoys/lib.rs"
    fi
    args+=("\$a")
done
exec "\${args[@]}"
EOF
    printf '#!/bin/bash\necho rustc >>"%s"\nexec "%s/.cargo/bin/rustc" "$@"\n' "$MARKS" "$PASSWD_HOME" >"$TMP/decoys/rustc.sh"
    printf '#!/bin/bash\necho linker >>"%s"\nexec cc "$@"\n' "$MARKS" >"$TMP/decoys/linker.sh"
    printf '#!/bin/bash\necho program >>"%s"\nexit 0\n' "$MARKS" >"$TMP/decoys/program.sh"
    chmod +x "$TMP/decoys/"*.sh
    # fake-ok of v13-alias.sh: an "audit"/"deny" that only says everything is clean.
    mkdir -p "$TMP/fake-ok/src"
    printf '[package]\nname = "fake-ok"\nversion = "0.1.0"\nedition = "2021"\n[workspace]\n' >"$TMP/fake-ok/Cargo.toml"
    printf 'fn main() { println!("audit/deny falsos: tudo limpo"); }\n' >"$TMP/fake-ok/src/main.rs"
}

# blocked_before <description> <copy> [VAR=value...]: `make check` exits ≠ 0 in the
# pre-check, before step 1, and nothing planted ran.
blocked_before() {
    local d="$1" dir="$2" rc=0 output
    shift 2
    rm -f "$MARKS"
    output="$(cd "$dir" && env "$@" make check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '==> 0/6 origens' <<<"$output" && ! grep -q '==> 1/6' <<<"$output" \
        && grep -q '^origens: ' <<<"$output" && [ ! -e "$MARKS" ]; then
        ok "pré-checagem: $d é barrado antes do passo 1"
    else
        failed "pré-checagem: $d passou ou rodou o programa plantado (exit $rc, marca: $([ -e "$MARKS" ] && echo sim || echo não))"
        grep -E '==>|^origens' <<<"$output" | tail -5 >&2
    fi
}

# config_in_copy <name> <content> [relative file]: a copy with that config.
config_in_copy() {
    local dir="$TMP/cfg-$1"
    rm -rf "$dir"
    copy_workspace "$dir"
    local file="${3:-.cargo/config.toml}"
    mkdir -p "$(dirname "$dir/$file")"
    printf '%s\n' "$2" >"$dir/$file"
    printf '%s' "$dir"
}

origins_allowlist() {
    prepare_cargo_decoys
    local dir triple="x86_64-unknown-linux-gnu"
    # v13-alias.sh: `audit`/`deny` alias with a dependency of a known advisory.
    dir="$(config_in_copy alias-run "[alias]
audit = [\"run\", \"-q\", \"--manifest-path\", \"$TMP/fake-ok/Cargo.toml\", \"--\"]
deny = [\"run\", \"-q\", \"--manifest-path\", \"$TMP/fake-ok/Cargo.toml\", \"--\"]")"
    add_attack_crate "$dir" 'time = "=0.1.43"'
    blocked_before "[alias] audit/deny para um fake-ok (v13-alias), com RUSTSEC-2020-0071" "$dir"
    # audit and deny go through the binary: even without the pre-check, the alias would not apply.
    local rc=0 output
    output="$(cd "$dir" && "$PASSWD_HOME/.cargo/bin/cargo-audit" audit --deny warnings 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q 'RUSTSEC-2020-0071' <<<"$output" && ! grep -q 'falsos' <<<"$output"; then
        ok "cargo-audit pelo caminho absoluto ignora o [alias] e acha RUSTSEC-2020-0071"
    else
        failed "cargo-audit pelo caminho absoluto passou pelo alias (exit $rc)"
    fi
    blocked_before "[alias] audit/deny = version" "$(config_in_copy alias "[alias]
audit = \"version\"
deny = \"version\"")"
    blocked_before "build.rustc" "$(config_in_copy build-rustc "[build]
rustc = \"$TMP/decoys/rustc.sh\"")"
    blocked_before "build.rustc-wrapper (wrap.sh)" "$(config_in_copy rustc-wrapper "[build]
rustc-wrapper = \"$TMP/decoys/wrap.sh\"")"
    blocked_before "build.rustc-workspace-wrapper" "$(config_in_copy ws-wrapper "[build]
rustc-workspace-wrapper = \"$TMP/decoys/wrap.sh\"")"
    blocked_before "target.<triplo>.linker" "$(config_in_copy linker "[target.$triple]
linker = \"$TMP/decoys/linker.sh\"")"
    blocked_before "build.rustflags com -C linker" "$(config_in_copy rustflags "[build]
rustflags = [\"-C\", \"linker=$TMP/decoys/linker.sh\"]")"
    blocked_before "target.<triplo>.runner" "$(config_in_copy runner "[target.$triple]
runner = \"$TMP/decoys/program.sh\"")"
    blocked_before "[env]" "$(config_in_copy env "[env]
RUSTC_BOOTSTRAP = \"1\"")"
    blocked_before "chave pontilhada build.rustc-wrapper" "$(config_in_copy dotted "build.rustc-wrapper = \"$TMP/decoys/wrap.sh\"")"
    # Any `.cargo/config*` in the repository, even with a harmless key.
    blocked_before ".cargo/config.toml só com [term]" "$(config_in_copy term "[term]
color = \"never\"")"
    blocked_before ".cargo/config sem extensão" "$(config_in_copy old "[term]
color = \"never\"" .cargo/config)"
    blocked_before ".cargo/config.toml numa subpasta (crates/ward-core)" \
        "$(config_in_copy sub "[term]
color = \"never\"" crates/ward-core/.cargo/config.toml)"
    blocked_before ".cargo/config.bak (qualquer config*)" "$(config_in_copy bak "x" .cargo/config.bak)"
    dir="$TMP/cfg-link"; rm -rf "$dir"; copy_workspace "$dir"
    mkdir -p "$TMP/outside-cargo"; printf '[build]\nrustc-wrapper = "%s"\n' "$TMP/decoys/wrap.sh" >"$TMP/outside-cargo/config.toml"
    ln -s "$TMP/outside-cargo" "$dir/crates/.cargo"
    blocked_before ".cargo como link simbólico para fora" "$dir"
    # Config in a parent directory, even a harmless one.
    mkdir -p "$TMP/parent2/.cargo"; printf '[term]\ncolor = "never"\n' >"$TMP/parent2/.cargo/config.toml"
    dir="$TMP/parent2/child"; copy_workspace "$dir"
    blocked_before ".cargo/config.toml inofensivo em diretório pai" "$dir"

    # Environment variables (ataque-config.sh with VAR=value).
    dir="$TMP/env"; rm -rf "$dir"; copy_workspace "$dir"
    local v
    for v in "RUSTC=$TMP/decoys/rustc.sh" "RUSTC_WRAPPER=$TMP/decoys/wrap.sh" \
        "RUSTC_WORKSPACE_WRAPPER=$TMP/decoys/wrap.sh" "CARGO_BUILD_RUSTC_WRAPPER=$TMP/decoys/wrap.sh" \
        "CARGO_BUILD_RUSTC=$TMP/decoys/rustc.sh" "RUSTFLAGS=-Clinker=$TMP/decoys/linker.sh" \
        "CARGO_BUILD_RUSTFLAGS=-Clinker=$TMP/decoys/linker.sh" \
        "CARGO_ENCODED_RUSTFLAGS=-Clinker=$TMP/decoys/linker.sh" "RUSTDOCFLAGS=-Zunstable-options" \
        "RUSTDOC=$TMP/decoys/program.sh" "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$TMP/decoys/linker.sh" \
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER=$TMP/decoys/program.sh" "CARGO_ALIAS_AUDIT=version" \
        "CARGO_TARGET_DIR=$TMP/target" "RUSTC_BOOTSTRAP=1" "RUSTUP_TOOLCHAIN=stable" \
        "RUSTUP_DIST_SERVER=https://example.invalid" "CARGO_UNSTABLE_BUILD_STD=true"; do
        blocked_before "${v%%=*} no ambiente" "$dir" "$v"
    done
    # Neutralized by the Makefile (override + export): not refused, but they have no effect.
    rm -f "$MARKS"
    mkdir -p "$TMP/home-cargo"
    printf '[build]\nrustc-wrapper = "%s"\n' "$TMP/decoys/wrap.sh" >"$TMP/home-cargo/config.toml"
    rc=0
    (cd "$dir" && CARGO_HOME="$TMP/home-cargo" RUSTUP_HOME="$TMP/home-rustup" CARGO="$TMP/decoys/program.sh" \
        make build >/dev/null 2>&1) || rc=$?
    if [ "$rc" -eq 0 ] && [ ! -e "$MARKS" ]; then
        ok "CARGO_HOME/RUSTUP_HOME/CARGO do ambiente não valem (o Makefile fixa os do passwd)"
    else
        failed "CARGO_HOME/RUSTUP_HOME/CARGO do ambiente mudaram o build (exit $rc)"
    fi
    # And origins refuses the different value when called without the Makefile.
    rc=0
    (cd "$dir" && CARGO_HOME="$TMP/home-cargo" /usr/bin/python3 -I -B dev/origins.py >/dev/null 2>&1) || rc=$?
    if [ "$rc" -ne 0 ]; then ok "origins.py sozinho recusa CARGO_HOME diferente do esperado"; else failed "origins.py aceitou CARGO_HOME de fora"; fi
    # Control: a harmless variable from the list passes.
    if (cd "$dir" && CARGO_TERM_COLOR=never make origins >/dev/null 2>&1); then
        ok "pré-checagem: CARGO_TERM_COLOR (da lista) passa"
    else
        failed "pré-checagem: CARGO_TERM_COLOR recusado"
    fi

    # rust-toolchain.toml: `path` points rustc to any dir; the channel has to be fixed.
    dir="$TMP/tc-path"; rm -rf "$dir"; copy_workspace "$dir"
    printf '[toolchain]\npath = "%s"\n' "$TMP/decoys" >"$dir/rust-toolchain.toml"
    blocked_before "rust-toolchain.toml com path" "$dir"
    dir="$TMP/tc-channel"; rm -rf "$dir"; copy_workspace "$dir"
    sed -i 's/^channel = .*/channel = "nightly"/' "$dir/rust-toolchain.toml"
    blocked_before "rust-toolchain.toml com canal móvel (nightly)" "$dir"
    dir="$TMP/tc-legacy"; rm -rf "$dir"; copy_workspace "$dir"
    printf 'nightly\n' >"$dir/rust-toolchain"
    blocked_before "rust-toolchain sem extensão" "$dir"

    # Control: the vector is real. Without the pre-check, a direct `cargo build` in the copy
    # with the rustc-wrapper runs wrap.sh (the mark shows up).
    rm -f "$MARKS"
    dir="$TMP/cfg-rustc-wrapper"
    (cd "$dir" && rm -rf crates/attack && cargo build --quiet --offline >/dev/null 2>&1) || true
    if [ -s "$MARKS" ]; then
        ok "controle: sem a pré-checagem o rustc-wrapper plantado roda (o vetor é real)"
    else
        failed "controle: o rustc-wrapper plantado não rodou nem sem a pré-checagem (teste sem efeito)"
    fi
}

# --- Phase 12b (S14): CARGO_HOME config only with harmless keys ---------------------------
# In a bwrap with the passwd home covered by tmpfs (the real ~/.cargo is never touched):
# only rustup and the binaries of ~/.cargo/bin come back, read-only, plus this script's
# `$TMP` (Phases 28d/28f: it lives in `~/.cache/`, in the home). The CARGO_HOME
# `config.toml` is written to the tmpfs. Minimal workspace without dependencies (cargo
# metadata does not need the registry). Also proves the active toolchain check with a
# simulated `rustup override`.
cargo_home_config() {
    local m="$TMP/minimal"
    mkdir -p "$m/dev" "$m/a/src"
    cp "$ROOT/dev/origins.py" "$m/dev/"
    cp "$ROOT/rust-toolchain.toml" "$m/"
    printf '[workspace]\nmembers = ["a"]\nresolver = "3"\n' >"$m/Cargo.toml"
    printf '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2024"\n' >"$m/a/Cargo.toml"
    : >"$m/a/src/lib.rs"
    (cd "$m" && cargo generate-lockfile --quiet --offline)
    cat >"$TMP/in-home.sh" <<'EOF'
#!/bin/bash -p
# Inside the bwrap: verifies the tmpfs, writes the CARGO_HOME config and runs origins.
set -u
home="$1" m="$2" content="$3"
[ "$(stat -f -c %T "$home")" = tmpfs ] || { echo "home não coberto" >&2; exit 9; }
mkdir -p "$home/.cargo"
[ -n "$content" ] && printf '%s\n' "$content" >"$home/.cargo/config.toml"
exec /usr/bin/python3 -I -B "$m/dev/origins.py"
EOF
    in_home() { # content [rustup settings.toml]
        local extra=()
        [ -n "${2:-}" ] && extra=(--ro-bind "$2" "$PASSWD_HOME/.rustup/settings.toml")
        /usr/bin/env -i PATH=/usr/bin:/bin bwrap --die-with-parent --dev-bind / / --tmpfs "$PASSWD_HOME" \
            --ro-bind "$PASSWD_HOME/.rustup" "$PASSWD_HOME/.rustup" "${extra[@]}" \
            --ro-bind "$PASSWD_HOME/.cargo/bin" "$PASSWD_HOME/.cargo/bin" --bind /tmp /tmp \
            --bind "$TMP" "$TMP" \
            -- /bin/bash -p "$TMP/in-home.sh" "$PASSWD_HOME" "$m" "$1"
    }
    local c rc
    for c in '[alias]
audit = "version"' "[build]
rustc-wrapper = \"$TMP/decoys/wrap.sh\"" "[build]
rustc = \"$TMP/decoys/rustc.sh\"" "[target.x86_64-unknown-linux-gnu]
linker = \"$TMP/decoys/linker.sh\"" '[build]
rustflags = ["-Cdebuginfo=0"]' '[env]
RUSTC_BOOTSTRAP = "1"' '[source.crates-io]
replace-with = "v"' '[registries.mau]
index = "sparse+https://example.invalid/"' '[http]
proxy = "http://127.0.0.1:9"' '[net]
git-fetch-with-cli = true' 'include = "other.toml"' '[term]
color = ["never"]' '[source'; do
        rc=0
        in_home "$c" >"$TMP/in-home.out" 2>&1 || rc=$?
        if [ "$rc" -eq 1 ] && grep -q '^origens: .*config' "$TMP/in-home.out"; then
            ok "CARGO_HOME: config com '$(tr '\n' ' ' <<<"$c" | sed 's/ $//')' é recusado"
        else
            failed "CARGO_HOME: config com '$(tr '\n' ' ' <<<"$c" | sed 's/ $//')' passou (exit $rc)"
        fi
    done
    # Controls: without a config and with keys from the list, it passes.
    for c in '' '[term]
color = "never"
progress.when = "never"
[net]
retry = 3
[build]
jobs = 4'; do
        rc=0
        in_home "$c" >"$TMP/in-home.out" 2>&1 || rc=$?
        local desc="ausente"
        [ -n "$c" ] && desc="só com chaves permitidas (term, net.retry, build.jobs)"
        if [ "$rc" -eq 0 ]; then ok "CARGO_HOME: config $desc passa"; else failed "CARGO_HOME: config permitido recusado"; cat "$TMP/in-home.out" >&2; fi
    done
    # `rustup override set` for the dir: the active toolchain stops being the file's.
    printf 'version = "12"\ndefault_toolchain = "stable-x86_64-unknown-linux-gnu"\nprofile = "minimal"\n\n[overrides]\n"%s" = "stable-x86_64-unknown-linux-gnu"\n' \
        "$m" >"$TMP/settings.toml"
    rc=0
    in_home '' "$TMP/settings.toml" >"$TMP/in-home.out" 2>&1 || rc=$?
    if [ "$rc" -eq 1 ] && grep -q 'toolchain ativo' "$TMP/in-home.out"; then
        ok "rustup override de diretório é recusado"
    else
        failed "rustup override de diretório passou (exit $rc)"
    fi
}

# --- Phase 12b: the user's environment does not swap what make check runs ---------------------
# PATH with fake cargo/python3/git/bash/sha256sum in front, MAKEFLAGS with CARGO/GITLEAKS
# swapped (beats `:=`, does not beat `override`), BASH_ENV, a fake HOME and a ~/.gitconfig
# with core.fsmonitor (runs a program on every `git diff`, including inside gitleaks). The
# whole `make check` in a copy has to pass without any mark.
make_environment() {
    local fakes="$TMP/fakes" p
    mkdir -p "$fakes"
    for p in cargo cargo-audit cargo-deny python3 git bash sh sha256sum gitleaks docker rustc make; do
        printf '#!/bin/sh\necho "%s" >>"%s"\nexit 0\n' "$p" "$MARKS" >"$fakes/$p"
        chmod +x "$fakes/$p"
    done
    # `make` itself has to be the system's: called by the absolute path below.
    printf 'echo bash_env >>"%s"\n' "$MARKS" >"$TMP/bash_env"
    printf '[core]\n\tfsmonitor = %s\n' "$TMP/decoys/program.sh" >"$TMP/gitconfig-bad"
    mkdir -p "$TMP/xdg/git"; printf '*\n' >"$TMP/xdg/git/ignore"
    local dir="$TMP/env-make"
    copy_workspace "$dir"
    (cd "$dir" && git init -q && git add -A)
    rm -f "$MARKS"
    local rc=0 output
    output="$(cd "$dir" && /usr/bin/env PATH="$fakes:$PATH" BASH_ENV="$TMP/bash_env" ENV="$TMP/bash_env" \
        MAKEFLAGS="CARGO=$fakes/cargo GITLEAKS=$fakes/gitleaks CARGO_AUDIT=$fakes/cargo-audit PYTHON=$fakes/python3" \
        HOME="$TMP/fake-home" GIT_CONFIG_GLOBAL="$TMP/gitconfig-bad" XDG_CONFIG_HOME="$TMP/xdg" \
        /usr/bin/make check 2>&1)" || rc=$?
    if [ "$rc" -eq 0 ] && [ ! -e "$MARKS" ] && grep -q '==> make check: tudo limpo' <<<"$output"; then
        ok "PATH/MAKEFLAGS/BASH_ENV/HOME/gitconfig do usuário não trocam nada no make check"
    else
        failed "o ambiente do usuário mudou o make check (exit $rc, marcas: $(tr '\n' ' ' <"$MARKS" 2>/dev/null))"
        tail -15 <<<"$output" >&2
    fi
    # Control: the planted fsmonitor runs on a plain `git diff` (the vector is real).
    rm -f "$MARKS"
    (cd "$dir" && /usr/bin/env -u GIT_CONFIG_COUNT GIT_CONFIG_GLOBAL="$TMP/gitconfig-bad" /usr/bin/git diff --staged --quiet) >/dev/null 2>&1 || true
    if [ -s "$MARKS" ]; then ok "controle: core.fsmonitor do gitconfig roda sem o isolamento"; else failed "controle: fsmonitor não rodou nem sem isolamento"; fi
    # The same through the local .git/config: the command line (GIT_CONFIG_COUNT) wins.
    rm -f "$MARKS"
    (cd "$dir" && /usr/bin/git config core.fsmonitor "$TMP/decoys/program.sh")
    rc=0
    (cd "$dir" && make gitleaks >/dev/null 2>&1) || rc=$?
    if [ "$rc" -eq 0 ] && [ ! -e "$MARKS" ]; then
        ok "core.fsmonitor no .git/config não roda no make gitleaks"
    else
        failed "core.fsmonitor do .git/config rodou no make gitleaks (exit $rc)"
    fi
}

# --- Phase 12b: the tests' git without the user's config -------------------------------------
# dev/env.sh: a global ignore (~/.config/git/ignore) would make the V1.5 `git check-ignore`
# pass without the repository's .gitignore.
git_without_user_config() {
    local dir="$TMP/git-isolated" rc=0
    mkdir -p "$dir"
    (cd "$dir" && /usr/bin/git init -q)
    printf '[core]\n\texcludesFile = %s/xdg/git/ignore\n' "$TMP" >"$TMP/gitconfig-ignore"
    /usr/bin/env GIT_CONFIG_GLOBAL="$TMP/gitconfig-ignore" XDG_CONFIG_HOME="$TMP/xdg" HOME="$TMP" \
        /bin/bash -p -c '. "$1/dev/env.sh"; cd "$2" && git check-ignore -q anything.txt' _ "$ROOT" "$dir" || rc=$?
    if [ "$rc" -eq 1 ]; then
        ok "git dos testes ignora o ignore global do usuário (check-ignore depende só do repositório)"
    else
        failed "git dos testes leu o ignore global do usuário (exit $rc)"
    fi
    # Control: without env.sh, the same git says it is ignored.
    rc=0
    (cd "$dir" && /usr/bin/env -u GIT_CONFIG_COUNT -u GIT_CONFIG_GLOBAL XDG_CONFIG_HOME="$TMP/xdg" HOME="$TMP" \
        /usr/bin/git check-ignore -q anything.txt) || rc=$?
    if [ "$rc" -eq 0 ]; then ok "controle: sem o isolamento o ignore global vale"; else failed "controle: ignore global não valeu nem sem isolamento"; fi
}

# --- Phase 28f: the copies with a planted `.cargo/config*` do not taint the root's `origins` ---
# With the run's copies still on disk (only the `target/` go away), the checkout's
# `make origins` has to stay clean. Before 28f they lived in `.dev/` and every `make` of the
# repository failed in `origins` while the test ran.
root_origins_during_run() {
    local planted rc=0 output
    planted="$(find "$TMP" -path '*/.cargo/config*' 2>/dev/null | wc -l)"
    if [ "$planted" -eq 0 ]; then
        failed "28f: nenhuma cópia com .cargo/config* plantado no disco (checagem sem efeito)"
        return
    fi
    output="$(cd "$ROOT" && make -s origins 2>&1)" || rc=$?
    if [ "$rc" -eq 0 ]; then
        ok "28f: make origins da raiz limpo com $planted .cargo/config* plantados nas cópias"
    else
        failed "28f: make origins da raiz falhou durante a rodada (exit $rc)"
        tail -10 <<<"$output" >&2
    fi
}

# Phase 28d: each copy compiles the whole workspace in its own `target/`; removes the
# copies' `target/` after each attack so as not to add up several GB on disk.
run() {
    "$1"
    rm -rf "$TMP"/*/target
}

run v1_2
run anchored_allowlist
run dev_forced_into_index
run v1_3_advisory
run v1_3_git
run v1_3_lockfile
run v1_5
run extra_without_path_allowlist
run cargo_origins
run fixed_tools
run origins_allowlist
run cargo_home_config
run root_origins_during_run
run make_environment
run git_without_user_config
run root_origins_during_run

if [ "$failures" -ne 0 ]; then
    printf '\n%d ataque(s) passaram: corrigir antes de seguir.\n' "$failures" >&2
    exit 1
fi
printf '\nTodos os ataques foram barrados.\n'
