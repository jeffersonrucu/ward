#!/usr/bin/python3 -I -B
"""V3.4 (S5, scrutiny do M3): o `wardd` de teste aguenta quadros ruins e 10 000
conexões caladas e continua atendendo um cliente normal.

Uso: socket_ataque.py <caminho do socket> <pid do wardd> [conexoes]
Chamado por `dev/testes/socket-ataques.sh` (`make teste-socket-ataques`), que sobe um
`wardd` só para o teste, num diretório próprio. Não imprime nada do que o daemon manda
além do tipo e do código das respostas (nenhuma tem segredo: o cofre fica travado).

Cada caso tem de passar; sai 1 na primeira rodada com falha (todas as falhas listadas).
"""

import json
import os
import resource
import signal
import socket
import struct
import sys
import time

CAMINHO = sys.argv[1]
PID = int(sys.argv[2])
CONEXOES = int(sys.argv[3]) if len(sys.argv) > 3 else 10_000
# Vagas de agente do daemon (`servidor::MAX_CONEXOES`): no fim da enxurrada, no máximo
# essas ficam abertas (mais as que ainda estavam na fila do `accept`).
VAGAS = 64

falhas = []


def ok(msg):
    print(f"ok    {msg}")


def falhou(msg):
    print(f"FALHA {msg}", file=sys.stderr)
    falhas.append(msg)


def novo(prazo=30):
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(prazo)
    s.connect(CAMINHO)
    return s


def ler_exato(s, n):
    d = b""
    while len(d) < n:
        c = s.recv(n - len(d))
        if not c:
            return None
        d += c
    return d


def resposta(s):
    """O próximo quadro (dict) ou None se a conexão fechou sem quadro."""
    try:
        n = ler_exato(s, 4)
        if n is None:
            return None
        corpo = ler_exato(s, struct.unpack(">I", n)[0])
        return None if corpo is None else json.loads(corpo)
    except ConnectionResetError:
        return None
    except socket.timeout:
        # Nem quadro nem fechamento no prazo: o daemon ficou esperando (falha).
        return {"tipo": "sem resposta no prazo"}


def pedir(s, op, dados=None):
    b = json.dumps({"v": 1, "op": op, "dados": dados or {}}).encode()
    s.sendall(struct.pack(">I", len(b)) + b)
    return resposta(s)


def fechada(s, prazo=10):
    """A conexão foi fechada pelo daemon (EOF ou reset) dentro do prazo."""
    s.settimeout(prazo)
    try:
        while True:
            c = s.recv(4096)
            if not c:
                return True
    except ConnectionResetError:
        return True
    except socket.timeout:
        return False


def tipo_codigo(r):
    if r is None:
        return "fechou"
    t = r.get("tipo")
    if t == "erro":
        return "erro:" + str(r.get("dados", {}).get("codigo"))
    return str(t)


def vivo():
    try:
        os.kill(PID, 0)
        return True
    except ProcessLookupError:
        return False


def normal(rotulo):
    t = time.monotonic()
    try:
        s = novo()
        r = pedir(s, "estado")
        s.close()
    except OSError as e:
        falhou(f"{rotulo}: cliente normal sem resposta ({e.__class__.__name__})")
        return
    dt = time.monotonic() - t
    if r is not None and r.get("tipo") == "estado":
        ok(f"{rotulo}: cliente normal atendido em {dt:.2f} s")
    else:
        falhou(f"{rotulo}: cliente normal recebeu {tipo_codigo(r)}")


# --- Quadros ruins: cada um recusado (erro) ou fechado, e o daemon segue -------------
normal("antes")
# (rótulo, bytes, o quadro em si é recusado: a conexão tem de fechar). JSON ruim num
# quadro bem formado recebe erro e a conexão pode seguir.
RUINS = [
    ("tamanho de 2 MiB", struct.pack(">I", 2 * 1024 * 1024) + b"x" * 1024, True),
    ("tamanho 0xFFFFFFFF (negativo em i32)", struct.pack(">i", -1), True),
    ("tamanho negativo com corpo", struct.pack(">i", -5) + b"{}{}{", True),
    ("tamanho 0", struct.pack(">I", 0), False),
    ("JSON inválido", struct.pack(">I", 5) + b"{nao}", False),
    ("JSON sem versão nem op", struct.pack(">I", 2) + b"{}", False),
    ("UTF-8 inválido", struct.pack(">I", 4) + b"\xff\xfe\xfd\xfc", False),
    ("versão errada", None, False),
    ("operação desconhecida", None, False),
]
for rotulo, bruto, deve_fechar in RUINS:
    s = novo(10)
    if rotulo == "versão errada":
        b = json.dumps({"v": 99, "op": "estado", "dados": {}}).encode()
        s.sendall(struct.pack(">I", len(b)) + b)
        r = resposta(s)
    elif rotulo == "operação desconhecida":
        r = pedir(s, "inventada")
    else:
        try:
            s.sendall(bruto)
        except (BrokenPipeError, ConnectionResetError):
            pass
        r = resposta(s)
    tc = tipo_codigo(r)
    if tc == "fechou" or tc.startswith("erro:"):
        ok(f"{rotulo}: {tc}")
    else:
        falhou(f"{rotulo}: aceito ({tc})")
    if deve_fechar and not fechada(s):
        falhou(f"{rotulo}: conexão continuou aberta")
    s.close()

# Quadro cortado: anuncia 100 bytes, manda 10 e fecha.
s = novo(10)
s.sendall(struct.pack(">I", 100) + b"{" * 10)
s.shutdown(socket.SHUT_WR)
if fechada(s):
    ok("quadro cortado: fechado")
else:
    falhou("quadro cortado: conexão continuou aberta")
s.close()
if vivo():
    ok("daemon vivo depois dos quadros ruins")
else:
    falhou("daemon morreu com os quadros ruins")
normal("depois dos quadros ruins")

# --- 10 000 conexões caladas ---------------------------------------------------------
mole, dura = resource.getrlimit(resource.RLIMIT_NOFILE)
precisa = CONEXOES + 256
if dura != resource.RLIM_INFINITY and dura < precisa:
    falhou(f"RLIMIT_NOFILE rígido {dura} < {precisa}: não dá para abrir {CONEXOES} conexões")
    sys.exit(1)
resource.setrlimit(resource.RLIMIT_NOFILE, (precisa, dura))
conexoes = []
erros_connect = 0
t = time.monotonic()
# `connect` bloqueante, como o de um cliente comum: com a fila do `listen` cheia, ele
# espera o daemon aceitar (um `connect` com prazo, que é não bloqueante, levaria
# EAGAIN na hora). Se o daemon parar de aceitar, o alarme encerra o teste com falha.
signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(TimeoutError()))
signal.alarm(180)
try:
    for _ in range(CONEXOES):
        c = socket.socket(socket.AF_UNIX)
        try:
            c.connect(CAMINHO)
            conexoes.append(c)
        except OSError:
            erros_connect += 1
            c.close()
except TimeoutError:
    falhou(f"connect parado: o daemon parou de aceitar depois de {len(conexoes)} conexões")
signal.alarm(0)
dt = time.monotonic() - t
print(f"info  {len(conexoes)} conexões caladas abertas em {dt:.1f} s ({erros_connect} connect falhou)")
if erros_connect:
    falhou(f"{erros_connect} connect recusado (o daemon parou de aceitar)")
normal(f"com {CONEXOES} conexões caladas")
# Ninguém pediu nada: as caladas antigas foram derrubadas para dar vaga às novas.
abertas = 0
for c in conexoes:
    c.setblocking(False)
    try:
        if c.recv(1, socket.MSG_PEEK) == b"":
            continue
        abertas += 1  # mandou algo? não deveria
    except BlockingIOError:
        abertas += 1
    except ConnectionResetError:
        pass
if abertas <= 2 * VAGAS + 1:
    ok(f"das {len(conexoes)} caladas, {abertas} seguem abertas (até {2 * VAGAS + 1})")
else:
    falhou(f"{abertas} conexões caladas seguem abertas (esperado até {2 * VAGAS + 1})")
for c in conexoes:
    c.close()
conexoes.clear()
if vivo():
    ok("daemon vivo depois da enxurrada")
else:
    falhou("daemon morreu com a enxurrada")
normal("depois da enxurrada")

if falhas:
    print(f"socket_ataque: {len(falhas)} falha(s)", file=sys.stderr)
    sys.exit(1)
print("socket_ataque: V3.4 ok")
