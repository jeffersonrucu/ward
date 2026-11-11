#!/usr/bin/python3 -IB
"""Origins of the Rust code (Phases 12a and 12b, S14). Pre-check of `make check`: runs
BEFORE step 1 and before every Makefile target that calls cargo.

An ALLOW list, not a deny list. Cargo and rustc accept many ways of swapping what gets
compiled or executed: `[alias]` (a `cargo audit` that runs another program),
`build.rustc`, `rustc-wrapper`, `rustc-workspace-wrapper`, `linker`, `rustflags`,
`runner`, `[env]`, `[source]`, `[patch]`, `[registries]`... and the matching environment
variables (`RUSTC_WRAPPER`, `RUSTFLAGS`, `CARGO_BUILD_RUSTC`,
`CARGO_TARGET_<triple>_LINKER`...). Instead of chasing each one, this script fails
(exits 1) when:

1. there is any `.cargo/config*` inside the repository (in any directory), a `.cargo`
   that is a symbolic link, or a `.cargo/config*` in a parent directory of the root
   that is not CARGO_HOME;
2. the `config`/`config.toml` of CARGO_HOME has a key outside `HOME_ALLOWED` (only
   terminal appearance, network and number of jobs), or is not valid TOML;
3. the environment has a `CARGO*`, `RUSTC*`, `RUSTDOC*`, `RUSTFLAGS*` or `RUSTUP*`
   variable outside `ENV_ALLOWED` (CARGO_HOME, RUSTUP_HOME and CARGO only with the
   expected value);
4. `rust-toolchain.toml` has a key outside `[toolchain] channel/components/profile/
   targets` (e.g. `path`, which points rustc at any directory), the channel is not a
   fixed version, a `rust-toolchain` without extension exists, or rustup says the
   active toolchain does not come from that file (e.g. `rustup override set`);
5. some workspace `Cargo.toml` has `[patch]` or `[replace]`;
6. some package without a source (path) is outside the repository (or in `target/`/
   `.dev/`), or some package comes from a source that is not crates.io.

Steps 1 to 4 run without executing cargo or rustc: with a bad config or environment the
script stops first (`cargo metadata` itself would run a planted `build.rustc`). Cargo
and rustup are called by absolute path in `~/.cargo/bin` (home from passwd).

Prints only file, key and variable names, never values.
Usage: python3 -I -B dev/origins.py   (or `make origins`)
"""

import json
import os
import pwd
import re
import subprocess
import sys
import tomllib

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
PASSWD_HOME = pwd.getpwuid(os.getuid()).pw_dir
CARGO_HOME = os.path.realpath(os.path.join(PASSWD_HOME, ".cargo"))
RUSTUP_HOME = os.path.realpath(os.path.join(PASSWD_HOME, ".rustup"))
CARGO = os.path.join(CARGO_HOME, "bin", "cargo")
RUSTUP = os.path.join(CARGO_HOME, "bin", "rustup")

# Keys accepted in the CARGO_HOME config: table -> keys (True = simple value).
# Nothing that picks a program, a source, a compiler flag or an environment variable.
HOME_ALLOWED = {
    "term": {
        "quiet": True,
        "verbose": True,
        "color": True,
        "hyperlinks": True,
        "unicode": True,
        "progress": {"when": True, "width": True, "term-integration": True},
    },
    "net": {"retry": True, "offline": True},
    "http": {"timeout": True, "low-speed-limit": True, "multiplexing": True},
    "build": {"jobs": True, "incremental": True},
    "cargo-new": {"vcs": True},
    "future-incompat-report": {"frequency": True},
    "cache": {"auto-clean-frequency": True},
}

# Accepted cargo/rustc/rustup environment variables. Value None: any value (only
# appearance and network); string: only that path.
ENV_ALLOWED = {
    "CARGO_HOME": CARGO_HOME,
    "RUSTUP_HOME": RUSTUP_HOME,
    "CARGO": CARGO,
    "CARGO_TERM_COLOR": None,
    "CARGO_TERM_QUIET": None,
    "CARGO_TERM_VERBOSE": None,
    "CARGO_TERM_PROGRESS_WHEN": None,
    "CARGO_TERM_PROGRESS_WIDTH": None,
    "CARGO_NET_RETRY": None,
    "CARGO_HTTP_TIMEOUT": None,
    "CARGO_BUILD_JOBS": None,
    "CARGO_INCREMENTAL": None,
}
CARGO_ENV = re.compile(r"^(CARGO|RUSTC|RUSTDOC|RUSTFLAGS|RUSTUP)")

TOOLCHAIN_ALLOWED = {"channel", "components", "profile", "targets"}
FIXED_CHANNEL = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
SIMPLE_NAME = re.compile(r"^[a-z0-9_.-]+$")

CRATES_IO = {
    "registry+https://github.com/rust-lang/crates.io-index",
    "sparse+https://index.crates.io/",
}
# Directories where cargo never runs as the current directory here and that can be huge.
SKIP = {".git", "target", "node_modules"}

errors = []


def inside(p, base):
    return p == base or p.startswith(base.rstrip("/") + "/")


def rel(p):
    return os.path.relpath(p, ROOT) if inside(p, ROOT) else p


# --- 1 and 2: cargo configs -------------------------------------------------------------
def configs_in_repo():
    for d, subdirs, files in os.walk(ROOT):
        for s in subdirs:
            if s == ".cargo" and os.path.islink(os.path.join(d, s)):
                errors.append(f"{rel(os.path.join(d, s))}: .cargo como link simbólico no repositório")
        subdirs[:] = [s for s in subdirs if s not in SKIP]
        if os.path.basename(d) != ".cargo":
            continue
        for name in sorted(files + subdirs):
            if name.startswith("config"):
                errors.append(f"{rel(os.path.join(d, name))}: .cargo/config* no repositório é proibido")


def verify_table(data, allowed, path, file):
    for key, value in data.items():
        name = f"{path}.{key}" if path else key
        rule = allowed.get(key)
        if rule is None:
            errors.append(f"{file}: chave {name} fora da lista de permitidas")
        elif isinstance(rule, dict):
            if isinstance(value, dict):
                verify_table(value, rule, name, file)
            else:
                errors.append(f"{file}: chave {name} devia ser tabela")
        elif not isinstance(value, (str, int, bool)):
            errors.append(f"{file}: chave {name} devia ter valor simples")


def verify_home_config(file):
    try:
        with open(file, "rb") as h:
            data = tomllib.load(h)
    except (OSError, tomllib.TOMLDecodeError):
        errors.append(f"{file}: config do cargo ilegível ou TOML inválido")
        return
    # tomllib already folds dotted keys (`build.rustc-wrapper = …`) into the tables.
    verify_table(data, HOME_ALLOWED, "", file)


def configs_outside_repo():
    d = os.path.dirname(ROOT)
    dirs = []
    while True:
        dirs.append(os.path.join(d, ".cargo"))
        parent = os.path.dirname(d)
        if parent == d:
            break
        d = parent
    dirs.append(CARGO_HOME)
    for folder in dict.fromkeys(os.path.realpath(p) for p in dirs):
        for name in ("config", "config.toml"):
            f = os.path.join(folder, name)
            if not os.path.lexists(f):
                continue
            if folder == CARGO_HOME:
                verify_home_config(f)
            else:
                errors.append(f"{f}: .cargo/config* em diretório pai do repositório é proibido")


# --- 3: environment ---------------------------------------------------------------------
def verify_environment():
    for name in sorted(os.environ):
        if not CARGO_ENV.match(name):
            continue
        if name not in ENV_ALLOWED:
            errors.append(f"variável de ambiente {name} fora da lista de permitidas")
            continue
        expected = ENV_ALLOWED[name]
        if expected is not None and os.path.realpath(os.environ[name]) != os.path.realpath(expected):
            errors.append(f"variável de ambiente {name} com valor diferente do esperado")


# --- 4: toolchain -----------------------------------------------------------------------
def verify_toolchain():
    if os.path.lexists(os.path.join(ROOT, "rust-toolchain")):
        errors.append("rust-toolchain (sem extensão) é proibido; use só o rust-toolchain.toml")
    file = os.path.join(ROOT, "rust-toolchain.toml")
    try:
        with open(file, "rb") as h:
            data = tomllib.load(h)
    except (OSError, tomllib.TOMLDecodeError):
        errors.append("rust-toolchain.toml ausente, ilegível ou TOML inválido")
        return None
    if set(data) != {"toolchain"} or not isinstance(data["toolchain"], dict):
        errors.append("rust-toolchain.toml: só a tabela [toolchain] é permitida")
        return None
    tc = data["toolchain"]
    for key in sorted(set(tc) - TOOLCHAIN_ALLOWED):
        errors.append(f"rust-toolchain.toml: chave toolchain.{key} fora da lista de permitidas")
    channel = tc.get("channel")
    if not isinstance(channel, str) or not FIXED_CHANNEL.match(channel):
        errors.append("rust-toolchain.toml: channel tem de ser uma versão fixa (ex.: 1.99.1)")
        channel = None
    for key in ("components", "targets"):
        v = tc.get(key, [])
        if not isinstance(v, list) or not all(isinstance(x, str) and SIMPLE_NAME.match(x) for x in v):
            errors.append(f"rust-toolchain.toml: toolchain.{key} tem de ser lista de nomes")
    if tc.get("profile", "minimal") not in ("minimal", "default"):
        errors.append("rust-toolchain.toml: profile tem de ser minimal ou default")
    return channel


def verify_active_toolchain(channel):
    r = subprocess.run([RUSTUP, "show", "active-toolchain"], cwd=ROOT, capture_output=True, text=True)
    out = r.stdout.strip()
    expected = f"(overridden by '{os.path.join(ROOT, 'rust-toolchain.toml')}')"
    if r.returncode != 0 or not out.startswith(f"{channel}-") or not out.endswith(expected):
        errors.append("o toolchain ativo do rustup não é o do rust-toolchain.toml (override de diretório?)")


# --- 5 and 6: manifests and packages ----------------------------------------------------
def metadata():
    r = subprocess.run(
        [CARGO, "metadata", "--format-version", "1", "--locked"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        errors.append("cargo metadata --locked falhou (lockfile desatualizado ou manifesto inválido)")
        sys.stderr.write(r.stderr[-2000:])
        return None
    return json.loads(r.stdout)


def verify_manifests(meta):
    members = set(meta["workspace_members"])
    manifests = {os.path.join(ROOT, "Cargo.toml")}
    for p in meta["packages"]:
        if p["id"] in members:
            manifests.add(p["manifest_path"])
    for m in sorted(manifests):
        try:
            with open(m, "rb") as h:
                data = tomllib.load(h)
        except (OSError, tomllib.TOMLDecodeError):
            errors.append(f"{m}: manifesto ilegível")
            continue
        for key in ("patch", "replace"):
            if key in data:
                errors.append(f"{rel(m)}: tem [{key}]")


def verify_packages(meta):
    for p in meta["packages"]:
        source = p.get("source")
        manifest = os.path.realpath(p["manifest_path"])
        if source is None:
            if not inside(manifest, ROOT):
                errors.append(f"pacote {p['name']}: dependência por path fora do repositório")
            elif inside(manifest, os.path.join(ROOT, "target")) or inside(manifest, os.path.join(ROOT, ".dev")):
                errors.append(f"pacote {p['name']}: dependência por path em diretório ignorado")
        elif source not in CRATES_IO:
            errors.append(f"pacote {p['name']}: fonte fora do crates.io")


def finish():
    for e in errors:
        print(f"origens: {e}", file=sys.stderr)
    print(f"origens: {len(errors)} problema(s); o build não pode confiar nas fontes", file=sys.stderr)
    return 1


def main():
    # Without executing anything: environment, configs and toolchain.
    verify_environment()
    configs_in_repo()
    configs_outside_repo()
    channel = verify_toolchain()
    if errors:
        return finish()
    # Now rustup and cargo may run: environment and configs are the allowed ones.
    verify_active_toolchain(channel)
    if errors:
        return finish()
    meta = metadata()
    if meta is not None:
        verify_manifests(meta)
        verify_packages(meta)
    if errors:
        return finish()
    print("origens: só configs e variáveis permitidas, toolchain fixo, crates.io e paths dentro do repositório")
    return 0


if __name__ == "__main__":
    sys.exit(main())
