#!/usr/bin/python3 -I -B
"""V3.4 (S5, M3 scrutiny): the test `wardd` withstands bad frames and 10 000 silent
connections and keeps serving a normal client.

Usage: socket_attack.py <socket path> <wardd pid> [connections]
Called by `dev/tests/socket-attacks.sh` (`make test-socket-attacks`), which starts a
`wardd` only for the test, in its own directory. Prints nothing the daemon sends beyond
the type and the code of the replies (none carries a secret: the vault stays locked).

Every case has to pass; exits 1 on the first run with a failure (all failures listed).
"""

import json
import os
import resource
import signal
import socket
import struct
import sys
import time

SOCKET_PATH = sys.argv[1]
PID = int(sys.argv[2])
CONNECTIONS = int(sys.argv[3]) if len(sys.argv) > 3 else 10_000
# Agent slots of the daemon (its maximum number of connections): at the end of the
# flood, at most these stay open (plus the ones still in the `accept` queue).
SLOTS = 64

failures = []


def ok(msg):
    print(f"ok    {msg}")


def fail(msg):
    print(f"FALHA {msg}", file=sys.stderr)
    failures.append(msg)


def connect_new(deadline=30):
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(deadline)
    s.connect(SOCKET_PATH)
    return s


def read_exact(s, n):
    d = b""
    while len(d) < n:
        c = s.recv(n - len(d))
        if not c:
            return None
        d += c
    return d


def reply(s):
    """The next frame (dict) or None if the connection closed without a frame."""
    try:
        n = read_exact(s, 4)
        if n is None:
            return None
        body = read_exact(s, struct.unpack(">I", n)[0])
        return None if body is None else json.loads(body)
    except ConnectionResetError:
        return None
    except socket.timeout:
        # Neither a frame nor a close in time: the daemon kept waiting (failure).
        return {"type": "sem resposta no prazo"}


def request(s, op, data=None):
    b = json.dumps({"v": 2, "op": op, "data": data or {}}).encode()
    s.sendall(struct.pack(">I", len(b)) + b)
    return reply(s)


def closed(s, deadline=10):
    """The connection was closed by the daemon (EOF or reset) within the deadline."""
    s.settimeout(deadline)
    try:
        while True:
            c = s.recv(4096)
            if not c:
                return True
    except ConnectionResetError:
        return True
    except socket.timeout:
        return False


def kind_code(r):
    if r is None:
        return "fechou"
    t = r.get("type")
    if t == "error":
        return "erro:" + str(r.get("data", {}).get("code"))
    return str(t)


def alive():
    try:
        os.kill(PID, 0)
        return True
    except ProcessLookupError:
        return False


def normal_client(label):
    t = time.monotonic()
    try:
        s = connect_new()
        r = request(s, "status")
        s.close()
    except OSError as e:
        fail(f"{label}: cliente normal sem resposta ({e.__class__.__name__})")
        return
    dt = time.monotonic() - t
    if r is not None and r.get("type") == "status":
        ok(f"{label}: cliente normal atendido em {dt:.2f} s")
    else:
        fail(f"{label}: cliente normal recebeu {kind_code(r)}")


# --- Bad frames: each one refused (error) or closed, and the daemon goes on -----------
normal_client("antes")
# (label, bytes, the frame itself is refused: the connection has to close). Bad JSON in
# a well-formed frame gets an error and the connection may go on.
BAD_FRAMES = [
    ("tamanho de 2 MiB", struct.pack(">I", 2 * 1024 * 1024) + b"x" * 1024, True),
    ("tamanho 0xFFFFFFFF (negativo em i32)", struct.pack(">i", -1), True),
    ("tamanho negativo com corpo", struct.pack(">i", -5) + b"{}{}{", True),
    ("tamanho 0", struct.pack(">I", 0), False),
    ("JSON inválido", struct.pack(">I", 5) + b"{nao}", False),
    ("JSON sem versão nem op", struct.pack(">I", 2) + b"{}", False),
    ("UTF-8 inválido", struct.pack(">I", 4) + b"\xff\xfe\xfd\xfc", False),
    # m2:DT8, m2:V4.13: a frame of another version gets unknown_version and the daemon
    # closes the connection.
    ("versão errada", None, True),
    ("quadro v1 (0.1.x)", None, True),
    ("operação desconhecida", None, False),
]
for label, raw, must_close in BAD_FRAMES:
    s = connect_new(10)
    if label == "versão errada":
        b = json.dumps({"v": 99, "op": "status", "data": {}}).encode()
        s.sendall(struct.pack(">I", len(b)) + b)
        r = reply(s)
        if kind_code(r) != "erro:unknown_version":
            fail(f"{label}: esperado erro:unknown_version, veio {kind_code(r)}")
    elif label == "quadro v1 (0.1.x)":
        b = json.dumps({"v": 1, "op": "estado", "dados": {}}).encode()
        s.sendall(struct.pack(">I", len(b)) + b)
        r = reply(s)
        if kind_code(r) != "erro:unknown_version" or sorted(r.get("data", {})) != ["code", "message"]:
            fail(f"{label}: esperado só erro:unknown_version, veio {kind_code(r)}")
    elif label == "operação desconhecida":
        r = request(s, "inventada")
    else:
        try:
            s.sendall(raw)
        except (BrokenPipeError, ConnectionResetError):
            pass
        r = reply(s)
    tc = kind_code(r)
    if tc == "fechou" or tc.startswith("erro:"):
        ok(f"{label}: {tc}")
    else:
        fail(f"{label}: aceito ({tc})")
    if must_close and not closed(s):
        fail(f"{label}: conexão continuou aberta")
    s.close()

# Truncated frame: announces 100 bytes, sends 10 and closes.
s = connect_new(10)
s.sendall(struct.pack(">I", 100) + b"{" * 10)
s.shutdown(socket.SHUT_WR)
if closed(s):
    ok("quadro cortado: fechado")
else:
    fail("quadro cortado: conexão continuou aberta")
s.close()
if alive():
    ok("daemon vivo depois dos quadros ruins")
else:
    fail("daemon morreu com os quadros ruins")
normal_client("depois dos quadros ruins")

# --- 10 000 silent connections --------------------------------------------------------
soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
needed = CONNECTIONS + 256
if hard != resource.RLIM_INFINITY and hard < needed:
    fail(f"RLIMIT_NOFILE rígido {hard} < {needed}: não dá para abrir {CONNECTIONS} conexões")
    sys.exit(1)
resource.setrlimit(resource.RLIMIT_NOFILE, (needed, hard))
conns = []
connect_errors = 0
t = time.monotonic()
# Blocking `connect`, like an ordinary client's: with the `listen` queue full, it waits
# for the daemon to accept (a `connect` with a timeout, which is non-blocking, would get
# EAGAIN right away). If the daemon stops accepting, the alarm ends the test with a failure.
signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(TimeoutError()))
signal.alarm(180)
try:
    for _ in range(CONNECTIONS):
        c = socket.socket(socket.AF_UNIX)
        try:
            c.connect(SOCKET_PATH)
            conns.append(c)
        except OSError:
            connect_errors += 1
            c.close()
except TimeoutError:
    fail(f"connect parado: o daemon parou de aceitar depois de {len(conns)} conexões")
signal.alarm(0)
dt = time.monotonic() - t
print(f"info  {len(conns)} conexões caladas abertas em {dt:.1f} s ({connect_errors} connect falhou)")
if connect_errors:
    fail(f"{connect_errors} connect recusado (o daemon parou de aceitar)")
normal_client(f"com {CONNECTIONS} conexões caladas")
# Nobody asked anything: the old silent ones were dropped to make room for the new ones.
still_open = 0
for c in conns:
    c.setblocking(False)
    try:
        if c.recv(1, socket.MSG_PEEK) == b"":
            continue
        still_open += 1  # did it send something? it should not
    except BlockingIOError:
        still_open += 1
    except ConnectionResetError:
        pass
if still_open <= 2 * SLOTS + 1:
    ok(f"das {len(conns)} caladas, {still_open} seguem abertas (até {2 * SLOTS + 1})")
else:
    fail(f"{still_open} conexões caladas seguem abertas (esperado até {2 * SLOTS + 1})")
for c in conns:
    c.close()
conns.clear()
if alive():
    ok("daemon vivo depois da enxurrada")
else:
    fail("daemon morreu com a enxurrada")
normal_client("depois da enxurrada")

if failures:
    print(f"socket_ataque: {len(failures)} falha(s)", file=sys.stderr)
    sys.exit(1)
print("socket_ataque: V3.4 ok")
