"""`make test-nm` (Phase 40): the test `ward-nm` run directly and through the real Chrome.

Called by `dev/tests/nm.sh`, which starts its own test `wardd`. Arguments: root, work
directory (in `.dev/`), socket directory, `ward-nm`, the `chrome-parent` example,
Chrome and the variation list. Never prints the list nor any value of it.

m2:V5.10: the v1 requests of the 0.1.x extension (directly and through the real Chrome)
get only unknown_version, with no username, password nor TOTP.
"""

import base64
import hashlib
import json
import os
import shutil
import socket
import struct
import subprocess
import sys
import time

ROOT, WORK_DIR, SOCKDIR, NM, PROBE, CHROME, PATTERN_LIST = sys.argv[1:8]
SOCKET_PATH = os.path.join(SOCKDIR, "wardd.sock")
failures = []


def fail(msg):
    print(f"FALHA {msg}", file=sys.stderr)
    failures.append(msg)


def ok(msg):
    print(f"ok    {msg}")


def verify(cond, msg):
    if cond:
        ok(msg)
    else:
        fail(msg)


patterns = [l for l in open(PATTERN_LIST, "rb").read().split(b"\n") if l]
if len(patterns) < 10:
    print("nm.py: lista de variações curta demais; rode 'make dev-variations'", file=sys.stderr)
    sys.exit(1)


def no_secret(data, where):
    found = any(p in data for p in patterns)
    verify(not found, f"{where}: nenhum padrão de .dev/variations.txt")


def req(obj):
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(60)
    s.connect(SOCKET_PATH)
    try:
        b = json.dumps(obj).encode()
        s.sendall(struct.pack(">I", len(b)) + b)
        d = b""
        while len(d) < 4:
            c = s.recv(4 - len(d))
            if not c:
                return b""
            d += c
        n = struct.unpack(">I", d)[0]
        d = b""
        while len(d) < n:
            c = s.recv(n - len(d))
            if not c:
                break
            d += c
        return d
    finally:
        s.close()


def frame(obj):
    b = obj if isinstance(obj, bytes) else json.dumps(obj).encode()
    return struct.pack("=I", len(b)) + b


def messages(out):
    v = []
    while len(out) >= 4:
        n = struct.unpack("=I", out[:4])[0]
        v.append(json.loads(out[4 : 4 + n]))
        out = out[4 + n :]
    return v


fixtures = json.load(open(f"{ROOT}/dev/fixtures/entries.json", encoding="utf-8"))
password = base64.b64encode(fixtures["master_password"].encode()).decode()
r = json.loads(req({"v": 2, "op": "unlock", "data": {"password": password}}))
verify(r.get("type") == "ok", "destravou o cofre de teste (cópia) pelo socket")

# Phase 41: `ward-nm` only accepts the Ward id; the test extension carries the same
# public key (`key`) of `extension/public-key.txt`, so it has the same id.
with open(os.path.join(ROOT, "extension", "public-key.txt"), encoding="ascii") as f:
    PUBLIC_KEY = "".join(l for l in f.read().split("\n") if l and not l.startswith("-----"))
eid = "".join(chr(ord("a") + int(c, 16)) for c in hashlib.sha256(base64.b64decode(PUBLIC_KEY)).hexdigest()[:32])
ORIGIN = f"chrome-extension://{eid}/"
FILL = {"v": 2, "op": "fill", "data": {"entry": "acme-wp-dev", "origin": "http://localhost:8081"}}
SUGGEST = {"v": 2, "op": "suggest", "data": {"origin": "http://localhost:8081"}}
# m2:V5.10: the same requests as the 0.1.x extension sends them (protocol v1).
FILL_V1 = {"v": 1, "op": "preencher", "dados": {"entrada": "acme-wp-dev", "origem": "http://localhost:8081"}}
SUGGEST_V1 = {"v": 1, "op": "sugerir", "dados": {"origem": "http://localhost:8081"}}

# --- V6.1: ward-nm run directly by a user process --------------------------------------
env = {"PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR, "HOME": WORK_DIR}
input_data = frame({"v": 2, "op": "status"}) + frame(FILL) + frame(SUGGEST)
p = subprocess.run([NM, ORIGIN], input=input_data, capture_output=True, env=env, timeout=60)
m = messages(p.stdout)
verify(p.returncode == 0 and len(m) == 3, "V6.1: o ward-nm respondeu as três mensagens")
if len(m) == 3:
    verify(m[0].get("data", {}).get("code") == "unauthorized",
            "V6.1: aberto por um processo do usuário (sem setgid, pai não é o Chrome), o daemon não dá o papel extensão")
    for i, name in ((1, "fill"), (2, "suggest")):
        verify(m[i].get("data", {}).get("code") == "unauthorized",
                f"V6.1: {name} do ward-nm aberto à mão: o daemon recusa (unauthorized)")
no_secret(p.stdout, "V6.1: stdout do ward-nm")
no_secret(p.stderr, "V6.1: stderr do ward-nm")

# Message over the limit and malformed, through the real binary.
p = subprocess.run([NM, ORIGIN], input=struct.pack("=I", 70000) + b"{" * 70000,
                   capture_output=True, env=env, timeout=60)
m = messages(p.stdout)
verify(p.returncode == 1 and m and m[0]["data"]["code"] == "invalid_frame",
        "mensagem de 70 000 bytes: invalid_frame e o host sai ≠ 0")
p = subprocess.run([NM, ORIGIN], input=frame(b"{nao json") + frame(
    {"v": 2, "op": "reveal", "data": {"entry": "acme-wp-dev", "field": "password"}}),
    capture_output=True, env=env, timeout=60)
m = messages(p.stdout)
verify([x["data"]["code"] for x in m] == ["invalid_request", "unauthorized"],
        "JSON malformado e reveal (operação do app): recusados no host")
no_secret(p.stdout, "mensagens ruins: stdout do ward-nm")


def only_unknown_version(r):
    """m2:V5.10: the reply to a v1 frame is only the unknown_version error, with no
    data (no username, password nor TOTP)."""
    return (
        isinstance(r, dict)
        and sorted(r) == ["data", "type", "v"]
        and r["v"] == 2
        and r["type"] == "error"
        and isinstance(r["data"], dict)
        and sorted(r["data"]) == ["code", "message"]
        and r["data"]["code"] == "unknown_version"
    )


# m2:V5.10 (S1): the 0.1.x extension talks v1 to this ward-nm; with the vault unlocked,
# every v1 request gets unknown_version and nothing else.
p = subprocess.run([NM, ORIGIN], input=frame({"v": 1, "op": "estado"}) + frame(FILL_V1) + frame(SUGGEST_V1),
                   capture_output=True, env=env, timeout=60)
m = messages(p.stdout)
verify(len(m) == 3 and all(only_unknown_version(x) for x in m),
        "m2:V5.10: estado, preencher e sugerir v1 (extensão 0.1.x): só unknown_version")
no_secret(p.stdout, "m2:V5.10: stdout do ward-nm com os pedidos v1")
for word in (b"password", b"senha", b"username", b"usuario", b"totp"):
    verify(word not in p.stdout, f"m2:V5.10: nada de {word.decode()} na resposta aos pedidos v1")

# --- Real Chrome ---------------------------------------------------------------------
ext = os.path.join(WORK_DIR, "ext")
profile = os.path.join(WORK_DIR, "perfil")
home = os.path.join(WORK_DIR, "home")
hosts = os.path.join(profile, "NativeMessagingHosts")
os.makedirs(ext)
os.makedirs(hosts)
os.makedirs(home)
report = os.path.join(WORK_DIR, "relato.json")
report_host = os.path.join(WORK_DIR, "relato.py")
with open(report_host, "w") as f:
    f.write(f"""#!/usr/bin/python3 -I
import struct, sys
n = struct.unpack("=I", sys.stdin.buffer.read(4))[0]
open({report!r}, "wb").write(sys.stdin.buffer.read(n))
o = b'{{"ok":true}}'
sys.stdout.buffer.write(struct.pack("=I", len(o)) + o)
sys.stdout.buffer.flush()
""")
os.chmod(report_host, 0o700)
for name, path in (("br.ward.nm", NM), ("br.ward.sonda", PROBE), ("br.ward.relato", report_host)):
    with open(os.path.join(hosts, name + ".json"), "w") as f:
        json.dump({"name": name, "description": "teste", "path": path, "type": "stdio",
                   "allowed_origins": [f"chrome-extension://{eid}/"]}, f)
with open(os.path.join(ext, "manifest.json"), "w") as f:
    json.dump({"manifest_version": 3, "name": "ward-test-nm", "version": "1", "key": PUBLIC_KEY,
               "permissions": ["nativeMessaging"], "background": {"service_worker": "sw.js"}}, f)
with open(os.path.join(ext, "sw.js"), "w") as f:
    f.write("""
const enviar = (host, m) => new Promise((r) => chrome.runtime.sendNativeMessage(host, m, (x) =>
  r(x === undefined ? {erro_chrome: String(chrome.runtime.lastError && chrome.runtime.lastError.message)} : x)));
(async () => {
  const res = {};
  res.sonda = await enviar("br.ward.sonda", {oi: 1});
  res.status = await enviar("br.ward.nm", {v: 2, op: "status"});
  res.fill = await enviar("br.ward.nm", %s);
  res.reveal = await enviar("br.ward.nm", {v: 2, op: "reveal", data: {entry: "acme-wp-dev", field: "password"}});
  res.grande = await enviar("br.ward.nm", {v: 2, op: "suggest", data: {origin: "http://localhost:8081/" + "a".repeat(70000)}});
  res.fill_v1 = await enviar("br.ward.nm", %s);
  res.suggest_v1 = await enviar("br.ward.nm", %s);
  await enviar("br.ward.relato", res);
})();
""" % (json.dumps(FILL), json.dumps(FILL_V1), json.dumps(SUGGEST_V1)))

probe_sock = os.path.join(WORK_DIR, "sonda.sock")
srv = subprocess.Popen([PROBE, "serve", probe_sock, CHROME], stdout=subprocess.PIPE, text=True)
verify(srv.stdout.readline().strip() == "pronto", "verificador do pai pronto")

r_cmd, w_cmd = os.pipe()
r_res, w_res = os.pipe()


def prep():
    os.dup2(r_cmd, 3)
    os.dup2(w_res, 4)


chrome_env = {"HOME": home, "PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR,
              "WARD_CHROME_PARENT_SOCK": probe_sock}
log_chrome = open(os.path.join(WORK_DIR, "chrome.log"), "w")
ch = subprocess.Popen(
    [CHROME, "--headless=new", f"--user-data-dir={profile}", "--remote-debugging-pipe",
     "--enable-unsafe-extension-debugging", "--no-first-run", "--no-default-browser-check",
     "--disable-gpu", "--disable-sync", "--disable-background-networking", "about:blank"],
    preexec_fn=prep, pass_fds=(3, 4), env=chrome_env, stdout=subprocess.DEVNULL, stderr=log_chrome)
os.close(r_cmd)
os.close(w_res)
try:
    os.write(w_cmd, json.dumps({"id": 1, "method": "Extensions.loadUnpacked",
                                "params": {"path": ext}}).encode() + b"\0")
    buf = b""
    resp = None
    deadline = time.time() + 30
    while resp is None and time.time() < deadline:
        d = os.read(r_res, 65536)
        if not d:
            break
        buf += d
        while b"\0" in buf:
            msg, _, buf = buf.partition(b"\0")
            j = json.loads(msg)
            if j.get("id") == 1:
                resp = j
    verify(resp is not None and resp.get("result", {}).get("id") == eid,
            "extensão de teste carregada no Chrome real (perfil temporário)")
    for _ in range(300):
        if os.path.exists(report):
            break
        time.sleep(0.1)
    line = srv.communicate(timeout=30)[0].strip()
    print(f"      verificador: {line}")
    verify(line.startswith("pai=Ok "),
            "D47: o host aberto pelo Chrome tem o Chrome instalado de pai (verify_parent pelo /proc)")
    verify(f"ppid={ch.pid} " in line, "D47: o Chrome abre o host como filho direto do navegador (sem intermediário)")
    verify("pai_dumpable=sim" in line, "D47: o processo do navegador é dumpable (exe legível pelo /proc)")
    try:
        res = json.load(open(report, "rb"))
    except (OSError, ValueError):
        res = {}
        fail("a extensão não mandou o relatório")
    no_secret(json.dumps(res).encode(), "relatório da extensão")
    verify(res.get("sonda") == {"ok": True}, "Chrome real: a sonda respondeu pelo native messaging")
    est = res.get("status", {})
    verify(est.get("data", {}).get("code") == "unauthorized",
            "Chrome real: o ward-nm de teste (sem setgid) não recebe o papel extensão (status: unauthorized)")
    for name in ("fill", "reveal"):
        verify(res.get(name, {}).get("data", {}).get("code") == "unauthorized",
                f"Chrome real: {name} pelo ward-nm sem o papel extensão é recusado")
    verify(res.get("grande", {}).get("data", {}).get("code") == "invalid_frame",
            "Chrome real: mensagem de mais de 64 KiB recusada pelo host")
    for name in ("fill_v1", "suggest_v1"):
        verify(only_unknown_version(res.get(name)),
                f"m2:V5.10: Chrome real: {name} (extensão 0.1.x, protocolo v1) recebe só unknown_version")
finally:
    ch.terminate()
    try:
        ch.wait(timeout=20)
    except subprocess.TimeoutExpired:
        ch.kill()
    if srv.poll() is None:
        srv.kill()

# Control: the same host opened by this process (not by Chrome).
srv = subprocess.Popen([PROBE, "serve", probe_sock, CHROME], stdout=subprocess.PIPE, text=True)
srv.stdout.readline()
subprocess.run([PROBE, ORIGIN], env={"WARD_CHROME_PARENT_SOCK": probe_sock}, capture_output=True, timeout=30)
line = srv.communicate(timeout=30)[0].strip()
verify(line.startswith("pai=ParentNotAllowed"), "controle: o host aberto por outro processo é recusado (ParentNotAllowed)")
shutil.rmtree(profile, ignore_errors=True)

if failures:
    print(f"nm.py: {len(failures)} conferência(s) falharam", file=sys.stderr)
    sys.exit(1)
