#!/usr/bin/python3 -IB
""""Terminal" da ponte de teste (Fase 39c; D3, V5.24). Só desenvolvimento.

A ponte aberta pelo daemon de teste monta o "Conectar" com o mesmo `TerminalDoSistema`
do app: este script faz o papel do `x-terminal-emulator`, recebe
`-e <repo>/target/debug/ward ssh --token-fd <n> <entrada>` e o fd `<n>` herdado (o
token vai só no socket; nunca em argv nem no ambiente). Não abre janela: roda o comando
num pty, espera o prompt do servidor e manda `echo WARD-E2E-$(id -un); exit`.

O modo vem de `$WARD_DEV_DIR/terminal-teste.modo` (uma palavra; o QA grava antes de
clicar em "Conectar"; sem o arquivo, `normal`):
- `normal`: roda o comando como veio (o que o terminal de verdade faz);
- `reuso`: lê o token do fd, procura-o em `/proc/*/cmdline` e `/proc/*/environ` (tem de
  achar 0), usa-o uma vez (conecta sem aprovação) e de novo (vale o nível: a janela de
  aprovação abre e o QA nega);
- `expirado`: lê o token, espera 31 s e usa (vale o nível);
- `outra-entrada`: lê o token de `acme-ssh-salto` e o usa em `acme-ssh-interno-aprov`, que
  salta pelo próprio `acme-ssh-salto` (o token não cobre outra entrada nem o salto dela:
  vale o nível, a janela abre).

Registra em `$WARD_DEV_DIR/logs/terminal-teste.log` (0600) uma linha por uso:
`uso=<n> entrada=<e> saida=<código> conectou=<sim|nao> negado=<sim|nao>`, e no reuso
`proc=<achados>/<lidos>`. Nunca grava o token nem a saída do servidor.
"""

import os
import re
import select
import socket
import subprocess
import sys
import time

PRAZO = 150
MARCA = b"WARD-E2E-"


def sair(msg):
    print(f"terminal-teste: {msg}", file=sys.stderr)
    sys.exit(2)


def dev_e_ward():
    dev = os.environ.get("WARD_DEV_DIR", "")
    if not dev.startswith("/") or not dev.endswith("/.dev") or "/../" in dev or "/./" in dev:
        sair("WARD_DEV_DIR fora da forma <repositório>/.dev")
    a = sys.argv[1:]
    if len(a) != 6 or a[0] != "-e" or a[2:4] != ["ssh", "--token-fd"]:
        sair("uso: terminal-test.py -e <ward> ssh --token-fd <n> <entrada>")
    ward = a[1]
    if ward != os.path.dirname(dev) + "/target/debug/ward":
        sair("o ward não é o do repositório")
    if not re.fullmatch(r"[0-9]{1,7}", a[4]) or not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", a[5]):
        sair("fd ou entrada fora da forma")
    return dev, ward, int(a[4]), a[5]


def registrar(dev, linha):
    os.makedirs(dev + "/logs", mode=0o700, exist_ok=True)
    fd = os.open(dev + "/logs/terminal-teste.log", os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "a") as f:
        f.write(f"{time.strftime('%H:%M:%S')} {linha}\n")


def ambiente():
    # O que o TerminalDoSistema passou (ambiente do zero): nada a mais.
    return dict(os.environ)


def rodar(argv, passar=()):
    """Roda `argv` num pty; manda o comando de teste depois do prompt. Devolve
    (código, conectou, negado)."""
    mestre, escravo = os.openpty()
    p = subprocess.Popen(argv, stdin=escravo, stdout=escravo, stderr=escravo, pass_fds=passar,
                         env=ambiente(), cwd="/", start_new_session=True)
    os.close(escravo)
    for fd in passar:
        os.close(fd)
    saida = bytearray()
    enviado = False
    fim = time.monotonic() + PRAZO
    while time.monotonic() < fim:
        r, _, _ = select.select([mestre], [], [], 0.2)
        if r:
            try:
                b = os.read(mestre, 4096)
            except OSError:
                b = b""
            if not b:
                break
            saida += b
        # O ash do busybox pergunta a posição do cursor (ESC[6n) depois do prompt: o
        # terminal responde, e o prompt é procurado sem as sequências de escape.
        if r and b"\x1b[6n" in b:
            os.write(mestre, b"\x1b[1;1R")
        fim_tela = re.sub(rb"\x1b\[[0-9;?]*[A-Za-z]", b"", bytes(saida[-128:]))
        if not enviado and re.search(rb"[$#] ?$", fim_tela.rstrip(b"\r\n ") + b" "):
            os.write(mestre, b"echo " + MARCA + b"$(id -un); exit\n")
            enviado = True
        if p.poll() is not None and not r:
            break
    try:
        codigo = p.wait(timeout=10)
    except subprocess.TimeoutExpired:
        p.kill()
        codigo = p.wait()
    os.close(mestre)
    texto = bytes(saida)
    # A saída do pty (prompt, banner, erro do `ward ssh`) vai para .dev/logs (0600), que a
    # varredura de logs do `make test-app-e2e` confere contra .dev/variations.txt.
    dev = os.environ["WARD_DEV_DIR"]
    fd = os.open(dev + "/logs/terminal-teste.saida", os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "ab") as f:
        f.write(b"--- " + " ".join(argv[1:3] + argv[4:]).encode() + b"\n" + texto[-2000:] + b"\n")
    conectou = re.search(MARCA + rb"[a-z]", texto) is not None
    negado = b"negad" in texto
    return codigo, conectou, negado


def canal(token):
    a, b = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
    a.sendall(token)
    a.close()
    return b.detach()


def ler_token(fd):
    partes = []
    while True:
        b = os.read(fd, 128)
        if not b:
            break
        partes.append(b)
    os.close(fd)
    return b"".join(partes)


def varrer_proc(token):
    achados = lidos = 0
    for pid in os.listdir("/proc"):
        if not pid.isdigit() or int(pid) == os.getpid():
            continue
        for arq in ("cmdline", "environ"):
            try:
                with open(f"/proc/{pid}/{arq}", "rb") as f:
                    dados = f.read()
            except OSError:
                continue
            lidos += 1
            if token in dados:
                achados += 1
    return achados, lidos


def main():
    dev, ward, fd, entrada = dev_e_ward()
    try:
        with open(dev + "/terminal-teste.modo") as f:
            modo = f.read().strip() or "normal"
    except FileNotFoundError:
        modo = "normal"
    if modo not in ("normal", "reuso", "expirado", "outra-entrada"):
        sair("modo desconhecido")
    registrar(dev, f"abriu modo={modo} entrada={entrada}")
    if modo == "normal":
        c, con, neg = rodar([ward, "ssh", "--token-fd", str(fd), entrada], (fd,))
        registrar(dev, f"uso=1 entrada={entrada} saida={c} conectou={'sim' if con else 'nao'} negado={'sim' if neg else 'nao'}")
        return 0
    token = ler_token(fd)
    usos = []
    if modo == "reuso":
        achados, lidos = varrer_proc(token)
        registrar(dev, f"proc={achados}/{lidos}")
        usos = [(entrada, 0), (entrada, 0)]
    elif modo == "expirado":
        usos = [(entrada, 31)]
    else:
        usos = [("acme-ssh-interno-aprov", 0)]
    for i, (e, espera) in enumerate(usos, 1):
        time.sleep(espera)
        n = canal(token)
        c, con, neg = rodar([ward, "ssh", "--token-fd", str(n), e], (n,))
        registrar(dev, f"uso={i} entrada={e} saida={c} conectou={'sim' if con else 'nao'} negado={'sim' if neg else 'nao'}")
    token = b"\0" * len(token)
    return 0


if __name__ == "__main__":
    sys.exit(main())
