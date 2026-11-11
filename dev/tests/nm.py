"""`make test-nm` (Fase 40): o `ward-nm` de teste direto e pelo Chrome real.

Chamado por `dev/tests/nm.sh`, que sobe o `wardd` de teste só dele. Argumentos: raiz,
pasta de trabalho (em `.dev/`), pasta do socket, `ward-nm`, exemplo `chrome-parent`,
Chrome e a lista de variações. Nunca imprime a lista nem um valor dela.
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

RAIZ, TRAB, SOCKDIR, NM, SONDA, CHROME, LISTA = sys.argv[1:8]
CAMINHO = os.path.join(SOCKDIR, "wardd.sock")
falhas = []


def falhou(msg):
    print(f"FALHA {msg}", file=sys.stderr)
    falhas.append(msg)


def ok(msg):
    print(f"ok    {msg}")


def confere(cond, msg):
    if cond:
        ok(msg)
    else:
        falhou(msg)


padroes = [l for l in open(LISTA, "rb").read().split(b"\n") if l]
if len(padroes) < 10:
    print("nm.py: lista de variações curta demais; rode 'make dev-variations'", file=sys.stderr)
    sys.exit(1)


def sem_segredo(dados, onde):
    achou = any(p in dados for p in padroes)
    confere(not achou, f"{onde}: nenhum padrão de .dev/variations.txt")


def req(obj):
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(60)
    s.connect(CAMINHO)
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


def quadro(obj):
    b = obj if isinstance(obj, bytes) else json.dumps(obj).encode()
    return struct.pack("=I", len(b)) + b


def mensagens(saida):
    v = []
    while len(saida) >= 4:
        n = struct.unpack("=I", saida[:4])[0]
        v.append(json.loads(saida[4 : 4 + n]))
        saida = saida[4 + n :]
    return v


fixtures = json.load(open(f"{RAIZ}/dev/fixtures/entries.json", encoding="utf-8"))
senha = base64.b64encode(fixtures["senha_mestra"].encode()).decode()
r = json.loads(req({"v": 1, "op": "destravar", "dados": {"senha": senha}}))
confere(r.get("tipo") == "ok", "destravou o cofre de teste (cópia) pelo socket")

# Fase 41: o `ward-nm` só aceita o id do Ward; a extensão de teste leva a mesma chave
# pública (`key`) de `extension/public-key.txt`, então tem o mesmo id.
with open(os.path.join(RAIZ, "extension", "public-key.txt"), encoding="ascii") as f:
    CHAVE = "".join(l for l in f.read().split("\n") if l and not l.startswith("-----"))
eid = "".join(chr(ord("a") + int(c, 16)) for c in hashlib.sha256(base64.b64decode(CHAVE)).hexdigest()[:32])
ORIGEM = f"chrome-extension://{eid}/"
PREENCHER = {"v": 1, "op": "preencher", "dados": {"entrada": "acme-wp-dev", "origem": "http://localhost:8081"}}
SUGERIR = {"v": 1, "op": "sugerir", "dados": {"origem": "http://localhost:8081"}}

# --- V6.1: ward-nm executado direto por um processo do usuário -----------------------
env = {"PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR, "HOME": TRAB}
entrada = quadro({"v": 1, "op": "estado"}) + quadro(PREENCHER) + quadro(SUGERIR)
p = subprocess.run([NM, ORIGEM], input=entrada, capture_output=True, env=env, timeout=60)
m = mensagens(p.stdout)
confere(p.returncode == 0 and len(m) == 3, "V6.1: o ward-nm respondeu as três mensagens")
if len(m) == 3:
    confere(m[0].get("dados", {}).get("codigo") == "nao_autorizado",
            "V6.1: aberto por um processo do usuário (sem setgid, pai não é o Chrome), o daemon não dá o papel extensão")
    for i, nome in ((1, "preencher"), (2, "sugerir")):
        confere(m[i].get("dados", {}).get("codigo") == "nao_autorizado",
                f"V6.1: {nome} do ward-nm aberto à mão: o daemon recusa (nao_autorizado)")
sem_segredo(p.stdout, "V6.1: stdout do ward-nm")
sem_segredo(p.stderr, "V6.1: stderr do ward-nm")

# Mensagem acima do limite e malformada, pelo binário real.
p = subprocess.run([NM, ORIGEM], input=struct.pack("=I", 70000) + b"{" * 70000,
                   capture_output=True, env=env, timeout=60)
m = mensagens(p.stdout)
confere(p.returncode == 1 and m and m[0]["dados"]["codigo"] == "quadro_invalido",
        "mensagem de 70 000 bytes: quadro_invalido e o host sai ≠ 0")
p = subprocess.run([NM, ORIGEM], input=quadro(b"{nao json") + quadro(
    {"v": 1, "op": "revelar", "dados": {"entrada": "acme-wp-dev", "campo": "senha"}}),
    capture_output=True, env=env, timeout=60)
m = mensagens(p.stdout)
confere([x["dados"]["codigo"] for x in m] == ["pedido_invalido", "nao_autorizado"],
        "JSON malformado e revelar (operação do app): recusados no host")
sem_segredo(p.stdout, "mensagens ruins: stdout do ward-nm")

# --- Chrome real ---------------------------------------------------------------------
ext = os.path.join(TRAB, "ext")
perfil = os.path.join(TRAB, "perfil")
home = os.path.join(TRAB, "home")
hosts = os.path.join(perfil, "NativeMessagingHosts")
os.makedirs(ext)
os.makedirs(hosts)
os.makedirs(home)
relato = os.path.join(TRAB, "relato.json")
host_relato = os.path.join(TRAB, "relato.py")
with open(host_relato, "w") as f:
    f.write(f"""#!/usr/bin/python3 -I
import struct, sys
n = struct.unpack("=I", sys.stdin.buffer.read(4))[0]
open({relato!r}, "wb").write(sys.stdin.buffer.read(n))
o = b'{{"ok":true}}'
sys.stdout.buffer.write(struct.pack("=I", len(o)) + o)
sys.stdout.buffer.flush()
""")
os.chmod(host_relato, 0o700)
for nome, caminho in (("br.ward.nm", NM), ("br.ward.sonda", SONDA), ("br.ward.relato", host_relato)):
    with open(os.path.join(hosts, nome + ".json"), "w") as f:
        json.dump({"name": nome, "description": "teste", "path": caminho, "type": "stdio",
                   "allowed_origins": [f"chrome-extension://{eid}/"]}, f)
with open(os.path.join(ext, "manifest.json"), "w") as f:
    json.dump({"manifest_version": 3, "name": "ward-teste-nm", "version": "1", "key": CHAVE,
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
""" % json.dumps(PREENCHER))

sock_sonda = os.path.join(TRAB, "sonda.sock")
srv = subprocess.Popen([SONDA, "serve", sock_sonda, CHROME], stdout=subprocess.PIPE, text=True)
confere(srv.stdout.readline().strip() == "pronto", "verificador do pai pronto")

r_cmd, w_cmd = os.pipe()
r_res, w_res = os.pipe()


def prep():
    os.dup2(r_cmd, 3)
    os.dup2(w_res, 4)


chrome_env = {"HOME": home, "PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR,
              "WARD_PAI_CHROME_SOCK": sock_sonda}
log_chrome = open(os.path.join(TRAB, "chrome.log"), "w")
ch = subprocess.Popen(
    [CHROME, "--headless=new", f"--user-data-dir={perfil}", "--remote-debugging-pipe",
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
    fim = time.time() + 30
    while resp is None and time.time() < fim:
        d = os.read(r_res, 65536)
        if not d:
            break
        buf += d
        while b"\0" in buf:
            msg, _, buf = buf.partition(b"\0")
            j = json.loads(msg)
            if j.get("id") == 1:
                resp = j
    confere(resp is not None and resp.get("result", {}).get("id") == eid,
            "extensão de teste carregada no Chrome real (perfil temporário)")
    for _ in range(300):
        if os.path.exists(relato):
            break
        time.sleep(0.1)
    linha = srv.communicate(timeout=30)[0].strip()
    print(f"      verificador: {linha}")
    confere(linha.startswith("pai=Ok "),
            "D47: o host aberto pelo Chrome tem o Chrome instalado de pai (verify_parent pelo /proc)")
    confere(f"ppid={ch.pid} " in linha, "D47: o Chrome abre o host como filho direto do navegador (sem intermediário)")
    confere("pai_dumpable=sim" in linha, "D47: o processo do navegador é dumpable (exe legível pelo /proc)")
    try:
        res = json.load(open(relato, "rb"))
    except (OSError, ValueError):
        res = {}
        falhou("a extensão não mandou o relatório")
    sem_segredo(json.dumps(res).encode(), "relatório da extensão")
    confere(res.get("sonda") == {"ok": True}, "Chrome real: a sonda respondeu pelo native messaging")
    est = res.get("estado", {})
    confere(est.get("dados", {}).get("codigo") == "nao_autorizado",
            "Chrome real: o ward-nm de teste (sem setgid) não recebe o papel extensão (estado: nao_autorizado)")
    for nome in ("preencher", "revelar"):
        confere(res.get(nome, {}).get("dados", {}).get("codigo") == "nao_autorizado",
                f"Chrome real: {nome} pelo ward-nm sem o papel extensão é recusado")
    confere(res.get("grande", {}).get("dados", {}).get("codigo") == "quadro_invalido",
            "Chrome real: mensagem de mais de 64 KiB recusada pelo host")
finally:
    ch.terminate()
    try:
        ch.wait(timeout=20)
    except subprocess.TimeoutExpired:
        ch.kill()
    if srv.poll() is None:
        srv.kill()

# Controle: o mesmo host aberto por este processo (não pelo Chrome).
srv = subprocess.Popen([SONDA, "serve", sock_sonda, CHROME], stdout=subprocess.PIPE, text=True)
srv.stdout.readline()
subprocess.run([SONDA, ORIGEM], env={"WARD_PAI_CHROME_SOCK": sock_sonda}, capture_output=True, timeout=30)
linha = srv.communicate(timeout=30)[0].strip()
confere(linha.startswith("pai=ParentNotAllowed"), "controle: o host aberto por outro processo é recusado (ParentNotAllowed)")
shutil.rmtree(perfil, ignore_errors=True)

if falhas:
    print(f"nm.py: {len(falhas)} conferência(s) falharam", file=sys.stderr)
    sys.exit(1)
