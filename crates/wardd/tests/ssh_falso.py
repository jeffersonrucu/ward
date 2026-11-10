"""`ssh`, `sshpass` e `sftp` falsos dos testes do `ward ssh` (Fase 27).

Nunca conecta em nada. Registra o argv, os nomes do ambiente (o valor de `SSHPASS` só
como sha256; arquivo `registro-<nome>-<pid>.txt`, uma linha `campo<TAB>valor`) e, quando o argv traz `IdentityAgent=<socket>`, faz o papel de cliente do
agente de D12: lista, assina e tenta adicionar, remover e travar; um neto (outro
processo) também tenta listar. O que fazer com a saída vem do arquivo `modo` da pasta
do teste. Chamado como `python3 -I ssh_falso.py <nome> <pasta> <argv…>`.

Fase 28 (salto e túnel): com `-W <alvo>`, faz o papel do `ssh` da ponte: lê uma linha
da entrada, responde `eco:<alvo>:<linha>` e escreve no stderr uma linha com o
`SSHPASS` (para conferir que a ponte mascara o stderr). Com `HostKeyAlias=…` (o `ssh`
principal do salto), conecta na ponte (`127.0.0.1:<-p>`), manda `oi`, imprime a
resposta e tenta uma segunda conexão (o salto aceita uma só).
"""

import hashlib
import os
import signal
import socket
import struct
import subprocess
import sys
import time

NOME, BASE = sys.argv[1], sys.argv[2]
ARGV = sys.argv[3:]


def ler(nome, padrao=""):
    try:
        with open(os.path.join(BASE, nome)) as f:
            return f.read().strip()
    except OSError:
        return padrao


def registrar(extra):
    """Uma linha `campo<TAB>valor` por valor (o teste não tem JSON); listas repetem o
    campo; `agente.x` é o resultado `x` do cliente do agente."""
    linhas = [("pid", os.getpid()), ("ppid", os.getppid()), ("tty", int(os.isatty(1)))]
    linhas += [("argv", a) for a in ARGV]
    linhas += [("env", n) for n in sorted(os.environ)]
    if "SSHPASS" in os.environ:
        h = hashlib.sha256(os.environ["SSHPASS"].encode()).hexdigest()
        linhas.append(("sshpass_sha256", h))
    for k, v in extra.items():
        if isinstance(v, dict):
            linhas += [(f"{k}.{k2}", v2) for k2, v2 in v.items()]
        else:
            linhas.append((k, v))
    tmp = os.path.join(BASE, f".registro-{NOME}-{os.getpid()}")
    with open(tmp, "w") as f:
        for k, v in linhas:
            v = str(v)
            assert "\n" not in v and "\t" not in v
            f.write(f"{k}\t{v}\n")
    os.rename(tmp, os.path.join(BASE, f"registro-{NOME}-{os.getpid()}.txt"))


def opcao(nome):
    for i, a in enumerate(ARGV[:-1]):
        if a == "-o" and ARGV[i + 1].startswith(nome + "="):
            return ARGV[i + 1][len(nome) + 1 :]
    return None


def string(b):
    return struct.pack(">I", len(b)) + b


def pedir(sock, msg):
    """Uma mensagem ao agente: o tipo da resposta (e o corpo), ou o que deu errado."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(5)
    try:
        s.connect(sock)
    except OSError as e:
        return ("conexao", type(e).__name__)
    try:
        s.sendall(struct.pack(">I", len(msg)) + msg)
        cab = b""
        while len(cab) < 4:
            d = s.recv(4 - len(cab))
            if not d:
                return ("fechada", "")
            cab += d
        n = struct.unpack(">I", cab)[0]
        corpo = b""
        while len(corpo) < n:
            d = s.recv(n - len(corpo))
            if not d:
                return ("fechada", "")
            corpo += d
        return (corpo[0], corpo[1:].hex())
    except OSError as e:
        return ("erro", type(e).__name__)
    finally:
        s.close()


def cliente_do_agente(sock):
    r = {}
    tipo, corpo = pedir(sock, bytes([11]))
    r["lista"] = tipo
    blob = b""
    if tipo == 12:
        b = bytes.fromhex(corpo)
        r["quantas"] = struct.unpack(">I", b[:4])[0]
        n = struct.unpack(">I", b[4:8])[0]
        blob = b[8 : 8 + n]
        r["blob"] = blob.hex()
    tipo, corpo = pedir(sock, bytes([13]) + string(blob) + string(b"dados-falsos") + struct.pack(">I", 0))
    r["assina"] = tipo
    r["assinatura"] = corpo if tipo == 14 else ""
    r["adiciona"] = pedir(sock, bytes([17]) + string(b"ssh-ed25519") + b"\0" * 40)[0]
    r["remove"] = pedir(sock, bytes([19]))[0]
    r["trava"] = pedir(sock, bytes([22]) + string(b"x"))[0]
    neto = subprocess.run(
        [sys.executable, "-I", "-c", NETO, sock], capture_output=True, text=True, timeout=20
    )
    r["neto"] = neto.stdout.strip()
    return r


NETO = """
import socket, struct, sys
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.settimeout(5)
try:
    s.connect(sys.argv[1])
    s.sendall(struct.pack(">I", 1) + bytes([11]))
    d = s.recv(64)
    print("respondeu" if d else "recusada")
except OSError:
    print("recusada")
"""


def bang(sock):
    """Um `!cmd` do sftp: outro processo filho do sftp tenta listar as chaves."""
    r = subprocess.run(
        [sys.executable, "-I", "-c", NETO, sock], capture_output=True, text=True, timeout=20
    )
    return r.stdout.strip()


def esperar_continuar():
    if not os.path.exists(os.path.join(BASE, "esperar")):
        return
    fim = time.time() + 30
    while time.time() < fim and not os.path.exists(os.path.join(BASE, "continuar")):
        time.sleep(0.02)


def esperar_arquivo(nome):
    if not os.path.exists(os.path.join(BASE, "esperar-" + nome)):
        return
    fim = time.time() + 30
    while time.time() < fim and not os.path.exists(os.path.join(BASE, "continuar-" + nome)):
        time.sleep(0.02)


def valor(op):
    i = ARGV.index(op)
    return ARGV[i + 1]


def ponte_do_lado_do_salto():
    """O `ssh -W` da ponte: eco de uma linha; o stderr leva o SSHPASS (a ponte mascara)."""
    alvo = valor("-W")
    sys.stderr.write("erro-salto " + os.environ.get("SSHPASS", "(nada)") + "\n")
    sys.stderr.flush()
    linha = sys.stdin.buffer.readline()
    sys.stdout.buffer.write(b"eco:" + alvo.encode() + b":" + linha)
    sys.stdout.flush()
    sys.stdin.buffer.read()
    sys.exit(0)


def conversar(porta, msg):
    # Prazo largo: sob carga, o `ssh` falso do salto com chave ainda faz o cliente do
    # agente (com um neto) antes de responder.
    s = socket.create_connection(("127.0.0.1", porta), timeout=60)
    try:
        s.sendall(msg)
        dados = b""
        while not dados.endswith(b"\n"):
            d = s.recv(4096)
            if not d:
                break
            dados += d
        return dados.decode(errors="replace").strip() or "fechada"
    except OSError as e:
        return "erro:" + type(e).__name__
    finally:
        s.close()


def principal():
    modo = ler("modo", "normal")
    extra = {}
    agente = opcao("IdentityAgent")
    if NOME == "sshpass":
        registrar(extra)
        # Como o sshpass de verdade (Fase 28g): o `ssh` roda num filho com sessão
        # própria (`setsid`), e o sshpass espera por ele e sai com o código dele.
        pid = os.fork()
        if pid == 0:
            os.setsid()
            os.execv(ARGV[1], ARGV[1:])
        _, st = os.waitpid(pid, 0)
        os._exit(os.waitstatus_to_exitcode(st) & 0xFF)
    if NOME == "sftp":
        registrar(extra)
        linhas = sys.stdin.read().splitlines()
        for l in linhas:
            print("sftp> " + l)
        if agente and agente != "none":
            # O sftp de verdade chama o ssh; o falso também.
            ssh = subprocess.Popen([os.path.join(BASE, "ssh")] + ARGV)
            if modo == "sftp-bang":
                # Auditoria do M4: `!ssh-add -L` (outro filho do sftp) com o ssh ainda
                # vivo e depois que ele saiu. Tem de ser recusado nas duas.
                fim = time.time() + 30
                while time.time() < fim and not any(
                    n.startswith("registro-ssh-") for n in os.listdir(BASE)
                ):
                    time.sleep(0.02)
                extra["bang_durante"] = bang(agente)
                open(os.path.join(BASE, "continuar"), "w").close()
                ssh.wait(timeout=60)
                extra["bang_depois"] = bang(agente)
                registrar(extra)
            ssh.wait(timeout=60)
        sys.exit(0)
    # ssh
    if "-W" in ARGV:
        if agente and agente != "none":
            extra["agente"] = cliente_do_agente(agente)
        registrar(extra)
        ponte_do_lado_do_salto()
    if opcao("HostKeyAlias"):
        if agente and agente != "none":
            extra["agente"] = cliente_do_agente(agente)
        registrar(extra)
        esperar_arquivo("ponte")
        porta = int(valor("-p"))
        print("ponte=" + conversar(porta, b"oi\n"), flush=True)
        print("segunda=" + conversar(porta, b"de-novo\n"), flush=True)
        sys.exit(0)
    if agente and agente != "none":
        extra["agente"] = cliente_do_agente(agente)
        registrar(extra)
        esperar_continuar()
        extra["depois"] = pedir(agente, bytes([11]))[0]
        registrar(extra)
    else:
        registrar(extra)
    if modo == "tempo":
        # `tempo.py` da auditoria do M4: candidato a prefixo parado entre `A` e `Z`.
        segredo = os.environ.get("SSHPASS", "")
        for i, cand in enumerate(["qqq", segredo[:3], segredo[:7], segredo[:-1]]):
            for pedaco, pausa in (("A", 0.2), (f"C{i}=" + cand, 0.6), (f"Z{i}\n", 0.1)):
                sys.stdout.write(pedaco)
                sys.stdout.flush()
                time.sleep(pausa)
        sys.exit(0)
    if modo == "pty":
        tam =os.get_terminal_size(1)
        mudou = []
        signal.signal(signal.SIGWINCH, lambda *_: mudou.append(1))
        print(f"tty={os.isatty(0)}/{os.isatty(1)} {tam.lines}x{tam.columns}", flush=True)
        # O teste só manda a janela nova depois deste aviso (sem corrida com a carga).
        open(os.path.join(BASE, "pty-pronto"), "w").close()
        segredo = os.environ.get("SSHPASS", "")
        time.sleep(0.2)
        # Oráculo da auditoria do M4: um prefixo errado do mesmo tamanho, parado, e
        # depois o certo, parado; a máscara não pode tratar um diferente do outro.
        errado = "".join("X" if a != "X" else "Y" for a in segredo[:10])
        sys.stdout.write("<" + errado)
        sys.stdout.flush()
        time.sleep(1.0)
        sys.stdout.write(">\n")
        sys.stdout.flush()
        time.sleep(0.2)
        sys.stdout.write("[" + segredo[:10])
        sys.stdout.flush()
        time.sleep(1.0)
        sys.stdout.write(segredo[10:] + "]\n")
        sys.stdout.flush()
        time.sleep(0.6)
        tam = os.get_terminal_size(1)
        print(f"fim {tam.lines}x{tam.columns}", flush=True)
        sys.exit(0)
    if modo == "parado":
        # Fase 54c: a sessão aberta, parada; a trava do cofre tem de derrubá-la antes.
        print("conectado", flush=True)
        time.sleep(30)
        print("depois-da-trava", flush=True)
        sys.exit(7)
    print("SSHPASS=" + os.environ.get("SSHPASS", "(nada)"), flush=True)
    print("remoto=" + " ".join(ARGV[ARGV.index("--") + 2 :]), flush=True)
    sys.exit(7)


principal()
