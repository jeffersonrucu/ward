"""`make test-nm` (Phase 40): the test `ward-nm` run directly and through the real Chrome.

Called by `dev/tests/nm.sh`, which starts its own test `wardd`. Arguments: root, work
directory (in `.dev/`), socket directory, `ward-nm`, the `chrome-parent` example,
Chrome and the variation list. Never prints the list nor any value of it.
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
r = json.loads(req({"v": 1, "op": "destravar", "dados": {"senha": password}}))
verify(r.get("tipo") == "ok", "destravou o cofre de teste (cópia) pelo socket")

# Phase 41: `ward-nm` only accepts the Ward id; the test extension carries the same
# public key (`key`) of `extension/public-key.txt`, so it has the same id.
with open(os.path.join(ROOT, "extension", "public-key.txt"), encoding="ascii") as f:
    PUBLIC_KEY = "".join(l for l in f.read().split("\n") if l and not l.startswith("-----"))
eid = "".join(chr(ord("a") + int(c, 16)) for c in hashlib.sha256(base64.b64decode(PUBLIC_KEY)).hexdigest()[:32])
ORIGIN = f"chrome-extension://{eid}/"
FILL = {"v": 1, "op": "preencher", "dados": {"entrada": "acme-wp-dev", "origem": "http://localhost:8081"}}
SUGGEST = {"v": 1, "op": "sugerir", "dados": {"origem": "http://localhost:8081"}}

# --- V6.1: ward-nm run directly by a user process --------------------------------------
env = {"PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR, "HOME": WORK_DIR}
input_data = frame({"v": 1, "op": "estado"}) + frame(FILL) + frame(SUGGEST)
p = subprocess.run([NM, ORIGIN], input=input_data, capture_output=True, env=env, timeout=60)
m = messages(p.stdout)
verify(p.returncode == 0 and len(m) == 3, "V6.1: o ward-nm respondeu as três mensagens")
if len(m) == 3:
    verify(m[0].get("dados", {}).get("codigo") == "nao_autorizado",
            "V6.1: aberto por um processo do usuário (sem setgid, pai não é o Chrome), o daemon não dá o papel extensão")
    for i, name in ((1, "preencher"), (2, "sugerir")):
        verify(m[i].get("dados", {}).get("codigo") == "nao_autorizado",
                f"V6.1: {name} do ward-nm aberto à mão: o daemon recusa (nao_autorizado)")
no_secret(p.stdout, "V6.1: stdout do ward-nm")
no_secret(p.stderr, "V6.1: stderr do ward-nm")

# Message over the limit and malformed, through the real binary.
p = subprocess.run([NM, ORIGIN], input=struct.pack("=I", 70000) + b"{" * 70000,
                   capture_output=True, env=env, timeout=60)
m = messages(p.stdout)
verify(p.returncode == 1 and m and m[0]["dados"]["codigo"] == "quadro_invalido",
        "mensagem de 70 000 bytes: quadro_invalido e o host sai ≠ 0")
p = subprocess.run([NM, ORIGIN], input=frame(b"{nao json") + frame(
    {"v": 1, "op": "revelar", "dados": {"entrada": "acme-wp-dev", "campo": "senha"}}),
    capture_output=True, env=env, timeout=60)
m = messages(p.stdout)
verify([x["dados"]["codigo"] for x in m] == ["pedido_invalido", "nao_autorizado"],
        "JSON malformado e revelar (operação do app): recusados no host")
no_secret(p.stdout, "mensagens ruins: stdout do ward-nm")

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
    json.dump({"manifest_version": 3, "name": "ward-teste-nm", "version": "1", "key": PUBLIC_KEY,
               "permissions": ["nativeMessaging"], "background": {"service_worker": "sw.js"}}, f)
with open(os.path.join(ext, "sw.js"), "w") as f:
    f.write("""
const enviar = (host, m) => new Promise((r) => chrome.runtime.sendNativeMessage(host, m, (x) =>
  r(x === undefined ? {erro_chrome: String(chrome.runtime.lastError && chrome.runtime.lastError.message)} : x)));
(async () => {
  const res = {};
  res.sonda = await enviar("br.ward.sonda", {oi: 1});
  res.estado = await enviar("br.ward.nm", {v: 1, op: "estado"});
  res.preencher = await enviar("br.ward.nm", %s);
  res.revelar = await enviar("br.ward.nm", {v: 1, op: "revelar", dados: {entrada: "acme-wp-dev", campo: "senha"}});
  res.grande = await enviar("br.ward.nm", {v: 1, op: "sugerir", dados: {origem: "http://localhost:8081/" + "a".repeat(70000)}});
  await enviar("br.ward.relato", res);
})();
""" % json.dumps(FILL))

probe_sock = os.path.join(WORK_DIR, "sonda.sock")
srv = subprocess.Popen([PROBE, "serve", probe_sock, CHROME], stdout=subprocess.PIPE, text=True)
verify(srv.stdout.readline().strip() == "pronto", "verificador do pai pronto")

r_cmd, w_cmd = os.pipe()
r_res, w_res = os.pipe()


def prep():
    os.dup2(r_cmd, 3)
    os.dup2(w_res, 4)


chrome_env = {"HOME": home, "PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR,
              "WARD_PAI_CHROME_SOCK": probe_sock}
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
    est = res.get("estado", {})
    verify(est.get("dados", {}).get("codigo") == "nao_autorizado",
            "Chrome real: o ward-nm de teste (sem setgid) não recebe o papel extensão (estado: nao_autorizado)")
    for name in ("preencher", "revelar"):
        verify(res.get(name, {}).get("dados", {}).get("codigo") == "nao_autorizado",
                f"Chrome real: {name} pelo ward-nm sem o papel extensão é recusado")
    verify(res.get("grande", {}).get("dados", {}).get("codigo") == "quadro_invalido",
            "Chrome real: mensagem de mais de 64 KiB recusada pelo host")
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
subprocess.run([PROBE, ORIGIN], env={"WARD_PAI_CHROME_SOCK": probe_sock}, capture_output=True, timeout=30)
line = srv.communicate(timeout=30)[0].strip()
verify(line.startswith("pai=ParentNotAllowed"), "controle: o host aberto por outro processo é recusado (ParentNotAllowed)")
shutil.rmtree(profile, ignore_errors=True)

if failures:
    print(f"nm.py: {len(failures)} conferência(s) falharam", file=sys.stderr)
    sys.exit(1)
