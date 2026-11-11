"""Fake `ssh`, `sshpass` and `sftp` for the `ward ssh` tests (Phase 27).

Never connects to anything. Records the argv, the names of the environment (the value
of `SSHPASS` only as sha256; one record file per process, one `field<TAB>value`
line) and, when the argv carries `IdentityAgent=<socket>`, plays the D12 agent client:
lists, signs and tries to add, remove and lock; a grandchild (another process) also
tries to list. What to do with the output comes from the mode file in the test
folder. Called as `python3 -I fake_ssh.py <name> <dir> <argv…>`.

Phase 28 (jump host and tunnel): with `-W <target>`, plays the bridge `ssh`: reads one
line of input, echoes the target and the line back and writes to stderr a line with the
`SSHPASS` (to check that the bridge masks the stderr). With `HostKeyAlias=…` (the main
`ssh` of the jump), connects to the bridge (`127.0.0.1:<-p>`), sends `oi`, prints the
answer and tries a second connection (the jump accepts only one).
"""

import hashlib
import os
import signal
import socket
import struct
import subprocess
import sys
import time

NAME, BASE = sys.argv[1], sys.argv[2]
ARGV = sys.argv[3:]


def read_file(name, default=""):
    try:
        with open(os.path.join(BASE, name)) as f:
            return f.read().strip()
    except OSError:
        return default


def record(extra):
    """One `field<TAB>value` line per value (the test has no JSON); lists repeat the
    field; a nested field holds the result of one step of the agent client."""
    lines = [("pid", os.getpid()), ("ppid", os.getppid()), ("tty", int(os.isatty(1)))]
    lines += [("argv", a) for a in ARGV]
    lines += [("env", n) for n in sorted(os.environ)]
    if "SSHPASS" in os.environ:
        h = hashlib.sha256(os.environ["SSHPASS"].encode()).hexdigest()
        lines.append(("sshpass_sha256", h))
    for k, v in extra.items():
        if isinstance(v, dict):
            lines += [(f"{k}.{k2}", v2) for k2, v2 in v.items()]
        else:
            lines.append((k, v))
    tmp = os.path.join(BASE, f".registro-{NAME}-{os.getpid()}")
    with open(tmp, "w") as f:
        for k, v in lines:
            v = str(v)
            assert "\n" not in v and "\t" not in v
            f.write(f"{k}\t{v}\n")
    os.rename(tmp, os.path.join(BASE, f"registro-{NAME}-{os.getpid()}.txt"))


def option(name):
    for i, a in enumerate(ARGV[:-1]):
        if a == "-o" and ARGV[i + 1].startswith(name + "="):
            return ARGV[i + 1][len(name) + 1 :]
    return None


def string(b):
    return struct.pack(">I", len(b)) + b


def ask(sock, msg):
    """One message to the agent: the type of the answer (and the body), or what went
    wrong."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(5)
    try:
        s.connect(sock)
    except OSError as e:
        return ("conexao", type(e).__name__)
    try:
        s.sendall(struct.pack(">I", len(msg)) + msg)
        header = b""
        while len(header) < 4:
            d = s.recv(4 - len(header))
            if not d:
                return ("fechada", "")
            header += d
        n = struct.unpack(">I", header)[0]
        body = b""
        while len(body) < n:
            d = s.recv(n - len(body))
            if not d:
                return ("fechada", "")
            body += d
        return (body[0], body[1:].hex())
    except OSError as e:
        return ("erro", type(e).__name__)
    finally:
        s.close()


def agent_client(sock):
    r = {}
    kind, body = ask(sock, bytes([11]))
    r["lista"] = kind
    blob = b""
    if kind == 12:
        b = bytes.fromhex(body)
        r["quantas"] = struct.unpack(">I", b[:4])[0]
        n = struct.unpack(">I", b[4:8])[0]
        blob = b[8 : 8 + n]
        r["blob"] = blob.hex()
    kind, body = ask(sock, bytes([13]) + string(blob) + string(b"dados-falsos") + struct.pack(">I", 0))
    r["assina"] = kind
    r["assinatura"] = body if kind == 14 else ""
    r["adiciona"] = ask(sock, bytes([17]) + string(b"ssh-ed25519") + b"\0" * 40)[0]
    r["remove"] = ask(sock, bytes([19]))[0]
    r["trava"] = ask(sock, bytes([22]) + string(b"x"))[0]
    grandchild = subprocess.run(
        [sys.executable, "-I", "-c", GRANDCHILD, sock], capture_output=True, text=True, timeout=20
    )
    r["neto"] = grandchild.stdout.strip()
    return r


GRANDCHILD = """
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
    """A `!cmd` of sftp: another child process of sftp tries to list the keys."""
    r = subprocess.run(
        [sys.executable, "-I", "-c", GRANDCHILD, sock], capture_output=True, text=True, timeout=20
    )
    return r.stdout.strip()


def wait_continue():
    if not os.path.exists(os.path.join(BASE, "esperar")):
        return
    end = time.time() + 30
    while time.time() < end and not os.path.exists(os.path.join(BASE, "continuar")):
        time.sleep(0.02)


def wait_file(name):
    if not os.path.exists(os.path.join(BASE, "esperar-" + name)):
        return
    end = time.time() + 30
    while time.time() < end and not os.path.exists(os.path.join(BASE, "continuar-" + name)):
        time.sleep(0.02)


def value(op):
    i = ARGV.index(op)
    return ARGV[i + 1]


def bridge_on_the_jump_side():
    """The bridge `ssh -W`: echoes one line; the stderr carries the SSHPASS (the bridge
    masks it)."""
    target = value("-W")
    sys.stderr.write("erro-salto " + os.environ.get("SSHPASS", "(nada)") + "\n")
    sys.stderr.flush()
    line = sys.stdin.buffer.readline()
    sys.stdout.buffer.write(b"eco:" + target.encode() + b":" + line)
    sys.stdout.flush()
    sys.stdin.buffer.read()
    sys.exit(0)


def talk(port, msg):
    # Wide deadline: under load, the fake `ssh` of the jump with a key still runs the
    # agent client (with a grandchild) before answering.
    s = socket.create_connection(("127.0.0.1", port), timeout=60)
    try:
        s.sendall(msg)
        data = b""
        while not data.endswith(b"\n"):
            d = s.recv(4096)
            if not d:
                break
            data += d
        return data.decode(errors="replace").strip() or "fechada"
    except OSError as e:
        return "erro:" + type(e).__name__
    finally:
        s.close()


def main():
    mode = read_file("modo", "normal")
    extra = {}
    agent = option("IdentityAgent")
    if NAME == "sshpass":
        record(extra)
        # Like the real sshpass (Phase 28g): the `ssh` runs in a child with its own
        # session (`setsid`), and sshpass waits for it and exits with its code.
        pid = os.fork()
        if pid == 0:
            os.setsid()
            os.execv(ARGV[1], ARGV[1:])
        _, st = os.waitpid(pid, 0)
        os._exit(os.waitstatus_to_exitcode(st) & 0xFF)
    if NAME == "sftp":
        record(extra)
        lines = sys.stdin.read().splitlines()
        for l in lines:
            print("sftp> " + l)
        if agent and agent != "none":
            # The real sftp calls ssh; the fake one does too.
            ssh = subprocess.Popen([os.path.join(BASE, "ssh")] + ARGV)
            if mode == "sftp-bang":
                # M4 audit: `!ssh-add -L` (another child of sftp) with the ssh still
                # alive and after it has exited. It must be refused in both.
                end = time.time() + 30
                while time.time() < end and not any(
                    n.startswith("registro-ssh-") for n in os.listdir(BASE)
                ):
                    time.sleep(0.02)
                extra["bang_durante"] = bang(agent)
                open(os.path.join(BASE, "continuar"), "w").close()
                ssh.wait(timeout=60)
                extra["bang_depois"] = bang(agent)
                record(extra)
            ssh.wait(timeout=60)
        sys.exit(0)
    # ssh
    if "-W" in ARGV:
        if agent and agent != "none":
            extra["agente"] = agent_client(agent)
        record(extra)
        bridge_on_the_jump_side()
    if option("HostKeyAlias"):
        if agent and agent != "none":
            extra["agente"] = agent_client(agent)
        record(extra)
        wait_file("ponte")
        port = int(value("-p"))
        print("ponte=" + talk(port, b"oi\n"), flush=True)
        print("segunda=" + talk(port, b"de-novo\n"), flush=True)
        sys.exit(0)
    if agent and agent != "none":
        extra["agente"] = agent_client(agent)
        record(extra)
        wait_continue()
        extra["depois"] = ask(agent, bytes([11]))[0]
        record(extra)
    else:
        record(extra)
    if mode == "tempo":
        # The timing script of the M4 audit: prefix candidate held between `A` and `Z`.
        secret = os.environ.get("SSHPASS", "")
        for i, candidate in enumerate(["qqq", secret[:3], secret[:7], secret[:-1]]):
            for piece, pause in (("A", 0.2), (f"C{i}=" + candidate, 0.6), (f"Z{i}\n", 0.1)):
                sys.stdout.write(piece)
                sys.stdout.flush()
                time.sleep(pause)
        sys.exit(0)
    if mode == "pty":
        size =os.get_terminal_size(1)
        changed = []
        signal.signal(signal.SIGWINCH, lambda *_: changed.append(1))
        print(f"tty={os.isatty(0)}/{os.isatty(1)} {size.lines}x{size.columns}", flush=True)
        # The test only sends the new window after this notice (no race with the load).
        open(os.path.join(BASE, "pty-pronto"), "w").close()
        secret = os.environ.get("SSHPASS", "")
        time.sleep(0.2)
        # Oracle of the M4 audit: a wrong prefix of the same length, held, and then the
        # right one, held; the mask must not treat one differently from the other.
        wrong = "".join("X" if a != "X" else "Y" for a in secret[:10])
        sys.stdout.write("<" + wrong)
        sys.stdout.flush()
        time.sleep(1.0)
        sys.stdout.write(">\n")
        sys.stdout.flush()
        time.sleep(0.2)
        sys.stdout.write("[" + secret[:10])
        sys.stdout.flush()
        time.sleep(1.0)
        sys.stdout.write(secret[10:] + "]\n")
        sys.stdout.flush()
        time.sleep(0.6)
        size = os.get_terminal_size(1)
        print(f"fim {size.lines}x{size.columns}", flush=True)
        sys.exit(0)
    if mode == "parado":
        # Phase 54c: the session open, idle; the vault lock must bring it down first.
        print("conectado", flush=True)
        time.sleep(30)
        print("depois-da-trava", flush=True)
        sys.exit(7)
    print("SSHPASS=" + os.environ.get("SSHPASS", "(nada)"), flush=True)
    print("remoto=" + " ".join(ARGV[ARGV.index("--") + 2 :]), flush=True)
    sys.exit(7)


main()
