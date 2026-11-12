"""fase62: m2:V4.14 by hand against the live test daemon (agent role).

Usage: v414.py <repo root>; socket = $XDG_RUNTIME_DIR/ward-test/wardd.sock
"""
import base64, json, os, socket, struct, sys, time

ROOT = sys.argv[1]
SOCK = os.path.join(os.environ["XDG_RUNTIME_DIR"], "ward-test", "wardd.sock")
VARIATIONS = f"{ROOT}/.dev/variations.txt"
fx = json.load(open(f"{ROOT}/dev/fixtures/entries.json"))
variations = [l.rstrip(b"\n") for l in open(VARIATIONS, "rb") if len(l.rstrip(b"\n")) >= 6]
MARK = f"{ROOT}/.dev/fase62-v414-mark"
b64 = lambda s: base64.b64encode(s.encode()).decode()


def rx(s, n):
    b = b""
    while len(b) < n:
        c = s.recv(n - len(b))
        if not c:
            return None
        b += c
    return b


def session(obj):
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(20)
    s.connect(SOCK)
    b = json.dumps(obj).encode()
    s.sendall(struct.pack(">I", len(b)) + b)
    out = []
    try:
        while True:
            n = rx(s, 4)
            if n is None:
                break
            body = rx(s, struct.unpack(">I", n)[0])
            if body is None:
                break
            out.append(body)
            r = json.loads(body)
            if r.get("type") in ("error", "exit", "list", "status", "ok"):
                break
    except socket.timeout:
        out.append(b'{"type":"timeout"}')
    s.close()
    return out


def kind(frames):
    if not frames:
        return "fechou"
    r = json.loads(frames[-1])
    return r.get("data", {}).get("code") if r.get("type") == "error" else r.get("type")


fails = 0


def check(cond, msg):
    global fails
    print(("ok    " if cond else "FALHA ") + msg)
    if not cond:
        fails += 1


print("info  unlock:", kind(session({"v": 2, "op": "unlock", "data": {"password": b64(fx["master_password"])}})))


def run_data(**over):
    d = {"entry": "acme-pg-dev", "argv": [b64("/bin/sh"), b64("-c"), b64(f"touch {MARK}")],
         "cwd": b64("/"), "stdin_kind": "null", "environment": []}
    d.update(over)
    return d


cases = []
d = run_data(); d["entrada"] = d.pop("entry"); cases.append(("run com 'entrada'", {"v": 2, "op": "run", "data": d}))
d = run_data(); d["entrada_padrao"] = d.pop("stdin_kind"); cases.append(("run com 'entrada_padrao'", {"v": 2, "op": "run", "data": d}))
d = run_data(); d["tipo_entrada"] = d.pop("stdin_kind"); cases.append(("run com 'tipo_entrada'", {"v": 2, "op": "run", "data": d}))
d = run_data(); d["confirmacao_pipe"] = True; cases.append(("run com 'confirmacao_pipe'", {"v": 2, "op": "run", "data": d}))
d = run_data(); d["nivel"] = "free"; cases.append(("run com 'nivel'", {"v": 2, "op": "run", "data": d}))
d = run_data(); d["extra_desconhecido"] = 1; cases.append(("run com campo extra desconhecido", {"v": 2, "op": "run", "data": d}))
cases.append(("envelope com campo extra", {"v": 2, "op": "run", "data": run_data(), "x": 1}))
cases.append(("envelope 'dados' no lugar de 'data'", {"v": 2, "op": "run", "dados": run_data()}))
d = run_data(); d.pop("stdin_kind"); cases.append(("run sem stdin_kind (não vira terminal)", {"v": 2, "op": "run", "data": d}))
cases.append(("run com stdin_kind 'nula' (v1)", {"v": 2, "op": "run", "data": run_data(stdin_kind="nula")}))
cases.append(("run com stdin_kind 'arquivo' (v1)", {"v": 2, "op": "run", "data": run_data(stdin_kind="arquivo")}))
for old in ("executar", "revelar", "destravar", "lista", "travar", "estado", "copiar", "definir_nivel"):
    if old == "executar":
        data = run_data()
    elif old == "destravar":
        data = {"password": b64(fx["master_password"])}
    else:
        data = {}
    cases.append((f"operação antiga '{old}'", {"v": 2, "op": old, "data": data}))

for label, obj in cases:
    if os.path.exists(MARK):
        os.unlink(MARK)
    fr = session(obj)
    k = kind(fr)
    joined = b"".join(fr)
    leak = any(v in joined for v in variations)
    time.sleep(0.3)
    ran = os.path.exists(MARK)
    check(k in ("invalid_request", "unknown_operation") and not ran and not leak,
          f"V4.14 {label}: {k}, executou={ran}, segredo={leak}")

if os.path.exists(MARK):
    os.unlink(MARK)
fr = session({"v": 2, "op": "run", "data": run_data()})
time.sleep(0.3)
check(os.path.exists(MARK), f"controle: run v2 correto executa ({kind(fr)})")
if os.path.exists(MARK):
    os.unlink(MARK)
print(f"{fails} falha(s)")
sys.exit(1 if fails else 0)
