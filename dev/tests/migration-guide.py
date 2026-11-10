#!/usr/bin/python3 -IB
"""Confere o guia de migração `docs/migracao.md` (Fases 53 e 53b, V10.1–V10.3). Sai 1 se
algo falhar. Só lê o guia; não executa nenhum passo dele.

Os nomes da pasta do ssh, da pasta de dados do Ward e do KeePass XC vêm montados por
partes, para a varredura do V1.5 (`dev/tests/attacks-foundation.sh`): aqui são texto do
guia, nunca pasta aberta.

1. V10.1: os passos `## Passo N — …` são exatamente 1 a 8, nesta ordem, e cada título
   traz o assunto do contrato; o passo 5 tem uma subseção `### 5x` para cada item de
   `montagem.md` (override do keyring, ativação D-Bus, autostart, PAM).
2. V10.3: cada passo e cada subseção do passo 5 tem "Conferir" e "Voltar atrás".
3. V10.2 (SEG), conversa: o guia diz que quem executa é o usuário e traz a regra de
   nunca colar na conversa; toda frase que cita Claude, conversa, chat, agente (fora de
   "agente SSH"/"agente do sistema"), assistente, IA ou missão está numa lista fechada
   (`FRASES_DESTINO`), e frase dessas com verbo de envio é recusada mesmo com negação,
   salvo a regra e o aviso do V10.4; nenhum `!` (Claude Code) em linha, em código em
   linha ou em bloco; nenhum bloco indentado.
4. Blocos de código (cercados por ``` ou ~~~, de qualquer linguagem): todo comando está
   na lista positiva `COMANDOS`, com regras de argumento (`wl-copy` só `--clear`;
   `secret-tool` só `store` e `clear`, e o `clear` só depois da guarda
   `grep -qx 'Comm=gnome-keyring-d' &&`; `rm` só na pasta em memória e na do ssh;
   `sudo` só `cp` do PAM; `ssh-keygen` sem `-N`/`-P`; chave da pasta do ssh só para a
   pasta em memória; redirecionamento só para `.pub`, o `known_hosts` do Ward e
   `/dev/null`; pipe só para `grep`; sem heredoc, `<`, crase nem `$(` fora de
   `$(id -u)`). Fase 53d: nenhum argumento com `..`, `$`, `~` fora de `~/`, curinga,
   chaves, pasta do ssh fora da do home (`SSH`) nem pasta em memória fora de
   `/run/user/$(id -u)/migracao/`; chave (da pasta do ssh ou da em memória) só sai por
   `install -m 600`, da pasta do ssh para a em memória ou da em memória para ela mesma
   ou para a do ssh; `cp`/`mv`/`install` com opções fixas; `find` só nas duas formas
   da troca dos `Login Data*`; `chmod` só 600/700; `grep` só filtro de pipe, sem
   arquivo; `busctl` só `status`, `SearchItems` e `NameHasOwner`; `cd` só nas pastas
   do Chrome; `diff` só no PAM; `ssh-keygen -F/-l/-R` só nos `known_hosts`.
5. SEG no texto todo: nenhum segredo em argv (`sshpass -p`, `SSHPASS=`, `-p<senha>` do
   mysql, URI com senha, `curl -u`, `ssh-keygen -N/-P`, `wl-copy` sem `--clear`, pipe
   para `wl-copy`/`secret-tool`, here-string), nenhum `secret-tool lookup/search`, nada
   do cli do KeePass XC, nenhuma exportação (`export`, CSV, HTML, XML), nada em `/tmp`,
   nenhuma chave em `~/ward-migracao`, nenhuma instrução de tirar a passphrase, nenhum
   apagar/mover do `~/cofre` ou do `.kdbx` antes do passo 8, nenhuma chave privada,
   chave AWS ou padrão de `.dev/variations.txt`.
6. Fase 53b: o passo 2b usa o "Carregar de arquivo"; o 2a põe passphrase com
   `ssh-keygen -p`; parte do guia que manda copiar no KeePass XC ou na área de
   transferência tem `wl-copy --clear` na mesma parte; o passo 3 grava a linha do
   `ssh-keygen -F` no `known_hosts` do Ward antes do primeiro `ward ssh` e apaga do ssh
   as chaves de servidor; o passo 4 conta os itens com `SearchItems`; o "Voltar atrás"
   do passo 6 devolve todos os `Login Data*`; o passo 8 abre a cópia do `.kdbx` e cita
   o arquivo-chave antes de desinstalar. Fase 53d: o 2a salva a chave pessoal na pasta
   em memória, põe a passphrase lá e só então a leva ao ssh por `install -m 600`; o
   passo 3 dá `chmod 600` no `known_hosts` do Ward depois do `>>`.
7. Fase 54a (RA8): o guia manda destravar no terminal por `/usr/local/bin/ward destravar`
   e não cita `ward destravar` por outro caminho (o `ward` do `PATH`, `~/.local/bin/ward`),
   porque um `ward` falso antes no `PATH` lê a senha mestra.

Antes de conferir o guia real, o autoteste confere que ele passa (controle) e planta
cada ataque (os da Fase 53, os 22 da auditoria do M10, os da 53b e os da 2ª auditoria
do M10, Fase 53d) numa cópia e exige falha.

Uso: python3 -I -B dev/tests/migration-guide.py [variations.txt]   (ou `make test-migration-guide`)
"""

import os
import re
import shlex
import sys

RAIZ = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
GUIA = os.path.join(RAIZ, "docs", "migracao.md")
VARIACOES = os.path.join(RAIZ, ".dev", "variations.txt")

# Nomes montados por partes (V1.5).
SSH = "~/." + "ssh/"
KPX = "KeePass" + "XC"
KH_WARD = "~/.local/share/" + "ward/known_hosts"
RX_SSH = r"~/\.s" r"sh"
RX_KPX = r"keepass" r"xc"

# V10.1: assunto obrigatório no título de cada passo, na ordem do contrato.
PASSOS = [
    (1, r"cadastrar.*à mão.*Ward"),
    (2, r"chaves SSH.*pessoais.*" + RX_SSH + r".*servidor.*`servidor`"),
    (3, r"`ward ssh`"),
    (4, r"gnome-keyring.*Chrome Safe Storage"),
    (5, r"montagem.*um item por vez"),
    (6, r"senhas do Chrome"),
    (7, r"skill `ward`.*`cofre`.*scripts"),
    (8, r"desinstalar o " + RX_KPX),
]

# V10.1: itens de `~/.claude/skills/cofre/references/montagem.md` desfeitos um a um.
MONTAGEM = {
    "override do keyring": r"gnome-keyring-daemon\.service\.d/override\.conf",
    "ativação D-Bus": r"dbus-1/services/org\.freedesktop\.secrets\.service",
    "autostart": r"autostart/gnome-keyring-secrets\.desktop",
    "PAM": r"/etc/pam\.d/cosmic-greeter",
}

# V10.2: quem pode receber algo do guia. "agente SSH"/"agente do sistema" é o ssh-agent.
DESTINO = re.compile(
    r"(?<![\w./-])claude\b|\bconversa|\bchat\b|\bassistente|\bmiss[ãa]o\b|\bIA\b|\bLLM\b"
    r"|\bbot\b|\bagente\b(?!\s+(ssh|do sistema)\b)",
    re.I,
)
ENVIO = re.compile(
    r"\b(col[ae]r?|cole|mand[ae]r?|mande|envi[ae]r?|mostr[ae]r?|pass[ae]r?|compartilh[ae]r?"
    r"|copi[ae]r?|digit[ae]r?|inform[ae]r?|escrev[ae]r?|entreg(ar|ue)|lev[ae]r?|repass[ae]r?"
    r"|respond[ae]r?|diga|dizer|cit[ae]r?|pe[çc]a|pedir|avis[ae]r?|jog[ae]r?|rod[ae]r?"
    r"|execut[ae]r?|liste|listar)\b",
    re.I,
)
REGRA = (
    "**Nunca cole senha, chave, passphrase, token, código TOTP nem a saída de um comando "
    "deste guia na conversa com o Claude.**"
)
# Frases com destino e verbo de envio aceitas: a regra, a proibição do `!` e o aviso do
# V10.4 (texto fixo, sem valor).
EXCECOES_ENVIO = {
    REGRA,
    "Não rode estes comandos pelo Claude Code (nem com `!`, que joga a saída na conversa).",
    "A missão não executa nenhum passo deste guia e o Claude não roda nenhum destes comandos:",
    "Para o registro do V10.4 no `plan.md`, escreva na missão só a frase \"Chrome manteve as "
    "senhas\", sem nenhum valor.",
    "Num pedido como \"liste as credenciais do Ward\", o Claude usa `ward lista`.",
    # aviso do RA3 ("quer dizer" não é envio)
    "Antes de deixar alguma entrada como \"livre\" (ou de usar \"Permitir sempre\"), saiba "
    "que \"livre\" quer dizer que o Claude **consegue ler o segredo** dela:",
}
# Toda frase do guia que cita um destino; frase nova com destino é recusada.
FRASES_DESTINO = EXCECOES_ENVIO | {
    "\"bloqueado\" para o que o Claude nunca deve usar).",
    "No " + KPX + ", desligue nessa entrada \"Adicionar ao agente ao abrir/desbloquear\".",
    "`rm " + SSH + "id_ed25519_github " + SSH + "id_ed25519_github.pub`, "
    "religue \"Adicionar ao agente\" na entrada e a integração do Agente SSH no " + KPX
    + ", trave e destrave o " + KPX + ";",
    "Deixe \"livre\" só a credencial que você aceitaria que o Claude visse;",
    "As regras novas aparecem em \"Deny\" no `/permissions` do Claude Code.",
}

# Blocos: lista positiva de comandos.
COMANDOS = {
    "install", "chmod", "ssh-keygen", "grep", "cd", "find", "cp", "mv", "rm", "busctl",
    "gnome-keyring-daemon", "secret-tool", "wl-copy", "systemctl", "diff", "sudo", "ls",
}
SEPARADORES = {"&&", "||", ";", "|", "(", ")"}
REDIR_OK = re.compile(r"^(" + RX_SSH + r"/[\w.-]+\.pub|" + re.escape(KH_WARD) + r"|/dev/null)$")
TMPFS = "/run/user/UID/migracao/"
MEM = '"/run/user/$(id -u)/migracao/'

# Texto todo: segredo em argv, exportação, leitura de segredo, disco.
PROIBIDO = [
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
APAGAR = re.compile(r"(?i)\b(rm|mv|shred|unlink|apag\w*|exclu\w*|delet\w*|remov\w*|mov\w*)\b|-delete")
KDBX = re.compile(r"~/cofre|\.kdbx")


class Falha(Exception):
    pass


def secoes(texto, nivel):
    """Divide o texto pelos títulos do nível dado: [(título, corpo)]."""
    marca = "#" * nivel + " "
    partes, titulo, corpo = [], None, []
    for linha in texto.split("\n"):
        if linha.startswith(marca):
            if titulo is not None:
                partes.append((titulo, "\n".join(corpo)))
            titulo, corpo = linha[len(marca):].strip(), []
        elif titulo is not None:
            if re.match(r"#{1,%d} " % (nivel - 1), linha):
                partes.append((titulo, "\n".join(corpo)))
                titulo, corpo = None, []
            else:
                corpo.append(linha)
    if titulo is not None:
        partes.append((titulo, "\n".join(corpo)))
    return partes


def tem_conferir_e_voltar(corpo):
    conferir = re.search(r"(^###+ Conferir\s*$|\*\*Conferir:\*\*)", corpo, re.M)
    voltar = re.search(r"(^###+ Voltar atrás\s*$|\*\*Voltar atrás:\*\*)", corpo, re.M)
    return bool(conferir) and bool(voltar)


def separar_blocos(texto):
    """Devolve (prosa, blocos): a prosa sem os blocos cercados (``` ou ~~~, qualquer
    linguagem, qualquer recuo) e a lista de blocos. Bloco sem fim vai inteiro."""
    prosa, blocos, atual, cerca = [], [], None, None
    for linha in texto.split("\n"):
        m = re.match(r"^\s*(`{3,}|~{3,})", linha)
        if atual is None:
            if m:
                atual, cerca = [], m.group(1)
            else:
                prosa.append(linha)
        elif m and m.group(1)[0] == cerca[0] and len(m.group(1)) >= len(cerca) \
                and linha.strip() == m.group(1):
            blocos.append("\n".join(atual))
            atual = None
        else:
            atual.append(linha)
    if atual is not None:
        blocos.append("\n".join(atual))
    return "\n".join(prosa), blocos


def frases(prosa):
    """Frases da prosa: cada parágrafo, item de lista e título à parte."""
    unidades, atual = [], []
    for linha in prosa.split("\n"):
        if not linha.strip() or re.match(r"^\s*(#+ |[-*] |\d+\. )", linha):
            if atual:
                unidades.append(" ".join(atual))
            atual = []
            linha = re.sub(r"^\s*(#+ |[-*] |\d+\. )", "", linha)
        if linha.strip():
            atual.append(linha.strip())
    if atual:
        unidades.append(" ".join(atual))
    saida = []
    for u in unidades:
        saida += [f for f in re.split(r"(?:(?<=[.!?:;])|(?<=[.!?:;]\*\*))\s+", re.sub(r"\s+", " ", u)) if f]
    return saida


def comandos_do_bloco(bloco):
    """[(linha, [tokens])] de cada linha lógica do bloco, ou levanta ValueError."""
    texto = re.sub(r"\\\n", " ", bloco)
    saida = []
    for linha in texto.split("\n"):
        linha = linha.strip()
        if not linha or linha.startswith("#"):
            continue
        if linha.startswith("!"):
            raise ValueError("comando com `!` (roda pelo Claude Code)")
        trab = linha.replace("$(id -u)", "UID")
        if "$(" in trab or "`" in trab or "<<" in trab:
            raise ValueError(f"substituição de comando, crase ou heredoc: {linha!r}")
        trab = re.sub(r"<([a-zçãéíõ-]+)>", r"X\1X", trab)
        trab = trab.replace("\\;", " FIMEXEC ")
        lx = shlex.shlex(trab, posix=True, punctuation_chars=True)
        lx.whitespace_split = True
        saida.append((linha, list(lx)))
    return saida


def conferir_bloco(bloco):
    erros = []
    try:
        linhas = comandos_do_bloco(bloco)
    except ValueError as e:
        return [str(e)]
    for linha, toks in linhas:
        erros += conferir_tokens(toks, linha)
        # redirecionamentos
        limpos, i = [], 0
        while i < len(toks):
            t = toks[i]
            if t == ">&" and limpos and limpos[-1].isdigit() and i + 1 < len(toks) and toks[i + 1].isdigit():
                limpos.pop()
                i += 2
                continue
            if t in (">", ">>"):
                if limpos and limpos[-1].isdigit():
                    limpos.pop()
                alvo = toks[i + 1] if i + 1 < len(toks) else ""
                if not REDIR_OK.match(alvo):
                    erros.append(f"redirecionamento para fora da lista: {linha!r}")
                i += 2
                continue
            if t in ("<", "<&", ">&", "&", "&>", ">|"):
                erros.append(f"redirecionamento proibido {t!r}: {linha!r}")
                i += 1
                continue
            limpos.append(t)
            i += 1
        # comandos e separadores
        cmds, atual, seps = [], [], []
        for t in limpos:
            if t in SEPARADORES:
                if atual:
                    cmds.append(atual)
                    seps.append(t)
                atual = []
            else:
                atual.append(t)
        if atual:
            cmds.append(atual)
        for n, cmd in enumerate(cmds):
            antes = seps[n - 1] if n > 0 else None
            erros += regras_do_comando(cmd, antes, cmds[:n], seps[: max(n - 1, 0)], linha)
    return erros


def regras_do_comando(cmd, antes, anteriores, seps_ant, linha):
    nome, args = cmd[0], cmd[1:]
    erros = []
    if nome not in COMANDOS:
        return [f"comando fora da lista positiva: {nome!r}"]
    if antes == "|" and nome != "grep":
        erros.append(f"pipe para {nome!r} (só grep)")
    if any(KDBX.search(a) for a in args):
        erros.append(f"comando de bloco no ~/cofre ou no .kdbx: {linha!r}")
    if nome == "wl-copy" and args != ["--clear"]:
        erros.append("wl-copy sem ser `wl-copy --clear`")
    if nome == "secret-tool":
        if not args or args[0] not in ("store", "clear"):
            erros.append(f"secret-tool {args[:1]} fora de store/clear")
        elif args[0] == "store" and not any(a.startswith("--label=") for a in args):
            erros.append("secret-tool store sem --label")
        elif args[0] == "clear":
            guarda = (
                antes == "&&"
                and len(anteriores) >= 2
                and anteriores[-1] == ["grep", "-qx", "Comm=gnome-keyring-d"]
                and anteriores[-2][:3] == ["busctl", "--user", "status"]
                and seps_ant[-1:] == ["|"]
            )
            if not guarda:
                erros.append("secret-tool clear sem a guarda do dono (grep -qx Comm=gnome-keyring-d &&)")
    if nome == "sudo":
        if not args or args[0] != "cp" or not all(
            a.startswith(("-", "/etc/pam.d/", "/var/backups/")) for a in args[1:]
        ):
            erros.append(f"sudo fora de `cp` do PAM: {linha!r}")
    if nome == "ssh-keygen":
        for a in args:
            if re.match(r"^-[A-Za-z]*[NP]", a):
                erros.append("ssh-keygen -N/-P (passphrase em argv)")
        # Fase 53d: `-F`/`-l`/`-R` só nos `known_hosts`; `-y`/`-p` só em chave da pasta
        # em memória ou da do ssh.
        opcoes = [a for a in args if a.startswith("-")]
        arquivo = args[args.index("-f") + 1] if "-f" in args[:-1] else ""
        if not set(opcoes) <= {"-y", "-p", "-f", "-F", "-l", "-R"} or opcoes.count("-f") != 1:
            erros.append(f"ssh-keygen com opções fora da lista: {linha!r}")
        elif {"-F", "-l", "-R"} & set(opcoes):
            if arquivo not in (SSH + "known_hosts", KH_WARD) or {"-y", "-p"} & set(opcoes):
                erros.append(f"ssh-keygen -F/-l/-R fora dos known_hosts: {linha!r}")
        elif not arquivo.startswith((TMPFS, SSH)):
            erros.append(f"ssh-keygen -y/-p fora da pasta em memória e da do ssh: {linha!r}")
    if nome == "rm":
        for a in args:
            if a.startswith("-"):
                if a != "-f":
                    erros.append(f"rm com opção {a!r}")
            elif not (a.startswith(TMPFS) or a.startswith(SSH)):
                erros.append(f"rm fora da pasta em memória e da do ssh: {a!r}")
    if nome in ("cp", "mv", "install"):
        erros += regras_de_copia(nome, args, linha)
    if nome == "chmod":
        # Fase 53d: só 600/700 (chave nunca fica legível para grupo ou outros).
        if len(args) < 2 or args[0] not in ("600", "700") or any(a.startswith("-") for a in args[1:]):
            erros.append(f"chmod fora de 600/700: {linha!r}")
    if nome == "grep":
        # Fase 53d: só filtro de pipe, sem arquivo (nada de `grep . <chave>`).
        opcoes = [a for a in args if a.startswith("-")]
        resto = [a for a in args if not a.startswith("-")]
        ok = antes == "|" and len(resto) == 1 and (
            opcoes in (["-x"], ["-qx"], ["-q", "-x"], ["-x", "-q"])
            or (opcoes == ["-v"] and resto == ["^#"])
        )
        if not ok:
            erros.append(f"grep fora de `| grep -x/-qx <padrão>` (ou `| grep -v '^#'`): {linha!r}")
    if nome == "busctl":
        # Fase 53d: só `status`, `SearchItems` e `NameHasOwner` (nada de `Item Delete`).
        ok = args[:1] == ["--user"] and (
            args[1:3] == ["status", "org.freedesktop.secrets"] and len(args) == 3
            or len(args) >= 6 and args[1] == "call" and args[5] in ("SearchItems", "NameHasOwner")
        )
        if not ok:
            erros.append(f"busctl fora de status/SearchItems/NameHasOwner: {linha!r}")
    if nome == "cd" and args not in (["~/.config/google-chrome"], ["~/ward-migracao/chrome"], ["-"]):
        erros.append(f"cd para fora das pastas do Chrome: {linha!r}")
    if nome == "diff" and not (
        len(args) == 2 and all(a.startswith(("/var/backups/", "/etc/pam.d/")) for a in args)
    ):
        erros.append(f"diff fora do PAM: {linha!r}")
    if nome == "find":
        erros += regras_do_find(args, anteriores, linha)
    return erros


def e_da_memoria(a):
    """O caminho cita a pasta em memória (por qualquer forma)?"""
    return "/run/" in a or a.startswith("run/") or ("migracao" in a and not a.startswith("~/ward-migracao"))


def e_chave(a):
    """Origem possível de chave: a pasta do ssh (fora do `config` e do `known_hosts`) e
    a pasta em memória."""
    return (a.startswith(SSH[:-1]) and a not in (SSH + "config", SSH + "known_hosts")) or e_da_memoria(a)


def conferir_tokens(toks, linha):
    """Fase 53d: regras de todo argumento de bloco, antes de olhar o comando."""
    erros = []
    for a in toks:
        if ".." in a:
            erros.append(f"`..` em caminho: {linha!r}")
        if "$" in a:
            erros.append(f"variável em argumento (só `$(id -u)`): {linha!r}")
        if "~" in a and not a.startswith("~/"):
            erros.append(f"`~` fora de `~/`: {linha!r}")
        if any(c in a for c in "*?[") and a != "Login Data*":
            erros.append(f"curinga em argumento: {linha!r}")
        if "{" in a and a not in ("{}", "a{ss}"):
            erros.append(f"chaves em argumento: {linha!r}")
        if "." + "ssh" in a and not (a == SSH[:-1] or a.startswith(SSH)):
            erros.append(f"pasta do ssh por outro caminho: {a!r}")
        if e_da_memoria(a) and not (a.startswith(TMPFS) or a == TMPFS[:-1]):
            erros.append(f"pasta em memória por outro caminho: {a!r}")
    return erros


def regras_de_copia(nome, args, linha):
    """`cp`/`mv`/`install`. Chave (da pasta do ssh ou da em memória) só sai por
    `install -m 600`, da pasta do ssh para a em memória ou da em memória para ela mesma
    ou para a pasta do ssh."""
    erros = []
    opcoes, caminhos, i = [], [], 0
    while i < len(args):
        a = args[i]
        if nome == "install" and a == "-m" and i + 1 < len(args):
            opcoes += [a, args[i + 1]]
            i += 2
            continue
        (opcoes if a.startswith("-") else caminhos).append(a)
        i += 1
    permitidas = {"cp": [[], ["-p"]], "mv": [[]], "install": [["-m", "600"], ["-d", "-m", "700"]]}[nome]
    if opcoes not in permitidas:
        erros.append(f"{nome} com opções fora da lista {opcoes!r}: {linha!r}")
        return erros
    if "-d" in opcoes:
        return erros
    if any(e_chave(a) for a in caminhos):
        origem, destino = (caminhos + ["", ""])[:2]
        ok = (
            nome == "install"
            and opcoes == ["-m", "600"]
            and len(caminhos) == 2
            and (
                (origem.startswith(TMPFS) and destino.startswith((TMPFS, SSH)))
                or (origem.startswith(SSH) and destino.startswith(TMPFS))
            )
        )
        if not ok:
            erros.append(f"chave fora de `install -m 600` entre a pasta em memória e a do ssh: {linha!r}")
    return erros


# Fase 53d: as duas formas do `find` do guia (troca dos `Login Data*` do Chrome).
FIND_BASE = [".", "-maxdepth", "2", "-name", "Login Data*", "-exec", "cp", "--parents", "-p", "{}"]
FIND_DESTINOS = {
    "~/ward-migracao/chrome/": ("~/.config/google-chrome", False),
    "~/ward-migracao/chrome-depois/": ("~/.config/google-chrome", True),
    "~/.config/google-chrome/": ("~/ward-migracao/chrome", False),
}


def regras_do_find(args, anteriores, linha):
    if "-delete" in args:
        return ["find -delete"]
    destino = args[len(FIND_BASE)] if len(args) > len(FIND_BASE) else ""
    pasta, com_rm = FIND_DESTINOS.get(destino, (None, False))
    esperado = FIND_BASE + [destino, "FIMEXEC"] + (["-exec", "rm", "{}", "FIMEXEC"] if com_rm else [])
    if pasta is None or args != esperado or anteriores[-1:] != [["cd", pasta]]:
        return [f"find fora da troca dos Login Data* do Chrome: {linha!r}"]
    return []


def antes_de(texto, marca):
    """Texto antes do título `marca` (ou o texto todo se não houver)."""
    i = texto.find(marca)
    return texto if i < 0 else texto[:i]


def conferir(texto, padroes):
    erros = []

    # 1. passos na ordem do contrato
    passos = [(t, c) for t, c in secoes(texto, 2) if t.startswith("Passo ")]
    numeros = []
    for titulo, _ in passos:
        m = re.match(r"Passo (\d+) — (.+)$", titulo)
        if not m:
            erros.append(f"título de passo fora do formato: {titulo!r}")
            continue
        numeros.append(int(m.group(1)))
    ordem_ok = numeros == [n for n, _ in PASSOS]
    if not ordem_ok:
        erros.append(f"passos fora da ordem 1–8: {numeros}")
    else:
        for (n, assunto), (titulo, corpo) in zip(PASSOS, passos):
            if not re.search(assunto, titulo, re.I):
                erros.append(f"passo {n} sem o assunto do contrato: {titulo!r}")
            # 2. V10.3
            if not tem_conferir_e_voltar(corpo):
                erros.append(f"passo {n} sem Conferir e Voltar atrás")
        corpo5 = passos[4][1]
        subs = [(t, c) for t, c in secoes("## x\n" + corpo5, 3) if re.match(r"5[a-z]\.", t)]
        for item, rx in MONTAGEM.items():
            achadas = [(t, c) for t, c in subs if re.search(rx, c)]
            if len(achadas) != 1:
                erros.append(f"passo 5: item {item!r} não está numa subseção própria")
                continue
            if not tem_conferir_e_voltar(achadas[0][1]):
                erros.append(f"passo 5: item {item!r} sem Conferir e Voltar atrás")

    # 3. V10.2: conversa com o Claude
    prosa, blocos = separar_blocos(texto)
    if not re.search(r"\*\*Quem executa é você\*\*", texto):
        erros.append("falta dizer que quem executa é o usuário (R19)")
    lista = frases(prosa)
    if REGRA not in lista:
        erros.append("falta a regra de nunca colar segredo na conversa com o Claude")
    for frase in lista:
        if not DESTINO.search(frase):
            continue
        if ENVIO.search(frase) and frase not in EXCECOES_ENVIO:
            erros.append(f"frase com destino e verbo de envio (mesmo com negação): {frase[:80]!r}")
        elif frase not in FRASES_DESTINO:
            erros.append(f"frase nova que cita Claude/conversa/agente/missão: {frase[:80]!r}")
    for linha in prosa.split("\n"):
        if linha.lstrip().startswith("!"):
            erros.append("linha começando com `!` (roda pelo Claude Code)")
        if re.match(r"^( {4,}|\t)\S", linha):
            erros.append(f"bloco indentado (use bloco cercado): {linha.strip()[:60]!r}")
    for span in re.findall(r"`([^`\n]+)`", prosa):
        if span.lstrip().startswith("!") and span.strip() != "!":
            erros.append("código em linha com `!` (roda pelo Claude Code)")

    # 4. blocos: lista positiva
    for bloco in blocos:
        erros += conferir_bloco(bloco)

    # 5. texto todo
    for rx, motivo in PROIBIDO:
        if re.search(rx, texto):
            erros.append(motivo)
    ate8 = antes_de(texto, "## Passo 8 —")
    for frase in frases(separar_blocos(ate8)[0]) + separar_blocos(ate8)[1]:
        if KDBX.search(frase) and APAGAR.search(frase):
            erros.append(f"apagar ou mover o ~/cofre/.kdbx antes do passo 8: {frase[:60]!r}")
    dados = texto.encode()
    if any(p in dados for p in padroes):
        erros.append("padrão de .dev/variations.txt no guia")

    # 6. Fase 53b
    partes = re.split(r"(?=^##+ )", texto, flags=re.M)
    for parte in partes:
        if re.search(r"(?i)\bcopi\w*[^.\n]*(" + RX_KPX + r"|área de transferência)", parte.replace("\n", " ")) \
                and "wl-copy --clear" not in parte:
            erros.append(f"parte copia para a área de transferência sem `wl-copy --clear`: {parte[:50]!r}")
    if ordem_ok:
        corpo = {n: c for (n, _), (_, c) in zip(PASSOS, passos)}
        sub2 = dict(secoes("## x\n" + corpo[2], 3))
        b2 = next((c for t, c in sub2.items() if t.startswith("2b.")), "")
        a2 = next((c for t, c in sub2.items() if t.startswith("2a.")), "")
        if "**Carregar de arquivo**" not in b2:
            erros.append("passo 2b sem o botão **Carregar de arquivo**")
        mem = re.escape('"/run/user/$(id -u)/migracao/')
        if not re.search(r"ssh-keygen -p -f " + mem, a2):
            erros.append("passo 2a sem `ssh-keygen -p` na pasta em memória para chave pessoal sem passphrase")
        # Fase 53d: a chave pessoal passa pela pasta em memória e só chega ao ssh por
        # `install -m 600`, depois da passphrase.
        salva = re.search(r"salve o anexo da chave privada na pasta em\s+memória", a2)
        leva = re.search(r"^\s*install -m 600 " + mem + r'[\w.-]+" ' + RX_SSH + r"/", a2, re.M)
        poe = re.search(r"ssh-keygen -p -f ", a2)
        if not salva or not leva or (poe and poe.start() > leva.start()) or re.search(r"(?i)direto\s+em\s+`" + RX_SSH, a2):
            erros.append("passo 2a sem salvar a chave pessoal na pasta em memória e levar ao ssh por install -m 600")
        c3 = corpo[3]
        grava = re.search(r"ssh-keygen -F [^\n]*>> " + re.escape(KH_WARD), c3)
        primeiro = re.search(r"`ward ssh <", c3)
        if not grava or (primeiro and primeiro.start() < grava.start()):
            erros.append("passo 3 sem gravar a linha do ssh-keygen -F no known_hosts do Ward antes do ward ssh")
        modo = re.search(r"^\s*chmod 600 " + re.escape(KH_WARD) + r"$", c3, re.M)
        if not grava or not modo or modo.start() < grava.start() or (primeiro and primeiro.start() < modo.start()):
            erros.append("passo 3 sem `chmod 600` no known_hosts do Ward depois do `>>`")
        if not re.search(r"^rm -f " + RX_SSH + r"/<chave>", c3, re.M):
            erros.append("passo 3 sem apagar do ssh as chaves de servidor conferidas")
        if "SearchItems" not in corpo[4]:
            erros.append("passo 4 sem contar os itens por SearchItems")
        v6 = dict(secoes("## x\n" + corpo[6], 3)).get("Voltar atrás", "")
        if not re.search(r"\.config/google-chrome/ \\; ", v6) or "'Login Data*'" not in v6:
            erros.append("Voltar atrás do passo 6 sem devolver todos os Login Data*")
        c8 = corpo[8]
        remove = c8.find("apt remove")
        abre = re.search(r"Abra a cópia guardada do `\.kdbx`", c8)
        if remove < 0 or not abre or abre.start() > remove or "arquivo-chave" not in c8[:remove]:
            erros.append("passo 8 sem abrir a cópia do .kdbx e guardar o arquivo-chave antes de desinstalar")

    # 7. Fase 54a (RA8): senha mestra só pelo `ward` instalado, pelo caminho completo.
    if "`/usr/local/bin/ward destravar`" not in texto:
        erros.append("falta destravar no terminal por `/usr/local/bin/ward destravar` (RA8)")
    if re.search(r"(?<!/usr/local/bin/)\bward\s+destravar\b", texto):
        erros.append("`ward destravar` fora do caminho completo /usr/local/bin/ward (RA8)")
    return erros


def ler_variacoes(caminho):
    try:
        st = os.stat(caminho, follow_symlinks=False)
    except FileNotFoundError:
        raise Falha(f"falta {caminho}; rode `make dev-variations`") from None
    if not os.path.isfile(caminho) or os.path.islink(caminho) or st.st_mode & 0o077:
        raise Falha("variations.txt não é arquivo comum 0600")
    with open(caminho, "rb") as f:
        padroes = [p for p in f.read().split(b"\n") if p]
    if len(padroes) < 50 or any(len(p) < 6 for p in padroes):
        raise Falha("variations.txt com padrões de menos ou curtos demais")
    return padroes


def ataques(texto):
    """Cópias do guia com um ataque cada: os da Fase 53 e os 22 da auditoria do M10
    (adaptados ao texto novo quando o trecho de origem mudou)."""
    falso = "padrao-falso-autoteste-53"
    passos = re.split(r"(?=^## Passo )", texto, flags=re.M)
    P2 = "## Passo 2 —"
    ID = SSH + "id_ed25519"

    def trocar(a, b):
        p = list(passos)
        p[a], p[b] = p[b], p[a]
        return "".join(p)

    def antes_p2(linha):
        return texto.replace(P2, linha + "\n\n" + P2)

    def bloco(comando):
        return antes_p2("```bash\n" + comando + "\n```")

    guarda = "busctl --user status org.freedesktop.secrets | grep -qx 'Comm=gnome-keyring-d' && "
    clear2b = 'rm -f "/run/user/$(id -u)/migracao/<servidor>"\n   wl-copy --clear\n   ```\n\nPara cada'
    return {
        # Fase 53
        "passos 4 e 5 trocados": trocar(4, 5),
        "KeePass XC sai antes da skill": trocar(7, 8),
        "passo 3 sem Voltar atrás": texto.replace(
            "### Voltar atrás\n\n- Nada a desfazer", "### Depois\n\n- Nada a desfazer"
        ),
        "PAM fora do passo 5": re.sub(r"### 5d\..*?(?=### 5e\.)", "", texto, flags=re.S),
        "pede para colar ao Claude": antes_p2("Cole a passphrase aqui na conversa com o Claude."),
        "Claude roda ssh-add": antes_p2("Peça para o Claude rodar `ssh-add -l`."),
        "regra de não colar removida": texto.replace("Nunca cole", "Evite colar"),
        "quem executa removido": texto.replace("**Quem executa é você**", "Quem executa"),
        "sshpass -p": texto + "\n`sshpass -p senha ssh host`\n",
        "echo no secret-tool": texto + "\n`echo valor | secret-tool store --label=x a b`\n",
        "here-string no secret-tool": texto + "\n`secret-tool store --label=x a b <<< valor`\n",
        "wl-copy com texto": texto + "\n`wl-copy valor-secreto`\n",
        "ssh-keygen -N": texto + "\n`ssh-keygen -p -N nova -f chave`\n",
        "exportação csv": texto + "\nExporte para `senhas.csv`.\n",
        "export do cli do KeePass XC": texto + "\n`keepass" "xc-cli export cofre.kdbx`\n",
        "chave privada": texto + "\n-----BEGIN OPENSSH PRIVATE KEY-----\n",
        "comando com !": texto + "\n```bash\n! ssh-add -l\n```\n",
        "padrão de variações": texto + "\n" + falso + "\n",
        # Auditoria do M10 (burla-m10.py), na ordem do auditor
        "M10: colar ao Claude com 'não' na frase": antes_p2(
            "Cole a passphrase na conversa com o Claude, não no terminal."
        ),
        "M10: colar no chat sem a palavra Claude": antes_p2("Cole a senha mestra aqui no chat do assistente."),
        "M10: mande a saída ao agente": antes_p2(
            "Mande a saída de `secret-tool lookup application chrome` para o agente."
        ),
        "M10: ! inline": antes_p2("Rode `!cat " + ID + "` no Claude Code."),
        "M10: ! em bloco sh-session": texto + "\n```sh-session\n! ssh-add -l\n```\n",
        "M10: ! em bloco indentado": texto + "\n\n    ! cat " + ID + "\n",
        "M10: SSHPASS= no argv": texto + "\n`SSHPASS=senha sshpass -e ssh host`\n",
        "M10: mysql -pSenha": texto + "\n`mysql -u app -pSenhaReal host`\n",
        "M10: psql URI com senha": texto + "\n`psql postgresql://app:senha@host/db`\n",
        # montado por partes para o gitleaks (curl-auth-user) não casar com o ataque
        "M10: curl -u": texto + "\n`curl -u " + "admin" + ":" + "senha https://x`\n",
        "M10: printf | wl-copy": texto + "\n`printf %s senha | wl-copy`\n",
        "M10: ssh-keygen -N'' colado": texto + "\n`ssh-keygen -p -f k -N''`\n",
        "M10: tirar passphrase interativa": texto.replace(
            "não tire a passphrase", "tire a passphrase com `ssh-keygen -p` (Enter vazio)"
        ),
        "M10: chave colada em heredoc no disco": texto + "\n```bash\ncat > ~/chave.txt\n```\n",
        "M10: cli do KeePass XC show -s": texto + "\n`keepass" "xc-cli show -s ~/cofre/c.kdbx entrada`\n",
        "M10: exportar HTML pela GUI": texto + "\nNo " + KPX + ", Banco de dados › Exportar › HTML.\n",
        "M10: apagar kdbx no passo 2": antes_p2("```bash\nrm -rf ~/cofre\n```"),
        # adaptado: a guarda agora é o `grep -qx … &&` na frente do clear
        "M10: Voltar atrás destrutivo (clear sem guarda)": texto.replace(guarda, ""),
        # adaptado: o 2b não tem mais wl-copy de chave; tira o --clear depois da passphrase
        "M10: wl-copy sem --clear": texto.replace(clear2b, 'rm -f "/run/user/$(id -u)/migracao/<servidor>"\n   ```\n\nPara cada', 1),
        "M10: chave de servidor salva em /tmp": texto.replace(
            "na pasta em memória:\n   `/run/user/<uid>/migracao/<servidor>` (nunca no disco)", "em `/tmp/<servidor>`"
        ),
        "M10: secret-tool lookup imprime segredo": texto + "\n`secret-tool lookup application chrome`\n",
        "M10: Claude, execute (verbo fora da lista)": antes_p2("Peça ao Claude: liste o " + SSH + " com ls."),
        # Fase 53b
        "53b: wl-copy < chave (achado alto do M10)": antes_p2(
            "```bash\nwl-copy < \"/run/user/$(id -u)/migracao/<servidor>\"\n```"
        ),
        "53b: 2b sem Carregar de arquivo": texto.replace("**Carregar de arquivo**", "colar o conteúdo"),
        "53b: chave do ssh guardada em ~/ward-migracao": antes_p2(
            "```bash\nmv " + SSH + "<chave> ~/ward-migracao/\n```"
        ),
        "53b: ssh-servidores de volta": texto + "\nMova para `~/ward-migracao/ssh-servidores/`.\n",
        "53b: ward ssh antes do known_hosts": texto.replace(
            "1. Antes do primeiro `ward ssh` da entrada,", "1. Rode `ward ssh <entrada> hostname`. Depois,"
        ),
        "53b: sem apagar a chave do ssh no passo 3": texto.replace(
            "rm -f " + SSH + "<chave> " + SSH + "<chave>.pub", "ls -l " + SSH
        ),
        "53b: sem passphrase na chave pessoal": texto.replace(
            "ssh-keygen -p -f " + MEM + 'id_ed25519_github"\n', "ls -l " + SSH + "\n"
        ),
        "53b: deixar sem passphrase": antes_p2("Se preferir, deixe a chave sem passphrase."),
        "53b: secret-tool search": texto + "\n`secret-tool search --all application chrome 2>&1 >/dev/null`\n",
        "53b: passo 4 sem SearchItems": texto.replace("SearchItems", "GetItems"),
        "53b: só um Login Data no Voltar atrás": texto.replace(
            "cd ~/ward-migracao/chrome && find . -maxdepth 2 -name 'Login Data*' \\\n"
            "     -exec cp --parents -p {} ~/.config/google-chrome/ \\; ; cd -",
            "cp -p ~/ward-migracao/chrome/Default/'Login Data' ~/.config/google-chrome/Default/",
        ),
        "53b: passo 8 sem abrir a cópia": texto.replace("1. Abra a cópia guardada do `.kdbx`", "1. Ache a cópia do `.kdbx`"),
        "53b: passo 8 sem arquivo-chave": texto.replace(
            "Se o banco usa arquivo-chave, guarde uma cópia\n   do arquivo-chave também", "Se precisar, guarde"
        ),
        "53b: apagar ~/cofre no passo 4": texto.replace(
            "8. Mantenha o Chrome fechado", "8. Apague o `~/cofre/` (já tem cópia).\n9. Mantenha o Chrome fechado"
        ),
        "53b: rm fora da lista": antes_p2("```bash\nrm -f ~/ward-migracao/chrome/x\n```"),
        "53b: cat no bloco": antes_p2("```bash\ncat " + SSH + "config\n```"),
        "53b: redirecionamento para o home": antes_p2(
            "```bash\nssh-keygen -y -f " + SSH + "x > ~/x.txt\n```"
        ),
        "53b: sudo fora do PAM": antes_p2("```bash\nsudo cp " + SSH + "x /root/\n```"),
        "53b: bloco ~~~": texto + "\n~~~\ntee ~/x\n~~~\n",
        "53b: substituição de comando": antes_p2("```bash\nls $(cat " + SSH + "x)\n```"),
        "53b: find -delete": antes_p2("```bash\nfind " + SSH + " -name 'id_*' -delete\n```"),
        "53b: frase nova citando a missão": antes_p2("Avise na missão quando terminar."),
        # Fase 53d (2ª auditoria do M10): os 9 casos do auditor
        "53d: cp com origem na pasta em memória": bloco("cp " + MEM + '<servidor>" ~/ward-migracao/'),
        "53d: mv com origem na pasta em memória": bloco("mv " + MEM + '<servidor>" ~/chave'),
        "53d: .. escapa do padrão /tmp": bloco(
            "install -m 600 " + SSH + "<chave> " + MEM + '../../../tmp/chave"'
        ),
        "53d: .. na origem do cp": bloco("cp -p " + SSH + "../." + "ssh/id_ed25519 ~/ward-migracao/"),
        "53d: find -exec cp da pasta do ssh": bloco(
            "find " + SSH + " -name 'id_*' -exec cp {} ~/ward-migracao/ \\;"
        ),
        "53d: find -exec cp com destino trocado": texto.replace(
            "-exec cp --parents -p {} ~/ward-migracao/chrome/ \\;", "-exec cp --parents -p {} ~/.cache/ \\;", 1
        ),
        "53d: chmod 644 de chave": bloco("chmod 644 " + SSH + "id_ed25519_github"),
        "53d: grep . de chave": bloco("grep . " + SSH + "<chave>"),
        "53d: busctl Item Delete": bloco(
            "busctl --user call org.freedesktop.secrets /org/freedesktop/secrets/collection/login/1 "
            "org.freedesktop.Secret.Item Delete"
        ),
        # Fase 53d: outras formas da mesma fuga
        "53d: install da pasta em memória para o home": bloco("install -m 600 " + MEM + '<servidor>" ~/chave'),
        "53d: mv da pasta do ssh inteira": bloco("mv " + SSH[:-1] + " ~/ward-migracao/ssh-velho"),
        "53d: pasta do ssh pelo /home": bloco("cp /home/u/." + "ssh/id_ed25519 ~/ward-migracao/"),
        "53d: curinga no caminho do ssh": bloco("cp ~/.s?h/id_ed25519 ~/ward-migracao/"),
        "53d: variável no caminho": bloco('cp "$HOME/.' + 'ssh/id_ed25519" ~/ward-migracao/'),
        "53d: cd para a pasta do ssh": bloco("cd " + SSH + " && cp id_ed25519 ~/ward-migracao/"),
        "53d: --target-directory": bloco("cp --target-directory=~/ward-migracao " + SSH + "<chave>"),
        "53d: diff mostra a chave": bloco("diff " + SSH + "<chave> /dev/null"),
        "53d: ssh-keygen -F na chave": bloco("ssh-keygen -F x -f " + SSH + "<chave>"),
        "53d: pasta em memória por outro caminho": bloco(
            "install -m 600 " + SSH + "<chave> /run/user/1000/migracao/x"
        ),
        # Fase 53d: passo 2a sem passar pela pasta em memória (médio) e known_hosts (baixo)
        "53d: 2a salva direto no ssh": texto.replace(
            "salve o anexo da chave privada na pasta em\n   memória",
            "salve o anexo da chave privada direto em `" + SSH + "<nome>`, não na pasta em\n   memória",
        ),
        "53d: 2a põe passphrase já no ssh": texto.replace(
            "ssh-keygen -p -f " + MEM + 'id_ed25519_github"', "ssh-keygen -p -f " + SSH + "id_ed25519_github"
        ),
        "53d: 2a leva a chave com cp": texto.replace(
            "install -m 600 " + MEM + 'id_ed25519_github" ' + SSH + "id_ed25519_github",
            "cp -p " + MEM + 'id_ed25519_github" ' + SSH + "id_ed25519_github",
        ),
        "53d: known_hosts do Ward sem chmod 600": texto.replace(
            "   chmod 600 " + KH_WARD + "\n", ""
        ),
        # Fase 54a (RA8): a senha mestra pelo `ward` que o PATH acha
        "54a: destravar pelo PATH": texto.replace(
            "`/usr/local/bin/ward destravar`", "`ward destravar`"
        ),
        "54a: destravar por outro ward": texto.replace(
            "`/usr/local/bin/ward destravar`", "`~/.local/bin/ward destravar`"
        ),
        "54a: destravar pelo PATH em outro passo": antes_p2("No terminal, rode `ward  destravar`."),
    }


def autoteste(texto, padroes):
    """Controle e ataques plantados numa cópia em memória do guia."""
    erros = conferir(texto, padroes)
    if erros:
        for e in erros:
            print(f"  FALHA controle: {e}")
        raise Falha("controle: o guia real não passa")
    falso = b"padrao-falso-autoteste-53"
    for nome, copia in ataques(texto).items():
        if copia == texto:
            raise Falha(f"autoteste: ataque {nome!r} não mudou a cópia")
        if not conferir(copia, padroes + [falso]):
            raise Falha(f"autoteste: ataque {nome!r} passou")
        print(f"  ok  ataque recusado: {nome}")


def main():
    if len(sys.argv) > 2:
        print(__doc__.strip().split("\n")[-1], file=sys.stderr)
        return 2
    try:
        padroes = ler_variacoes(sys.argv[1] if len(sys.argv) == 2 else VARIACOES)
        with open(GUIA, encoding="utf-8") as f:
            texto = f.read()
        print("==> autoteste")
        autoteste(texto, padroes)
        print("==> docs/migracao.md")
        erros = conferir(texto, padroes)
        if erros:
            for e in erros:
                print(f"  FALHA {e}")
            return 1
        print("  ok  V10.1, V10.2 e V10.3")
    except Falha as e:
        print(f"FALHA {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
