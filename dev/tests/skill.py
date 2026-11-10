#!/usr/bin/python3 -IB
"""Verifies the `ward` skill and the deny excerpt (Phase 50, V8.1, V8.2, V8.4). Exits 1
if anything fails.

1. `skill/ward/SKILL.md` has a frontmatter with only `name: ward` and a `description`
   that triggers on requests for login, password, SSH, token and database.
2. Every `ward <sub>` command quoted in code (block or excerpt between backticks) in any
   `.md` of `skill/ward/` is in the `ward --help` of the given binary and in the V8.1
   list (`ALLOWED`: list, run, ssh, login, playwright, lock). `ward login` (Phase 43) is
   accepted as pending while it is not in `--help`, with a notice.
3. The skill does not mention KeePass XC, its old helper scripts (V8.1) or `--token-fd`.
4. No pattern of `.dev/variations.txt` (secrets of the test vault and variations) shows
   up in any file of `skill/` (never prints the pattern; only the file).
5. `skill/settings-deny.json` is valid JSON (no repeated key), with only
   `permissions.deny`, and covers `Read` and `Edit` of every V8.2 path and of the ssh
   dir.
6. The skill says never to read those paths nor print a variable or attachment in any
   encoding (V8.2) and says that SSH from the user's ssh dir is direct, without Ward
   (V8.4).
7. (Phase 50a; Phase 54b) The deny `Bash` rules, read the way Claude Code reads them
   (`*` = any text), deny every command of `BASH_ATTACKS` (env, `sh -c`, od, rev, fold,
   tr, cut, `python3 -c`, `>`, `/usr/local/bin/ward run`, wildcards in Ward's paths,
   `/proc/N/env*`…), looking at the whole command and at each subcommand, and deny no
   command of `LEGITIMATE` nor of the skill's ```sh blocks. The ones in
   `WHOLE_BASH_ATTACKS` (script piped into `ward run`, Phase 54b) are only required on
   the whole command: split into subcommands, the `|` goes away.
8. (Phase 54a, RA8) The skill and `skill/README.md` tell the user to unlock in the
   terminal by the full path of the installed `ward` (`UNLOCK`), and the skill does not
   say to unlock through the `ward` of the `PATH` (a fake `ward` earlier in the `PATH`
   reads the master password).

The names of the ssh dir, of the vault and of KeePass XC are built in pieces (`SSH`,
`VAULT`), for the V1.5 scan (`dev/tests/attacks-foundation.sh`): here they are text of
the skill and of the deny, never an opened dir.

Before verifying the real tree, the self-test verifies that it passes (control) and plants
each attack in a copy of `skill/` (secret, made-up command, old helper script, invalid JSON,
missing deny path, wrong frontmatter, anchored or missing Bash rule, rule that denies the
normal use, skill example that the deny denies) and requires the verification to fail.

Usage: python3 -I -B dev/tests/skill.py target/no-test-mode/debug/ward
       (or `make test-skill`)
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
VARIATIONS = os.path.join(ROOT, ".dev", "variations.txt")

# V8.1: the only subcommands the skill may use.
ALLOWED = {"lista", "run", "ssh", "login", "playwright", "travar"}
# Subcommands accepted outside `--help` while their phase has not come (none since
# Phase 43).
PENDING = set()

# Built in pieces (V1.5, see the top).
SSH = "~/." + "ssh"
VAULT = ".local/share/" + "ward"

# V8.2: each path, in Claude Code rule syntax (`~/` = home, `//` = absolute).
DENY_PATHS = {
    f"~/{VAULT}/**": f"~/{VAULT}/**",
    "~/.config/ward/**": "~/.config/ward/**",
    "/run/user/*/ward/**": "//run/user/*/ward/**",
    "/media/**/ward-backup/**": "//media/**/ward-backup/**",
    "/run/media/**/ward-backup/**": "//run/media/**/ward-backup/**",
    f"{SSH}/**": f"{SSH}/**",
}

# `Bash` rules (seat belt, RA3; Phase 50a): every command of BASH_ATTACKS has to be
# denied, and no example command of the skill (```sh blocks) nor of LEGITIMATE may be.
# Claude Code compares the rule text with `*` as a wildcard; it is not verified (M10)
# whether it looks at the whole command or at each subcommand (`&&`, `;`, `|`), so both
# models are required. `{e}` = database entry, `{a}` = entry with an attachment,
# `{h}` = home (swapped by `str.replace`, without `format`).
BASH_ATTACKS = [
    # Forms that escaped the rules anchored at `ward run * -- …` (M8 audit).
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
    # Phase 54b: `od` without `<` or `|` (the standard input rules catch the others).
    "ward run {a} -- sh -c 'od -c \"$WARD_ARQUIVO_CHAVE\"'",
    # Ward's paths: spelled out, with a wildcard, through a variable and through `cd`.
    "cat {h}/" + VAULT + "/cofre.ward",
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
    # Phase 54b (D68): the script through standard input, which the approval window does
    # not show: file, here-doc and here-string, after and before `ward run`.
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

# Phase 54b (D68): the script piped into `ward run`. Only the whole-command model sees
# the `|`: split into subcommands (`printf …` and `ward run … -- python3`), no piece is
# an attack. What really blocks it is the daemon (warning and confirmation in the
# window); the rule is a seat belt. Required only on the whole command.
WHOLE_BASH_ATTACKS = [
    "printf 'import os\nprint(os.environ[\"PGPASSWORD\"][::-1])\n' | ward run {e} -- /usr/bin/python3",
    "cat /tmp/script.py | ward run {e} -- /usr/bin/python3",
    "echo 'select 1' | ward run {e} -- /usr/bin/psql",
    "cat /tmp/consulta.sql|/usr/local/bin/ward run {e} -- /usr/bin/psql",
    "printf x |& ward run {e} -- /usr/bin/python3",
]

# Normal-use commands that no rule may deny (besides the skill's ```sh blocks).
LEGITIMATE = [
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

# Phase 54a (RA8): the user's unlock in the terminal, by the full path.
UNLOCK = "/usr/local/bin/ward destravar"

# The description has to trigger on these requests (lowercase).
TRIGGERS = ["login", "senha", "ssh", "token", "banco"]

# V8.2/RA3/V8.4: what the skill has to say.
REQUIRED_IN_SKILL = [
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

FORBIDDEN = [
    (re.compile(r"keepass", re.I), "cita o KeePass XC"),
    (re.compile(r"cofre-[a-z]"), "cita script cofre-*"),
    (re.compile(r"--token"), "cita o --token-fd do app"),
]


class Failure(Exception):
    pass


def help_subcommands(ward):
    r = subprocess.run(
        [ward, "--help"],
        env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
        stdin=subprocess.DEVNULL,
        capture_output=True,
        timeout=30,
        check=False,
    )
    if r.returncode != 0:
        raise Failure(f"`ward --help` saiu {r.returncode}")
    subs = set(re.findall(r"^  ward ([a-z][a-z-]*)", r.stdout.decode("utf-8"), re.M))
    if not {"lista", "run", "ssh", "travar"} <= subs:
        raise Failure("`ward --help` sem os comandos básicos; binário errado?")
    return subs


def read_frontmatter(text):
    if not text.startswith("---\n"):
        raise Failure("SKILL.md sem frontmatter na primeira linha")
    end = text.find("\n---\n", 4)
    if end < 0:
        raise Failure("frontmatter sem o `---` de fechamento")
    fields = {}
    for line in text[4:end].split("\n"):
        m = re.fullmatch(r"([a-z-]+): (.+)", line)
        if not m:
            raise Failure("linha do frontmatter fora do formato `chave: valor`")
        if m.group(1) in fields:
            raise Failure(f"chave repetida no frontmatter: {m.group(1)}")
        fields[m.group(1)] = m.group(2).strip()
    if set(fields) != {"name", "description"}:
        raise Failure(f"frontmatter tem de ter só name e description: {sorted(fields)}")
    if fields["name"] != "ward":
        raise Failure("name do frontmatter não é `ward`")
    desc = fields["description"].lower()
    if not 100 <= len(desc) <= 1024:
        raise Failure("description com tamanho fora de 100–1024")
    missing = [g for g in TRIGGERS if g not in desc]
    if missing:
        raise Failure(f"description não dispara em: {missing}")


def code_excerpts(text):
    """``` blocks and excerpts between single backticks."""
    blocks = re.findall(r"^```[^\n]*\n(.*?)^```", text, re.M | re.S)
    without_blocks = re.sub(r"^```[^\n]*\n.*?^```", "", text, flags=re.M | re.S)
    return blocks + re.findall(r"`([^`\n]+)`", without_blocks)


def verify_commands(text, help_subs, name, notices):
    for excerpt in code_excerpts(text):
        for sub in re.findall(r"(?:^|[\s;|&(])ward\s+([^\s`]+)", excerpt, re.M):
            if sub.startswith("<") or sub in ("[", "--help", "--version"):
                if sub.startswith("<"):
                    raise Failure(f"{name}: `ward <…>` sem subcomando")
                continue
            if sub not in ALLOWED:
                raise Failure(f"{name}: comando fora de V8.1: ward {sub}")
            if sub not in help_subs:
                if sub in PENDING:
                    notices.add(sub)
                    continue
                raise Failure(f"{name}: comando que não está no `ward --help`: ward {sub}")


def read_patterns():
    try:
        st = os.stat(VARIATIONS, follow_symlinks=False)
    except FileNotFoundError:
        raise Failure("falta .dev/variations.txt; rode `make dev-variations`") from None
    if not os.path.isfile(VARIATIONS) or os.path.islink(VARIATIONS) or st.st_mode & 0o077:
        raise Failure(".dev/variations.txt não é arquivo comum 0600")
    with open(VARIATIONS, "rb") as f:
        patterns = [p for p in f.read().split(b"\n") if p]
    if len(patterns) < 50 or any(len(p) < 6 for p in patterns):
        raise Failure(".dev/variations.txt com padrões de menos ou curtos demais")
    return patterns


def verify_secrets(tree, patterns):
    for root, dirs, files in os.walk(tree):
        dirs.sort()
        for a in sorted(files):
            path = os.path.join(root, a)
            if os.path.islink(path):
                raise Failure(f"link simbólico em skill/: {os.path.relpath(path, tree)}")
            with open(path, "rb") as f:
                data = f.read()
            found = sum(1 for p in patterns if p in data)
            if found:
                rel = os.path.relpath(path, tree)
                raise Failure(f"{rel}: {found} padrão(ões) de .dev/variations.txt")


def no_repeated_key(pairs):
    obj = {}
    for k, v in pairs:
        if k in obj:
            raise Failure(f"chave repetida no JSON: {k}")
        obj[k] = v
    return obj


def verify_deny(path):
    try:
        with open(path, encoding="utf-8") as f:
            data = json.load(f, object_pairs_hook=no_repeated_key)
    except (OSError, ValueError) as e:
        raise Failure(f"settings-deny.json inválido: {type(e).__name__}") from None
    if not isinstance(data, dict) or set(data) != {"permissions"}:
        raise Failure("settings-deny.json tem de ter só `permissions`")
    perm = data["permissions"]
    if not isinstance(perm, dict) or set(perm) != {"deny"}:
        raise Failure("`permissions` tem de ter só `deny` (nada de allow/ask)")
    deny = perm["deny"]
    if not isinstance(deny, list) or not all(isinstance(r, str) and r for r in deny):
        raise Failure("`deny` tem de ser lista de textos")
    if len(set(deny)) != len(deny):
        raise Failure("regra repetida no deny")
    for rule in deny:
        # No parenthesis inside: Claude Code's rule reader may cut at the `)`.
        if not re.fullmatch(r"(Read|Edit|Bash)\([^()]+\)", rule):
            raise Failure(f"regra fora do formato: {rule}")
    for scope, rule in DENY_PATHS.items():
        for tool in ("Read", "Edit"):
            if f"{tool}({rule})" not in deny:
                raise Failure(f"deny sem {tool}({rule}) (V8.2: {scope})")
    return [r[len("Bash(") : -1] for r in deny if r.startswith("Bash(")]


def pattern_regex(pattern):
    """Claude Code `Bash(...)` rule: `*` matches any text; the rest is literal."""
    return re.compile("".join(".*" if c == "*" else re.escape(c) for c in pattern), re.S)


def subcommands(command):
    """Splits the command at `;`, `&`, `|` and end of line outside quotes (an
    approximation of what Claude Code does with `&&`, `||`, `;` and `|`)."""
    parts, current, quote, i = [], [], None, 0
    while i < len(command):
        c = command[i]
        if quote:
            if c == "\\" and quote == '"' and i + 1 < len(command):
                current.append(command[i : i + 2])
                i += 2
                continue
            if c == quote:
                quote = None
        elif c in "'\"":
            quote = c
        elif c in ";&|\n":
            parts.append("".join(current))
            current = []
            i += 1
            continue
        current.append(c)
        i += 1
    parts.append("".join(current))
    return [p.strip() for p in parts if p.strip()]


def denied(rules, command):
    """(whole, per subcommand): whether some rule denies, in each model."""
    whole = any(r.fullmatch(command.strip()) for r in rules)
    parts = any(r.fullmatch(p) for r in rules for p in subcommands(command))
    return whole, parts


def skill_commands(text):
    """Lines of the skill's ```sh blocks, without comments."""
    lines = []
    for block in re.findall(r"^```sh\n(.*?)^```", text, re.M | re.S):
        for line in block.split("\n"):
            line = re.sub(r"\s+#.*$", "", line).strip()
            if line and not line.startswith("#"):
                lines.append(line)
    return lines


def verify_bash(patterns, text):
    rules = [pattern_regex(p) for p in patterns]
    swaps = {"{e}": "acme-pg-dev", "{a}": "acme-ssh-chave-prod", "{h}": "$HOME"}
    for template in BASH_ATTACKS:
        command = template
        for old, new in swaps.items():
            command = command.replace(old, new)
        whole, parts = denied(rules, command)
        if not (whole and parts):
            which = "inteiro" if not whole else "por subcomando"
            raise Failure(f"deny Bash não nega ({which}): {command!r}")
    for template in WHOLE_BASH_ATTACKS:
        command = template
        for old, new in swaps.items():
            command = command.replace(old, new)
        whole, _parts = denied(rules, command)
        if not whole:
            raise Failure(f"deny Bash não nega (inteiro): {command!r}")
    commands = skill_commands(text)
    if len(commands) < 8:
        raise Failure("SKILL.md com poucos exemplos em blocos ```sh")
    for command in LEGITIMATE + commands:
        if any(denied(rules, command)):
            raise Failure(f"deny Bash nega comando legítimo: {command!r}")


def verify(tree, ward):
    """Verifies the tree `tree` (a copy of `skill/`). Returns the pending ones noticed."""
    # Links first: nothing is read through a link (a link to .dev/ would bring a secret).
    for root, dirs, files in os.walk(tree):
        for a in dirs + files:
            if os.path.islink(os.path.join(root, a)):
                raise Failure(f"link simbólico em skill/: {a}")
    help_subs = help_subcommands(ward)
    skill_md = os.path.join(tree, "ward", "SKILL.md")
    try:
        with open(skill_md, encoding="utf-8") as f:
            text = f.read()
    except (OSError, UnicodeDecodeError):
        raise Failure("skill/ward/SKILL.md ausente ou não UTF-8") from None
    read_frontmatter(text)
    notices = set()
    # Commands and forbidden mentions: the whole skill (`skill/ward/`). The README of
    # `skill/` is the user's and describes the deny rules (it mentions what they block).
    for root, _dirs, files in os.walk(os.path.join(tree, "ward")):
        for a in files:
            if not a.endswith(".md"):
                continue
            path = os.path.join(root, a)
            rel = os.path.relpath(path, tree)
            try:
                with open(path, encoding="utf-8") as f:
                    content = f.read()
            except UnicodeDecodeError:
                raise Failure(f"{rel}: não é UTF-8") from None
            verify_commands(content, help_subs, rel, notices)
            for pattern, reason in FORBIDDEN:
                if pattern.search(content):
                    raise Failure(f"{rel}: {reason}")
    for scope in DENY_PATHS:
        if scope != f"{SSH}/**" and f"`{scope}`" not in text:
            raise Failure(f"SKILL.md não manda evitar {scope}")
    for term in REQUIRED_IN_SKILL:
        if term not in text:
            raise Failure(f"SKILL.md não cita {term!r}")
    # Phase 54a (RA8): the master password only through the installed `ward`, by the full path.
    if UNLOCK not in text:
        raise Failure(f"SKILL.md não manda destravar por {UNLOCK} (RA8)")
    if re.search(r"(?<!/usr/local/bin/)\bward\s+destravar\b", text):
        raise Failure("SKILL.md manda destravar pelo `ward` do PATH (RA8)")
    try:
        with open(os.path.join(tree, "README.md"), encoding="utf-8") as f:
            readme = f.read()
    except (OSError, UnicodeDecodeError):
        raise Failure("skill/README.md ausente ou não UTF-8") from None
    if f"`{UNLOCK}`" not in readme:
        raise Failure(f"skill/README.md não manda destravar por `{UNLOCK}` (RA8)")
    verify_secrets(tree, read_patterns())
    bash_patterns = verify_deny(os.path.join(tree, "settings-deny.json"))
    verify_bash(bash_patterns, text)
    return notices


def attack(name, change, ward, base):
    """Plants the attack in a copy and requires the verification to fail."""
    with tempfile.TemporaryDirectory(prefix="skill-ataque-") as tmp:
        copy = os.path.join(tmp, "skill")
        shutil.copytree(base, copy, symlinks=True)
        change(copy)
        try:
            verify(copy, ward)
        except Failure as e:
            # The reason never carries the pattern (only file and count).
            print(f"  ok   ataque recusado: {name} ({e})", flush=True)
            return
    raise Failure(f"ataque passou pela conferência: {name}")


def append(rel, data):
    def f(copy):
        with open(os.path.join(copy, rel), "ab") as fh:
            fh.write(data)

    return f


def replace(rel, old, new, every=False):
    def f(copy):
        p = os.path.join(copy, rel)
        with open(p, encoding="utf-8") as fh:
            t = fh.read()
        if old not in t:
            raise Failure(f"autoteste: trecho não achado em {rel}")
        with open(p, "w", encoding="utf-8") as fh:
            fh.write(t.replace(old, new) if every else t.replace(old, new, 1))

    return f


def remove_deny(*rules):
    def f(copy):
        p = os.path.join(copy, "settings-deny.json")
        with open(p, encoding="utf-8") as fh:
            d = json.load(fh)
        for rule in rules:
            d["permissions"]["deny"].remove(rule)
        with open(p, "w", encoding="utf-8") as fh:
            json.dump(d, fh)

    return f


def append_deny(rule):
    def f(copy):
        p = os.path.join(copy, "settings-deny.json")
        with open(p, encoding="utf-8") as fh:
            d = json.load(fh)
        d["permissions"]["deny"].append(rule)
        with open(p, "w", encoding="utf-8") as fh:
            json.dump(d, fh)

    return f


def self_test(ward, base):
    # Control: the real tree passes; otherwise every attack would "fail" for nothing.
    verify(base, ward)
    patterns = read_patterns()
    # A whole secret, a variation (the one in the middle of the list) and the last one,
    # in different places: the example, a new file and the JSON.
    for i, (rel, p) in enumerate(
        [
            ("ward/SKILL.md", patterns[0]),
            ("ward/reference.md", patterns[len(patterns) // 2]),
            ("settings-deny.json", patterns[-1]),
        ]
    ):
        attack(f"segredo de teste plantado ({i + 1}/3)", append(rel, b"\n" + p + b"\n"),
               ward, base)
    attack("comando inexistente", append("ward/SKILL.md", b"\n`ward revelar acme-pg-dev`\n"),
           ward, base)
    attack("comando fora de V8.1",
           append("ward/SKILL.md", b"\n```sh\nward destravar\n```\n"), ward, base)
    attack("script antigo", append("ward/SKILL.md", b"\nuse `cofre-run`\n"), ward, base)
    attack("KeePass XC", append("ward/SKILL.md", b"\nKeePass" b"XC\n"), ward, base)
    attack("comando inexistente em outro arquivo da skill",
           append("ward/reference.md", b"\n```sh\nward exportar --tudo\n```\n"), ward, base)
    attack("token no argv", append("ward/SKILL.md", b"\n`ward ssh --token-fd 3 x`\n"),
           ward, base)
    attack("frontmatter com outro nome", replace("ward/SKILL.md", "name: ward", "name: cofre"),
           ward, base)
    attack("description sem gatilho de banco",
           replace("ward/SKILL.md", "banco", "base", every=True), ward, base)
    attack("JSON inválido", append("settings-deny.json", b",}"), ward, base)
    attack("JSON com chave repetida",
           replace("settings-deny.json", '"permissions": {', '"permissions": {"deny": [],'),
           ward, base)
    attack("JSON com allow", replace("settings-deny.json", '"deny": [',
                                    '"allow": ["Bash(*)"], "deny": ['), ward, base)
    for rule in DENY_PATHS.values():
        attack(f"deny sem Read({rule})", remove_deny(f"Read({rule})"), ward, base)
    attack("skill sem a regra de base64", replace("ward/SKILL.md", "base64", "b64", every=True),
           ward, base)
    attack("skill sem o caminho do backup",
           replace("ward/SKILL.md", "`/media/**/ward-backup/**`", "pendrive"),
           ward, base)

    # Phase 50a: the Bash rules anchored at the start let these forms through.
    attack("deny com a regra de env ancorada no início",
           replace("settings-deny.json", "Bash(*ward*run*env*)", "Bash(ward run * -- env*)"),
           ward, base)
    attack("deny sem a regra de env", remove_deny("Bash(*ward*run*env*)"), ward, base)
    attack("deny sem as regras de od",
           remove_deny(*(f"Bash(*ward*run*{b}od*)" for b in (" ", "/", "'", "\"", "|"))),
           ward, base)
    attack("deny sem a regra de redirecionamento", remove_deny("Bash(*ward*run*>*)"), ward, base)
    # Phase 54b (D68): the standard input that the window does not show.
    attack("deny sem a regra de entrada por arquivo", remove_deny("Bash(*ward*run*<*)"),
           ward, base)
    attack("deny sem a regra de entrada antes do ward", remove_deny("Bash(*<*ward*run*)"),
           ward, base)
    attack("deny sem a regra de pipe para o ward run", remove_deny("Bash(*|*ward*run*)"),
           ward, base)
    attack("deny sem a regra de /run/user", remove_deny("Bash(*/run/user*)"), ward, base)
    attack("deny sem a regra de /proc", remove_deny("Bash(*/proc/*)"), ward, base)
    attack("deny que nega o uso normal", append_deny("Bash(*ward run*)"), ward, base)
    attack("deny com parênteses no padrão", append_deny("Bash(*ward*run*(set*)"), ward, base)
    attack("skill com o caminho do storage state num comando",
           replace("ward/SKILL.md",
                  'WARD_ESTADO="$(ward playwright acme-wp-prod)" /usr/bin/python3 tests/test_painel.py',
                  "/usr/bin/python3 tests/test_painel.py /run/user/1000/ward/playwright/x.json"),
           ward, base)
    attack("skill com exemplo que imprime o token",
           append("ward/SKILL.md",
                       b"\n```sh\nward run beta-github-api -- sh -c 'echo $GITHUB_TOKEN'\n```\n"),
           ward, base)
    attack("skill sem o backup em /run/media",
           replace("ward/SKILL.md", "/run/media/**/ward-backup/**", "pendrive"), ward, base)

    # Phase 54a (RA8): unlock through the `ward` of the PATH, which a fake `ward` imitates.
    attack("skill manda destravar pelo PATH",
           replace("ward/SKILL.md", UNLOCK, "ward destravar"), ward, base)
    attack("skill manda destravar por outro ward",
           replace("ward/SKILL.md", UNLOCK, "~/.local/bin/ward destravar"), ward, base)
    attack("skill cita destravar pelo PATH além do caminho completo",
           append("ward/SKILL.md", b"\nOu rode ward  destravar no terminal.\n"), ward, base)
    attack("README sem o caminho completo",
           replace("README.md", f"`{UNLOCK}`", "`ward destravar`", every=True), ward, base)

    def link(copy):
        os.symlink(VARIATIONS, os.path.join(copy, "ward", "variations.md"))

    attack("link para fora de skill/", link, ward, base)


def main():
    if len(sys.argv) != 2:
        print("uso: skill.py <binário ward>", file=sys.stderr)
        return 2
    ward = os.path.realpath(sys.argv[1])
    base = os.path.join(ROOT, "skill")
    try:
        print("test-skill: autoteste (cada ataque tem de falhar)", flush=True)
        self_test(ward, base)
        print("test-skill: árvore real", flush=True)
        notices = verify(base, ward)
    except Failure as e:
        print(f"test-skill: FALHOU: {e}", file=sys.stderr)
        return 1
    for sub in sorted(notices):
        print(f"  aviso: `ward {sub}` ainda não está no `ward --help` (pendente da Fase 43)")
    print("test-skill: ok (V8.1, V8.2, V8.4)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
