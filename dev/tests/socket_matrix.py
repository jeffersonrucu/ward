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
- m2:V4.13: each v1 request of `dev/fixtures/legacy/protocol-v1.jsonl` (protocol of
  the 0.1.x) gets only the unknown_version error, with no data, and the connection
  closes.
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
CREDENTIAL = {"secret", "token", "totp_code", "fill"}
AGENT_ROLES = {"ALL", "AGENT_AND_APP", "AGENT_ONLY"}
# Never requested here: they change the state of the test itself (the rest is refused or harmless).
SKIPPED = {"lock", "unlock"}

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

r = json.loads(req({"v": 2, "op": "unlock", "data": {"password": b64(TEST_PASSWORD)}}))
reply_kind = r.get("type")
if reply_kind != "ok":
    print(f"socket_matriz: unlock respondeu {reply_kind}", file=sys.stderr)
    sys.exit(1)
listing = json.loads(req({"v": 2, "op": "list", "data": {}}))
seen = {i["name"] for i in listing["data"]["items"]}
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
        for extra in ({}, {"reveal": True, "field": "password"}):
            data = {
                "entry": name,
                "field": "password",
                "target": {"field": "password"},
                "request": 1,
                "level": "free",
                "pin": b64(TEST_PIN),
            }
            data.update(extra)
            frames = [
                {"v": 2, "op": op, "data": data},
                {"v": 2, "op": op, "reveal": True, "field": "password", "data": data},
                {"v": 2, "op": op},
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
                    kind = resp.get("type")
                    code = resp.get("data", {}).get("code") if kind == "error" else kind
                except (ValueError, AttributeError):
                    kind, code = None, "fechou" if not raw else "invalida"
                counts[(op, code)] += 1
                if kind in CREDENTIAL:
                    fail(f"{op} ({name}): resposta de credencial ({kind}) para o agente")
                if not agent_allowed and code != "unauthorized":
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
            kind = json.loads(body).get("type")
            if kind in ("exit", "error"):
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
        "v": 2,
        "op": "run",
        "data": {
            "entry": name,
            "argv": [b64("env")],
            "cwd": b64("/"),
            "stdin_kind": "null",
            "environment": [],
        },
    }
    frames = session(request)
    total += 1
    joined = b"".join(frames)
    for v in variations:
        if v in joined:
            fail(f"segredo na saída de run ({name})")
            break
    kinds = []
    for q in frames:
        r = json.loads(q)
        kinds.append(r.get("data", {}).get("code") if r.get("type") == "error" else r.get("type"))
        reply_kind = r.get("type")
        if reply_kind in CREDENTIAL:
            fail(f"run ({name}): resposta de credencial ({reply_kind})")
    run_replies[name] = joined
    counts[("run(env)", kinds[-1] if kinds else "fechou")] += 1
if run_replies.get("gama-bloqueada") != run_replies.get("inexistente-ward"):
    fail("run: a bloqueada responde diferente da inexistente")


def v1_request(raw):
    """A v1 frame as it is in the fixture: the reply, and whether the daemon closed after."""
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(30)
    s.connect(SOCKET_PATH)
    try:
        s.sendall(struct.pack(">I", len(raw)) + raw)
        n = read_exact(s, 4)
        body = b"" if n is None else (read_exact(s, struct.unpack(">I", n)[0]) or b"")
        try:
            closed = s.recv(1) == b""
        except ConnectionResetError:
            closed = True
        except socket.timeout:
            closed = False
        return body, closed
    finally:
        s.close()


# m2:V4.13 (S1, RM3): each v1 request line of the 0.1.4 fixture, with the vault
# unlocked: the reply is only the unknown_version error (code and message, no data at
# all), no pattern of the list, and the daemon closes the connection.
V1_FIXTURE = f"{ROOT}/dev/fixtures/legacy/protocol-v1.jsonl"
v1_lines = 0
with open(V1_FIXTURE, encoding="utf-8") as f:
    for line in f:
        row = json.loads(line)
        if row["kind"] != "request":
            continue
        v1_lines += 1
        op = row["frame"].get("op")
        raw = json.dumps(row["frame"], separators=(",", ":")).encode()
        body, closed = v1_request(raw)
        total += 1
        for v in variations:
            if v in body:
                fail(f"V4.13: segredo na resposta ao v1 {op}")
                break
        try:
            resp = json.loads(body)
        except ValueError:
            resp = None
        data = resp.get("data") if isinstance(resp, dict) else None
        if (
            not isinstance(resp, dict)
            or sorted(resp) != ["data", "type", "v"]
            or resp["v"] != 2
            or resp["type"] != "error"
            or not isinstance(data, dict)
            or sorted(data) != ["code", "message"]
            or data["code"] != "unknown_version"
        ):
            fail(f"V4.13: v1 {op} não respondeu só unknown_version")
        if not closed:
            fail(f"V4.13: v1 {op}: o daemon não fechou a conexão")
        counts[(f"v1 {op}", data.get("code") if isinstance(data, dict) else "invalida")] += 1
if v1_lines < 30:
    fail(f"V4.13: só {v1_lines} pedidos v1 lidos de {V1_FIXTURE}")
# The v1 unlock line carries the test password: the vault stays as it was (unlocked
# here) and the list still answers v2.
after = json.loads(req({"v": 2, "op": "list", "data": {}}))
if after.get("type") != "list":
    fail("V4.13: depois dos quadros v1, o list v2 não respondeu")

print(f"info  {total} pedidos")
for (op, code), n in sorted(counts.items(), key=lambda x: (x[0][0], str(x[0][1]))):
    print(f"info  {op:24} {code} × {n}")
if failures:
    print(f"socket_matriz: {len(failures)} falha(s)", file=sys.stderr)
    sys.exit(1)
print(f"socket_matriz: V3.3 ok ({total} pedidos, nenhum segredo, app/extensão não autorizado)")
