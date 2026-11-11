"""`make test-login` (Phase 43): `ward login` end to end, in the real Chrome.

Called by `dev/tests/login.sh`, which starts a test `wardd` of its own with the
extension variable set to `chrome`. Arguments: root, work dir (in `.dev/`), socket dir,
test `ward-nm`, Chrome, variations list, the built extension and the test `ward` CLI.
Never prints the list or any value of it; compares the submitted values with the fixture
by sha256, without showing them.

Chrome runs headless with `--remote-debugging-pipe` (no TCP port) and the extension is
loaded by `Extensions.loadUnpacked`. Its service worker opens `ward-nm` (native
messaging, manifest only in the temporary profile) and waits for the daemon's request.
Nothing here clicks the icon: `ward login` acts on its own on the active tab. The hostile
pages of the right origin (a fixed path under `http://localhost:8081/`) are served by CDP
itself (`Fetch.fulfillRequest`), which also sees what each submission sent.
"""

import base64
import hashlib
import json
import os
import select
import socket
import struct
import subprocess
import sys
import time
from urllib.parse import parse_qs

ROOT, WORK_DIR, SOCKDIR, NM, CHROME, PATTERN_LIST, EXT, WARD = sys.argv[1:9]
SOCKET_PATH = os.path.join(SOCKDIR, "wardd.sock")
EVIDENCE = os.path.join(ROOT, "docs", "mission", "evidence", "test-login")
HOST = "br.ward.nm"
failures = []
report = {}


def fail(msg):
    print(f"FALHA {msg}", file=sys.stderr)
    failures.append(msg)


def ok(msg):
    print(f"ok    {msg}")


def verify(cond, msg, key=None):
    if key:
        report[key] = bool(cond)
    if cond:
        ok(msg)
    else:
        fail(msg)
    return cond


patterns = [l for l in open(PATTERN_LIST, "rb").read().split(b"\n") if l]
if len(patterns) < 10:
    print("login.py: lista de variações curta demais; rode 'make dev-variations'", file=sys.stderr)
    sys.exit(1)


def has_secret(data):
    if isinstance(data, str):
        data = data.encode()
    return any(p in data for p in patterns)


fixtures = json.load(open(f"{ROOT}/dev/fixtures/entries.json", encoding="utf-8"))
ENTRIES = {e["name"]: e for e in fixtures["entries"]}
PROD_PASSWORD = ENTRIES["acme-wp-prod"]["secrets"]["password"]
HASH_PROD = hashlib.sha256(PROD_PASSWORD.encode()).hexdigest()
ID = None
with open(os.path.join(ROOT, "crates", "ward-nm", "src", "lib.rs"), encoding="utf-8") as f:
    for line in f:
        if line.startswith("pub const EXTENSION_ID"):
            ID = line.split('"')[1]
assert ID and len(ID) == 32


# --- daemon socket -------------------------------------------------------------------
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
                return {}
            d += c
        n = struct.unpack(">I", d)[0]
        d = b""
        while len(d) < n:
            c = s.recv(n - len(d))
            if not c:
                break
            d += c
        return json.loads(d)
    finally:
        s.close()


def unlock():
    password = base64.b64encode(fixtures["master_password"].encode()).decode()
    return req({"v": 2, "op": "unlock", "data": {"password": password}}).get("type") == "ok"


# --- Phase 44c: the test window (the test approver binary) ------------------------------
APPROVER_LOG = os.path.join(WORK_DIR, "logs", "wardd-test-approver.log")
APPROVER_MODE = os.path.join(WORK_DIR, "approver.mode")


def set_window_mode(m):
    """What the test window answers to the next request (confirm, deny…)."""
    with open(APPROVER_MODE, "w") as f:
        f.write(m + "\n")


def window_requests():
    """The request lines the test window saw, in order."""
    try:
        with open(APPROVER_LOG, encoding="utf-8", errors="replace") as f:
            return [l.strip() for l in f if l.startswith("pedido ")]
    except OSError:
        return []


def posts_phish():
    p = subprocess.run([f"{ROOT}/dev/services/compose.sh", "exec", "-T", "phish", "cat", "/registro/posts.log"],
                       capture_output=True, timeout=120)
    if p.returncode == 0:
        return p.stdout
    if b"No such file or directory" in p.stderr:
        return b""
    return None


# --- CDP over the pipe -----------------------------------------------------------------
class Cdp:
    def __init__(self, write_fd, read_fd):
        self.w = write_fd
        self.r = read_fd
        self.n = 0
        self.buf = b""
        self.events = []
        self.on_paused = None
        self.ignored = set()

    def send(self, method, params, session=None):
        self.n += 1
        m = {"id": self.n, "method": method, "params": params}
        if session:
            m["sessionId"] = session
        self.ignored.add(self.n)
        os.write(self.w, json.dumps(m).encode() + b"\0")

    def _read(self, deadline):
        while b"\0" not in self.buf:
            remaining = deadline - time.time()
            if remaining <= 0:
                return None
            ready, _, _ = select.select([self.r], [], [], remaining)
            if not ready:
                return None
            d = os.read(self.r, 1 << 20)
            if not d:
                raise RuntimeError("o Chrome fechou o pipe")
            self.buf += d
        msg, _, self.buf = self.buf.partition(b"\0")
        j = json.loads(msg)
        if j.get("method") == "Fetch.requestPaused" and self.on_paused:
            self.on_paused(j)
            return self._read(deadline)
        if j.get("id") in self.ignored:
            self.ignored.discard(j["id"])
            return self._read(deadline)
        return j

    def cmd(self, method, params=None, session=None, deadline=30):
        self.n += 1
        mine = self.n
        m = {"id": mine, "method": method, "params": params or {}}
        if session:
            m["sessionId"] = session
        os.write(self.w, json.dumps(m).encode() + b"\0")
        end = time.time() + deadline
        while True:
            j = self._read(end)
            if j is None:
                raise TimeoutError(method)
            if j.get("id") == mine:
                if "error" in j:
                    raise RuntimeError(f"{method}: {j['error'].get('message')}")
                return j.get("result", {})
            self.events.append(j)

    def pump(self, seconds=0.2):
        end = time.time() + seconds
        while True:
            j = self._read(end)
            if j is None:
                return
            self.events.append(j)


console = open(os.path.join(WORK_DIR, "console.log"), "w", encoding="utf-8")
cli_log = open(os.path.join(WORK_DIR, "cli.log"), "wb")


def log_console(cdp):
    rest = []
    for e in cdp.events:
        m = e.get("method")
        if m == "Runtime.consoleAPICalled":
            args = e["params"].get("args", [])
            console.write(" ".join(str(a.get("value", a.get("description", ""))) for a in args) + "\n")
        elif m == "Log.entryAdded":
            console.write(str(e["params"]["entry"].get("text", "")) + "\n")
        elif m == "Runtime.exceptionThrown":
            console.write(json.dumps(e["params"].get("exceptionDetails", {}))[:2000] + "\n")
        else:
            rest.append(e)
    cdp.events = rest
    console.flush()


def evaluate(cdp, session, expr, deadline=30):
    r = cdp.cmd("Runtime.evaluate", {"expression": expr, "awaitPromise": True, "returnByValue": True}, session, deadline)
    if "exceptionDetails" in r:
        raise RuntimeError("avaliar: " + json.dumps(r["exceptionDetails"])[:300])
    return r.get("result", {}).get("value")


# --- pages of the right origin served by CDP ------------------------------------------
def form(action="/ward-test/fim.html", method="post", extra_form="", extra_fields="", button='<button type="submit">Acessar</button>'):
    return (
        f'<form id="f" action="{action}" method="{method}" {extra_form}>'
        '<input type="text" name="log" id="user_login" autocomplete="username">'
        '<input type="password" name="pwd" id="user_pass" autocomplete="current-password">'
        f"{extra_fields}{button}</form>"
    )


PAGES = {
    # The site refuses: the POST returns to the same login page (empty fields).
    "recusa.html": "<h1>Login</h1>" + form(action="/ward-test/recusa.html"),
    # A page that does not navigate (the submission is handled by a script and
    # cancelled): after the submission the fields are empty.
    "nao-navega.html": "<h1>Login por script</h1>" + form() + '<script src="/ward-test/nao-navega.js"></script>',
    "nao-navega.js": (
        "window.__envios = 0;"
        "document.getElementById('f').addEventListener('submit', (e) => { window.__envios += 1; e.preventDefault(); });"
    ),
    "fim.html": "<h1>Entrou</h1>",
    # Form attacks on the right origin: nothing may go outside.
    "fora.html": "<h1>Destino fora</h1>" + form(action="http://localhost:8082/wp-login.php"),
    "formaction.html": "<h1>formaction fora</h1>" + form(
        button='<button type="submit" formaction="http://localhost:8082/wp-login.php">Acessar</button>'),
    "get.html": "<h1>GET</h1>" + form(method="get"),
    "alvo.html": "<h1>target</h1>" + form(extra_form='target="_blank"'),
    "duas.html": "<h1>Duas senhas</h1>" + form(extra_fields='<input type="password" name="pwd2" id="pwd2">'),
    # The real `action` points outside, masked by a `name="action"` field that names the
    # right origin.
    "mascarado.html": "<h1>action mascarado</h1>" + form(
        action="http://localhost:8082/wp-login.php",
        extra_fields='<input type="hidden" name="action" value="/ward-test/fim.html">'),
}
submissions = []  # (page, sha256 of the submitted password or None)


def serve_pages(cdp, session):
    cdp.cmd("Fetch.enable", {"patterns": [{"urlPattern": "http://localhost:8081/ward-test/*", "requestStage": "Request"}]}, session)


def handle_pause(e):
    p = e["params"]
    name = p["request"]["url"].split("?")[0].rsplit("/", 1)[-1]
    if p["request"].get("method") == "POST":
        body = p["request"].get("postData", "")
        try:
            pwd = parse_qs(body).get("pwd", [None])[0]
        except ValueError:
            pwd = None
        submissions.append((name, hashlib.sha256(pwd.encode()).hexdigest() if pwd else None))
    body = PAGES.get(name)
    if body is None:
        cdp.send("Fetch.failRequest", {"requestId": p["requestId"], "errorReason": "Failed"}, e.get("sessionId"))
        return
    content_type = "text/javascript" if name.endswith(".js") else "text/html; charset=utf-8"
    if not name.endswith(".js"):
        body = f'<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><title>{name}</title></head><body>{body}</body></html>'
    cdp.send("Fetch.fulfillRequest", {
        "requestId": p["requestId"], "responseCode": 200,
        "responseHeaders": [{"name": "Content-Type", "value": content_type}],
        "body": base64.b64encode(body.encode()).decode()}, e.get("sessionId"))


# --- Chrome ---------------------------------------------------------------------------
profile = os.path.join(WORK_DIR, "perfil")
home = os.path.join(WORK_DIR, "home")
hosts = os.path.join(profile, "NativeMessagingHosts")
os.makedirs(hosts)
os.makedirs(home)
# Without Chrome's own password manager: it would keep the submitted password and fill
# it by itself on the next pages, which would confuse the "nothing filled" checks (see
# the handoff of Phase 43).
os.makedirs(os.path.join(profile, "Default"))
with open(os.path.join(profile, "Default", "Preferences"), "w") as f:
    json.dump({"credentials_enable_service": False, "profile": {"password_manager_enabled": False},
               "autofill": {"enabled": False, "profile_enabled": False}}, f)
with open(os.path.join(hosts, HOST + ".json"), "w") as f:
    json.dump({"name": HOST, "description": "Ward (teste)", "path": NM, "type": "stdio",
               "allowed_origins": [f"chrome-extension://{ID}/"]}, f)

if not unlock():
    fail("não destravou o cofre de teste (cópia)")
    sys.exit(1)
ok("destravou o cofre de teste (cópia) pelo socket")
posts_before = posts_phish()
verify(posts_before is not None, "posts.log do phishing lido antes do teste")

r_cmd, w_cmd = os.pipe()
r_res, w_res = os.pipe()


def prep():
    os.dup2(r_cmd, 3)
    os.dup2(w_res, 4)


chrome_env = {"HOME": home, "PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR, "LANG": "pt_BR.UTF-8"}
log_chrome = open(os.path.join(WORK_DIR, "chrome.log"), "w")
ch = subprocess.Popen(
    [CHROME, "--headless=new", f"--user-data-dir={profile}", "--remote-debugging-pipe",
     "--enable-unsafe-extension-debugging", "--no-first-run", "--no-default-browser-check",
     "--disable-gpu", "--disable-sync", "--disable-background-networking", "--lang=pt-BR",
     "--force-device-scale-factor=1", "--window-size=1280,800", "about:blank"],
    preexec_fn=prep, pass_fds=(3, 4), env=chrome_env, stdout=subprocess.DEVNULL, stderr=log_chrome)
os.close(r_cmd)
os.close(w_res)
cdp = Cdp(w_cmd, r_res)
cdp.on_paused = handle_pause
pages = {}


def open_tab(url):
    t = cdp.cmd("Target.createTarget", {"url": "about:blank"})["targetId"]
    s = cdp.cmd("Target.attachToTarget", {"targetId": t, "flatten": True})["sessionId"]
    cdp.cmd("Page.enable", {}, s)
    cdp.cmd("Runtime.enable", {}, s)
    cdp.cmd("Log.enable", {}, s)
    serve_pages(cdp, s)
    pages[t] = s
    navigate(t, url)
    return t, s


def navigate(t, url):
    s = pages[t]
    cdp.cmd("Page.navigate", {"url": url}, s)
    end = time.time() + 20
    while time.time() < end:
        cdp.pump(0.2)
        try:
            ready = evaluate(cdp, s, "document.readyState === 'complete' && location.href", deadline=5)
        except (RuntimeError, TimeoutError):
            ready = False
        if ready and ready != "about:blank":
            return ready
    raise TimeoutError(url)


def ward_login(entry, t):
    """`ward login <entry>` with tab `t` active; pumps CDP while the CLI runs."""
    cdp.cmd("Target.activateTarget", {"targetId": t})
    env = {"HOME": home, "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8", "WARD_DEV_SOCKET_DIR": SOCKDIR,
           "XDG_RUNTIME_DIR": os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")}
    p = subprocess.Popen([WARD, "login", entry], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, env=env)
    end = time.time() + 120
    while p.poll() is None and time.time() < end:
        cdp.pump(0.2)
    if p.poll() is None:
        p.kill()
    out, err = p.communicate()
    cli_log.write(b"$ ward login " + entry.encode() + b"\n" + out + err)
    cli_log.flush()
    # The next page finishes loading.
    cdp.pump(1.0)
    log_console(cdp)
    return p.returncode, out.decode(errors="replace"), err.decode(errors="replace")


def values(s):
    try:
        return evaluate(cdp, s, "(() => { const u = document.getElementById('user_login'), p = document.getElementById('user_pass');"
                               " return {u: u ? u.value : null, p: p ? p.value : null, url: location.href}; })()", deadline=10)
    except (RuntimeError, TimeoutError):
        return None


def ward_nm_alive():
    """A `ward-nm` child of Chrome (the service worker opened the port)."""
    for pid in os.listdir("/proc"):
        if not pid.isdigit():
            continue
        try:
            with open(f"/proc/{pid}/cmdline", "rb") as f:
                argv = f.read().split(b"\0")
            with open(f"/proc/{pid}/status") as f:
                ppid = next(int(l.split()[1]) for l in f if l.startswith("PPid:"))
        except (OSError, StopIteration):
            continue
        if argv and argv[0] == NM.encode() and ppid == ch.pid:
            return True
    return False


try:
    r = cdp.cmd("Extensions.loadUnpacked", {"path": EXT})
    verify(r.get("id") == ID, f"extensão carregada com o id fixo ({ID})")
    cdp.cmd("Target.setDiscoverTargets", {"discover": True})
    end = time.time() + 20
    while time.time() < end and not ward_nm_alive():
        cdp.pump(0.3)
    verify(ward_nm_alive(), "o service worker abriu o ward-nm (filho direto do Chrome) e espera o pedido", "porta_aberta")

    # --- Phase 44c: the `[::1]` attack from the M6 audit -----------------------------------
    # A process of the uid listens on [::1]:8081 (WordPress is only on 127.0.0.1:8081). The
    # user's Chrome loads the fake one with the origin http://localhost:8081, the entry's.
    # Without the confirmation, nothing goes out; the request shows the origin. It comes
    # before every other visit to localhost:8081: after a connection to 127.0.0.1, Chrome
    # reuses the address and does not even try [::1].
    fake_output = os.path.join(WORK_DIR, "falso-v6.bin")
    fake = subprocess.Popen(["/usr/bin/python3", "-I", "-B", os.path.join(ROOT, "dev", "tests", "fake_v6.py"),
                              "8081", fake_output], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                             stderr=subprocess.DEVNULL)
    try:
        end = time.time() + 10
        while time.time() < end:
            try:
                socket.create_connection(("::1", 8081), 1).close()
                break
            except OSError:
                time.sleep(0.1)
        t, s = open_tab("http://localhost:8081/wp-login.php")
        title = evaluate(cdp, s, "document.title", deadline=10)
        verify(title == "falso-v6", f"ataque [::1]: a aba em localhost:8081 caiu no falso de [::1] (título {title!r})",
                "c44_v6_aba_no_falso")
        # The mode file takes the approver's input; its log prints the protocol v2 decision.
        DECISION = {"negar": "deny"}
        for m in ("negar",):
            set_window_mode(m)
            seen = len(window_requests())
            code, out, err = ward_login("acme-wp-prod", t)
            new_requests = window_requests()[seen:]
            v = values(s)
            verify(code != 0 and v is not None and not v["u"] and not v["p"],
                    f"ataque [::1], janela {m}: `ward login acme-wp-prod` sai ≠ 0 e nada é preenchido (saiu {code};"
                    f" {err.strip()[:80]})", f"c44_v6_{m}_nada")
            verify(new_requests == [f"pedido operacao=login entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao={DECISION[m]}"],
                    f"ataque [::1], janela {m}: o pedido mostra a origem ({new_requests})", f"c44_v6_{m}_origem")
        try:
            received = os.path.getsize(fake_output)
        except FileNotFoundError:
            received = 0
        verify(received == 0, f"ataque [::1]: o falso não recebeu nenhum POST ({received} bytes)", "c44_v6_falso_vazio")
    finally:
        fake.terminate()
        fake.wait(timeout=10)
        set_window_mode("confirmar")

    # --- V6.11 (daemon side): approval level and denied in the window, nothing is filled ----
    set_window_mode("negar")
    t, s = open_tab("http://localhost:8081/wp-login.php")
    seen = len(window_requests())
    code, out, err = ward_login("acme-wp-dev", t)
    v = values(s)
    verify(code != 0 and v is not None and not v["u"] and not v["p"],
            f"V6.11: acme-wp-dev (com aprovação) negado na janela: sai ≠ 0 e nada é preenchido (saiu {code};"
            f" {err.strip()[:100]}; campos {None if v is None else (bool(v['u']), bool(v['p']), v['url'][:60])})",
            "v611_sem_aprovacao")
    # 44b: the real message of a denied `ward login` (goes to the M6 user guide).
    print(f"info  mensagem do `ward login` negado: {err.strip()[:120]!r}")
    report["v611_mensagem_negado"] = err.strip()[:120]
    new_requests = window_requests()[seen:]
    verify(new_requests == ["pedido operacao=login entradas=acme-wp-dev origem=http://localhost:8081 confirmacao=0 decisao=deny"],
            f"Fase 44c: o pedido de acme-wp-dev (com aprovação) mostra a origem http e pede o PIN ({new_requests})",
            "c44_com_aprovacao_origem")
    set_window_mode("confirmar")

    # --- V6.10: real login on WordPress ---------------------------------------------
    t, s = open_tab("http://localhost:8081/wp-login.php")
    seen = len(window_requests())
    code, out, err = ward_login("acme-wp-prod", t)
    verify(code == 0, f"V6.10: `ward login acme-wp-prod` sai 0 (saiu {code}: {err.strip()[:120]})", "v610_sai_0")
    new_requests = window_requests()[seen:]
    verify(new_requests == ["pedido operacao=login entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao=confirm"],
            f"Fase 44c: acme-wp-prod (livre, http) só depois da confirmação da origem, sem PIN ({new_requests})",
            "c44_livre_confirmacao")
    verify(not has_secret(out + err), "V6.10: a saída da CLI não tem padrão de .dev/variations.txt", "v610_saida_sem_segredo")
    end = time.time() + 15
    panel = False
    while time.time() < end and not panel:
        cdp.pump(0.3)
        try:
            panel = bool(evaluate(cdp, s, "location.pathname.startsWith('/wp-admin') && document.readyState === 'complete'"
                                          " && !!document.getElementById('wpadminbar')", deadline=5))
        except (RuntimeError, TimeoutError):
            panel = False
    verify(panel, "V6.10: a aba chega ao painel do WordPress logado (/wp-admin/ com a barra de admin)", "v610_painel")
    os.makedirs(EVIDENCE, exist_ok=True)
    try:
        png = cdp.cmd("Page.captureScreenshot", {"format": "png"}, s)["data"]
        with open(os.path.join(EVIDENCE, "v6-10-painel-wordpress.png"), "wb") as f:
            f.write(base64.b64decode(png))
    except (RuntimeError, TimeoutError):
        pass

    # --- V6.10: the site refuses; the fields are left empty -------------------------------
    n_submissions = len(submissions)
    t, s = open_tab("http://localhost:8081/ward-test/recusa.html")
    code, out, err = ward_login("acme-wp-prod", t)
    sent = [h for (n, h) in submissions[n_submissions:] if n == "recusa.html"]
    verify(sent == [HASH_PROD], "V6.10: o formulário foi enviado uma vez, à origem certa, com a senha da entrada", "v610_recusa_enviado")
    verify(code != 0 and "não aceitou o login" in err,
            f"V6.10: site que recusa o login: a CLI sai ≠ 0 dizendo isso (saiu {code})", "v610_recusa_sai")
    v = values(s)
    verify(v is not None and v["u"] == "" and v["p"] == "", "V6.10: depois do envio recusado os campos estão vazios", "v610_recusa_vazios")

    t, s = open_tab("http://localhost:8081/ward-test/nao-navega.html")
    code, out, err = ward_login("acme-wp-prod", t)
    v = values(s)
    script_submissions = evaluate(cdp, s, "window.__envios", deadline=10)
    verify(script_submissions == 1, "página que trata o envio por script: o envio aconteceu (submit)", "nao_navega_enviou")
    verify(v is not None and v["u"] == "" and v["p"] == "",
            "página que não navega: depois do envio os campos ficam vazios (no mesmo documento)", "nao_navega_vazios")
    verify(code != 0, f"página que ainda pede a senha: a CLI sai ≠ 0 (saiu {code})", "nao_navega_sai")

    # --- V6.3: tab on the phishing site ---------------------------------------------------
    t, s = open_tab("http://localhost:8082/wp-login.php")
    code, out, err = ward_login("acme-wp-prod", t)
    verify(code != 0 and "domínio não confere" in err,
            f"V6.3: aba no phishing (8082): sai ≠ 0 com \"domínio não confere\" (saiu {code})", "v63_dominio")
    v = values(s)
    verify(v is not None and not v["u"] and not v["p"], "V6.3: nada preenchido na página do phishing", "v63_nada_preenchido")

    # --- V6.15: delta-redir (8084 → 8082) ---------------------------------------------------
    t, s = open_tab("http://localhost:8084/login")
    url_final = evaluate(cdp, s, "location.origin", deadline=10)
    verify(url_final == "http://localhost:8082", "V6.15: a aba do 8084 terminou no phishing (8082)")
    code, out, err = ward_login("delta-redir", t)
    verify(code != 0 and "domínio não confere" in err,
            f"V6.15: `ward login delta-redir` recusa por origem (saiu {code})", "v615_dominio")
    v = values(s)
    verify(v is not None and not v["u"] and not v["p"], "V6.15: nada preenchido", "v615_nada_preenchido")

    # --- Form attacks on the right origin --------------------------------------------------
    for page, label in [
        ("fora.html", "action para o phishing"),
        ("formaction.html", "formaction do botão para o phishing"),
        ("get.html", "formulário GET"),
        ("alvo.html", "target=_blank"),
        ("duas.html", "dois campos de senha"),
        ("mascarado.html", "action para fora tapado por <input name=action>"),
    ]:
        n_submissions = len(submissions)
        t, s = open_tab(f"http://localhost:8081/ward-test/{page}")
        code, out, err = ward_login("acme-wp-prod", t)
        v = values(s)
        verify(code != 0 and "formulário de login seguro" in err,
                f"ataque {label}: a CLI sai ≠ 0 (formulário recusado; saiu {code})", f"ataque_{page}")
        verify(v is not None and not v["u"] and not v["p"] and len(submissions) == n_submissions
                and v["url"].endswith(page),
                f"ataque {label}: nada preenchido, nada enviado, a aba não saiu da página", f"ataque_{page}_nada")

    # --- Blocked entry and wrong type -------------------------------------------------------
    code, out, err = ward_login("gama-bloqueada", t)
    verify(code != 0 and "entrada inexistente" in err, "gama-bloqueada: \"entrada inexistente\" (V3.17)", "bloqueada")
    code, out, err = ward_login("acme-pg-dev", t)
    verify(code != 0, "acme-pg-dev (banco): recusado", "tipo_errado")

    # --- The phishing site received nothing ---------------------------------------------------
    posts_after = posts_phish()
    verify(posts_before is not None and posts_after == posts_before, "posts.log do phishing (8082) sem mudança", "posts_log")

    # --- Nothing of the extension kept in the profile -------------------------------------------
    log_console(cdp)
    found = []
    for dir_path, _, names in os.walk(profile):
        for n in names:
            path = os.path.join(dir_path, n)
            rel = os.path.relpath(path, profile)
            if ID not in rel and "Service Worker" not in rel and "Extension" not in rel:
                continue
            try:
                with open(path, "rb") as f:
                    if has_secret(f.read()):
                        found.append(rel)
            except OSError:
                pass
    verify(not found, f"perfil: nada da extensão com padrão de .dev/variations.txt ({len(found)} arquivo(s))", "perfil_limpo")
finally:
    try:
        cdp.cmd("Browser.close", deadline=5)
    except Exception:
        pass
    try:
        ch.wait(timeout=10)
    except subprocess.TimeoutExpired:
        ch.kill()
    console.close()
    cli_log.close()
    os.makedirs(EVIDENCE, exist_ok=True)
    with open(os.path.join(EVIDENCE, "qa.json"), "w", encoding="utf-8") as f:
        json.dump(report, f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write("\n")

if failures:
    print(f"login.py: {len(failures)} falha(s)", file=sys.stderr)
    sys.exit(1)
print("login.py: tudo certo")
