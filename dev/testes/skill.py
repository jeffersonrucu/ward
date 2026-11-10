#!/usr/bin/python3 -IB
"""Confere a skill `ward` e o trecho de deny (Fase 50, V8.1, V8.2, V8.4). Sai 1 se algo
falhar.

1. `skill/ward/SKILL.md` tem frontmatter só com `name: ward` e uma `description` que
   dispara em pedidos de login, senha, SSH, token e banco.
2. Todo comando `ward <sub>` citado em código (bloco ou trecho entre crases) em
   qualquer `.md` de `skill/ward/` está no `ward --help` do binário dado e na lista de
   V8.1 (lista, run, ssh, login, playwright, travar). `ward login` (Fase 43) é aceito
   como pendente enquanto não estiver no `--help`, com aviso.
3. A skill não cita o KeePass XC nem os scripts `cofre-*` (V8.1), nem o `--token-fd`.
4. Nenhum padrão de `.dev/variacoes.txt` (segredos do cofre de teste e variações)
   aparece em nenhum arquivo de `skill/` (nunca imprime o padrão; só o arquivo).
5. `skill/settings-deny.json` é JSON válido (sem chave repetida), só com
   `permissions.deny`, e cobre `Read` e `Edit` de cada caminho de V8.2 e da pasta do ssh.
6. A skill manda nunca ler esses caminhos nem imprimir variável ou anexo em nenhuma
   codificação (V8.2) e diz que o SSH da pasta do ssh do usuário é direto, sem o Ward
   (V8.4).
7. (Fase 50a; Fase 54b) As regras `Bash` do deny, lidas como o Claude Code lê (`*` = qualquer
   texto), negam cada comando de `ATAQUES_BASH` (env, `sh -c`, od, rev, fold, tr,
   cut, `python3 -c`, `>`, `/usr/local/bin/ward run`, curingas nos caminhos do Ward,
   `/proc/N/env*`…), olhando o comando inteiro e cada subcomando, e não negam nenhum
   comando de `LEGITIMOS` nem dos blocos ```sh da skill. Os de `ATAQUES_BASH_INTEIRO`
   (script por pipe para o `ward run`, Fase 54b) só são exigidos no comando inteiro:
   separado em subcomandos, o `|` some.
8. (Fase 54a, RA8) A skill e o `skill/README.md` mandam o usuário destravar no terminal
   por `/usr/local/bin/ward destravar`, e a skill não manda destravar pelo `ward` do
   `PATH` (um `ward` falso antes no `PATH` lê a senha mestra).

Os nomes da pasta do ssh, do cofre e do KeePass XC vêm montados por partes (`SSH`,
`COFRE`), para a varredura do V1.5 (`dev/testes/ataques-fundacao.sh`): aqui são texto
da skill e do deny, nunca pasta aberta.

Antes de conferir a árvore real, o autoteste confere que ela passa (controle) e planta cada ataque numa cópia de `skill/`
(segredo, comando inventado, `cofre-run`, JSON inválido, caminho de deny faltando,
frontmatter errado, regra Bash ancorada ou faltando, regra que nega o uso normal,
exemplo da skill que o deny nega) e exige que a conferência falhe.

Uso: python3 -I -B dev/testes/skill.py target/sem-modo-teste/debug/ward
     (ou `make teste-skill`)
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

# Montados por partes (V1.5, ver o topo).
SSH = "~/." + "ssh"
COFRE = ".local/share/" + "ward"

# V8.2: cada caminho, na sintaxe de regra do Claude Code (`~/` = home, `//` = absoluto).
CAMINHOS_DENY = {
    f"~/{COFRE}/**": f"~/{COFRE}/**",
    "~/.config/ward/**": "~/.config/ward/**",
    "/run/user/*/ward/**": "//run/user/*/ward/**",
    "/media/**/ward-backup/**": "//media/**/ward-backup/**",
    "/run/media/**/ward-backup/**": "//run/media/**/ward-backup/**",
    f"{SSH}/**": f"{SSH}/**",
}

# Regras `Bash` (cinto de segurança, RA3; Fase 50a): cada comando de ATAQUES_BASH tem
# de ser negado, e nenhum comando de exemplo da skill (blocos ```sh) nem de LEGITIMOS
# pode ser. O Claude Code compara o texto da regra com `*` como curinga; não está
# conferido (M10) se ele olha o comando inteiro ou cada subcomando (`&&`, `;`, `|`),
# então os dois modelos são exigidos. `{e}` = entrada de banco, `{a}` = entrada com
# anexo, `{h}` = home (trocados por `str.replace`, sem `format`).
ATAQUES_BASH = [
    # Formas que escapavam das regras com âncora `ward run * -- …` (auditoria do M8).
    "ward run {e} -- /usr/bin/env",
    "ward run {e} -- env",
    "/usr/local/bin/ward run {e} -- printenv PGPASSWORD",
    "cd /tmp && ward run {e} -- env",
    "ward  run {e} -- env",
    '"ward" run {e} -- env',
    "ward run {e} -- sh -c env",
    "ward run {e} -- sh -c 'set'",
    'ward run {e} -- bash -c "set"',
    "ward run {e} -- sh -c 'true;set'",
    "ward run {e} -- sh -c 'export -p'",
    "ward run {e} -- bash -c 'declare -p PGPASSWORD'",
    "ward run {e} -- bash -c 'typeset -p'",
    "ward run {e} -- bash -c 'compgen -v'",
    "ward run {e} -- bash -xc '/usr/bin/psql -c 1'",
    "ward run {e} -- sh -x -c '/usr/bin/psql'",
    "ward run {e} -- sh -c 'set -x; /usr/bin/psql'",
    "ward run {e} -- bash -o xtrace -c '/usr/bin/psql'",
    "ward run {e} -- sh -c 'echo $PGPASSWORD'",
    "ward run {e} -- sh -c 'echo \"${PGPASSWORD}\"'",
    "ward run {e} -- sh -c 'printf %s \"$PGPASSWORD\"'",
    "ward run {e} -- sh -c 'cat <<<$PGPASSWORD'",
    "ward run {e} -- sh -c 'od -c /proc/self/environ'",
    "ward run {e} -- /usr/bin/od -An -c /proc/self/environ",
    "ward run {e} -- sh -c 'cat /proc/$$/environ | od -c'",
    "ward run {e} -- sh -c 'base64 -w4 /proc/self/environ'",
    "ward run {e} -- sh -c 'basenc --base16 /proc/self/environ'",
    "ward run {e} -- python3 -c 'import os; print(os.environ[\"PGPASSWORD\"][::-1])'",
    "ward run {e} -- python3 -c 'import os; print(os.getenv(\"PGPASSWORD\")[::-1])'",
    "ward run {e} -- /usr/bin/python3 -c 'import os; print(os.environb)'",
    "ward run {e} -- node -e 'console.log(process.env)'",
    "ward run {e} -- node -p 'process.env.PGPASSWORD'",
    "ward run {e} -- perl -e 'print $ENV{PGPASSWORD}'",
    "ward run {e} -- ruby -e 'puts ENV[\"PGPASSWORD\"]'",
    "ward run {e} -- awk 'BEGIN{print ENVIRON[\"PGPASSWORD\"]}'",
    "ward run {e} -- php -r 'echo getenv(\"PGPASSWORD\");'",
    "ward run {e} -- sh -c 'od -c <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'rev /dev/stdin <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c '/usr/bin/rev <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'tr a-z n-za-m <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'cut -c1-4 <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'fold -w1 <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'xxd <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'hexdump -C <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'base32 <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'head -c 4 <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'openssl enc -base64 <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- /usr/bin/psql -c 'select 1' > /tmp/saida",
    "ward run {e} -- sh -c '/usr/bin/psql -c 1 2>&1'",
    "ward run {e} -- printenv > /tmp/x",
    "ward run {e} -- sh -c 'tee /tmp/x <<X\n$PGPASSWORD\nX'",
    "ward run {e} -- sh -c 'cp {h}/.pgpass /tmp'",
    "ward run {e} -- sh -c 'git config --global user.senha \"$PGPASSWORD\"'",
    "ward run {a} -- sh -c 'cat \"$WARD_ARQUIVO_CHAVE\"'",
    "ward run {a} -- sh -c 'head -n 3 \"$WARD_ARQUIVO_CHAVE\"'",
    "ward run {a} -- sh -c 'cp \"$WARD_ARQUIVO_CHAVE\" /tmp/k'",
    "ward run {a} -- sh -c 'base64 \"$WARD_ARQUIVO_CHAVE\"'",
    "ward run {a} -- sh -c 'xxd < \"$WARD_ARQUIVO_CHAVE\"'",
    "ward run {a} -- sh -c 'ln -s \"$WARD_ARQUIVO_CHAVE\" /tmp/k'",
    # Fase 54b: o `od` sem `<` nem `|` (as regras de entrada padrão pegam os outros).
    "ward run {a} -- sh -c 'od -c \"$WARD_ARQUIVO_CHAVE\"'",
    # Caminhos do Ward: por extenso, com curinga, por variável e por `cd`.
    "cat {h}/" + COFRE + "/cofre.ward",
    "ls ~/.local/share/w*",
    "ls ~/.local/sh*/w*",
    "cp ~/.local/share/w?rd/* /tmp",
    "ls ~/.local/*/w?rd",
    "ls $HOME/.local/share/[w]ard",
    'ls "$XDG_DATA_HOME"/w*',
    "cd ~/.local && ls share/w*",
    "cat ~/.config/ward/x",
    "cat ~/.config/w*/x",
    "cat /run/user/1000/ward/tmp/x",
    "ls /run/user/*/w*",
    "cat /run/user/1000/ward/playwright/acme-wp-prod.json",
    'ls "$XDG_RUNTIME_DIR"/ward',
    "cd /run/user/1000 && ls ward",
    "ls /media/u/PEN/ward-backup",
    "ls /run/media/u/PEN/*",
    "cat /proc/self/environ",
    "cat /proc/1234/env*",
    "cat /proc/*/environ",
    "cat /proc/1234/e?viron",
    "cd /proc/1234 && cat env*",
    "cd /proc && cat 1234/env*",
    "cd / && cat proc/1234/env*",
    "cat " + SSH + "/id_ed25519",
    "cat " + SSH + "/id*",
    "ward destravar",
    "/usr/local/bin/ward ssh --token-fd 3 acme-ssh-senha-stage id",
    # Fase 54b (D68): o script pela entrada padrão, que a janela de aprovação não
    # mostra: arquivo, here-doc e here-string, depois e antes do `ward run`.
    "ward run {e} -- /usr/bin/python3 < /tmp/script.py",
    "ward run {e} -- /usr/bin/python3 -I </tmp/script.py",
    "ward run {e} -- /usr/bin/python3 0< /tmp/script.py",
    "< /tmp/script.py ward run {e} -- /usr/bin/python3",
    "cd /tmp && <script.py /usr/local/bin/ward run {e} -- python3",
    "ward run {e} -- /usr/bin/python3 <<EOF\nimport os\nprint(os.environ['PGPASSWORD'][::-1])\nEOF",
    "ward run {e} -- /usr/bin/python3 <<'EOF'\nimport os\nEOF",
    "ward run {e} -- /usr/bin/python3 <<< 'import os; print(os.environ)'",
    "ward run {e} -- /usr/bin/psql <<< 'select 1'",
    "ward run {e} -- /usr/bin/psql < /tmp/consulta.sql",
    "ward run {e} -- /usr/bin/python3 <(cat /tmp/script.py)",
]

# Fase 54b (D68): o script por pipe para o `ward run`. Só o modelo do comando inteiro
# vê o `|`: separado em subcomandos (`printf …` e `ward run … -- python3`), nenhum
# pedaço é ataque. Quem barra de verdade é o daemon (aviso e confirmação na janela);
# a regra é cinto de segurança. Exigido só no comando inteiro.
ATAQUES_BASH_INTEIRO = [
    "printf 'import os\nprint(os.environ[\"PGPASSWORD\"][::-1])\n' | ward run {e} -- /usr/bin/python3",
    "cat /tmp/script.py | ward run {e} -- /usr/bin/python3",
    "echo 'select 1' | ward run {e} -- /usr/bin/psql",
    "cat /tmp/consulta.sql|/usr/local/bin/ward run {e} -- /usr/bin/psql",
    "printf x |& ward run {e} -- /usr/bin/python3",
]

# Comandos de uso normal que nenhuma regra pode negar (além dos blocos ```sh da skill).
LEGITIMOS = [
    "ward lista",
    "ward lista --json",
    "ward travar",
    "ward ssh acme-ssh-senha-stage -- uname -a",
    "ward run acme-pg-dev -- /usr/bin/psql -tAc 'select count(*) from pedidos'",
    "ward run beta-aws-dev -- /usr/local/bin/aws s3 ls",
    "ward run beta-github-api -- /usr/bin/curl -sf --variable %GITHUB_TOKEN"
    " --expand-header 'Authorization: Bearer {{GITHUB_TOKEN}}' https://api.github.com/user",
    "ward run acme-wp-dev -- /bin/sh -c '/usr/bin/curl -sf -u \"$WARD_USER:$WARD_PASS\""
    " \"$WARD_URL/wp-json/wp/v2/users/me\"'",
    'WARD_ESTADO="$(ward playwright acme-wp-prod)" /usr/bin/python3 tests/test_painel.py',
    "ls ~/.local/bin",
    "~/.local/bin/uv run pytest -q",
    "/usr/bin/env python3 scripts/x.py",
    "git push",
    "ssh git@github.com",
]

# Fase 54a (RA8): o destrave do usuário no terminal, pelo caminho completo.
DESTRAVAR = "/usr/local/bin/ward destravar"

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
    SSH,
    "PATH=/usr/local/bin:/usr/bin:/bin",
    "caminho absoluto",
    "travado",
    "pedido negado",
    "pedido expirado",
    "entrada inexistente",
]

PROIBIDOS = [
    (re.compile(r"keepass", re.I), "cita o KeePass XC"),
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
        # Sem parêntese dentro: o leitor de regras do Claude Code pode cortar no `)`.
        if not re.fullmatch(r"(Read|Edit|Bash)\([^()]+\)", regra):
            raise Falha(f"regra fora do formato: {regra}")
    for escopo, regra in CAMINHOS_DENY.items():
        for ferramenta in ("Read", "Edit"):
            if f"{ferramenta}({regra})" not in deny:
                raise Falha(f"deny sem {ferramenta}({regra}) (V8.2: {escopo})")
    return [r[len("Bash(") : -1] for r in deny if r.startswith("Bash(")]


def padrao_regex(padrao):
    """Regra `Bash(...)` do Claude Code: `*` casa qualquer texto; o resto é literal."""
    return re.compile("".join(".*" if c == "*" else re.escape(c) for c in padrao), re.S)


def subcomandos(comando):
    """Parte o comando em `;`, `&`, `|` e fim de linha fora de aspas (aproximação do
    que o Claude Code faz com `&&`, `||`, `;` e `|`)."""
    partes, atual, aspa, i = [], [], None, 0
    while i < len(comando):
        c = comando[i]
        if aspa:
            if c == "\\" and aspa == '"' and i + 1 < len(comando):
                atual.append(comando[i : i + 2])
                i += 2
                continue
            if c == aspa:
                aspa = None
        elif c in "'\"":
            aspa = c
        elif c in ";&|\n":
            partes.append("".join(atual))
            atual = []
            i += 1
            continue
        atual.append(c)
        i += 1
    partes.append("".join(atual))
    return [p.strip() for p in partes if p.strip()]


def negado(regras, comando):
    """(inteiro, por subcomando): se alguma regra nega, em cada modelo."""
    inteiro = any(r.fullmatch(comando.strip()) for r in regras)
    partes = any(r.fullmatch(p) for r in regras for p in subcomandos(comando))
    return inteiro, partes


def comandos_da_skill(texto):
    """Linhas dos blocos ```sh da skill, sem comentários."""
    linhas = []
    for bloco in re.findall(r"^```sh\n(.*?)^```", texto, re.M | re.S):
        for linha in bloco.split("\n"):
            linha = re.sub(r"\s+#.*$", "", linha).strip()
            if linha and not linha.startswith("#"):
                linhas.append(linha)
    return linhas


def conferir_bash(padroes, texto):
    regras = [padrao_regex(p) for p in padroes]
    trocas = {"{e}": "acme-pg-dev", "{a}": "acme-ssh-chave-prod", "{h}": "$HOME"}
    for modelo in ATAQUES_BASH:
        comando = modelo
        for velho, novo in trocas.items():
            comando = comando.replace(velho, novo)
        inteiro, partes = negado(regras, comando)
        if not (inteiro and partes):
            qual = "inteiro" if not inteiro else "por subcomando"
            raise Falha(f"deny Bash não nega ({qual}): {comando!r}")
    for modelo in ATAQUES_BASH_INTEIRO:
        comando = modelo
        for velho, novo in trocas.items():
            comando = comando.replace(velho, novo)
        inteiro, _partes = negado(regras, comando)
        if not inteiro:
            raise Falha(f"deny Bash não nega (inteiro): {comando!r}")
    comandos = comandos_da_skill(texto)
    if len(comandos) < 8:
        raise Falha("SKILL.md com poucos exemplos em blocos ```sh")
    for comando in LEGITIMOS + comandos:
        if any(negado(regras, comando)):
            raise Falha(f"deny Bash nega comando legítimo: {comando!r}")


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
        if escopo != f"{SSH}/**" and f"`{escopo}`" not in texto:
            raise Falha(f"SKILL.md não manda evitar {escopo}")
    for termo in OBRIGATORIOS_SKILL:
        if termo not in texto:
            raise Falha(f"SKILL.md não cita {termo!r}")
    # Fase 54a (RA8): a senha mestra só pelo `ward` instalado, pelo caminho completo.
    if DESTRAVAR not in texto:
        raise Falha(f"SKILL.md não manda destravar por {DESTRAVAR} (RA8)")
    if re.search(r"(?<!/usr/local/bin/)\bward\s+destravar\b", texto):
        raise Falha("SKILL.md manda destravar pelo `ward` do PATH (RA8)")
    try:
        with open(os.path.join(arvore, "README.md"), encoding="utf-8") as f:
            leia = f.read()
    except (OSError, UnicodeDecodeError):
        raise Falha("skill/README.md ausente ou não UTF-8") from None
    if f"`{DESTRAVAR}`" not in leia:
        raise Falha(f"skill/README.md não manda destravar por `{DESTRAVAR}` (RA8)")
    conferir_segredos(arvore, ler_padroes())
    padroes_bash = conferir_deny(os.path.join(arvore, "settings-deny.json"))
    conferir_bash(padroes_bash, texto)
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


def tirar_deny(*regras):
    def f(copia):
        p = os.path.join(copia, "settings-deny.json")
        with open(p, encoding="utf-8") as arq:
            d = json.load(arq)
        for regra in regras:
            d["permissions"]["deny"].remove(regra)
        with open(p, "w", encoding="utf-8") as arq:
            json.dump(d, arq)

    return f


def acrescentar_deny(regra):
    def f(copia):
        p = os.path.join(copia, "settings-deny.json")
        with open(p, encoding="utf-8") as arq:
            d = json.load(arq)
        d["permissions"]["deny"].append(regra)
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
    ataque("KeePass XC", acrescentar("ward/SKILL.md", b"\nKeePass" b"XC\n"), ward, base)
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
           trocar("ward/SKILL.md", "`/media/**/ward-backup/**`", "pendrive"),
           ward, base)

    # Fase 50a: as regras Bash ancoradas no início deixavam passar estas formas.
    ataque("deny com a regra de env ancorada no início",
           trocar("settings-deny.json", "Bash(*ward*run*env*)", "Bash(ward run * -- env*)"),
           ward, base)
    ataque("deny sem a regra de env", tirar_deny("Bash(*ward*run*env*)"), ward, base)
    ataque("deny sem as regras de od",
           tirar_deny(*(f"Bash(*ward*run*{b}od*)" for b in (" ", "/", "'", "\"", "|"))),
           ward, base)
    ataque("deny sem a regra de redirecionamento", tirar_deny("Bash(*ward*run*>*)"), ward, base)
    # Fase 54b (D68): a entrada padrão que a janela não mostra.
    ataque("deny sem a regra de entrada por arquivo", tirar_deny("Bash(*ward*run*<*)"),
           ward, base)
    ataque("deny sem a regra de entrada antes do ward", tirar_deny("Bash(*<*ward*run*)"),
           ward, base)
    ataque("deny sem a regra de pipe para o ward run", tirar_deny("Bash(*|*ward*run*)"),
           ward, base)
    ataque("deny sem a regra de /run/user", tirar_deny("Bash(*/run/user*)"), ward, base)
    ataque("deny sem a regra de /proc", tirar_deny("Bash(*/proc/*)"), ward, base)
    ataque("deny que nega o uso normal", acrescentar_deny("Bash(*ward run*)"), ward, base)
    ataque("deny com parênteses no padrão", acrescentar_deny("Bash(*ward*run*(set*)"), ward, base)
    ataque("skill com o caminho do storage state num comando",
           trocar("ward/SKILL.md",
                  'WARD_ESTADO="$(ward playwright acme-wp-prod)" /usr/bin/python3 tests/test_painel.py',
                  "/usr/bin/python3 tests/test_painel.py /run/user/1000/ward/playwright/x.json"),
           ward, base)
    ataque("skill com exemplo que imprime o token",
           acrescentar("ward/SKILL.md",
                       b"\n```sh\nward run beta-github-api -- sh -c 'echo $GITHUB_TOKEN'\n```\n"),
           ward, base)
    ataque("skill sem o backup em /run/media",
           trocar("ward/SKILL.md", "/run/media/**/ward-backup/**", "pendrive"), ward, base)

    # Fase 54a (RA8): destrave pelo `ward` do PATH, que um `ward` falso imita.
    ataque("skill manda destravar pelo PATH",
           trocar("ward/SKILL.md", DESTRAVAR, "ward destravar"), ward, base)
    ataque("skill manda destravar por outro ward",
           trocar("ward/SKILL.md", DESTRAVAR, "~/.local/bin/ward destravar"), ward, base)
    ataque("skill cita destravar pelo PATH além do caminho completo",
           acrescentar("ward/SKILL.md", b"\nOu rode ward  destravar no terminal.\n"), ward, base)
    ataque("README sem o caminho completo",
           trocar("README.md", f"`{DESTRAVAR}`", "`ward destravar`", todos=True), ward, base)

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
