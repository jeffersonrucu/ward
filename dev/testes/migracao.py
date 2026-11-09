#!/usr/bin/python3 -IB
"""Confere o guia de migração `docs/migracao.md` (Fase 53, V10.1–V10.3). Sai 1 se algo
falhar. Só lê o guia; não executa nenhum passo dele.

1. V10.1: os passos `## Passo N — …` são exatamente 1 a 8, nesta ordem, e cada título
   traz o assunto do contrato (cadastrar no Ward; chaves SSH pessoais para o `~/.ssh` e
   de servidor para entradas `servidor`; `ward ssh`; gnome-keyring com "Chrome Safe
   Storage"; montagem; senhas do Chrome; skill `ward` no lugar da `cofre`; desinstalar
   o KeePassXC por último).
2. V10.1: o passo 5 tem uma subseção `### 5x` para cada item de `montagem.md`
   (override do keyring, ativação D-Bus, autostart, PAM), um por um.
3. V10.3: cada passo e cada subseção do passo 5 tem "Conferir" e "Voltar atrás".
4. V10.2 (SEG): o guia diz que quem executa é o usuário e que nada vai para a conversa
   com o Claude; nenhuma frase que cita o Claude pede para colar, mandar ou mostrar
   algo a ele sem negação; nenhuma frase manda o Claude rodar algo no `~/.ssh` ou no
   chaveiro; nenhum comando em linha `!` (Claude Code).
5. SEG: nenhum comando põe segredo em argv (`sshpass -p`, `--password`, `--passphrase`,
   `ssh-keygen -N/-P`, `wl-copy <texto>`, `echo … | secret-tool`, `secret-tool … <<<`),
   nenhuma exportação em claro (`keepassxc-cli export`, `.csv`), nenhuma chave privada
   nem chave AWS e nenhum padrão de `.dev/variacoes.txt` (nunca imprime o padrão).

Antes de conferir o guia real, o autoteste confere que ele passa (controle) e planta
cada ataque numa cópia e exige que a conferência falhe.

Uso: python3 -I -B dev/testes/migracao.py [variacoes.txt]   (ou `make teste-migracao`)
"""

import os
import re
import sys
import tempfile

RAIZ = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
GUIA = os.path.join(RAIZ, "docs", "migracao.md")
VARIACOES = os.path.join(RAIZ, ".dev", "variacoes.txt")

# V10.1: assunto obrigatório no título de cada passo, na ordem do contrato.
PASSOS = [
    (1, r"cadastrar.*à mão.*Ward"),
    (2, r"chaves SSH.*pessoais.*~/\.ssh.*servidor.*`servidor`"),
    (3, r"`ward ssh`"),
    (4, r"gnome-keyring.*Chrome Safe Storage"),
    (5, r"montagem.*um item por vez"),
    (6, r"senhas do Chrome"),
    (7, r"skill `ward`.*`cofre`.*scripts"),
    (8, r"desinstalar o KeePassXC"),
]

# V10.1: itens de `~/.claude/skills/cofre/references/montagem.md` desfeitos um a um.
MONTAGEM = {
    "override do keyring": r"gnome-keyring-daemon\.service\.d/override\.conf",
    "ativação D-Bus": r"dbus-1/services/org\.freedesktop\.secrets\.service",
    "autostart": r"autostart/gnome-keyring-secrets\.desktop",
    "PAM": r"/etc/pam\.d/cosmic-greeter",
}

NEGACAO = re.compile(r"\b(nunca|não|nem|sem)\b", re.I)
PEDE_AO_CLAUDE = re.compile(r"\b(col[ea]r?|cola|mand[ea]r?|envi[ea]r?|mostr[ea]r?|pass[ea]r?)\b", re.I)
RODA = re.compile(r"\b(rod[ea]r?|execut[ea]r?|fa[zç]a?)\b", re.I)
SENSIVEL = re.compile(r"~/\.ssh|chaveiro|keyring|secret-tool|ssh-add|pam\.d|KeePassXC", re.I)

ARGV_PROIBIDO = [
    (r"sshpass\s+-p", "sshpass -p (senha em argv)"),
    (r"--password\b", "--password em argv"),
    (r"--passphrase\b", "--passphrase em argv"),
    (r"ssh-keygen\b[^\n]*\s-[NP]\s", "ssh-keygen -N/-P (passphrase em argv)"),
    (r"wl-copy(?!\s*(<|--clear\b))\s+\S", "wl-copy com texto em argv"),
    (r"(echo|printf)\b[^\n]*\|\s*secret-tool", "segredo por echo/printf no secret-tool"),
    (r"secret-tool\b[^\n]*<<<", "segredo por here-string no secret-tool"),
    (r"keepassxc-cli\s+export", "exportação do .kdbx em claro"),
    (r"\.csv\b", "arquivo .csv (exportação em claro)"),
    (r"-----BEGIN [A-Z ]*PRIVATE KEY-----", "chave privada no guia"),
    (r"\bAKIA[0-9A-Z]{16}\b", "chave AWS no guia"),
]


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


def frases(texto):
    plano = re.sub(r"\s+", " ", texto)
    return re.split(r"(?<=[.!?:])\s+", plano)


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
    if numeros != [n for n, _ in PASSOS]:
        erros.append(f"passos fora da ordem 1–8: {numeros}")
    else:
        for (n, assunto), (titulo, corpo) in zip(PASSOS, passos):
            if not re.search(assunto, titulo, re.I):
                erros.append(f"passo {n} sem o assunto do contrato: {titulo!r}")
            # 3. V10.3
            if not tem_conferir_e_voltar(corpo):
                erros.append(f"passo {n} sem Conferir e Voltar atrás")
        # 2. montagem no passo 5, um por subseção
        corpo5 = passos[4][1]
        subs = [(t, c) for t, c in secoes("## x\n" + corpo5, 3) if re.match(r"5[a-z]\.", t)]
        for item, rx in MONTAGEM.items():
            achadas = [(t, c) for t, c in subs if re.search(rx, c)]
            if len(achadas) != 1:
                erros.append(f"passo 5: item {item!r} não está numa subseção própria")
                continue
            if not tem_conferir_e_voltar(achadas[0][1]):
                erros.append(f"passo 5: item {item!r} sem Conferir e Voltar atrás")

    # 4. V10.2
    if not re.search(r"\*\*Quem executa é você\*\*", texto):
        erros.append("falta dizer que quem executa é o usuário (R19)")
    if not re.search(r"Nunca cole[^\n]*\n?[^\n]*conversa com o Claude", texto):
        erros.append("falta a regra de nunca colar segredo na conversa com o Claude")
    for frase in frases(texto):
        if "Claude" not in frase or NEGACAO.search(frase):
            continue
        if PEDE_AO_CLAUDE.search(frase):
            erros.append("frase pede para passar algo ao Claude")
        elif RODA.search(frase) and SENSIVEL.search(frase):
            erros.append("frase manda o Claude mexer no ~/.ssh ou no chaveiro")
    blocos = re.findall(r"```[a-z]*\n(.*?)```", texto, re.S)
    for bloco in blocos:
        if any(l.lstrip().startswith("!") for l in bloco.split("\n")):
            erros.append("comando com `!` (roda pelo Claude Code)")

    # 5. segredo em argv, exportação em claro, chave no texto
    for rx, motivo in ARGV_PROIBIDO:
        if re.search(rx, texto):
            erros.append(motivo)
    dados = texto.encode()
    if any(p in dados for p in padroes):
        erros.append("padrão de .dev/variacoes.txt no guia")
    return erros


def ler_variacoes(caminho):
    try:
        st = os.stat(caminho, follow_symlinks=False)
    except FileNotFoundError:
        raise Falha(f"falta {caminho}; rode `make dev-variacoes`") from None
    if not os.path.isfile(caminho) or os.path.islink(caminho) or st.st_mode & 0o077:
        raise Falha("variacoes.txt não é arquivo comum 0600")
    with open(caminho, "rb") as f:
        padroes = [p for p in f.read().split(b"\n") if p]
    if len(padroes) < 50 or any(len(p) < 6 for p in padroes):
        raise Falha("variacoes.txt com padrões de menos ou curtos demais")
    return padroes


def autoteste(texto, padroes):
    """Controle e ataques plantados numa cópia em memória do guia."""
    if conferir(texto, padroes):
        raise Falha("controle: o guia real não passa")
    falso = b"padrao-falso-autoteste-53"
    passos = re.split(r"(?=^## Passo )", texto, flags=re.M)

    def trocar(a, b):
        p = list(passos)
        p[a], p[b] = p[b], p[a]
        return "".join(p)

    ataques = {
        "passos 4 e 5 trocados": trocar(4, 5),
        "KeePassXC sai antes da skill": trocar(7, 8),
        "passo 3 sem Voltar atrás": texto.replace(
            "### Voltar atrás\n\n- Nada a desfazer", "### Depois\n\n- Nada a desfazer"
        ),
        "PAM fora do passo 5": re.sub(r"### 5d\..*?(?=### 5e\.)", "", texto, flags=re.S),
        "pede para colar ao Claude": texto.replace(
            "## Passo 2 —", "Cole a passphrase aqui na conversa com o Claude.\n\n## Passo 2 —"
        ),
        "Claude roda ssh-add": texto.replace(
            "## Passo 2 —", "Peça para o Claude rodar `ssh-add -l`.\n\n## Passo 2 —"
        ),
        "regra de não colar removida": texto.replace("Nunca cole", "Evite colar"),
        "quem executa removido": texto.replace("**Quem executa é você**", "Quem executa"),
        "sshpass -p": texto + "\n`sshpass -p senha ssh host`\n",
        "echo no secret-tool": texto + "\n`echo valor | secret-tool store --label=x a b`\n",
        "here-string no secret-tool": texto + "\n`secret-tool store --label=x a b <<< valor`\n",
        "wl-copy com texto": texto + "\n`wl-copy valor-secreto`\n",
        "ssh-keygen -N": texto + "\n`ssh-keygen -p -N nova -f chave`\n",
        "exportação csv": texto + "\nExporte para `senhas.csv`.\n",
        "keepassxc-cli export": texto + "\n`keepassxc-cli export cofre.kdbx`\n",
        "chave privada": texto + "\n-----BEGIN OPENSSH PRIVATE KEY-----\n",
        "comando com !": texto + "\n```bash\n! ssh-add -l\n```\n",
        "padrão de variações": texto + "\n" + falso.decode() + "\n",
    }
    for nome, copia in ataques.items():
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
