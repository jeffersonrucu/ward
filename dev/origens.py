#!/usr/bin/python3 -IB
"""Origens do código Rust (Fases 12a e 12b, S14). Pré-checagem do `make check`: roda
ANTES do passo 1 e antes de todo alvo do Makefile que chama o cargo.

Lista de PERMITIDAS, não de proibidas. O cargo e o rustc aceitam muitas formas de
trocar o que é compilado ou executado: `[alias]` (um `cargo audit` que roda outro
programa), `build.rustc`, `rustc-wrapper`, `rustc-workspace-wrapper`, `linker`,
`rustflags`, `runner`, `[env]`, `[source]`, `[patch]`, `[registries]`... e as variáveis
de ambiente equivalentes (`RUSTC_WRAPPER`, `RUSTFLAGS`, `CARGO_BUILD_RUSTC`,
`CARGO_TARGET_<triplo>_LINKER`...). Em vez de correr atrás de cada uma, este script
falha (sai 1) quando:

1. existe qualquer `.cargo/config*` dentro do repositório (em qualquer pasta), um
   `.cargo` que é link simbólico, ou `.cargo/config*` num diretório pai da raiz que não
   seja o CARGO_HOME;
2. o `config`/`config.toml` do CARGO_HOME tem alguma chave fora de `PERMITIDAS_HOME`
   (só aparência do terminal, rede e número de jobs), ou não é TOML válido;
3. há no ambiente variável `CARGO*`, `RUSTC*`, `RUSTDOC*`, `RUSTFLAGS*` ou `RUSTUP*`
   fora de `ENV_PERMITIDAS` (CARGO_HOME, RUSTUP_HOME e CARGO só com o valor esperado);
4. o `rust-toolchain.toml` tem chave fora de `[toolchain] channel/components/profile/
   targets` (ex.: `path`, que aponta o rustc para qualquer pasta), o canal não é uma
   versão fixa, existe um `rust-toolchain` sem extensão, ou o rustup diz que o
   toolchain ativo não vem desse arquivo (ex.: `rustup override set`);
5. algum `Cargo.toml` do workspace tem `[patch]` ou `[replace]`;
6. algum pacote sem fonte (path) está fora do repositório (ou em `target/`/`.dev/`), ou
   algum pacote vem de fonte que não é o crates.io.

Os passos 1 a 4 rodam sem executar o cargo nem o rustc: com config ou ambiente ruim, o
script para antes (o próprio `cargo metadata` rodaria o `build.rustc` plantado). O cargo
e o rustup são chamados pelo caminho absoluto em `~/.cargo/bin` (home do passwd).

Só imprime nomes de arquivo, de chave e de variável, nunca valores.
Uso: python3 -I -B dev/origens.py   (ou `make origens`)
"""

import json
import os
import pwd
import re
import subprocess
import sys
import tomllib

RAIZ = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
CASA = pwd.getpwuid(os.getuid()).pw_dir
CARGO_HOME = os.path.realpath(os.path.join(CASA, ".cargo"))
RUSTUP_HOME = os.path.realpath(os.path.join(CASA, ".rustup"))
CARGO = os.path.join(CARGO_HOME, "bin", "cargo")
RUSTUP = os.path.join(CARGO_HOME, "bin", "rustup")

# Chaves aceitas no config do CARGO_HOME: tabela -> chaves (True = valor simples).
# Nada que escolha programa, fonte, flag de compilação ou variável de ambiente.
PERMITIDAS_HOME = {
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

# Variáveis de ambiente do cargo/rustc/rustup aceitas. Valor None: qualquer valor
# (só aparência e rede); string: só aquele caminho.
ENV_PERMITIDAS = {
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
ENV_DO_CARGO = re.compile(r"^(CARGO|RUSTC|RUSTDOC|RUSTFLAGS|RUSTUP)")

TOOLCHAIN_PERMITIDAS = {"channel", "components", "profile", "targets"}
CANAL_FIXO = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
NOME_SIMPLES = re.compile(r"^[a-z0-9_.-]+$")

CRATES_IO = {
    "registry+https://github.com/rust-lang/crates.io-index",
    "sparse+https://index.crates.io/",
}
# Pastas onde o cargo nunca roda como diretório atual aqui e que podem ser enormes.
PULAR = {".git", "target", "node_modules"}

erros = []


def dentro(p, base):
    return p == base or p.startswith(base.rstrip("/") + "/")


def rel(p):
    return os.path.relpath(p, RAIZ) if dentro(p, RAIZ) else p


# --- 1 e 2: configs do cargo ------------------------------------------------------------
def configs_no_repositorio():
    for d, subdirs, arquivos in os.walk(RAIZ):
        for s in subdirs:
            if s == ".cargo" and os.path.islink(os.path.join(d, s)):
                erros.append(f"{rel(os.path.join(d, s))}: .cargo como link simbólico no repositório")
        subdirs[:] = [s for s in subdirs if s not in PULAR]
        if os.path.basename(d) != ".cargo":
            continue
        for nome in sorted(arquivos + subdirs):
            if nome.startswith("config"):
                erros.append(f"{rel(os.path.join(d, nome))}: .cargo/config* no repositório é proibido")


def conferir_tabela(dados, permitidas, caminho, arquivo):
    for chave, valor in dados.items():
        nome = f"{caminho}.{chave}" if caminho else chave
        regra = permitidas.get(chave)
        if regra is None:
            erros.append(f"{arquivo}: chave {nome} fora da lista de permitidas")
        elif isinstance(regra, dict):
            if isinstance(valor, dict):
                conferir_tabela(valor, regra, nome, arquivo)
            else:
                erros.append(f"{arquivo}: chave {nome} devia ser tabela")
        elif not isinstance(valor, (str, int, bool)):
            erros.append(f"{arquivo}: chave {nome} devia ter valor simples")


def conferir_config_home(arquivo):
    try:
        with open(arquivo, "rb") as h:
            dados = tomllib.load(h)
    except (OSError, tomllib.TOMLDecodeError):
        erros.append(f"{arquivo}: config do cargo ilegível ou TOML inválido")
        return
    # tomllib já junta chaves pontilhadas (`build.rustc-wrapper = …`) nas tabelas.
    conferir_tabela(dados, PERMITIDAS_HOME, "", arquivo)


def configs_fora_do_repositorio():
    d = os.path.dirname(RAIZ)
    pastas = []
    while True:
        pastas.append(os.path.join(d, ".cargo"))
        pai = os.path.dirname(d)
        if pai == d:
            break
        d = pai
    pastas.append(CARGO_HOME)
    for pasta in dict.fromkeys(os.path.realpath(p) for p in pastas):
        for nome in ("config", "config.toml"):
            f = os.path.join(pasta, nome)
            if not os.path.lexists(f):
                continue
            if pasta == CARGO_HOME:
                conferir_config_home(f)
            else:
                erros.append(f"{f}: .cargo/config* em diretório pai do repositório é proibido")


# --- 3: ambiente ------------------------------------------------------------------------
def conferir_ambiente():
    for nome in sorted(os.environ):
        if not ENV_DO_CARGO.match(nome):
            continue
        if nome not in ENV_PERMITIDAS:
            erros.append(f"variável de ambiente {nome} fora da lista de permitidas")
            continue
        esperado = ENV_PERMITIDAS[nome]
        if esperado is not None and os.path.realpath(os.environ[nome]) != os.path.realpath(esperado):
            erros.append(f"variável de ambiente {nome} com valor diferente do esperado")


# --- 4: toolchain -----------------------------------------------------------------------
def conferir_toolchain():
    if os.path.lexists(os.path.join(RAIZ, "rust-toolchain")):
        erros.append("rust-toolchain (sem extensão) é proibido; use só o rust-toolchain.toml")
    arquivo = os.path.join(RAIZ, "rust-toolchain.toml")
    try:
        with open(arquivo, "rb") as h:
            dados = tomllib.load(h)
    except (OSError, tomllib.TOMLDecodeError):
        erros.append("rust-toolchain.toml ausente, ilegível ou TOML inválido")
        return None
    if set(dados) != {"toolchain"} or not isinstance(dados["toolchain"], dict):
        erros.append("rust-toolchain.toml: só a tabela [toolchain] é permitida")
        return None
    tc = dados["toolchain"]
    for chave in sorted(set(tc) - TOOLCHAIN_PERMITIDAS):
        erros.append(f"rust-toolchain.toml: chave toolchain.{chave} fora da lista de permitidas")
    canal = tc.get("channel")
    if not isinstance(canal, str) or not CANAL_FIXO.match(canal):
        erros.append("rust-toolchain.toml: channel tem de ser uma versão fixa (ex.: 1.99.1)")
        canal = None
    for chave in ("components", "targets"):
        v = tc.get(chave, [])
        if not isinstance(v, list) or not all(isinstance(x, str) and NOME_SIMPLES.match(x) for x in v):
            erros.append(f"rust-toolchain.toml: toolchain.{chave} tem de ser lista de nomes")
    if tc.get("profile", "minimal") not in ("minimal", "default"):
        erros.append("rust-toolchain.toml: profile tem de ser minimal ou default")
    return canal


def conferir_toolchain_ativo(canal):
    r = subprocess.run([RUSTUP, "show", "active-toolchain"], cwd=RAIZ, capture_output=True, text=True)
    saida = r.stdout.strip()
    esperado = f"(overridden by '{os.path.join(RAIZ, 'rust-toolchain.toml')}')"
    if r.returncode != 0 or not saida.startswith(f"{canal}-") or not saida.endswith(esperado):
        erros.append("o toolchain ativo do rustup não é o do rust-toolchain.toml (override de diretório?)")


# --- 5 e 6: manifestos e pacotes --------------------------------------------------------
def metadata():
    r = subprocess.run(
        [CARGO, "metadata", "--format-version", "1", "--locked"],
        cwd=RAIZ,
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        erros.append("cargo metadata --locked falhou (lockfile desatualizado ou manifesto inválido)")
        sys.stderr.write(r.stderr[-2000:])
        return None
    return json.loads(r.stdout)


def conferir_manifestos(meta):
    membros = set(meta["workspace_members"])
    manifestos = {os.path.join(RAIZ, "Cargo.toml")}
    for p in meta["packages"]:
        if p["id"] in membros:
            manifestos.add(p["manifest_path"])
    for m in sorted(manifestos):
        try:
            with open(m, "rb") as h:
                dados = tomllib.load(h)
        except (OSError, tomllib.TOMLDecodeError):
            erros.append(f"{m}: manifesto ilegível")
            continue
        for chave in ("patch", "replace"):
            if chave in dados:
                erros.append(f"{rel(m)}: tem [{chave}]")


def conferir_pacotes(meta):
    for p in meta["packages"]:
        fonte = p.get("source")
        manifesto = os.path.realpath(p["manifest_path"])
        if fonte is None:
            if not dentro(manifesto, RAIZ):
                erros.append(f"pacote {p['name']}: dependência por path fora do repositório")
            elif dentro(manifesto, os.path.join(RAIZ, "target")) or dentro(manifesto, os.path.join(RAIZ, ".dev")):
                erros.append(f"pacote {p['name']}: dependência por path em diretório ignorado")
        elif fonte not in CRATES_IO:
            erros.append(f"pacote {p['name']}: fonte fora do crates.io")


def terminar():
    for e in erros:
        print(f"origens: {e}", file=sys.stderr)
    print(f"origens: {len(erros)} problema(s); o build não pode confiar nas fontes", file=sys.stderr)
    return 1


def main():
    # Sem executar nada: ambiente, configs e toolchain.
    conferir_ambiente()
    configs_no_repositorio()
    configs_fora_do_repositorio()
    canal = conferir_toolchain()
    if erros:
        return terminar()
    # Agora o rustup e o cargo podem rodar: ambiente e configs são os permitidos.
    conferir_toolchain_ativo(canal)
    if erros:
        return terminar()
    meta = metadata()
    if meta is not None:
        conferir_manifestos(meta)
        conferir_pacotes(meta)
    if erros:
        return terminar()
    print("origens: só configs e variáveis permitidas, toolchain fixo, crates.io e paths dentro do repositório")
    return 0


if __name__ == "__main__":
    sys.exit(main())
