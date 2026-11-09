#!/usr/bin/python3 -IB
"""Confere a skill `ward` e o trecho de deny (Fase 50, V8.1, V8.2, V8.4). Sai 1 se algo
falhar.

1. `skill/ward/SKILL.md` tem frontmatter só com `name: ward` e uma `description` que
   dispara em pedidos de login, senha, SSH, token e banco.
2. Todo comando `ward <sub>` citado em código (bloco ou trecho entre crases) em
   qualquer `.md` de `skill/ward/` está no `ward --help` do binário dado e na lista de
   V8.1 (lista, run, ssh, login, playwright, travar). `ward login` (Fase 43) é aceito
   como pendente enquanto não estiver no `--help`, com aviso.
3. A skill não cita KeePassXC nem os scripts `cofre-*` (V8.1), nem o `--token-fd`.
4. Nenhum padrão de `.dev/variacoes.txt` (segredos do cofre de teste e variações)
   aparece em nenhum arquivo de `skill/` (nunca imprime o padrão; só o arquivo).
5. `skill/settings-deny.json` é JSON válido (sem chave repetida), só com
   `permissions.deny`, e cobre `Read` e `Edit` de cada caminho de V8.2 e do `~/.ssh`.
6. A skill manda nunca ler esses caminhos nem imprimir variável ou anexo em nenhuma
   codificação (V8.2) e diz que o SSH do `~/.ssh` é direto, sem o Ward (V8.4).

Antes de conferir a árvore real, o autoteste confere que ela passa (controle) e planta cada ataque numa cópia de `skill/`
(segredo, comando inventado, `cofre-run`, JSON inválido, caminho de deny faltando,
frontmatter errado) e exige que a conferência falhe.

Uso: python3 -I -B dev/testes/skill.py target/debug/ward   (ou `make teste-skill`)
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

RAIZ = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
VARIACOES = os.path.join(RAIZ, ".dev", "variacoes.txt")

# V8.1: os únicos subcomandos que a skill pode usar.
PERMITIDOS = {"lista", "run", "ssh", "login", "playwright", "travar"}
# Subcomandos aceitos fora do `--help` enquanto a fase deles não chega (nenhum desde a
# Fase 43).
PENDENTES = set()

# V8.2: cada caminho, na sintaxe de regra do Claude Code (`~/` = home, `//` = absoluto).
CAMINHOS_DENY = {
    "~/.local/share/ward/**": "~/.local/share/ward/**",
    "~/.config/ward/**": "~/.config/ward/**",
    "/run/user/*/ward/**": "//run/user/*/ward/**",
    "/media/**/ward-backup/**": "//media/**/ward-backup/**",
    "~/.ssh/**": "~/.ssh/**",
}

# A description tem de disparar nestes pedidos (minúsculas).
GATILHOS = ["login", "senha", "ssh", "token", "banco"]

# V8.2/RA3/V8.4: o que a skill precisa dizer.
OBRIGATORIOS_SKILL = [
    "base64",
    "hex",
    "xxd",
    "`rev`",
    "printenv",
    "set -x",
    "/proc/self/environ",
    "`env`",
    "anexo",
    "RA3",
    "git push",
    "~/.ssh",
    "PATH=/usr/local/bin:/usr/bin:/bin",
    "caminho absoluto",
    "travado",
    "pedido negado",
    "pedido expirado",
    "entrada inexistente",
]

PROIBIDOS = [
    (re.compile(r"keepass", re.I), "cita o KeePassXC"),
    (re.compile(r"cofre-[a-z]"), "cita script cofre-*"),
    (re.compile(r"--token"), "cita o --token-fd do app"),
]


class Falha(Exception):
    pass


def subcomandos_da_ajuda(ward):
    r = subprocess.run(
        [ward, "--help"],
        env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
        stdin=subprocess.DEVNULL,
        capture_output=True,
        timeout=30,
        check=False,
    )
    if r.returncode != 0:
        raise Falha(f"`ward --help` saiu {r.returncode}")
    subs = set(re.findall(r"^  ward ([a-z][a-z-]*)", r.stdout.decode("utf-8"), re.M))
    if not {"lista", "run", "ssh", "travar"} <= subs:
        raise Falha("`ward --help` sem os comandos básicos; binário errado?")
    return subs


def ler_frontmatter(texto):
    if not texto.startswith("---\n"):
        raise Falha("SKILL.md sem frontmatter na primeira linha")
    fim = texto.find("\n---\n", 4)
    if fim < 0:
        raise Falha("frontmatter sem o `---` de fechamento")
    campos = {}
    for linha in texto[4:fim].split("\n"):
        m = re.fullmatch(r"([a-z-]+): (.+)", linha)
        if not m:
            raise Falha("linha do frontmatter fora do formato `chave: valor`")
        if m.group(1) in campos:
            raise Falha(f"chave repetida no frontmatter: {m.group(1)}")
        campos[m.group(1)] = m.group(2).strip()
    if set(campos) != {"name", "description"}:
        raise Falha(f"frontmatter tem de ter só name e description: {sorted(campos)}")
    if campos["name"] != "ward":
        raise Falha("name do frontmatter não é `ward`")
    desc = campos["description"].lower()
    if not 100 <= len(desc) <= 1024:
        raise Falha("description com tamanho fora de 100–1024")
    faltam = [g for g in GATILHOS if g not in desc]
    if faltam:
        raise Falha(f"description não dispara em: {faltam}")


def trechos_de_codigo(texto):
    """Blocos ``` e trechos entre crases simples."""
    blocos = re.findall(r"^```[^\n]*\n(.*?)^```", texto, re.M | re.S)
    sem_blocos = re.sub(r"^```[^\n]*\n.*?^```", "", texto, flags=re.M | re.S)
    return blocos + re.findall(r"`([^`\n]+)`", sem_blocos)


def conferir_comandos(texto, ajuda, nome, avisos):
    for trecho in trechos_de_codigo(texto):
        for sub in re.findall(r"(?:^|[\s;|&(])ward\s+([^\s`]+)", trecho, re.M):
            if sub.startswith("<") or sub in ("[", "--help", "--version"):
                if sub.startswith("<"):
                    raise Falha(f"{nome}: `ward <…>` sem subcomando")
                continue
            if sub not in PERMITIDOS:
                raise Falha(f"{nome}: comando fora de V8.1: ward {sub}")
            if sub not in ajuda:
                if sub in PENDENTES:
                    avisos.add(sub)
                    continue
                raise Falha(f"{nome}: comando que não está no `ward --help`: ward {sub}")


def ler_padroes():
    try:
        st = os.stat(VARIACOES, follow_symlinks=False)
    except FileNotFoundError:
        raise Falha("falta .dev/variacoes.txt; rode `make dev-variacoes`") from None
    if not os.path.isfile(VARIACOES) or os.path.islink(VARIACOES) or st.st_mode & 0o077:
        raise Falha(".dev/variacoes.txt não é arquivo comum 0600")
    with open(VARIACOES, "rb") as f:
        padroes = [p for p in f.read().split(b"\n") if p]
    if len(padroes) < 50 or any(len(p) < 6 for p in padroes):
        raise Falha(".dev/variacoes.txt com padrões de menos ou curtos demais")
    return padroes


def conferir_segredos(arvore, padroes):
    for raiz, pastas, arquivos in os.walk(arvore):
        pastas.sort()
        for a in sorted(arquivos):
            caminho = os.path.join(raiz, a)
            if os.path.islink(caminho):
                raise Falha(f"link simbólico em skill/: {os.path.relpath(caminho, arvore)}")
            with open(caminho, "rb") as f:
                dados = f.read()
            achados = sum(1 for p in padroes if p in dados)
            if achados:
                rel = os.path.relpath(caminho, arvore)
                raise Falha(f"{rel}: {achados} padrão(ões) de .dev/variacoes.txt")


def sem_chave_repetida(pares):
    obj = {}
    for k, v in pares:
        if k in obj:
            raise Falha(f"chave repetida no JSON: {k}")
        obj[k] = v
    return obj


def conferir_deny(caminho):
    try:
        with open(caminho, encoding="utf-8") as f:
            dados = json.load(f, object_pairs_hook=sem_chave_repetida)
    except (OSError, ValueError) as e:
        raise Falha(f"settings-deny.json inválido: {type(e).__name__}") from None
    if not isinstance(dados, dict) or set(dados) != {"permissions"}:
        raise Falha("settings-deny.json tem de ter só `permissions`")
    perm = dados["permissions"]
    if not isinstance(perm, dict) or set(perm) != {"deny"}:
        raise Falha("`permissions` tem de ter só `deny` (nada de allow/ask)")
    deny = perm["deny"]
    if not isinstance(deny, list) or not all(isinstance(r, str) and r for r in deny):
        raise Falha("`deny` tem de ser lista de textos")
    if len(set(deny)) != len(deny):
        raise Falha("regra repetida no deny")
    for regra in deny:
        if not re.fullmatch(r"(Read|Edit|Bash)\([^()]+\)", regra):
            raise Falha(f"regra fora do formato: {regra}")
    for escopo, regra in CAMINHOS_DENY.items():
        for ferramenta in ("Read", "Edit"):
            if f"{ferramenta}({regra})" not in deny:
                raise Falha(f"deny sem {ferramenta}({regra}) (V8.2: {escopo})")
    for trecho in ("ward-backup", "/run/user/", ".local/share/ward", ".config/ward"):
        if not any(r.startswith("Bash(") and trecho in r for r in deny):
            raise Falha(f"deny sem regra Bash para {trecho}")


def conferir(arvore, ward):
    """Confere a árvore `arvore` (cópia de `skill/`). Devolve os pendentes avisados."""
    # Links primeiro: nada é lido por um link (um link para .dev/ traria segredo).
    for raiz, pastas, arquivos in os.walk(arvore):
        for a in pastas + arquivos:
            if os.path.islink(os.path.join(raiz, a)):
                raise Falha(f"link simbólico em skill/: {a}")
    ajuda = subcomandos_da_ajuda(ward)
    skill_md = os.path.join(arvore, "ward", "SKILL.md")
    try:
        with open(skill_md, encoding="utf-8") as f:
            texto = f.read()
    except (OSError, UnicodeDecodeError):
        raise Falha("skill/ward/SKILL.md ausente ou não UTF-8") from None
    ler_frontmatter(texto)
    avisos = set()
    # Comandos e citações proibidas: a skill inteira (`skill/ward/`). O README de
    # `skill/` é do usuário e descreve as regras de deny (cita o que elas barram).
    for raiz, _pastas, arquivos in os.walk(os.path.join(arvore, "ward")):
        for a in arquivos:
            if not a.endswith(".md"):
                continue
            caminho = os.path.join(raiz, a)
            rel = os.path.relpath(caminho, arvore)
            try:
                with open(caminho, encoding="utf-8") as f:
                    conteudo = f.read()
            except UnicodeDecodeError:
                raise Falha(f"{rel}: não é UTF-8") from None
            conferir_comandos(conteudo, ajuda, rel, avisos)
            for padrao, motivo in PROIBIDOS:
                if padrao.search(conteudo):
                    raise Falha(f"{rel}: {motivo}")
    for escopo in CAMINHOS_DENY:
        if escopo != "~/.ssh/**" and escopo not in texto:
            raise Falha(f"SKILL.md não manda evitar {escopo}")
    for termo in OBRIGATORIOS_SKILL:
        if termo not in texto:
            raise Falha(f"SKILL.md não cita {termo!r}")
    conferir_segredos(arvore, ler_padroes())
    conferir_deny(os.path.join(arvore, "settings-deny.json"))
    return avisos


def ataque(nome, mudar, ward, base):
    """Planta o ataque numa cópia e exige que a conferência falhe."""
    with tempfile.TemporaryDirectory(prefix="skill-ataque-") as tmp:
        copia = os.path.join(tmp, "skill")
        shutil.copytree(base, copia, symlinks=True)
        mudar(copia)
        try:
            conferir(copia, ward)
        except Falha as e:
            # O motivo nunca traz o padrão (só arquivo e contagem).
            print(f"  ok   ataque recusado: {nome} ({e})", flush=True)
            return
    raise Falha(f"ataque passou pela conferência: {nome}")


def acrescentar(rel, dados):
    def f(copia):
        with open(os.path.join(copia, rel), "ab") as arq:
            arq.write(dados)

    return f


def trocar(rel, velho, novo, todos=False):
    def f(copia):
        p = os.path.join(copia, rel)
        with open(p, encoding="utf-8") as arq:
            t = arq.read()
        if velho not in t:
            raise Falha(f"autoteste: trecho não achado em {rel}")
        with open(p, "w", encoding="utf-8") as arq:
            arq.write(t.replace(velho, novo) if todos else t.replace(velho, novo, 1))

    return f


def tirar_deny(regra):
    def f(copia):
        p = os.path.join(copia, "settings-deny.json")
        with open(p, encoding="utf-8") as arq:
            d = json.load(arq)
        d["permissions"]["deny"].remove(regra)
        with open(p, "w", encoding="utf-8") as arq:
            json.dump(d, arq)

    return f


def autoteste(ward, base):
    # Controle: a árvore real passa; senão todo ataque "falharia" à toa.
    conferir(base, ward)
    padroes = ler_padroes()
    # Um segredo inteiro, uma variação (a do meio da lista) e a última, em lugares
    # diferentes: o exemplo, um arquivo novo e o JSON.
    for i, (rel, p) in enumerate(
        [
            ("ward/SKILL.md", padroes[0]),
            ("ward/referencia.md", padroes[len(padroes) // 2]),
            ("settings-deny.json", padroes[-1]),
        ]
    ):
        ataque(f"segredo de teste plantado ({i + 1}/3)", acrescentar(rel, b"\n" + p + b"\n"),
               ward, base)
    ataque("comando inexistente", acrescentar("ward/SKILL.md", b"\n`ward revelar acme-pg-dev`\n"),
           ward, base)
    ataque("comando fora de V8.1",
           acrescentar("ward/SKILL.md", b"\n```sh\nward destravar\n```\n"), ward, base)
    ataque("script antigo", acrescentar("ward/SKILL.md", b"\nuse `cofre-run`\n"), ward, base)
    ataque("KeePassXC", acrescentar("ward/SKILL.md", b"\nKeePassXC\n"), ward, base)
    ataque("comando inexistente em outro arquivo da skill",
           acrescentar("ward/referencia.md", b"\n```sh\nward exportar --tudo\n```\n"), ward, base)
    ataque("token no argv", acrescentar("ward/SKILL.md", b"\n`ward ssh --token-fd 3 x`\n"),
           ward, base)
    ataque("frontmatter com outro nome", trocar("ward/SKILL.md", "name: ward", "name: cofre"),
           ward, base)
    ataque("description sem gatilho de banco",
           trocar("ward/SKILL.md", "banco", "base", todos=True), ward, base)
    ataque("JSON inválido", acrescentar("settings-deny.json", b",}"), ward, base)
    ataque("JSON com chave repetida",
           trocar("settings-deny.json", '"permissions": {', '"permissions": {"deny": [],'),
           ward, base)
    ataque("JSON com allow", trocar("settings-deny.json", '"deny": [',
                                    '"allow": ["Bash(*)"], "deny": ['), ward, base)
    for regra in CAMINHOS_DENY.values():
        ataque(f"deny sem Read({regra})", tirar_deny(f"Read({regra})"), ward, base)
    ataque("skill sem a regra de base64", trocar("ward/SKILL.md", "base64", "b64", todos=True),
           ward, base)
    ataque("skill sem o caminho do backup",
           trocar("ward/SKILL.md", "/media/**/ward-backup/**", "pendrive"),
           ward, base)

    def link(copia):
        os.symlink(VARIACOES, os.path.join(copia, "ward", "variacoes.md"))

    ataque("link para fora de skill/", link, ward, base)


def main():
    if len(sys.argv) != 2:
        print("uso: skill.py <binário ward>", file=sys.stderr)
        return 2
    ward = os.path.realpath(sys.argv[1])
    base = os.path.join(RAIZ, "skill")
    try:
        print("teste-skill: autoteste (cada ataque tem de falhar)", flush=True)
        autoteste(ward, base)
        print("teste-skill: árvore real", flush=True)
        avisos = conferir(base, ward)
    except Falha as e:
        print(f"teste-skill: FALHOU: {e}", file=sys.stderr)
        return 1
    for sub in sorted(avisos):
        print(f"  aviso: `ward {sub}` ainda não está no `ward --help` (pendente da Fase 43)")
    print("teste-skill: ok (V8.1, V8.2, V8.4)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
