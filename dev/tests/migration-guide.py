#!/usr/bin/python3 -IB
"""Verifies the migration guide (`GUIDE`) (Phases 53 and 53b, V10.1–V10.3).
Exits 1 if anything fails. Only reads the guide; runs none of its steps.

The names of the ssh dir, of Ward's data dir and of KeePass XC are built in pieces, for
the V1.5 scan (`dev/tests/attacks-foundation.sh`): here they are text of the guide,
never an opened dir.

1. V10.1: the level-2 step headings are exactly 1 to 8, in this order, and each title
   carries the contract's subject; step 5 has a `### 5x` subsection for each item of
   the old setup notes (keyring override, D-Bus activation, autostart, PAM).
2. V10.3: each step and each subsection of step 5 has the verify and go-back parts.
3. V10.2 (SEG), conversation: the guide says the user is the one who runs it and carries
   the rule of never pasting into the conversation; every sentence that mentions
   Claude, the conversation, chat, agent (other than the SSH agent or the system's
   agent), assistant, AI or the mission is in a closed list (`RECIPIENT_SENTENCES`), and
   such a sentence with a sending verb is refused even when negated, except the rule
   and the V10.4 notice; no `!` (Claude Code) in a line, in inline code or in a block;
   no indented block.
4. Code blocks (fenced by ``` or ~~~, any language): every command is in the allowlist
   `COMMANDS`, with argument rules (`wl-copy` only `--clear`; `secret-tool` only
   `store` and `clear`, and `clear` only after the guard
   `grep -qx 'Comm=gnome-keyring-d' &&`; `rm` only in the in-memory dir and in the ssh
   one; `sudo` only the PAM `cp`; `ssh-keygen` without `-N`/`-P`; a key of the ssh dir
   only to the in-memory dir; redirection only to `.pub`, Ward's `known_hosts` and
   `/dev/null`; pipe only into `grep`; no heredoc, `<`, backtick nor `$(` other than
   `$(id -u)`). Phase 53d: no argument with `..`, `$`, `~` other than `~/`, wildcard,
   braces, an ssh dir other than the home one (`SSH`) nor an in-memory dir other than
   `/run/user/$(id -u)/<in-memory dir>/`; a key (of the ssh dir or of the in-memory one)
   only leaves through `install -m 600`, from the ssh dir to the in-memory one or from
   the in-memory one to itself or to the ssh one; `cp`/`mv`/`install` with fixed
   options; `find` only in the two forms of the `Login Data*` swap; `chmod` only
   600/700; `grep` only as a pipe filter, without a file; `busctl` only `status`,
   `SearchItems` and `NameHasOwner`; `cd` only into the Chrome dirs; `diff` only on PAM;
   `ssh-keygen -F/-l/-R` only on the `known_hosts`.
5. SEG in the whole text: no secret in argv (`sshpass -p`, `SSHPASS=`, mysql's
   `-p<password>`, URI with a password, `curl -u`, `ssh-keygen -N/-P`, `wl-copy` without
   `--clear`, pipe into `wl-copy`/`secret-tool`, here-string), no
   `secret-tool lookup/search`, nothing of the KeePass XC cli, no export (`export`, CSV,
   HTML, XML), nothing in `/tmp`, no key in the guide's home work dir, no instruction to
   remove the passphrase, no deleting/moving of the old vault dir or of the `.kdbx`
   before step 8, no private key, AWS key or pattern of `.dev/variations.txt`.
6. Phase 53b: step 2b uses the load-from-file button; 2a sets a passphrase with
   `ssh-keygen -p`; a part of the guide that says to copy into KeePass XC or into the
   clipboard has `wl-copy --clear` in the same part; step 3 writes the `ssh-keygen -F`
   line into Ward's `known_hosts` before the first `ward ssh` and deletes the server
   keys from ssh; step 4 counts the items with `SearchItems`; the go-back part of
   step 6 brings back every `Login Data*`; step 8 opens the copy of the `.kdbx` and
   mentions the key file before uninstalling. Phase 53d: 2a saves the personal key in
   the in-memory dir, sets the passphrase there and only then takes it to ssh through
   `install -m 600`; step 3 runs `chmod 600` on Ward's `known_hosts` after the `>>`.
7. Phase 54a (RA8): the guide says to unlock in the terminal by the full path of the
   installed `ward` and does not mention unlocking by another path (the `ward` of the
   `PATH`, `~/.local/bin/ward`), because a fake `ward` earlier in the `PATH` reads the
   master password.

Before verifying the real guide, the self-test verifies that it passes (control) and
plants each attack (the ones of Phase 53, the 22 of the M10 audit, the ones of 53b and
the ones of the 2nd M10 audit, Phase 53d) in a copy and requires a failure.

Usage: python3 -I -B dev/tests/migration-guide.py [variations.txt]
       (or `make test-migration-guide`)
"""

import os
import re
import shlex
import sys

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
GUIDE = os.path.join(ROOT, "docs", "migracao.md")
# Printed on a wrong call; it was the last line of the module docstring until Phase 41.
USAGE = "Uso: python3 -I -B dev/tests/migration-guide.py [variations.txt]   (ou `make test-migration-guide`)"
VARIATIONS = os.path.join(ROOT, ".dev", "variations.txt")

# Names built in pieces (V1.5).
SSH = "~/." + "ssh/"
KPX = "KeePass" + "XC"
KH_WARD = "~/.local/share/" + "ward/known_hosts"
RX_SSH = r"~/\.s" r"sh"
RX_KPX = r"keepass" r"xc"

# V10.1: subject required in the title of each step, in the contract's order.
STEPS = [
    (1, r"cadastrar.*à mão.*Ward"),
    (2, r"chaves SSH.*pessoais.*" + RX_SSH + r".*servidor.*`servidor`"),
    (3, r"`ward ssh`"),
    (4, r"gnome-keyring.*Chrome Safe Storage"),
    (5, r"montagem.*um item por vez"),
    (6, r"senhas do Chrome"),
    (7, r"skill `ward`.*`cofre`.*scripts"),
    (8, r"desinstalar o " + RX_KPX),
]

# V10.1: items of the old skill's setup notes (`references/` of the old skill), undone
# one by one.
SETUP = {
    "override do keyring": r"gnome-keyring-daemon\.service\.d/override\.conf",
    "ativação D-Bus": r"dbus-1/services/org\.freedesktop\.secrets\.service",
    "autostart": r"autostart/gnome-keyring-secrets\.desktop",
    "PAM": r"/etc/pam\.d/cosmic-greeter",
}

# V10.2: who may receive something from the guide. The SSH agent and the system's agent
# are the ssh-agent.
RECIPIENT = re.compile(
    r"(?<![\w./-])claude\b|\bconversa|\bchat\b|\bassistente|\bmiss[ãa]o\b|\bIA\b|\bLLM\b"
    r"|\bbot\b|\bagente\b(?!\s+(ssh|do sistema)\b)",
    re.I,
)
SENDING = re.compile(
    r"\b(col[ae]r?|cole|mand[ae]r?|mande|envi[ae]r?|mostr[ae]r?|pass[ae]r?|compartilh[ae]r?"
    r"|copi[ae]r?|digit[ae]r?|inform[ae]r?|escrev[ae]r?|entreg(ar|ue)|lev[ae]r?|repass[ae]r?"
    r"|respond[ae]r?|diga|dizer|cit[ae]r?|pe[çc]a|pedir|avis[ae]r?|jog[ae]r?|rod[ae]r?"
    r"|execut[ae]r?|liste|listar)\b",
    re.I,
)
RULE = (
    "**Nunca cole senha, chave, passphrase, token, código TOTP nem a saída de um comando "
    "deste guia na conversa com o Claude.**"
)
# Accepted sentences with a recipient and a sending verb: the rule, the ban on `!` and
# the V10.4 notice (fixed text, no value).
SEND_EXCEPTIONS = {
    RULE,
    "Não rode estes comandos pelo Claude Code (nem com `!`, que joga a saída na conversa).",
    "A missão não executa nenhum passo deste guia e o Claude não roda nenhum destes comandos:",
    "Para o registro do V10.4 no `plan.md`, escreva na missão só a frase \"Chrome manteve as "
    "senhas\", sem nenhum valor.",
    "Num pedido como \"liste as credenciais do Ward\", o Claude usa `ward lista`.",
    # RA3 notice (the "means" phrasing is not sending)
    "Antes de deixar alguma entrada como \"livre\" (ou de usar \"Permitir sempre\"), saiba "
    "que \"livre\" quer dizer que o Claude **consegue ler o segredo** dela:",
}
# Every sentence of the guide that mentions a recipient; a new one is refused.
RECIPIENT_SENTENCES = SEND_EXCEPTIONS | {
    "\"bloqueado\" para o que o Claude nunca deve usar).",
    "No " + KPX + ", desligue nessa entrada \"Adicionar ao agente ao abrir/desbloquear\".",
    "`rm " + SSH + "id_ed25519_github " + SSH + "id_ed25519_github.pub`, "
    "religue \"Adicionar ao agente\" na entrada e a integração do Agente SSH no " + KPX
    + ", trave e destrave o " + KPX + ";",
    "Deixe \"livre\" só a credencial que você aceitaria que o Claude visse;",
    "As regras novas aparecem em \"Deny\" no `/permissions` do Claude Code.",
}

# Blocks: allowlist of commands.
COMMANDS = {
    "install", "chmod", "ssh-keygen", "grep", "cd", "find", "cp", "mv", "rm", "busctl",
    "gnome-keyring-daemon", "secret-tool", "wl-copy", "systemctl", "diff", "sudo", "ls",
}
SEPARATORS = {"&&", "||", ";", "|", "(", ")"}
REDIR_OK = re.compile(r"^(" + RX_SSH + r"/[\w.-]+\.pub|" + re.escape(KH_WARD) + r"|/dev/null)$")
TMPFS = "/run/user/UID/migracao/"
MEM = '"/run/user/$(id -u)/migracao/'

# Whole text: secret in argv, export, secret reading, disk.
FORBIDDEN = [
    (r"sshpass\s+-p", "sshpass -p (senha em argv)"),
    (r"\b[A-Z_]*(PASS|SENHA|TOKEN|SECRET|KEY)[A-Z_]*=", "segredo em variável na linha de comando"),
    (r"--password\b", "--password em argv"),
    (r"--passphrase\b", "--passphrase em argv"),
    (r"\bmysql\b[^\n`]*\s-p\S", "mysql -p<senha> (senha em argv)"),
    (r"\b[a-z][a-z0-9+.-]*://[^\s/:@`]+:[^\s/@`]+@", "URI com senha"),
    (r"\bcurl\b[^\n`]*\s(-u|--user)\b", "curl -u (senha em argv)"),
    (r"ssh-keygen\b[^\n`]*\s-[A-Za-z]*[NP]", "ssh-keygen -N/-P (passphrase em argv)"),
    (r"wl-copy(?!\s+--clear\b)", "wl-copy sem ser `wl-copy --clear`"),
    (r"\|\s*(wl-copy|secret-tool)", "pipe para wl-copy ou secret-tool"),
    (r"<<", "heredoc ou here-string"),
    (r"secret-tool\s+(lookup|search)\b", "secret-tool lookup/search (imprime o segredo)"),
    (RX_KPX + r"-cli", "cli do " + KPX),
    (r"(?i)\bexport", "exportação"),
    (r"(?i)\b(csv|html|xml)\b", "formato de exportação em claro"),
    (r"(?<![\w.])/(var/)?tmp\b", "/tmp (disco)"),
    (r"ward-migracao/[^\s`]*(ssh-servidores|chave|id_)", "chave guardada em ~/ward-migracao"),
    (r"(?i)(enter|senha|passphrase|frase)\s+(vazi[ao]|em branco)", "passphrase vazia"),
    (r"(?i)(?<!não )(?<!nunca )\b(tire|tirar|remova|remover|retire|retirar|apague|apagar)\s+a\s+passphrase",
     "tirar a passphrase"),
    (r"(?i)deix[ae]r?\s+(a chave\s+)?sem\s+passphrase", "deixar a chave sem passphrase"),
    (r"-----BEGIN [A-Z ]*PRIVATE KEY-----", "chave privada no guia"),
    (r"\bAKIA[0-9A-Z]{16}\b", "chave AWS no guia"),
]
DELETE = re.compile(r"(?i)\b(rm|mv|shred|unlink|apag\w*|exclu\w*|delet\w*|remov\w*|mov\w*)\b|-delete")
KDBX = re.compile(r"~/cofre|\.kdbx")


class Failure(Exception):
    pass


def sections(text, level):
    """Splits the text by the titles of the given level: [(title, body)]."""
    marker = "#" * level + " "
    parts, title, body = [], None, []
    for line in text.split("\n"):
        if line.startswith(marker):
            if title is not None:
                parts.append((title, "\n".join(body)))
            title, body = line[len(marker):].strip(), []
        elif title is not None:
            if re.match(r"#{1,%d} " % (level - 1), line):
                parts.append((title, "\n".join(body)))
                title, body = None, []
            else:
                body.append(line)
    if title is not None:
        parts.append((title, "\n".join(body)))
    return parts


def has_verify_and_go_back(body):
    verify_heading = re.search(r"(^###+ Conferir\s*$|\*\*Conferir:\*\*)", body, re.M)
    go_back = re.search(r"(^###+ Voltar atrás\s*$|\*\*Voltar atrás:\*\*)", body, re.M)
    return bool(verify_heading) and bool(go_back)


def split_blocks(text):
    """Returns (prose, blocks): the prose without the fenced blocks (``` or ~~~, any
    language, any indentation) and the list of blocks. A block without an end goes in
    whole."""
    prose, blocks, current, fence = [], [], None, None
    for line in text.split("\n"):
        m = re.match(r"^\s*(`{3,}|~{3,})", line)
        if current is None:
            if m:
                current, fence = [], m.group(1)
            else:
                prose.append(line)
        elif m and m.group(1)[0] == fence[0] and len(m.group(1)) >= len(fence) \
                and line.strip() == m.group(1):
            blocks.append("\n".join(current))
            current = None
        else:
            current.append(line)
    if current is not None:
        blocks.append("\n".join(current))
    return "\n".join(prose), blocks


def sentences(prose):
    """Sentences of the prose: each paragraph, list item and title on its own."""
    units, current = [], []
    for line in prose.split("\n"):
        if not line.strip() or re.match(r"^\s*(#+ |[-*] |\d+\. )", line):
            if current:
                units.append(" ".join(current))
            current = []
            line = re.sub(r"^\s*(#+ |[-*] |\d+\. )", "", line)
        if line.strip():
            current.append(line.strip())
    if current:
        units.append(" ".join(current))
    output = []
    for u in units:
        output += [f for f in re.split(r"(?:(?<=[.!?:;])|(?<=[.!?:;]\*\*))\s+", re.sub(r"\s+", " ", u)) if f]
    return output


def block_commands(block):
    """[(line, [tokens])] of each logical line of the block, or raises ValueError."""
    text = re.sub(r"\\\n", " ", block)
    output = []
    for line in text.split("\n"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("!"):
            raise ValueError("comando com `!` (roda pelo Claude Code)")
        work = line.replace("$(id -u)", "UID")
        if "$(" in work or "`" in work or "<<" in work:
            raise ValueError(f"substituição de comando, crase ou heredoc: {line!r}")
        work = re.sub(r"<([a-zçãéíõ-]+)>", r"X\1X", work)
        work = work.replace("\\;", " FIMEXEC ")
        lx = shlex.shlex(work, posix=True, punctuation_chars=True)
        lx.whitespace_split = True
        output.append((line, list(lx)))
    return output


def verify_block(block):
    errors = []
    try:
        lines = block_commands(block)
    except ValueError as e:
        return [str(e)]
    for line, toks in lines:
        errors += verify_tokens(toks, line)
        # redirections
        clean, i = [], 0
        while i < len(toks):
            t = toks[i]
            if t == ">&" and clean and clean[-1].isdigit() and i + 1 < len(toks) and toks[i + 1].isdigit():
                clean.pop()
                i += 2
                continue
            if t in (">", ">>"):
                if clean and clean[-1].isdigit():
                    clean.pop()
                target = toks[i + 1] if i + 1 < len(toks) else ""
                if not REDIR_OK.match(target):
                    errors.append(f"redirecionamento para fora da lista: {line!r}")
                i += 2
                continue
            if t in ("<", "<&", ">&", "&", "&>", ">|"):
                errors.append(f"redirecionamento proibido {t!r}: {line!r}")
                i += 1
                continue
            clean.append(t)
            i += 1
        # commands and separators
        cmds, current, seps = [], [], []
        for t in clean:
            if t in SEPARATORS:
                if current:
                    cmds.append(current)
                    seps.append(t)
                current = []
            else:
                current.append(t)
        if current:
            cmds.append(current)
        for n, cmd in enumerate(cmds):
            before = seps[n - 1] if n > 0 else None
            errors += command_rules(cmd, before, cmds[:n], seps[: max(n - 1, 0)], line)
    return errors


def command_rules(cmd, before, previous, prev_seps, line):
    name, args = cmd[0], cmd[1:]
    errors = []
    if name not in COMMANDS:
        return [f"comando fora da lista positiva: {name!r}"]
    if before == "|" and name != "grep":
        errors.append(f"pipe para {name!r} (só grep)")
    if any(KDBX.search(a) for a in args):
        errors.append(f"comando de bloco no ~/cofre ou no .kdbx: {line!r}")
    if name == "wl-copy" and args != ["--clear"]:
        errors.append("wl-copy sem ser `wl-copy --clear`")
    if name == "secret-tool":
        if not args or args[0] not in ("store", "clear"):
            errors.append(f"secret-tool {args[:1]} fora de store/clear")
        elif args[0] == "store" and not any(a.startswith("--label=") for a in args):
            errors.append("secret-tool store sem --label")
        elif args[0] == "clear":
            guard = (
                before == "&&"
                and len(previous) >= 2
                and previous[-1] == ["grep", "-qx", "Comm=gnome-keyring-d"]
                and previous[-2][:3] == ["busctl", "--user", "status"]
                and prev_seps[-1:] == ["|"]
            )
            if not guard:
                errors.append("secret-tool clear sem a guarda do dono (grep -qx Comm=gnome-keyring-d &&)")
    if name == "sudo":
        if not args or args[0] != "cp" or not all(
            a.startswith(("-", "/etc/pam.d/", "/var/backups/")) for a in args[1:]
        ):
            errors.append(f"sudo fora de `cp` do PAM: {line!r}")
    if name == "ssh-keygen":
        for a in args:
            if re.match(r"^-[A-Za-z]*[NP]", a):
                errors.append("ssh-keygen -N/-P (passphrase em argv)")
        # Phase 53d: `-F`/`-l`/`-R` only on the `known_hosts`; `-y`/`-p` only on a key of
        # the in-memory dir or of the ssh one.
        options = [a for a in args if a.startswith("-")]
        file = args[args.index("-f") + 1] if "-f" in args[:-1] else ""
        if not set(options) <= {"-y", "-p", "-f", "-F", "-l", "-R"} or options.count("-f") != 1:
            errors.append(f"ssh-keygen com opções fora da lista: {line!r}")
        elif {"-F", "-l", "-R"} & set(options):
            if file not in (SSH + "known_hosts", KH_WARD) or {"-y", "-p"} & set(options):
                errors.append(f"ssh-keygen -F/-l/-R fora dos known_hosts: {line!r}")
        elif not file.startswith((TMPFS, SSH)):
            errors.append(f"ssh-keygen -y/-p fora da pasta em memória e da do ssh: {line!r}")
    if name == "rm":
        for a in args:
            if a.startswith("-"):
                if a != "-f":
                    errors.append(f"rm com opção {a!r}")
            elif not (a.startswith(TMPFS) or a.startswith(SSH)):
                errors.append(f"rm fora da pasta em memória e da do ssh: {a!r}")
    if name in ("cp", "mv", "install"):
        errors += copy_rules(name, args, line)
    if name == "chmod":
        # Phase 53d: only 600/700 (a key never becomes readable by group or others).
        if len(args) < 2 or args[0] not in ("600", "700") or any(a.startswith("-") for a in args[1:]):
            errors.append(f"chmod fora de 600/700: {line!r}")
    if name == "grep":
        # Phase 53d: only a pipe filter, without a file (no `grep . <key>`).
        options = [a for a in args if a.startswith("-")]
        rest = [a for a in args if not a.startswith("-")]
        ok = before == "|" and len(rest) == 1 and (
            options in (["-x"], ["-qx"], ["-q", "-x"], ["-x", "-q"])
            or (options == ["-v"] and rest == ["^#"])
        )
        if not ok:
            errors.append(f"grep fora de `| grep -x/-qx <padrão>` (ou `| grep -v '^#'`): {line!r}")
    if name == "busctl":
        # Phase 53d: only `status`, `SearchItems` and `NameHasOwner` (no `Item Delete`).
        ok = args[:1] == ["--user"] and (
            args[1:3] == ["status", "org.freedesktop.secrets"] and len(args) == 3
            or len(args) >= 6 and args[1] == "call" and args[5] in ("SearchItems", "NameHasOwner")
        )
        if not ok:
            errors.append(f"busctl fora de status/SearchItems/NameHasOwner: {line!r}")
    if name == "cd" and args not in (["~/.config/google-chrome"], ["~/ward-migracao/chrome"], ["-"]):
        errors.append(f"cd para fora das pastas do Chrome: {line!r}")
    if name == "diff" and not (
        len(args) == 2 and all(a.startswith(("/var/backups/", "/etc/pam.d/")) for a in args)
    ):
        errors.append(f"diff fora do PAM: {line!r}")
    if name == "find":
        errors += find_rules(args, previous, line)
    return errors


def is_in_memory(a):
    """Does the path mention the in-memory dir (in any form)?"""
    return "/run/" in a or a.startswith("run/") or ("migracao" in a and not a.startswith("~/ward-migracao"))


def is_key(a):
    """Possible source of a key: the ssh dir (other than `config` and `known_hosts`) and
    the in-memory dir."""
    return (a.startswith(SSH[:-1]) and a not in (SSH + "config", SSH + "known_hosts")) or is_in_memory(a)


def verify_tokens(toks, line):
    """Phase 53d: rules for every block argument, before looking at the command."""
    errors = []
    for a in toks:
        if ".." in a:
            errors.append(f"`..` em caminho: {line!r}")
        if "$" in a:
            errors.append(f"variável em argumento (só `$(id -u)`): {line!r}")
        if "~" in a and not a.startswith("~/"):
            errors.append(f"`~` fora de `~/`: {line!r}")
        if any(c in a for c in "*?[") and a != "Login Data*":
            errors.append(f"curinga em argumento: {line!r}")
        if "{" in a and a not in ("{}", "a{ss}"):
            errors.append(f"chaves em argumento: {line!r}")
        if "." + "ssh" in a and not (a == SSH[:-1] or a.startswith(SSH)):
            errors.append(f"pasta do ssh por outro caminho: {a!r}")
        if is_in_memory(a) and not (a.startswith(TMPFS) or a == TMPFS[:-1]):
            errors.append(f"pasta em memória por outro caminho: {a!r}")
    return errors


def copy_rules(name, args, line):
    """`cp`/`mv`/`install`. A key (of the ssh dir or of the in-memory one) only leaves
    through `install -m 600`, from the ssh dir to the in-memory one or from the
    in-memory one to itself or to the ssh dir."""
    errors = []
    options, paths, i = [], [], 0
    while i < len(args):
        a = args[i]
        if name == "install" and a == "-m" and i + 1 < len(args):
            options += [a, args[i + 1]]
            i += 2
            continue
        (options if a.startswith("-") else paths).append(a)
        i += 1
    allowed = {"cp": [[], ["-p"]], "mv": [[]], "install": [["-m", "600"], ["-d", "-m", "700"]]}[name]
    if options not in allowed:
        errors.append(f"{name} com opções fora da lista {options!r}: {line!r}")
        return errors
    if "-d" in options:
        return errors
    if any(is_key(a) for a in paths):
        source, dest = (paths + ["", ""])[:2]
        ok = (
            name == "install"
            and options == ["-m", "600"]
            and len(paths) == 2
            and (
                (source.startswith(TMPFS) and dest.startswith((TMPFS, SSH)))
                or (source.startswith(SSH) and dest.startswith(TMPFS))
            )
        )
        if not ok:
            errors.append(f"chave fora de `install -m 600` entre a pasta em memória e a do ssh: {line!r}")
    return errors


# Phase 53d: the two forms of the guide's `find` (swap of Chrome's `Login Data*`).
FIND_BASE = [".", "-maxdepth", "2", "-name", "Login Data*", "-exec", "cp", "--parents", "-p", "{}"]
FIND_DESTINATIONS = {
    "~/ward-migracao/chrome/": ("~/.config/google-chrome", False),
    "~/ward-migracao/chrome-depois/": ("~/.config/google-chrome", True),
    "~/.config/google-chrome/": ("~/ward-migracao/chrome", False),
}


def find_rules(args, previous, line):
    if "-delete" in args:
        return ["find -delete"]
    dest = args[len(FIND_BASE)] if len(args) > len(FIND_BASE) else ""
    cd_dir, with_rm = FIND_DESTINATIONS.get(dest, (None, False))
    expected = FIND_BASE + [dest, "FIMEXEC"] + (["-exec", "rm", "{}", "FIMEXEC"] if with_rm else [])
    if cd_dir is None or args != expected or previous[-1:] != [["cd", cd_dir]]:
        return [f"find fora da troca dos Login Data* do Chrome: {line!r}"]
    return []


def before_heading(text, marker):
    """Text before the title `marker` (or the whole text if there is none)."""
    i = text.find(marker)
    return text if i < 0 else text[:i]


def verify(text, patterns):
    errors = []

    # 1. steps in the contract's order
    steps = [(t, c) for t, c in sections(text, 2) if t.startswith("Passo ")]
    numbers = []
    for title, _ in steps:
        m = re.match(r"Passo (\d+) — (.+)$", title)
        if not m:
            errors.append(f"título de passo fora do formato: {title!r}")
            continue
        numbers.append(int(m.group(1)))
    order_ok = numbers == [n for n, _ in STEPS]
    if not order_ok:
        errors.append(f"passos fora da ordem 1–8: {numbers}")
    else:
        for (n, subject), (title, body) in zip(STEPS, steps):
            if not re.search(subject, title, re.I):
                errors.append(f"passo {n} sem o assunto do contrato: {title!r}")
            # 2. V10.3
            if not has_verify_and_go_back(body):
                errors.append(f"passo {n} sem Conferir e Voltar atrás")
        body5 = steps[4][1]
        subs = [(t, c) for t, c in sections("## x\n" + body5, 3) if re.match(r"5[a-z]\.", t)]
        for item, rx in SETUP.items():
            matches = [(t, c) for t, c in subs if re.search(rx, c)]
            if len(matches) != 1:
                errors.append(f"passo 5: item {item!r} não está numa subseção própria")
                continue
            if not has_verify_and_go_back(matches[0][1]):
                errors.append(f"passo 5: item {item!r} sem Conferir e Voltar atrás")

    # 3. V10.2: conversation with Claude
    prose, blocks = split_blocks(text)
    if not re.search(r"\*\*Quem executa é você\*\*", text):
        errors.append("falta dizer que quem executa é o usuário (R19)")
    all_sentences = sentences(prose)
    if RULE not in all_sentences:
        errors.append("falta a regra de nunca colar segredo na conversa com o Claude")
    for sentence in all_sentences:
        if not RECIPIENT.search(sentence):
            continue
        if SENDING.search(sentence) and sentence not in SEND_EXCEPTIONS:
            errors.append(f"frase com destino e verbo de envio (mesmo com negação): {sentence[:80]!r}")
        elif sentence not in RECIPIENT_SENTENCES:
            errors.append(f"frase nova que cita Claude/conversa/agente/missão: {sentence[:80]!r}")
    for line in prose.split("\n"):
        if line.lstrip().startswith("!"):
            errors.append("linha começando com `!` (roda pelo Claude Code)")
        if re.match(r"^( {4,}|\t)\S", line):
            errors.append(f"bloco indentado (use bloco cercado): {line.strip()[:60]!r}")
    for span in re.findall(r"`([^`\n]+)`", prose):
        if span.lstrip().startswith("!") and span.strip() != "!":
            errors.append("código em linha com `!` (roda pelo Claude Code)")

    # 4. blocks: allowlist
    for block in blocks:
        errors += verify_block(block)

    # 5. whole text
    for rx, reason in FORBIDDEN:
        if re.search(rx, text):
            errors.append(reason)
    up_to_8 = before_heading(text, "## Passo 8 —")
    for sentence in sentences(split_blocks(up_to_8)[0]) + split_blocks(up_to_8)[1]:
        if KDBX.search(sentence) and DELETE.search(sentence):
            errors.append(f"apagar ou mover o ~/cofre/.kdbx antes do passo 8: {sentence[:60]!r}")
    data = text.encode()
    if any(p in data for p in patterns):
        errors.append("padrão de .dev/variations.txt no guia")

    # 6. Phase 53b
    parts = re.split(r"(?=^##+ )", text, flags=re.M)
    for part in parts:
        if re.search(r"(?i)\bcopi\w*[^.\n]*(" + RX_KPX + r"|área de transferência)", part.replace("\n", " ")) \
                and "wl-copy --clear" not in part:
            errors.append(f"parte copia para a área de transferência sem `wl-copy --clear`: {part[:50]!r}")
    if order_ok:
        body = {n: c for (n, _), (_, c) in zip(STEPS, steps)}
        sub2 = dict(sections("## x\n" + body[2], 3))
        b2 = next((c for t, c in sub2.items() if t.startswith("2b.")), "")
        a2 = next((c for t, c in sub2.items() if t.startswith("2a.")), "")
        if "**Carregar de arquivo**" not in b2:
            errors.append("passo 2b sem o botão **Carregar de arquivo**")
        mem_rx = re.escape('"/run/user/$(id -u)/migracao/')
        if not re.search(r"ssh-keygen -p -f " + mem_rx, a2):
            errors.append("passo 2a sem `ssh-keygen -p` na pasta em memória para chave pessoal sem passphrase")
        # Phase 53d: the personal key goes through the in-memory dir and only reaches ssh
        # through `install -m 600`, after the passphrase.
        saves = re.search(r"salve o anexo da chave privada na pasta em\s+memória", a2)
        carries = re.search(r"^\s*install -m 600 " + mem_rx + r'[\w.-]+" ' + RX_SSH + r"/", a2, re.M)
        sets = re.search(r"ssh-keygen -p -f ", a2)
        if not saves or not carries or (sets and sets.start() > carries.start()) or re.search(r"(?i)direto\s+em\s+`" + RX_SSH, a2):
            errors.append("passo 2a sem salvar a chave pessoal na pasta em memória e levar ao ssh por install -m 600")
        c3 = body[3]
        writes = re.search(r"ssh-keygen -F [^\n]*>> " + re.escape(KH_WARD), c3)
        first = re.search(r"`ward ssh <", c3)
        if not writes or (first and first.start() < writes.start()):
            errors.append("passo 3 sem gravar a linha do ssh-keygen -F no known_hosts do Ward antes do ward ssh")
        mode = re.search(r"^\s*chmod 600 " + re.escape(KH_WARD) + r"$", c3, re.M)
        if not writes or not mode or mode.start() < writes.start() or (first and first.start() < mode.start()):
            errors.append("passo 3 sem `chmod 600` no known_hosts do Ward depois do `>>`")
        if not re.search(r"^rm -f " + RX_SSH + r"/<chave>", c3, re.M):
            errors.append("passo 3 sem apagar do ssh as chaves de servidor conferidas")
        if "SearchItems" not in body[4]:
            errors.append("passo 4 sem contar os itens por SearchItems")
        v6 = dict(sections("## x\n" + body[6], 3)).get("Voltar atrás", "")
        if not re.search(r"\.config/google-chrome/ \\; ", v6) or "'Login Data*'" not in v6:
            errors.append("Voltar atrás do passo 6 sem devolver todos os Login Data*")
        c8 = body[8]
        removal = c8.find("apt remove")
        opens = re.search(r"Abra a cópia guardada do `\.kdbx`", c8)
        if removal < 0 or not opens or opens.start() > removal or "arquivo-chave" not in c8[:removal]:
            errors.append("passo 8 sem abrir a cópia do .kdbx e guardar o arquivo-chave antes de desinstalar")

    # 7. Phase 54a (RA8): master password only through the installed `ward`, by the full path.
    if "`/usr/local/bin/ward destravar`" not in text:
        errors.append("falta destravar no terminal por `/usr/local/bin/ward destravar` (RA8)")
    if re.search(r"(?<!/usr/local/bin/)\bward\s+destravar\b", text):
        errors.append("`ward destravar` fora do caminho completo /usr/local/bin/ward (RA8)")
    return errors


def read_variations(path):
    try:
        st = os.stat(path, follow_symlinks=False)
    except FileNotFoundError:
        raise Failure(f"falta {path}; rode `make dev-variations`") from None
    if not os.path.isfile(path) or os.path.islink(path) or st.st_mode & 0o077:
        raise Failure("variations.txt não é arquivo comum 0600")
    with open(path, "rb") as f:
        patterns = [p for p in f.read().split(b"\n") if p]
    if len(patterns) < 50 or any(len(p) < 6 for p in patterns):
        raise Failure("variations.txt com padrões de menos ou curtos demais")
    return patterns


def attacks(text):
    """Copies of the guide with one attack each: the ones of Phase 53 and the 22 of the
    M10 audit (adapted to the new text when the source excerpt changed)."""
    fake = "padrao-falso-autoteste-53"
    steps = re.split(r"(?=^## Passo )", text, flags=re.M)
    P2 = "## Passo 2 —"
    ID = SSH + "id_ed25519"

    def swap(a, b):
        p = list(steps)
        p[a], p[b] = p[b], p[a]
        return "".join(p)

    def before_p2(line):
        return text.replace(P2, line + "\n\n" + P2)

    def block(command):
        return before_p2("```bash\n" + command + "\n```")

    guard = "busctl --user status org.freedesktop.secrets | grep -qx 'Comm=gnome-keyring-d' && "
    clear2b = 'rm -f "/run/user/$(id -u)/migracao/<servidor>"\n   wl-copy --clear\n   ```\n\nPara cada'
    return {
        # Phase 53
        "passos 4 e 5 trocados": swap(4, 5),
        "KeePass XC sai antes da skill": swap(7, 8),
        "passo 3 sem Voltar atrás": text.replace(
            "### Voltar atrás\n\n- Nada a desfazer", "### Depois\n\n- Nada a desfazer"
        ),
        "PAM fora do passo 5": re.sub(r"### 5d\..*?(?=### 5e\.)", "", text, flags=re.S),
        "pede para colar ao Claude": before_p2("Cole a passphrase aqui na conversa com o Claude."),
        "Claude roda ssh-add": before_p2("Peça para o Claude rodar `ssh-add -l`."),
        "regra de não colar removida": text.replace("Nunca cole", "Evite colar"),
        "quem executa removido": text.replace("**Quem executa é você**", "Quem executa"),
        "sshpass -p": text + "\n`sshpass -p senha ssh host`\n",
        "echo no secret-tool": text + "\n`echo valor | secret-tool store --label=x a b`\n",
        "here-string no secret-tool": text + "\n`secret-tool store --label=x a b <<< valor`\n",
        "wl-copy com texto": text + "\n`wl-copy valor-secreto`\n",
        "ssh-keygen -N": text + "\n`ssh-keygen -p -N nova -f chave`\n",
        "exportação csv": text + "\nExporte para `senhas.csv`.\n",
        "export do cli do KeePass XC": text + "\n`keepass" "xc-cli export cofre.kdbx`\n",
        "chave privada": text + "\n-----BEGIN OPENSSH PRIVATE KEY-----\n",
        "comando com !": text + "\n```bash\n! ssh-add -l\n```\n",
        "padrão de variações": text + "\n" + fake + "\n",
        # M10 audit (the auditor's bypass script), in the auditor's order
        "M10: colar ao Claude com 'não' na frase": before_p2(
            "Cole a passphrase na conversa com o Claude, não no terminal."
        ),
        "M10: colar no chat sem a palavra Claude": before_p2("Cole a senha mestra aqui no chat do assistente."),
        "M10: mande a saída ao agente": before_p2(
            "Mande a saída de `secret-tool lookup application chrome` para o agente."
        ),
        "M10: ! inline": before_p2("Rode `!cat " + ID + "` no Claude Code."),
        "M10: ! em bloco sh-session": text + "\n```sh-session\n! ssh-add -l\n```\n",
        "M10: ! em bloco indentado": text + "\n\n    ! cat " + ID + "\n",
        "M10: SSHPASS= no argv": text + "\n`SSHPASS=senha sshpass -e ssh host`\n",
        "M10: mysql -pSenha": text + "\n`mysql -u app -pSenhaReal host`\n",
        "M10: psql URI com senha": text + "\n`psql postgresql://app:senha@host/db`\n",
        # built in pieces so that gitleaks (curl-auth-user) does not match the attack
        "M10: curl -u": text + "\n`curl -u " + "admin" + ":" + "senha https://x`\n",
        "M10: printf | wl-copy": text + "\n`printf %s senha | wl-copy`\n",
        "M10: ssh-keygen -N'' colado": text + "\n`ssh-keygen -p -f k -N''`\n",
        "M10: tirar passphrase interativa": text.replace(
            "não tire a passphrase", "tire a passphrase com `ssh-keygen -p` (Enter vazio)"
        ),
        "M10: chave colada em heredoc no disco": text + "\n```bash\ncat > ~/chave.txt\n```\n",
        "M10: cli do KeePass XC show -s": text + "\n`keepass" "xc-cli show -s ~/cofre/c.kdbx entrada`\n",
        "M10: exportar HTML pela GUI": text + "\nNo " + KPX + ", Banco de dados › Exportar › HTML.\n",
        "M10: apagar kdbx no passo 2": before_p2("```bash\nrm -rf ~/cofre\n```"),
        # adapted: the guard is now the `grep -qx … &&` in front of clear
        "M10: Voltar atrás destrutivo (clear sem guarda)": text.replace(guard, ""),
        # adapted: 2b no longer has a key wl-copy; removes the --clear after the passphrase
        "M10: wl-copy sem --clear": text.replace(clear2b, 'rm -f "/run/user/$(id -u)/migracao/<servidor>"\n   ```\n\nPara cada', 1),
        "M10: chave de servidor salva em /tmp": text.replace(
            "na pasta em memória:\n   `/run/user/<uid>/migracao/<servidor>` (nunca no disco)", "em `/tmp/<servidor>`"
        ),
        "M10: secret-tool lookup imprime segredo": text + "\n`secret-tool lookup application chrome`\n",
        "M10: Claude, execute (verbo fora da lista)": before_p2("Peça ao Claude: liste o " + SSH + " com ls."),
        # Phase 53b
        "53b: wl-copy < chave (achado alto do M10)": before_p2(
            "```bash\nwl-copy < \"/run/user/$(id -u)/migracao/<servidor>\"\n```"
        ),
        "53b: 2b sem Carregar de arquivo": text.replace("**Carregar de arquivo**", "colar o conteúdo"),
        "53b: chave do ssh guardada em ~/ward-migracao": before_p2(
            "```bash\nmv " + SSH + "<chave> ~/ward-migracao/\n```"
        ),
        "53b: ssh-servidores de volta": text + "\nMova para `~/ward-migracao/ssh-servidores/`.\n",
        "53b: ward ssh antes do known_hosts": text.replace(
            "1. Antes do primeiro `ward ssh` da entrada,", "1. Rode `ward ssh <entrada> hostname`. Depois,"
        ),
        "53b: sem apagar a chave do ssh no passo 3": text.replace(
            "rm -f " + SSH + "<chave> " + SSH + "<chave>.pub", "ls -l " + SSH
        ),
        "53b: sem passphrase na chave pessoal": text.replace(
            "ssh-keygen -p -f " + MEM + 'id_ed25519_github"\n', "ls -l " + SSH + "\n"
        ),
        "53b: deixar sem passphrase": before_p2("Se preferir, deixe a chave sem passphrase."),
        "53b: secret-tool search": text + "\n`secret-tool search --all application chrome 2>&1 >/dev/null`\n",
        "53b: passo 4 sem SearchItems": text.replace("SearchItems", "GetItems"),
        "53b: só um Login Data no Voltar atrás": text.replace(
            "cd ~/ward-migracao/chrome && find . -maxdepth 2 -name 'Login Data*' \\\n"
            "     -exec cp --parents -p {} ~/.config/google-chrome/ \\; ; cd -",
            "cp -p ~/ward-migracao/chrome/Default/'Login Data' ~/.config/google-chrome/Default/",
        ),
        "53b: passo 8 sem abrir a cópia": text.replace("1. Abra a cópia guardada do `.kdbx`", "1. Ache a cópia do `.kdbx`"),
        "53b: passo 8 sem arquivo-chave": text.replace(
            "Se o banco usa arquivo-chave, guarde uma cópia\n   do arquivo-chave também", "Se precisar, guarde"
        ),
        "53b: apagar ~/cofre no passo 4": text.replace(
            "8. Mantenha o Chrome fechado", "8. Apague o `~/cofre/` (já tem cópia).\n9. Mantenha o Chrome fechado"
        ),
        "53b: rm fora da lista": before_p2("```bash\nrm -f ~/ward-migracao/chrome/x\n```"),
        "53b: cat no bloco": before_p2("```bash\ncat " + SSH + "config\n```"),
        "53b: redirecionamento para o home": before_p2(
            "```bash\nssh-keygen -y -f " + SSH + "x > ~/x.txt\n```"
        ),
        "53b: sudo fora do PAM": before_p2("```bash\nsudo cp " + SSH + "x /root/\n```"),
        "53b: bloco ~~~": text + "\n~~~\ntee ~/x\n~~~\n",
        "53b: substituição de comando": before_p2("```bash\nls $(cat " + SSH + "x)\n```"),
        "53b: find -delete": before_p2("```bash\nfind " + SSH + " -name 'id_*' -delete\n```"),
        "53b: frase nova citando a missão": before_p2("Avise na missão quando terminar."),
        # Phase 53d (2nd M10 audit): the auditor's 9 cases
        "53d: cp com origem na pasta em memória": block("cp " + MEM + '<servidor>" ~/ward-migracao/'),
        "53d: mv com origem na pasta em memória": block("mv " + MEM + '<servidor>" ~/chave'),
        "53d: .. escapa do padrão /tmp": block(
            "install -m 600 " + SSH + "<chave> " + MEM + '../../../tmp/chave"'
        ),
        "53d: .. na origem do cp": block("cp -p " + SSH + "../." + "ssh/id_ed25519 ~/ward-migracao/"),
        "53d: find -exec cp da pasta do ssh": block(
            "find " + SSH + " -name 'id_*' -exec cp {} ~/ward-migracao/ \\;"
        ),
        "53d: find -exec cp com destino trocado": text.replace(
            "-exec cp --parents -p {} ~/ward-migracao/chrome/ \\;", "-exec cp --parents -p {} ~/.cache/ \\;", 1
        ),
        "53d: chmod 644 de chave": block("chmod 644 " + SSH + "id_ed25519_github"),
        "53d: grep . de chave": block("grep . " + SSH + "<chave>"),
        "53d: busctl Item Delete": block(
            "busctl --user call org.freedesktop.secrets /org/freedesktop/secrets/collection/login/1 "
            "org.freedesktop.Secret.Item Delete"
        ),
        # Phase 53d: other forms of the same escape
        "53d: install da pasta em memória para o home": block("install -m 600 " + MEM + '<servidor>" ~/chave'),
        "53d: mv da pasta do ssh inteira": block("mv " + SSH[:-1] + " ~/ward-migracao/ssh-velho"),
        "53d: pasta do ssh pelo /home": block("cp /home/u/." + "ssh/id_ed25519 ~/ward-migracao/"),
        "53d: curinga no caminho do ssh": block("cp ~/.s?h/id_ed25519 ~/ward-migracao/"),
        "53d: variável no caminho": block('cp "$HOME/.' + 'ssh/id_ed25519" ~/ward-migracao/'),
        "53d: cd para a pasta do ssh": block("cd " + SSH + " && cp id_ed25519 ~/ward-migracao/"),
        "53d: --target-directory": block("cp --target-directory=~/ward-migracao " + SSH + "<chave>"),
        "53d: diff mostra a chave": block("diff " + SSH + "<chave> /dev/null"),
        "53d: ssh-keygen -F na chave": block("ssh-keygen -F x -f " + SSH + "<chave>"),
        "53d: pasta em memória por outro caminho": block(
            "install -m 600 " + SSH + "<chave> /run/user/1000/migracao/x"
        ),
        # Phase 53d: step 2a without going through the in-memory dir (medium) and known_hosts (low)
        "53d: 2a salva direto no ssh": text.replace(
            "salve o anexo da chave privada na pasta em\n   memória",
            "salve o anexo da chave privada direto em `" + SSH + "<nome>`, não na pasta em\n   memória",
        ),
        "53d: 2a põe passphrase já no ssh": text.replace(
            "ssh-keygen -p -f " + MEM + 'id_ed25519_github"', "ssh-keygen -p -f " + SSH + "id_ed25519_github"
        ),
        "53d: 2a leva a chave com cp": text.replace(
            "install -m 600 " + MEM + 'id_ed25519_github" ' + SSH + "id_ed25519_github",
            "cp -p " + MEM + 'id_ed25519_github" ' + SSH + "id_ed25519_github",
        ),
        "53d: known_hosts do Ward sem chmod 600": text.replace(
            "   chmod 600 " + KH_WARD + "\n", ""
        ),
        # Phase 54a (RA8): the master password through the `ward` that the PATH finds
        "54a: destravar pelo PATH": text.replace(
            "`/usr/local/bin/ward destravar`", "`ward destravar`"
        ),
        "54a: destravar por outro ward": text.replace(
            "`/usr/local/bin/ward destravar`", "`~/.local/bin/ward destravar`"
        ),
        "54a: destravar pelo PATH em outro passo": before_p2("No terminal, rode `ward  destravar`."),
    }


def self_test(text, patterns):
    """Control and attacks planted in an in-memory copy of the guide."""
    errors = verify(text, patterns)
    if errors:
        for e in errors:
            print(f"  FALHA controle: {e}")
        raise Failure("controle: o guia real não passa")
    fake = b"padrao-falso-autoteste-53"
    for name, copy in attacks(text).items():
        if copy == text:
            raise Failure(f"autoteste: ataque {name!r} não mudou a cópia")
        if not verify(copy, patterns + [fake]):
            raise Failure(f"autoteste: ataque {name!r} passou")
        print(f"  ok  ataque recusado: {name}")


def main():
    if len(sys.argv) > 2:
        print(USAGE, file=sys.stderr)
        return 2
    try:
        patterns = read_variations(sys.argv[1] if len(sys.argv) == 2 else VARIATIONS)
        with open(GUIDE, encoding="utf-8") as f:
            text = f.read()
        print("==> autoteste")
        self_test(text, patterns)
        print("==> docs/migracao.md")
        errors = verify(text, patterns)
        if errors:
            for e in errors:
                print(f"  FALHA {e}")
            return 1
        print("  ok  V10.1, V10.2 e V10.3")
    except Failure as e:
        print(f"FALHA {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
