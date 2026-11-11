#!/usr/bin/python3 -I -B
"""V3.3 (S1, M3 scrutiny): operation × entry × field matrix through the socket, as agent.

Usage: socket_matrix.py <socket> <repository root> <variations.txt>
Called by `dev/tests/socket-attacks.sh` (`make test-socket-attacks`), with its own test
`wardd`, over a copy of the test vault.

- Unlocks with the test password (fake, D15) and takes the agent listing.
- For each operation of the protocol table (read from the ward-core protocol source,
  so it never goes stale), each entry of the test vault (all of
  `dev/fixtures/entries.json`, the blocked one included, plus a nonexistent one), with
  and without made-up fields (reveal and field flags, at the top level and in the
  data): no reply has a pattern of `.dev/variations.txt`, none is of a type that
  carries a credential, and every app-only or extension-only operation answers
  unauthorized.
- Never prints a reply nor a pattern: only operation, entry and counts.
"""

import base64
import collections
import json
import re
import socket
import struct
import sys

SOCKET_PATH, ROOT, VARIATIONS = sys.argv[1], sys.argv[2], sys.argv[3]
CREDENTIAL = {"segredo", "token", "codigo_totp", "preenchimento"}
AGENT_ROLES = {"ALL", "AGENT_AND_APP", "AGENT_ONLY"}
# Never requested here: they change the state of the test itself (the rest is refused or harmless).
SKIPPED = {"travar", "destravar"}

failures = []


def fail(msg):
    print(f"FALHA {msg}", file=sys.stderr)
    failures.append(msg)


def b64(x):
    return base64.b64encode(x.encode()).decode()


def read_exact(s, n):
    d = b""
    while len(d) < n:
        c = s.recv(n - len(d))
        if not c:
            return None
        d += c
    return d


def req(obj):
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(60)
    s.connect(SOCKET_PATH)
    try:
        b = json.dumps(obj).encode()
        s.sendall(struct.pack(">I", len(b)) + b)
        n = read_exact(s, 4)
        if n is None:
            return b""
        return read_exact(s, struct.unpack(">I", n)[0]) or b""
    finally:
        s.close()


def operations():
    """(name, roles) of each operation, from the operation table of the protocol."""
    text = open(f"{ROOT}/crates/ward-core/src/protocol.rs", encoding="utf-8").read()
    block = text.split("Operation, ", 1)[1].split("table!", 1)[0]
    ops = re.findall(r'^\s*\w+ = "([a-z_]+)", ([A-Z_]+),', block, re.M)
    if len(ops) < 25:
        print(f"socket_matriz: só {len(ops)} operações lidas da tabela", file=sys.stderr)
        sys.exit(1)
    return ops


variations = []
with open(VARIATIONS, "rb") as f:
    for line in f:
        line = line.rstrip(b"\n")
        if len(line) >= 6:
            variations.append(line)
if len(variations) < 50:
    print("socket_matriz: lista de variações curta demais; rode 'make dev-variations'", file=sys.stderr)
    sys.exit(1)

fixtures = json.load(open(f"{ROOT}/dev/fixtures/entries.json", encoding="utf-8"))
# Test master password and PIN (D15, fake), from the fixtures.
TEST_PASSWORD, TEST_PIN = fixtures["master_password"], fixtures["pin"]
names = sorted({e["name"] for e in fixtures["entries"]} | {"inexistente-ward"})

r = json.loads(req({"v": 1, "op": "destravar", "dados": {"senha": b64(TEST_PASSWORD)}}))
reply_kind = r.get("tipo")
if reply_kind != "ok":
    print(f"socket_matriz: destravar respondeu {reply_kind}", file=sys.stderr)
    sys.exit(1)
listing = json.loads(req({"v": 1, "op": "lista", "dados": {}}))
seen = {i["nome"] for i in listing["dados"]["itens"]}
if not seen or not seen <= set(names):
    fail("lista do agente fora das fixtures")
if "gama-bloqueada" in seen:
    fail("a bloqueada apareceu na lista do agente")
print(f"info  lista do agente: {len(seen)} entradas; matriz com {len(names)}")

counts = collections.Counter()
total = 0
for op, roles in operations():
    if op in SKIPPED:
        continue
    agent_allowed = roles in AGENT_ROLES
    for name in names:
        for extra in ({}, {"revelar": True, "campo": "senha"}):
            data = {
                "entrada": name,
                "campo": "senha",
                "alvo": {"campo": "senha"},
                "pedido": 1,
                "nivel": "livre",
                "pin": b64(TEST_PIN),
            }
            data.update(extra)
            frames = [
                {"v": 1, "op": op, "dados": data},
                {"v": 1, "op": op, "revelar": True, "campo": "senha", "dados": data},
                {"v": 1, "op": op},
            ]
            for q in frames:
                total += 1
                raw = req(q)
                for v in variations:
                    if v in raw:
                        fail(f"segredo na resposta de {op} ({name})")
                        break
                try:
                    resp = json.loads(raw)
                    kind = resp.get("tipo")
                    code = resp.get("dados", {}).get("codigo") if kind == "erro" else kind
                except (ValueError, AttributeError):
                    kind, code = None, "fechou" if not raw else "invalida"
                counts[(op, code)] += 1
                if kind in CREDENTIAL:
                    fail(f"{op} ({name}): resposta de credencial ({kind}) para o agente")
                if not agent_allowed and code != "nao_autorizado":
                    fail(f"{op} ({name}): só de {roles}, mas respondeu {code}")



def session(obj):
    """Every frame of a run session up to the exit frame, an error or the close."""
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(60)
    s.connect(SOCKET_PATH)
    frames = []
    try:
        b = json.dumps(obj).encode()
        s.sendall(struct.pack(">I", len(b)) + b)
        while True:
            n = read_exact(s, 4)
            if n is None:
                break
            body = read_exact(s, struct.unpack(">I", n)[0]) or b""
            frames.append(body)
            kind = json.loads(body).get("tipo")
            if kind in ("fim", "erro"):
                break
    finally:
        s.close()
    return frames


# A valid run (`env`, which prints the environment with the credential) with each entry:
# the output arrives masked, with no pattern of the list; with approval, without an app
# in the test daemon, it is "approval unavailable"; the blocked one answers like the
# nonexistent one.
run_replies = {}
for name in names:
    request = {
        "v": 1,
        "op": "executar",
        "dados": {
            "entrada": name,
            "argv": [b64("env")],
            "cwd": b64("/"),
            "tipo_entrada": "nula",
            "ambiente": [],
        },
    }
    frames = session(request)
    total += 1
    joined = b"".join(frames)
    for v in variations:
        if v in joined:
            fail(f"segredo na saída de executar ({name})")
            break
    kinds = []
    for q in frames:
        r = json.loads(q)
        kinds.append(r.get("dados", {}).get("codigo") if r.get("tipo") == "erro" else r.get("tipo"))
        reply_kind = r.get("tipo")
        if reply_kind in CREDENTIAL:
            fail(f"executar ({name}): resposta de credencial ({reply_kind})")
    run_replies[name] = joined
    counts[("executar(env)", kinds[-1] if kinds else "fechou")] += 1
if run_replies.get("gama-bloqueada") != run_replies.get("inexistente-ward"):
    fail("executar: a bloqueada responde diferente da inexistente")

print(f"info  {total} pedidos")
for (op, code), n in sorted(counts.items(), key=lambda x: (x[0][0], str(x[0][1]))):
    print(f"info  {op:24} {code} × {n}")
if failures:
    print(f"socket_matriz: {len(failures)} falha(s)", file=sys.stderr)
    sys.exit(1)
print(f"socket_matriz: V3.3 ok ({total} pedidos, nenhum segredo, app/extensão não autorizado)")
