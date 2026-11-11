"""`make test-extension` (Phase 41): the extension in the real Chrome, through CDP on the pipe.

Called by `dev/tests/extension.sh`, which starts a test `wardd` of its own with the
test-mode Chrome extension exception on. Arguments: root, work dir (in `.dev/`), socket
dir, test `ward-nm`, the fake Chrome, Chrome, variations list and the built extension. Never prints
the list nor a value of it; compares the filled values with the fixture without showing
them.

Chrome runs headless with `--remote-debugging-pipe` (no TCP port) and
`--enable-unsafe-extension-debugging`: the extension comes in through
`Extensions.loadUnpacked` (`--load-extension` does not work in branded Chrome 137+), and
the click on the icon is `Extensions.triggerAction`, which grants `activeTab` like the
real click. The hostile pages of the right origin (a test path under `http://localhost:8081/`)
are served by CDP itself (`Fetch.fulfillRequest`): for Chrome, they are documents of
that origin.
"""

import base64
import codecs
import json
import re
import secrets
import os
import shutil
import socket
import struct
import subprocess
import sys
import time

ROOT, WORK_DIR, SOCKDIR, NM, FAKE_CHROME, CHROME, PATTERN_LIST, EXT, WARD = sys.argv[1:10]
SOCKET_PATH = os.path.join(SOCKDIR, "wardd.sock")
EVIDENCE = os.path.join(ROOT, "docs", "mission", "evidence", "test-extension")
EVIDENCE_POPUP = os.path.join(ROOT, "docs", "mission", "evidence", "test-extension", "popup")
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
    print("extension.py: lista de variações curta demais; rode 'make dev-variations'", file=sys.stderr)
    sys.exit(1)


def has_secret(data):
    if isinstance(data, str):
        data = data.encode()
    return any(p in data for p in patterns)


# --- Dictionary of common passwords (zxcvbn) in the profile (Phase 44a) ---------------
# The test WordPress (8081) serves `wp-includes/js/zxcvbn.min.js`, a password strength
# meter with a list of common passwords in rot13; Chrome keeps its compiled code in the
# profile's `Code Cache/js/`. Chrome can also download `ZxcvbnData/<version>/` (the same
# idea, for the password manager). A pattern of `.dev/variations.txt` that is also a
# common password (the test PIN, digits only) shows up there without having left the
# vault. A finding only counts as "zxcvbn dictionary" if (1) the file is from
# `ZxcvbnData/` or `Code Cache/` and, in `Code Cache/`, it is the one of `zxcvbn.min.js`
# (the URL is in the file); (2) every pattern found in it is a dictionary word, taken
# from the `zxcvbn.min.js` of the WordPress container itself and from the profile's
# `ZxcvbnData/`; (3) in `Code Cache/`, each occurrence sits among dictionary words: the
# text around it is the list, not a loose value. Any other finding stays in the report
# key of the profile findings outside the extension.
ZXCVBN_WP = "/var/www/html/wp-includes/js/zxcvbn.min.js"


def words(data):
    return [t for t in re.split(rb"[^0-9A-Za-z]+", data) if t]


def rot13(data):
    return codecs.encode(data.decode("latin-1"), "rot13").encode("latin-1")


def zxcvbn_dictionary(root):
    """Words of the test WordPress `zxcvbn.min.js` and of the profile's `ZxcvbnData/`,
    in clear and in rot13."""
    sources = []
    p = subprocess.run([f"{ROOT}/dev/services/compose.sh", "exec", "-T", "wordpress", "cat", ZXCVBN_WP],
                       capture_output=True, timeout=120)
    if p.returncode == 0:
        sources.append(p.stdout)
    for r, _, ns in os.walk(os.path.join(root, "ZxcvbnData")):
        for n in ns:
            try:
                sources.append(open(os.path.join(r, n), "rb").read())
            except OSError:
                pass
    d = set()
    for data in sources:
        for t in words(data):
            d.add(t)
            d.add(rot13(t))
    return d


def dictionary_finding(rel, data, dic):
    """(yes/no, indexes of the patterns found) for a profile file with a pattern."""
    found = [i for i, p in enumerate(patterns) if p in data]
    if not dic or not found:
        return False, found
    if not (rel.startswith("ZxcvbnData" + os.sep)
            or (("Code Cache" + os.sep) in rel and b"zxcvbn.min.js" in data)):
        return False, found
    if any(patterns[i] not in dic for i in found):
        return False, found
    if rel.startswith("ZxcvbnData" + os.sep):
        return True, found
    for i in found:
        p = patterns[i]
        j = data.find(p)
        while j >= 0:
            window = words(data[max(0, j - 60): j + len(p) + 60])
            middle = window[1:-1]  # the ends may be cut
            if len(middle) < 5 or any(t not in dic for t in middle):
                return False, found
            j = data.find(p, j + 1)
    return True, found


fixtures = json.load(open(f"{ROOT}/dev/fixtures/entries.json", encoding="utf-8"))
ENTRIES = {e["name"]: e for e in fixtures["entries"]}
ID = None
with open(os.path.join(ROOT, "crates", "ward-nm", "src", "lib.rs"), encoding="utf-8") as f:
    for line in f:
        if line.startswith("pub const EXTENSION_ID"):
            ID = line.split('"')[1]
assert ID and len(ID) == 32


# --- Phase 44c: the test approval window (the test approver binary) -------------------
APPROVER_LOG = os.path.join(WORK_DIR, "logs", "wardd-aprovador-teste.log")
APPROVER_MODE = os.path.join(WORK_DIR, "aprovador.modo")


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


# --- daemon socket --------------------------------------------------------------------
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


def posts_phish():
    p = subprocess.run([f"{ROOT}/dev/services/compose.sh", "exec", "-T", "phish", "cat", "/registro/posts.log"],
                       capture_output=True, timeout=120)
    if p.returncode == 0:
        return p.stdout
    # Freshly started service (clean tmpfs): the phishing site has not received any POST yet.
    if b"No such file or directory" in p.stderr:
        return b""
    return None


# --- CDP on the pipe ------------------------------------------------------------------
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
        """Command without waiting for the answer (to serve Fetch in the middle of another)."""
        self.n += 1
        m = {"id": self.n, "method": method, "params": params}
        if session:
            m["sessionId"] = session
        self.ignored.add(self.n)
        os.write(self.w, json.dumps(m).encode() + b"\0")

    def _read(self, deadline):
        import select
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

    def wait_for(self, cond, deadline=20):
        end = time.time() + deadline
        while True:
            for i, e in enumerate(self.events):
                if cond(e):
                    return self.events.pop(i)
            j = self._read(end)
            if j is None:
                return None
            self.events.append(j)

    def pump(self, seconds=0.2):
        end = time.time() + seconds
        while True:
            j = self._read(end)
            if j is None:
                return
            self.events.append(j)


console = open(os.path.join(WORK_DIR, "console.log"), "w", encoding="utf-8")


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


def evaluate(cdp, session, expr, deadline=30, context=None):
    p = {"expression": expr, "awaitPromise": True, "returnByValue": True}
    if context is not None:
        p["contextId"] = context
    r = cdp.cmd("Runtime.evaluate", p, session, deadline)
    if "exceptionDetails" in r:
        raise RuntimeError("avaliar: " + json.dumps(r["exceptionDetails"])[:300])
    return r.get("result", {}).get("value")


# --- hostile pages of the right origin (served by CDP) --------------------------------
FORM = (
    '<form id="f" action="/wp-login.php" method="post">'
    '<input type="text" name="log" id="user_login" autocomplete="username">'
    '<input type="password" name="pwd" id="user_pass" autocomplete="current-password">'
    '<button type="submit">Acessar</button></form>'
)
PAGES = {
    # V6.4: phishing iframe inside the right origin.
    "iframe.html": "<h1>Origem certa com iframe</h1>" + FORM
    + '<iframe id="isca" src="http://localhost:8082/wp-login.php" width="400" height="400"></iframe>',
    # V6.4: only the phishing iframe has a form; the main document, none.
    "iframe-sem-campo.html": "<h1>Origem certa sem formulário</h1>"
    + '<iframe id="isca" src="http://localhost:8082/wp-login.php" width="400" height="400"></iframe>',
    # DOM clobbering: names that shadow the methods of the document and of the form, plus
    # the page world's prototypes swapped (the extension uses the isolated world's).
    "clobbering.html": (
        '<form name="querySelectorAll"></form><img name="documentElement"><img name="elementFromPoint">'
        '<form id="f" name="getElementsByTagName">'
        '<input name="querySelectorAll"><input name="elements"><input name="action">'
        '<input type="text" name="log" id="user_login" autocomplete="username">'
        '<input type="password" name="pwd" id="user_pass">'
        '<input type="hidden" name="form" value="x"></form>'
        '<div style="position:absolute;left:-10000px" aria-hidden="true">'
        '<input type="password" id="isca_pw"></div>'
        '<script src="/ward-teste/clobbering.js"></script>'
    ),
    "clobbering.js": (
        "window.__chamadas = [];"
        "const d = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');"
        "Object.defineProperty(HTMLInputElement.prototype, 'value', {get(){window.__chamadas.push('get');return d.get.call(this);},"
        "set(v){window.__chamadas.push('set');d.set.call(this, v);}});"
        "const q = Document.prototype.querySelectorAll;"
        "Document.prototype.querySelectorAll = function(s){window.__chamadas.push('qsa');"
        "return q.call(this, s === 'input' ? '#isca_pw' : s);};"
        "Element.prototype.checkVisibility = function(){window.__chamadas.push('vis');return true;};"
        "window.atob = function(){window.__chamadas.push('atob');return '';};"
    ),
    # Hidden password field: the decoys come before the real field.
    "escondido.html": (
        '<form id="f">'
        '<input type="text" id="isca_user" style="display:none">'
        '<input type="password" id="isca_none" style="display:none">'
        '<input type="password" id="isca_vis" style="visibility:hidden">'
        '<input type="password" id="isca_opac" style="opacity:0">'
        '<div style="opacity:0"><input type="password" id="isca_pai"></div>'
        '<input type="password" id="isca_1px" style="width:1px;height:1px;padding:0;border:0">'
        '<input type="password" id="isca_fora" style="position:absolute;left:-9999px;top:-9999px">'
        '<input type="password" id="isca_clip" style="position:absolute;clip-path:inset(50%)">'
        '<div style="position:relative;height:40px"><input type="password" id="isca_coberta" style="position:absolute;top:0;left:0">'
        '<div style="position:absolute;top:0;left:0;width:300px;height:40px;background:#fff"></div></div>'
        '<input type="password" id="isca_desab" disabled>'
        '<input type="text" name="log" id="user_login" autocomplete="username">'
        '<input type="password" name="pwd" id="user_pass">'
        '<button type="submit">Acessar</button></form>'
    ),
    # Only hidden fields: nothing is filled.
    "so-escondido.html": (
        '<form><input type="text" id="isca_user" style="opacity:0">'
        '<input type="password" id="isca_pw" style="display:none"></form>'
    ),
    # Page that tries to talk to the extension and to read the popup.
    "mensagens.html": "<h1>Mensagens</h1>" + FORM + '<script src="/ward-teste/mensagens.js"></script>',
    "mensagens.js": (
        "window.__res = {};"
        f"const ID = '{ID}';"
        "window.__res.runtime = typeof chrome === 'undefined' || !chrome.runtime ? 'sem chrome.runtime' : typeof chrome.runtime.sendMessage;"
        "try { chrome.runtime.sendMessage(ID, {v:2, op:'fill', data:{entry:'acme-wp-prod', origin: location.origin}},"
        " (r) => { window.__res.resposta = r === undefined ? 'nada' : 'algo'; }); window.__res.envio = 'chamou'; }"
        " catch (e) { window.__res.envio = 'erro'; }"
        "window.postMessage({v:2, op:'fill', data:{entry:'acme-wp-prod'}}, '*');"
        "window.postMessage({type:'ward', entry:'acme-wp-prod'}, '*');"
        "fetch('chrome-extension://' + ID + '/popup.html').then(() => { window.__res.fetch = 'leu'; }, () => { window.__res.fetch = 'bloqueado'; });"
        "fetch('chrome-extension://' + ID + '/manifest.json').then(() => { window.__res.fetch_manifesto = 'leu'; }, () => { window.__res.fetch_manifesto = 'bloqueado'; });"
        "const f = document.createElement('iframe'); f.src = 'chrome-extension://' + ID + '/popup.html';"
        "f.onload = () => { try { window.__res.iframe = f.contentDocument ? 'leu' : 'sem documento'; } catch (e) { window.__res.iframe = 'bloqueado'; } };"
        "document.body.append(f);"
    ),
}


# Phase 42: submit forged by the page's script (no user activation) and a form only in an
# iframe of another origin.
PAGES.update({
    "envio-forjado.html": (
        '<form id="f" action="/ward-teste/fim.html" method="get">'
        '<input type="text" name="u" id="u" autocomplete="username">'
        '<input type="password" name="p" id="p"><button type="submit">Entrar</button></form>'
        '<script src="/ward-teste/envio-forjado.js"></script>'
    ),
    "envio-forjado.js": (
        "document.getElementById('u').value = 'forjado.usuario';"
        "document.getElementById('p').value = 'forjado-nao-segredo-01';"
        "window.__enviou = 0;"
        "setTimeout(() => { window.__enviou = 1; document.getElementById('f').requestSubmit(); }, 300);"
    ),
    "fim.html": "<h1>Fim</h1>",
    "iframe-cadastro.html": (
        "<h1>Cadastro só no iframe</h1>"
        '<iframe id="cad" src="http://localhost:8083/cadastro" width="600" height="500" style="position:absolute;left:0;top:80px;border:0"></iframe>'
    ),
})


def serve_pages(cdp, session):
    cdp.cmd("Fetch.enable", {"patterns": [{"urlPattern": "http://localhost:8081/ward-teste/*", "requestStage": "Request"}]}, session)


def handle_pause(e):
    p = e["params"]
    name = p["request"]["url"].rsplit("/", 1)[-1]
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


def handle_fetch(cdp):
    pass


# --- Chrome ---------------------------------------------------------------------------
profile = os.path.join(WORK_DIR, "perfil")
home = os.path.join(WORK_DIR, "home")
hosts = os.path.join(profile, "NativeMessagingHosts")
os.makedirs(hosts)
os.makedirs(home)
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
     "--force-device-scale-factor=1", "--window-size=1280,800",
     # The pages served by CDP have no IP (for Chrome, they are "public") and Local
     # Network Access would block the phishing iframe on localhost:8082 (V6.4). It is not
     # what protects the fill: the test wants the iframe loaded.
     "--disable-features=LocalNetworkAccessChecks", "about:blank"],
    preexec_fn=prep, pass_fds=(3, 4), env=chrome_env, stdout=subprocess.DEVNULL, stderr=log_chrome)
os.close(r_cmd)
os.close(w_res)
cdp = Cdp(w_cmd, r_res)
cdp.on_paused = handle_pause

GEOMETRY = """(() => {
  const r = (s) => { const e = document.querySelector(s); if (!e) return null;
    const b = e.getBoundingClientRect(); return [Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)]; };
  const todos = (s) => [...document.querySelectorAll(s)].map((e) => { const b = e.getBoundingClientRect();
    return [Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)]; });
  const cor = (s, p) => { const e = document.querySelector(s); return e ? getComputedStyle(e)[p] : null; };
  return { popup: r('#popup'), cabecalho: r('.header'), simbolo: r('.header .symbol'), estado: r('.status'),
    origem: r('.origin'), rotulo: r('.body > p.secondary'), itens: todos('.item'), avatares: todos('.item .avatar'),
    badges: todos('.item .badge-environment'), tipos: todos('.item .badge-type'), nota: r('.body > p.note:last-child'),
    rodape: r('.footer'), botao: r('.footer .button'),
    travado: { avatar: r('.avatar-large'), titulo: r('.locked-title'), texto: r('.locked-text'),
      botao: r('.locked-body .button'), nota: r('.locked-body .note') },
    fonte: document.fonts.check('500 14px Inter') && document.fonts.check('13px "JetBrains Mono"'),
    cores: { fundo: cor('body', 'backgroundColor'), item0: cor('.item', 'backgroundColor'),
      estado: cor('.status', 'color'), nome: cor('.item-name', 'color'), detalhe: cor('.item-detail', 'color'),
      stage: cor('.badge-environment', 'color') },
    textos: [...document.querySelectorAll('.item')].map((b) => ({ nome: b.querySelector('.item-name').textContent,
      amb: b.querySelector('.badge-environment').textContent, det: b.querySelector('.item-detail').textContent })),
    origemTexto: (document.querySelector('.origin-value') || {}).textContent || null,
    rotuloTexto: (document.querySelector('.body > p.secondary') || {}).textContent || null,
    notaTexto: (document.querySelector('.note') || {}).textContent || null,
    estadoTexto: (document.querySelector('.status') || {}).textContent || null,
    html: document.documentElement.outerHTML };
})()"""

pages = {}  # targetId -> sessionId
seen_popups = set()
tabs = {}  # page targetId -> targetId of the "tab" target (the triggerAction one)


def tab_targets():
    return {a["targetId"] for a in cdp.cmd("Target.getTargets", {"filter": [{"type": "tab"}]})["targetInfos"]}


def open_tab(url):
    before = tab_targets()
    t = cdp.cmd("Target.createTarget", {"url": "about:blank"})["targetId"]
    new = tab_targets() - before
    if len(new) != 1:
        raise RuntimeError("não achei o alvo tab da aba nova")
    tabs[t] = new.pop()
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
        handle_fetch(cdp)
        try:
            ready = evaluate(cdp, s, "document.readyState === 'complete' && location.href", deadline=5)
        except (RuntimeError, TimeoutError):
            ready = False
        if ready and ready != "about:blank":
            return ready
    raise TimeoutError(url)


def activate(t):
    cdp.cmd("Target.activateTarget", {"targetId": t})


def open_popup(t, wait_selector=".item, .locked-body, .note"):
    """The click on the icon (`Extensions.triggerAction`): opens the popup and grants activeTab."""
    activate(t)
    cdp.cmd("Extensions.triggerAction", {"id": ID, "targetId": tabs[t]})
    popup_url = f"chrome-extension://{ID}/popup.html"
    pt = None
    end = time.time() + 20
    while pt is None and time.time() < end:
        for a in cdp.cmd("Target.getTargets")["targetInfos"]:
            if a["url"].startswith(popup_url) and a["targetId"] not in seen_popups:
                pt = a["targetId"]
                break
        else:
            cdp.pump(0.2)
    if pt is None:
        raise TimeoutError("o popup não abriu")
    seen_popups.add(pt)
    ps = cdp.cmd("Target.attachToTarget", {"targetId": pt, "flatten": True})["sessionId"]
    cdp.cmd("Runtime.enable", {}, ps)
    cdp.cmd("Log.enable", {}, ps)
    cdp.cmd("Page.enable", {}, ps)
    end = time.time() + 20
    while time.time() < end:
        try:
            if evaluate(cdp, ps, f"document.fonts.status === 'loaded' && !!document.querySelector({json.dumps(wait_selector)})", deadline=5):
                break
        except (RuntimeError, TimeoutError):
            pass
        cdp.pump(0.1)
    return pt, ps


def closed(pt, deadline=10):
    end = time.time() + deadline
    while time.time() < end:
        targets = cdp.cmd("Target.getTargets")["targetInfos"]
        if not any(a["targetId"] == pt for a in targets):
            return True
        cdp.pump(0.2)
    return False


def close_popup(pt):
    try:
        cdp.cmd("Target.closeTarget", {"targetId": pt})
    except RuntimeError:
        pass


def screenshot(ps, name, height, dir_=EVIDENCE, beyond=True, y=0):
    os.makedirs(dir_, exist_ok=True)
    img = cdp.cmd("Page.captureScreenshot", {"format": "png", "clip": {"x": 0, "y": y, "width": 360, "height": height, "scale": 1},
                                             "captureBeyondViewport": beyond}, ps)["data"]
    with open(os.path.join(dir_, name), "wb") as f:
        f.write(base64.b64decode(img))
    ok(f"screenshot {name}")


def values(s, ids, context=None):
    js = "(() => { const v = {}; for (const id of %s) { const e = document.getElementById(id); v[id] = e ? e.value : null; } return v; })()" % json.dumps(ids)
    return evaluate(cdp, s, js, context=context)


def click_entry(ps, name):
    evaluate(cdp, ps, f"document.querySelector('[data-entry=\"{name}\"]').click(), true")


PASSWORD = {n: e["secrets"]["password"] for n, e in ENTRIES.items() if e["type"] == "login"}
USERNAME = {n: e["fields"]["username"] for n, e in ENTRIES.items() if e["type"] == "login"}


def wait_for_value(s, field, deadline=10):
    end = time.time() + deadline
    while time.time() < end:
        v = values(s, [field]).get(field)
        if v:
            return v
        cdp.pump(0.2)
    return ""


def forced_fill(ps, tab_origin, requested_origin, entry):
    """From the popup's context: a fill straight to the `ward-nm` (like a content script
    that forced the message). Returns only the type/code, never values."""
    js = """(async () => { const r = await chrome.runtime.sendNativeMessage(%s, {v: 2, op: 'fill',
      data: {entry: %s, origin: %s}}); return r ? (r.type === 'error' ? 'erro:' + r.data.code : r.type) : 'nada'; })()""" % (
        json.dumps(HOST), json.dumps(entry), json.dumps(requested_origin))
    return evaluate(cdp, ps, js)


# --- Phase 42 (V6.8, V6.9) ------------------------------------------------------------
URL_POPUP = f"chrome-extension://{ID}/popup.html"


def theme_stays_dark(ps):
    """V6.9 (D1): with `prefers-color-scheme: light` (the light theme of the system and of
    Chrome), the popup keeps the dark colors and changes nothing."""
    before = evaluate(cdp, ps, "[getComputedStyle(document.body).backgroundColor, getComputedStyle(document.body).color,"
                      " getComputedStyle(document.documentElement).colorScheme]")
    cdp.cmd("Emulation.setEmulatedMedia", {"features": [{"name": "prefers-color-scheme", "value": "light"}]}, ps)
    cdp.pump(0.3)
    light = evaluate(cdp, ps, "[matchMedia('(prefers-color-scheme: light)').matches, getComputedStyle(document.body).backgroundColor,"
                     " getComputedStyle(document.body).color, getComputedStyle(document.documentElement).colorScheme]")
    return light[0] is True and light[1:] == before and before[0] == "rgb(16, 17, 20)" and before[1] == "rgb(247, 248, 250)"


def unstick(ps, on):
    """Header and footer without `sticky` (through the CSSOM, which the CSP allows): the
    layout becomes the one of frame 18850, with the whole content and no scrolling."""
    v = "static" if on else ""
    evaluate(cdp, ps, f"document.querySelector('.header').style.position = '{v}';"
             f" document.querySelector('.save-footer').style.position = '{v}'; true")
    cdp.pump(0.2)


def new_popup(deadline=10):
    """An extension popup not seen yet (opened by the service worker's
    `chrome.action.openPopup`, without a click on the icon)."""
    end = time.time() + deadline
    while time.time() < end:
        for a in cdp.cmd("Target.getTargets")["targetInfos"]:
            if a["url"].startswith(URL_POPUP) and a["targetId"] not in seen_popups:
                return a["targetId"]
        cdp.pump(0.2)
    return None


def attach_popup(pt, wait_selector):
    seen_popups.add(pt)
    ps = cdp.cmd("Target.attachToTarget", {"targetId": pt, "flatten": True})["sessionId"]
    cdp.cmd("Runtime.enable", {}, ps)
    cdp.cmd("Log.enable", {}, ps)
    cdp.cmd("Page.enable", {}, ps)
    end = time.time() + 20
    while time.time() < end:
        try:
            if evaluate(cdp, ps, f"document.fonts.status === 'loaded' && !!document.querySelector({json.dumps(wait_selector)})", deadline=5):
                break
        except (RuntimeError, TimeoutError):
            pass
        cdp.pump(0.1)
    return ps


def type_text(s, field, value):
    """Focus on the field and text through `Input.insertText` (user input, trusted)."""
    evaluate(cdp, s, f"document.getElementById({json.dumps(field)}).focus(), true")
    cdp.cmd("Input.insertText", {"text": value}, s)


def click(s, selector, context=None):
    """Real mouse click (`Input.dispatchMouseEvent`) at the center of the element: gives
    the page user activation."""
    r = evaluate(cdp, s, f"(() => {{ const b = document.querySelector({json.dumps(selector)}).getBoundingClientRect();"
                 " return [b.x + b.width / 2, b.y + b.height / 2]; })()", context=context)
    for kind in ("mousePressed", "mouseReleased"):
        cdp.cmd("Input.dispatchMouseEvent", {"type": kind, "x": r[0], "y": r[1], "button": "left", "clickCount": 1}, s)


def wait_for_load(s, cond, deadline=15):
    end = time.time() + deadline
    while time.time() < end:
        cdp.pump(0.3)
        try:
            if evaluate(cdp, s, cond, deadline=5):
                return True
        except (RuntimeError, TimeoutError):
            pass
    return False


def pending_from_popup(ps):
    """What the service worker gives the popup about the pending login of the active tab."""
    return evaluate(cdp, ps, """(async () => { const [a] = await chrome.tabs.query({active: true, currentWindow: true});
        try { const r = await chrome.runtime.sendMessage({t: 'pending', tab: a.id}); return r === undefined ? null : r; }
        catch (e) { return 'erro'; } })()""")


def content_script_context(s):
    """The extension's isolated world in the tab's main frame (where `submit.js` runs)."""
    cdp.pump(0.3)
    main_frame = cdp.cmd("Page.getFrameTree", {}, s)["frameTree"]["frame"]["id"]
    found = None
    for e in cdp.events:
        if e.get("method") == "Runtime.executionContextCreated" and e.get("sessionId") == s:
            c = e["params"]["context"]
            aux = c.get("auxData", {})
            if c.get("origin", "").startswith(f"chrome-extension://{ID}") and aux.get("frameId") == main_frame \
                    and aux.get("type") == "isolated":
                found = c["id"]
    return found


SAVE_GEOMETRY = """(() => {
  const r = (e) => { if (!e) return null; const b = e.getBoundingClientRect();
    return [Math.round(b.x), Math.round(b.y + window.scrollY), Math.round(b.width), Math.round(b.height)]; };
  const q = (s) => document.querySelector(s);
  const campo = (id) => document.getElementById(id) ? document.getElementById(id).closest('.field') : null;
  const cor = (e, p) => e ? getComputedStyle(e)[p] : null;
  return { vh: window.innerHeight, cabecalho: r(q('.header')), introducao: r(q('.intro')),
    titulo: r(q('.save-title')), nome: r(campo('entry-name')), usuario: r(campo('entry-username')),
    senha: r(q('.field-input-mask').closest('.field')), entradaNome: r(q('#entry-name')), entradaSenha: r(q('.field-input-mask')),
    linha: r(q('.double-row')), tipo: r(campo('entry-type')), ambiente: r(campo('entry-environment')), grupo: r(campo('entry-group')),
    nivel: r(q('.initial-level')), rodape: r(q('.save-footer')), botoes: [...document.querySelectorAll('.save-footer .button')].map(r),
    textos: { titulo: q('.save-title').textContent, intro: q('.save-text').textContent,
      nome: q('#entry-name').value, usuario: q('#entry-username').value, grupo: q('#entry-group').value, ambiente: q('#entry-environment').value,
      tipo: q('#entry-type').value, tipos: [...q('#entry-type').options].map((o) => o.value), mascara: q('.mask').textContent,
      nivel: q('.level-title').textContent, botoes: [...document.querySelectorAll('.save-footer .button')].map((b) => b.textContent) },
    cores: { fundo: cor(document.body, 'backgroundColor'), entrada: cor(q('#entry-name'), 'backgroundColor'),
      borda: cor(q('#entry-name'), 'borderTopColor'), nivel: cor(q('.initial-level'), 'backgroundColor'),
      primario: cor(q('.button.primary'), 'backgroundColor'), intro: cor(q('.save-text'), 'color') },
    fonte: document.fonts.check('600 18px Inter') && document.fonts.check('500 13px Inter'),
    html: document.documentElement.outerHTML };
})()"""


def phase_42_section(t, s):
    username = "novo." + secrets.token_hex(4)
    # Fake password generated on the spot (never written in the repository nor printed).
    new_password = "falso-" + secrets.token_urlsafe(18)
    new_passwords.append(new_password)

    # 1. Submit forged by the page's script (requestSubmit without activation): nothing.
    navigate(t, "http://localhost:8081/ward-teste/envio-forjado.html")
    wait_for_load(s, "location.pathname === '/ward-teste/fim.html'")
    verify(new_popup(3) is None, "ataque: requestSubmit do script da página (sem ativação do usuário) não oferece salvar", "f42_forjado_sem_popup")
    pt, ps = open_popup(t)
    verify(pending_from_popup(ps) is None and evaluate(cdp, ps, "!document.querySelector('.popup-save')"),
           "ataque: nada pendente depois do envio forjado", "f42_forjado_sem_pendente")
    close_popup(pt)

    # 2. Login the vault already has (the TOTP dev entry), submitted by the user: nothing
    # to save.
    navigate(t, "http://localhost:8083/login")
    activate(t)
    type_text(s, "usuario", USERNAME["acme-totp-dev"])
    type_text(s, "senha", PASSWORD["acme-totp-dev"])
    click(s, "#login button")
    wait_for_load(s, "document.readyState === 'complete' && location.pathname === '/login' && !!document.getElementById('mensagem')")
    verify(new_popup(3) is None, "login já cadastrado (acme-totp-dev): enviar não oferece salvar", "f42_conhecido_sem_popup")

    # 3. Form only in an iframe of another origin: the content script does not run in it.
    navigate(t, "http://localhost:8081/ward-teste/iframe-cadastro.html")
    cdp.pump(1.0)
    children = cdp.cmd("Page.getFrameTree", {}, s)["frameTree"].get("childFrames", [])
    if children:
        ctx = cdp.cmd("Page.createIsolatedWorld", {"frameId": children[0]["frame"]["id"], "worldName": "conferencia"}, s)["executionContextId"]
        activate(t)
        for field, value in (("usuario", "iframe." + secrets.token_hex(3)), ("senha", "falso-iframe-" + secrets.token_hex(6))):
            evaluate(cdp, s, f"document.getElementById({json.dumps(field)}).focus(), true", context=ctx)
            cdp.cmd("Input.insertText", {"text": value}, s)
        b = evaluate(cdp, s, "(() => { const b = document.querySelector('#cadastro button').getBoundingClientRect(); return [b.x + b.width/2, b.y + b.height/2]; })()", context=ctx)
        for kind in ("mousePressed", "mouseReleased"):
            cdp.cmd("Input.dispatchMouseEvent", {"type": kind, "x": b[0], "y": b[1] + 80, "button": "left", "clickCount": 1}, s)
        cdp.pump(2.0)
    verify(children and new_popup(3) is None, "ataque: envio dentro do iframe (outra origem) não oferece salvar", "f42_iframe_sem_popup")

    # 4. Vault locked at submit: the login is discarded at once (nothing stays in the
    # service worker).
    verify(req({"v": 2, "op": "lock"}).get("type") == "ok", "cofre travado antes do envio")
    navigate(t, "http://localhost:8083/cadastro")
    activate(t)
    type_text(s, "usuario", "travado." + secrets.token_hex(3))
    type_text(s, "senha", "falso-travado-" + secrets.token_hex(6))
    click(s, "#cadastro button")
    wait_for_load(s, "!!document.getElementById('mensagem')")
    verify(new_popup(3) is None, "cofre travado no envio: não oferece salvar", "f42_travado_sem_popup")
    unlock()
    pt, ps = open_popup(t)
    verify(pending_from_popup(ps) is None and evaluate(cdp, ps, "!document.querySelector('.popup-save')"),
           "cofre travado no envio: nada ficou pendente para depois do destravar", "f42_travado_descartado")
    # V6.9: suggestions with the light theme stay dark.
    verify(theme_stays_dark(ps), "V6.9: sugestões do domínio continuam escuras com prefers-color-scheme: light", "v69_tema_sugestoes")
    close_popup(pt)

    # 5. V6.8: new sign-up on localhost:8083, submitted by the user.
    navigate(t, "http://localhost:8083/cadastro")
    activate(t)
    type_text(s, "usuario", username)
    type_text(s, "senha", new_password)
    click(s, "#cadastro button")
    pt = new_popup(15)
    alone = pt is not None
    report["f42_popup_abre_sozinho"] = alone
    print(f"      popup aberto pelo service worker (openPopup): {alone}")
    wait_for_load(s, "!!document.getElementById('mensagem')")
    account_msg = evaluate(cdp, s, "document.getElementById('mensagem').textContent")
    verify(account_msg == "Conta criada. Já pode entrar.", "o cadastro no localhost:8083 foi aceito pelo site")
    if pt is None:
        pt, ps = open_popup(t, ".popup-save")
    else:
        ps = attach_popup(pt, ".popup-save")
    verify(alone, "V6.8: enviar o login novo abre o popup \"Salvar login novo\" sozinho", "v68_oferece")
    # Chrome's popup window goes up to 600 high (frame 17414); in headless it is born
    # smaller (cannot be changed through CDP): the footer stays stuck at its bottom.
    g = evaluate(cdp, ps, SAVE_GEOMETRY)
    tx = g["textos"]
    verify(tx["titulo"] == "Salvar este login no Ward?" and tx["intro"] ==
           "O formulário de localhost:8083 foi enviado com um login que ainda não está no cofre.",
           "V6.8: título e texto do frame 17414 com o host da aba", "v68_textos")
    suggested = (tx["nome"], tx["grupo"], tx["ambiente"])
    verify(tx["usuario"] == username and tx["nome"] == "localhost-8083" and tx["grupo"] == "localhost"
           and tx["ambiente"] == "dev" and tx["tipos"] == ["login"] and tx["tipo"] == "login",
           f"V6.8: pede tipo, grupo e ambiente, com sugestões do host ({suggested[0]}, {suggested[1]}, {suggested[2]})", "v68_pede_campos")
    verify(tx["mascara"] == "••••••••••••" and new_password not in g["html"] and not has_secret(g["html"]),
           "V6.8: a senha não chega ao popup (máscara fixa, DOM sem o valor)", "v68_dom_sem_senha")
    pend = pending_from_popup(ps)
    verify(isinstance(pend, dict) and sorted(pend) == ["origin", "username"] and pend["origin"] == "http://localhost:8083"
           and new_password not in json.dumps(pend), "V6.8: o service worker dá ao popup só a origem e o usuário", "v68_pendente_sem_senha")
    # Measurements of frame 17414 (360×600; the body scrolls) and of 18850 (whole
    # content, 806).
    expected = {"cabecalho": [0, 0, 360, 57], "introducao": [16, 73, 328, 66], "nome": [16, 155, 328, 70],
                "entradaNome": [16, 181, 328, 44], "usuario": [16, 241, 328, 70], "senha": [16, 327, 328, 110],
                "entradaSenha": [16, 353, 328, 44], "linha": [16, 453, 328, 70], "tipo": [16, 453, 160, 70],
                "ambiente": [184, 453, 160, 70], "grupo": [16, 539, 328, 70], "nivel": [16, 625, 328, 88]}
    diffs = [f"{k}: {g[k]} ≠ {v}" for k, v in expected.items() if g[k] != v]
    # The footer stays stuck at the bottom of the popup window (up to 600); with the
    # window larger than the content, it comes after the body, as in 18850 (y 729).
    footer_y = min(g["vh"], 806) - 77
    footer, buttons = g["rodape"], g["botoes"]
    if footer != [0, footer_y, 360, 77]:
        diffs.append(f"rodape: {footer} ≠ {[0, footer_y, 360, 77]} (janela {g['vh']})")
    if buttons != [[16, footer_y + 17, 158, 44], [186, footer_y + 17, 158, 44]]:
        diffs.append(f"botoes: {buttons}")
    verify(not diffs, "medidas do popup iguais às dos frames 17414/18850 (Extensão / Salvar login novo)"
           + ("" if not diffs else ": " + "; ".join(diffs)), "medidas_17414")
    verify(g["cores"] == {"fundo": "rgb(16, 17, 20)", "entrada": "rgb(22, 24, 29)", "borda": "rgb(36, 40, 47)",
                          "nivel": "rgb(28, 31, 37)", "primario": "rgb(74, 108, 240)", "intro": "rgb(154, 161, 174)"}
           and g["fonte"], f"cores e fontes iguais às do FigCraft ({g['cores']})", "cores_17414")
    report["f42_janela_popup"] = g["vh"]
    screenshot(ps, "salvar-login-novo-localhost-8083.png", g["vh"], EVIDENCE_POPUP, beyond=False)
    # 18850: without sticking the header and footer, the footer comes after the body (y 729).
    unstick(ps, True)
    g2 = evaluate(cdp, ps, SAVE_GEOMETRY)
    footer2 = g2["rodape"]
    verify(footer2 == [0, 729, 360, 77] and g2["nivel"] == [16, 625, 328, 88]
           and evaluate(cdp, ps, "document.documentElement.scrollHeight") == 806,
           f"medidas do conteúdo inteiro iguais às do frame 18850 (360×806; rodapé {footer2})", "medidas_18850")
    unstick(ps, False)
    # The headless popup does not capture beyond the window (it repeats the top): the
    # evidence of the rest of the body is the window scrolled to the end, with header and
    # footer stuck.
    evaluate(cdp, ps, "window.scrollTo(0, document.documentElement.scrollHeight), true")
    cdp.pump(0.3)
    screenshot(ps, "salvar-login-novo-rolado.png", g["vh"], EVIDENCE_POPUP, beyond=False,
               y=evaluate(cdp, ps, "Math.round(window.scrollY)"))
    evaluate(cdp, ps, "window.scrollTo(0, 0), true")
    verify(theme_stays_dark(ps), "V6.9: salvar login novo continua escuro com prefers-color-scheme: light", "v69_tema_salvar")
    screenshot(ps, "salvar-login-novo-tema-claro.png", g["vh"], EVIDENCE_POPUP, beyond=False)

    # 6. Attack: the content script (the tab's isolated world) asks for the pending login
    # or saves on its own; the service worker answers nothing to whoever has a tab.
    ctx = content_script_context(s)
    if ctx is not None:
        r = evaluate(cdp, s, """(async () => { const out = {};
            for (const m of [{t: 'pending', tab: 0}, {t: 'save', tab: 0, name: 'invasor', username: 'x', group: 'g', environment: 'dev'},
                             {t: 'discard', tab: 0}]) {
              try { const r = await chrome.runtime.sendMessage(m); out[m.t] = r === undefined ? 'nada' : JSON.stringify(r); }
              catch (e) { out[m.t] = 'sem resposta'; } }
            return out; })()""", context=ctx)
    else:
        r = None
    verify(r is not None and all(v in ("nada", "sem resposta") for v in r.values()),
           f"ataque: mensagens do content script (pendente/salvar/descartar) ficam sem resposta ({r})", "f42_cs_sem_resposta")
    verify(pending_from_popup(ps) is not None, "o pendente continua lá depois do ataque do content script (só o popup descarta)")

    # 7. Invalid name: the daemon refuses and the popup shows the reason in the field.
    evaluate(cdp, ps, "(() => { const n = document.getElementById('entry-name'); n.value = 'Nome Inválido'; return true; })()")
    evaluate(cdp, ps, "document.querySelector('.button.primary').click(), true")
    cdp.pump(1.5)
    name_error = evaluate(cdp, ps, "(() => { const c = document.getElementById('entry-name').closest('.field');"
                          " return c.classList.contains('with-error') ? c.querySelector('.help').textContent : ''; })()")
    verify(name_error.startswith("use letras minúsculas"), f"nome inválido: o motivo do daemon aparece no campo ({name_error})", "f42_erro_validacao")
    screenshot(ps, "salvar-login-novo-erro-nome.png", g["vh"], EVIDENCE_POPUP, beyond=False)

    # 8. Save in Ward (the user's name, group and environment).
    evaluate(cdp, ps, """(() => { document.getElementById('entry-name').value = 'localhost-8083-novo';
        document.getElementById('entry-group').value = 'teste-fase-42'; document.getElementById('entry-environment').value = 'dev'; return true; })()""")
    evaluate(cdp, ps, "document.querySelector('.button.primary').click(), true")
    verify(closed(pt), "V6.8: \"Salvar no Ward\" grava e o popup fecha", "v68_salvou")
    p = subprocess.run([WARD, "lista", "--json"], capture_output=True, timeout=60,
                       env={"PATH": "/usr/bin:/bin", "HOME": WORK_DIR, "WARD_DEV_SOCKET_DIR": SOCKDIR, "LANG": "C.UTF-8"})
    try:
        items = json.loads(p.stdout)
        items = items.get("entries", items.get("items", items)) if isinstance(items, dict) else items
        item = next((i for i in items if i.get("name") == "localhost-8083-novo"), None)
    except (ValueError, AttributeError):
        item = None
    verify(p.returncode == 0 and item is not None and item.get("group") == "teste-fase-42" and item.get("environment") == "dev"
           and item.get("type") == "login" and "level" not in item and new_password.encode() not in p.stdout,
           f"V6.8: a entrada aparece no `ward lista --json` (login, grupo e ambiente escolhidos; sem nível, D50) ({item})", "v68_ward_lista")
    # Level "with approval": the use by the agent asks for approval with PIN (the test
    # window denies); a free entry would only ask for the origin confirmation (Phase 44c).
    set_window_mode("negar")
    seen = len(window_requests())
    r = req({"v": 2, "op": "login", "data": {"entry": "localhost-8083-novo"}})
    code = (r.get("data") or {}).get("code")
    from_window = window_requests()[seen:]
    set_window_mode("confirmar")
    verify(code == "denied" and len(from_window) == 1 and " confirmacao=0 " in from_window[0],
           f"V6.8: nível inicial \"com aprovação\" (o uso pelo agente pede aprovação com PIN: {code}, {from_window})", "v68_com_aprovacao")
    # The fill of the new entry gives back the submitted password (compared without printing).
    navigate(t, "http://localhost:8083/login")
    pt, ps = open_popup(t)
    names = [x["nome"] for x in evaluate(cdp, ps, GEOMETRY)["textos"]]
    verify(names == ["acme-totp-dev", "localhost-8083-novo"], f"a entrada nova é sugerida em localhost:8083 ({names})", "f42_sugerida")
    click_entry(ps, "localhost-8083-novo")
    closed(pt)
    v = values(s, ["usuario", "senha"])
    verify(v["usuario"] == username and v["senha"] == new_password, "a entrada nova preenche o usuário e a senha enviados", "f42_preenche")
    evaluate(cdp, s, "document.getElementById('usuario').value = ''; document.getElementById('senha').value = ''; true")

    # 9. "Not now" discards the pending login.
    navigate(t, "http://localhost:8083/cadastro")
    activate(t)
    type_text(s, "usuario", "agora.nao." + secrets.token_hex(3))
    type_text(s, "senha", "falso-agora-nao-" + secrets.token_hex(6))
    click(s, "#cadastro button")
    pt = new_popup(15)
    ps = attach_popup(pt, ".popup-save") if pt else None
    if ps:
        evaluate(cdp, ps, "document.querySelector('.button.secondary-button').click(), true")
    verify(pt is not None and closed(pt), "\"Agora não\" fecha o popup", "f42_agora_nao")
    pt, ps = open_popup(t)
    verify(pending_from_popup(ps) is None, "\"Agora não\": nada fica pendente", "f42_agora_nao_descarta")
    close_popup(pt)


new_passwords = []


try:
    # Built extension, through the pipe, with the manifest's fixed id.
    r = cdp.cmd("Extensions.loadUnpacked", {"path": EXT})
    verify(r.get("id") == ID, f"extensão carregada com o id fixo do manifesto ({ID})", "id_fixo")
    cdp.cmd("Target.setDiscoverTargets", {"discover": True})

    # --- Phase 44e: the popup against the fake `[::1]` of the M6 audit ----------------
    # A process of the uid listens on [::1]:8081 (WordPress is only on 127.0.0.1:8081)
    # and Chrome loads its page with the origin http://localhost:8081, the entry's.
    # With the confirmation denied in the window, nothing is filled and the fake receives
    # nothing. It comes before any other visit to localhost:8081 (the test pages above
    # are served by CDP, without a connection): after a connection to 127.0.0.1, Chrome
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
               "c44_popup_v6_aba_no_falso")
        set_window_mode("negar")
        seen = len(window_requests())
        pt, ps = open_popup(t)
        click_entry(ps, "acme-wp-prod")
        end = time.time() + 20
        note = ""
        while time.time() < end and "negado" not in note:
            cdp.pump(0.3)
            try:
                note = evaluate(cdp, ps, "document.querySelector('.note.error') ? document.querySelector('.note.error').textContent : ''", deadline=5)
            except (RuntimeError, TimeoutError):
                note = ""
        v = values(s, ["user_login", "user_pass"])
        from_window = window_requests()[seen:]
        verify(v == {"user_login": "", "user_pass": ""} and "negado" in note,
               f"ataque [::1], popup negado na janela: nada preenchido no falso e o popup avisa ({note!r})",
               "c44_popup_v6_negado")
        verify(from_window == ["pedido operacao=fill entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao=deny"],
               f"ataque [::1], popup: o pedido mostra a origem e não pede PIN ({from_window})", "c44_popup_v6_pedido")
        close_popup(pt)
        try:
            received = os.path.getsize(fake_output)
        except FileNotFoundError:
            received = 0
        verify(received == 0, f"ataque [::1], popup: o falso não recebeu nenhum POST ({received} bytes)",
               "c44_popup_v6_falso_vazio")
        cdp.cmd("Target.closeTarget", {"targetId": t})
    finally:
        fake.terminate()
        fake.wait(timeout=10)
        set_window_mode("confirmar")

    # --- V6.7: localhost:8081 ---------------------------------------------------------
    t, s = open_tab("http://localhost:8081/wp-login.php")
    pt, ps = open_popup(t)
    g = evaluate(cdp, ps, GEOMETRY)
    names = [x["nome"] for x in g["textos"]]
    verify(names == ["acme-wp-dev", "acme-wp-prod"], "V6.7: em localhost:8081 aparecem acme-wp-dev e acme-wp-prod (só elas)", "v67_sugestoes_8081")
    verify(g["origemTexto"] == "http://localhost:8081", "popup mostra a origem da aba", "origem_mostrada")
    verify(g["rotuloTexto"] == "2 entradas para esta origem" and g["estadoTexto"] == "destravado",
           "rótulo da lista e estado do cabeçalho", "rotulo_estado")
    with_totp = lambda n: USERNAME[n] + (" · com TOTP" if ENTRIES[n]["secrets"].get("totp_seed") else "")
    env_details = [(x["amb"], x["det"]) for x in g["textos"]]
    verify([x["amb"] for x in g["textos"]] == ["dev", "prod"] and g["textos"][0]["det"] == with_totp("acme-wp-dev")
           and g["textos"][1]["det"] == with_totp("acme-wp-prod"),
           f"ambiente e detalhe (usuário · com TOTP) de cada entrada: {env_details}", "detalhes")
    verify(g["fonte"], "Inter e JetBrains Mono carregadas do pacote", "fontes")
    # Measurements of frame 17332 (360×494): header 57; body with padding 16 and gap 12;
    # origin 328×70; label at y 155; items 328×76 at y 185 and 265 (gap 4); avatar 40;
    # note 328×48 at y 353; footer at y 417 (77); button 328×44 at (16, 434).
    expected = {"popup": [0, 0, 360, 494], "cabecalho": [0, 0, 360, 57], "simbolo": [16, 16, 24, 24],
                "estado": [257, 18, 87, 20], "origem": [16, 73, 328, 70], "rotulo": [16, 155, 173, 18],
                "itens": [[16, 185, 328, 76], [16, 265, 328, 76]], "avatares": [[32, 203, 40, 40], [32, 283, 40, 40]],
                "nota": [16, 353, 328, 48], "rodape": [0, 417, 360, 77], "botao": [16, 434, 328, 44]}
    diffs = []
    for k, v in expected.items():
        real = g[k]
        # The width of a text that hugs its content depends on the font's drawing: of the
        # state pill it checks the right edge (344), y and height; of the label, x, y and
        # height.
        if k == "estado" and real:
            real = [v[0], real[1], v[2], real[3]] if real[0] + real[2] == v[0] + v[2] else real
        elif k == "rotulo" and real:
            real = [real[0], real[1], v[2], real[3]]
        if real != v:
            diffs.append(f"{k}: {real} ≠ {v}")
    verify(not diffs, "medidas do popup iguais às do frame 17332 (Extensão / Sugestões do domínio)" + ("" if not diffs else ": " + "; ".join(diffs)), "medidas_17332")
    verify(g["cores"]["fundo"] == "rgb(16, 17, 20)" and g["cores"]["nome"] == "rgb(247, 248, 250)"
           and g["cores"]["detalhe"] == "rgb(154, 161, 174)" and g["cores"]["estado"] == "rgb(76, 199, 126)",
           "cores do fundo, nome, detalhe e estado iguais às do FigCraft", "cores_17332")
    verify(not has_secret(g["html"]), "V6.5: DOM do popup sem segredo", "v65_dom_popup")
    storage = evaluate(cdp, ps, """(async () => ({ storage: typeof chrome.storage, local: localStorage.length,
        sessao: sessionStorage.length, idb: (await indexedDB.databases()).length,
        cookies: document.cookie }))()""")
    verify(storage == {"storage": "undefined", "local": 0, "sessao": 0, "idb": 0, "cookies": ""},
           "V6.5: sem chrome.storage (sem a permissão) e nada no armazenamento do popup", "v65_armazenamento")
    # Hover on the first item, as in the frame (hover state of the list Item).
    cdp.cmd("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": 100, "y": 220}, ps)
    cdp.pump(0.3)
    item0 = evaluate(cdp, ps, "getComputedStyle(document.querySelector('.item')).backgroundColor")
    verify(item0 == "rgb(36, 40, 47)", "hover do item com a superfície elevada (#24282F)", "hover")
    screenshot(ps, "sugestoes-do-dominio-localhost-8081.png", 494)
    # Phase 44c (D9): http origin, so the window asks for the origin confirmation.
    # Denied, nothing is filled and the popup says why.
    set_window_mode("negar")
    seen = len(window_requests())
    click_entry(ps, "acme-wp-prod")
    end = time.time() + 20
    note = ""
    while time.time() < end and "negado" not in note:
        cdp.pump(0.3)
        try:
            note = evaluate(cdp, ps, "document.querySelector('.note.error') ? document.querySelector('.note.error').textContent : ''", deadline=5)
        except (RuntimeError, TimeoutError):
            note = ""
    v = values(s, ["user_login", "user_pass"])
    from_window = window_requests()[seen:]
    verify(v == {"user_login": "", "user_pass": ""} and "negado" in note,
           f"Fase 44c: preencher do popup em http negado na janela: nada preenchido e o popup avisa ({note!r})", "c44_popup_negado")
    verify(from_window == ["pedido operacao=fill entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao=deny"],
           f"Fase 44c: o pedido do popup mostra a origem http e não pede PIN ({from_window})", "c44_popup_pedido")
    close_popup(pt)
    set_window_mode("confirmar")
    pt, ps = open_popup(t)
    # Clicking the prod WordPress entry fills username and password (and the popup
    # closes) after the origin confirmation.
    click_entry(ps, "acme-wp-prod")
    verify(closed(pt, 30), "o popup fecha depois de preencher (o contexto com a resposta some)", "popup_fecha")
    v = values(s, ["user_login", "user_pass"])
    verify(v["user_login"] == USERNAME["acme-wp-prod"] and v["user_pass"] == PASSWORD["acme-wp-prod"],
           "V6.7: clicar em acme-wp-prod preenche usuário e senha em localhost:8081 (origem confirmada)", "v67_preenche_8081")
    log_console(cdp)

    # --- V6.7: localhost:8083 with TOTP ------------------------------------------------
    navigate(t, "http://localhost:8083/login")
    pt, ps = open_popup(t)
    g = evaluate(cdp, ps, GEOMETRY)
    verify([x["nome"] for x in g["textos"]] == ["acme-totp-dev"], "V6.7: em localhost:8083 só acme-totp-dev", "v67_sugestoes_8083")
    screenshot(ps, "sugestoes-do-dominio-localhost-8083.png", g["popup"][3])
    click_entry(ps, "acme-totp-dev")
    closed(pt)
    v = values(s, ["usuario", "senha", "codigo"])
    verify(v["usuario"] == USERNAME["acme-totp-dev"] and v["senha"] == PASSWORD["acme-totp-dev"]
           and len(v["codigo"] or "") == 6 and v["codigo"].isdigit(),
           "V6.7: acme-totp-dev preenche usuário, senha e TOTP", "v67_totp")
    evaluate(cdp, s, "document.getElementById('login').requestSubmit(), true")
    end = time.time() + 15
    logged_in = False
    while time.time() < end and not logged_in:
        cdp.pump(0.3)
        try:
            logged_in = evaluate(cdp, s, "!!document.getElementById('painel')", deadline=5)
        except (RuntimeError, TimeoutError):
            logged_in = False
    verify(logged_in, "V6.7: o login no localhost:8083 entra (Painel)", "v67_login_entra")

    # --- V6.2: 127.0.0.1:8081 ----------------------------------------------------------
    navigate(t, "http://127.0.0.1:8081/wp-login.php")
    pt, ps = open_popup(t)
    g = evaluate(cdp, ps, GEOMETRY)
    verify(g["itens"] == [] and g["rotuloTexto"] == "Nenhuma entrada para esta origem",
           "V6.2: em 127.0.0.1:8081, acme-wp-* não é sugerida", "v62_nao_sugere")
    screenshot(ps, "sugestoes-do-dominio-sem-entrada-127.png", g["popup"][3])
    verify(forced_fill(ps, "", "http://127.0.0.1:8081", "acme-wp-dev") == "erro:origin_mismatch",
           "V6.2: preencher forçado para a aba 127.0.0.1:8081: o daemon recusa (origin_mismatch)", "v62_daemon_recusa")
    # The other layer: the injection checks the document's origin before writing.
    res = evaluate(cdp, ps, """(async () => { const m = await import('./page.js');
        const [aba] = await chrome.tabs.query({active: true, currentWindow: true});
        const [r] = await chrome.scripting.executeScript({target: {tabId: aba.id, frameIds: [0]}, world: 'ISOLATED',
          func: m.fillInPage, args: ['http://localhost:8081', 'x', btoa('nao-e-segredo'), '']});
        return r.result; })()""")
    v = values(s, ["user_login", "user_pass"])
    verify(res == {"ok": False, "reason": "origin"} and v == {"user_login": "", "user_pass": ""},
           "injeção com origem esperada diferente da do documento: nada escrito", "injecao_confere_origem")
    close_popup(pt)

    # --- phishing (8082) and redir (8084) ----------------------------------------------
    navigate(t, "http://localhost:8082/wp-login.php")
    pt, ps = open_popup(t)
    g = evaluate(cdp, ps, GEOMETRY)
    verify(g["itens"] == [], "phishing localhost:8082: nada sugerido", "phish_nada")
    verify(forced_fill(ps, "", "http://localhost:8082", "acme-wp-prod") == "erro:origin_mismatch",
           "phishing: preencher acme-wp-prod forçado com a origem da aba: recusado", "phish_recusa")
    close_popup(pt)
    final = navigate(t, "http://localhost:8084/login")
    verify(final.startswith("http://localhost:8082/"), "redir 8084 leva a aba ao phishing 8082")
    pt, ps = open_popup(t)
    g = evaluate(cdp, ps, GEOMETRY)
    verify(g["itens"] == [] and g["origemTexto"] == "http://localhost:8082",
           "redir: o popup usa a origem final (8082), delta-redir não é sugerida", "redir_nada")
    verify(forced_fill(ps, "", "http://localhost:8082", "delta-redir") == "erro:origin_mismatch",
           "redir: preencher delta-redir com a origem final: recusado (D9)", "redir_recusa")
    close_popup(pt)

    # --- tab that navigates with the popup open ----------------------------------------
    navigate(t, "http://localhost:8081/wp-login.php")
    pt, ps = open_popup(t)
    navigate(t, "http://localhost:8082/wp-login.php")
    click_entry(ps, "acme-wp-prod")
    cdp.pump(1.5)
    note = evaluate(cdp, ps, "document.querySelector('.note.error') ? document.querySelector('.note.error').textContent : ''")
    v = values(s, ["user_login", "user_pass"])
    verify(note.startswith("A aba mudou") and v == {"user_login": "", "user_pass": ""},
           "aba que navegou para o phishing com o popup aberto: nada preenchido", "aba_navegou")
    close_popup(pt)

    # --- V6.4: phishing iframe in the right origin -------------------------------------
    cdp.cmd("Network.enable", {}, s)
    navigate(t, "http://localhost:8081/ward-teste/iframe.html")
    cdp.pump(1.0)
    for e in cdp.events:
        if e.get("method") == "Network.loadingFailed":
            print("      falhou carga:", e["params"].get("errorText"), e["params"].get("blockedReason"), e["params"].get("corsErrorStatus"))
    pt, ps = open_popup(t)
    click_entry(ps, "acme-wp-prod")
    closed(pt)
    v = values(s, ["user_login", "user_pass"])
    tree = cdp.cmd("Page.getFrameTree", {}, s)["frameTree"]
    children = tree.get("childFrames", [])
    print("      quadros:", json.dumps([{k: f["frame"].get(k) for k in ("url", "unreachableUrl", "securityOrigin")} for f in children]))
    ctx = None
    if children:
        ctx = cdp.cmd("Page.createIsolatedWorld", {"frameId": children[0]["frame"]["id"], "worldName": "conferencia"}, s)["executionContextId"]
    vi = values(s, ["user_login", "user_pass"], context=ctx) if ctx else None
    url_iframe = evaluate(cdp, s, "location.href", context=ctx) if ctx else None
    verify(url_iframe == "http://localhost:8082/wp-login.php", f"V6.4: o iframe é o phishing carregado ({url_iframe})")
    verify(v["user_login"] == USERNAME["acme-wp-prod"] and v["user_pass"] == PASSWORD["acme-wp-prod"],
           "V6.4: o documento da origem certa recebe usuário e senha", "v64_principal")
    verify(vi == {"user_login": "", "user_pass": ""},
           f"V6.4: o iframe do phishing (localhost:8082) não recebe nada (iframe: {None if vi is None else {k: (None if x is None else bool(x)) for k, x in vi.items()}}, quadros filhos: {len(children)})",
           "v64_iframe_vazio")

    navigate(t, "http://localhost:8081/ward-teste/iframe-sem-campo.html")
    cdp.pump(1.0)
    pt, ps = open_popup(t)
    click_entry(ps, "acme-wp-prod")
    cdp.pump(1.5)
    note = evaluate(cdp, ps, "document.querySelector('.note.error') ? document.querySelector('.note.error').textContent : ''")
    children = cdp.cmd("Page.getFrameTree", {}, s)["frameTree"].get("childFrames", [])
    ctx = cdp.cmd("Page.createIsolatedWorld", {"frameId": children[0]["frame"]["id"], "worldName": "conferencia"}, s)["executionContextId"] if children else None
    vi = values(s, ["user_login", "user_pass"], context=ctx) if ctx else None
    verify(note.startswith("Não achei um campo de senha visível") and vi == {"user_login": "", "user_pass": ""},
           "V6.4: formulário só no iframe do phishing: nada preenchido", "v64_so_iframe")
    close_popup(pt)

    # --- DOM clobbering -----------------------------------------------------------------
    navigate(t, "http://localhost:8081/ward-teste/clobbering.html")
    pt, ps = open_popup(t)
    click_entry(ps, "acme-wp-prod")
    closed(pt)
    v = values(s, ["user_login", "user_pass", "isca_pw"])
    calls = evaluate(cdp, s, "window.__chamadas.filter((c) => c !== 'get')")
    verify(v["user_login"] == USERNAME["acme-wp-prod"] and v["user_pass"] == PASSWORD["acme-wp-prod"] and v["isca_pw"] == "",
           "DOM clobbering: preenche os campos certos, a isca fica vazia", "clobbering_campos")
    verify(calls == [], "DOM clobbering: os protótipos trocados pela página não são chamados", "clobbering_prototipos")

    # --- hidden password field ----------------------------------------------------------
    navigate(t, "http://localhost:8081/ward-teste/escondido.html")
    pt, ps = open_popup(t)
    click_entry(ps, "acme-wp-prod")
    closed(pt)
    decoys = ["isca_user", "isca_none", "isca_vis", "isca_opac", "isca_pai", "isca_1px", "isca_fora",
              "isca_clip", "isca_coberta", "isca_desab"]
    v = values(s, decoys + ["user_login", "user_pass"])
    filled = [i for i in decoys if v[i]]
    verify(not filled, "campo escondido: nenhuma das 10 iscas recebe valor" + (f" ({filled})" if filled else ""), "escondido_iscas")
    verify(v["user_login"] == USERNAME["acme-wp-prod"] and v["user_pass"] == PASSWORD["acme-wp-prod"],
           "campo escondido: só os campos visíveis recebem", "escondido_visiveis")
    navigate(t, "http://localhost:8081/ward-teste/so-escondido.html")
    pt, ps = open_popup(t)
    click_entry(ps, "acme-wp-prod")
    cdp.pump(1.5)
    note = evaluate(cdp, ps, "document.querySelector('.note.error') ? document.querySelector('.note.error').textContent : ''")
    v = values(s, ["isca_user", "isca_pw"])
    verify(note.startswith("Não achei um campo de senha visível") and v == {"isca_user": "", "isca_pw": ""},
           "só campos escondidos: nada preenchido e o popup avisa", "so_escondido")
    screenshot(ps, "sugestoes-do-dominio-sem-campo.png", evaluate(cdp, ps, "Math.round(document.getElementById('popup').getBoundingClientRect().height)"))
    close_popup(pt)

    # --- V6.6: page that talks to the extension ----------------------------------------
    navigate(t, "http://localhost:8081/ward-teste/mensagens.html")
    cdp.pump(2.0)
    res = evaluate(cdp, s, "window.__res")
    v = values(s, ["user_login", "user_pass"])
    verify(res.get("runtime") == "sem chrome.runtime" or res.get("resposta") in (None, "nada"),
           "V6.6: chrome.runtime.sendMessage(<id>) da página não chega a nada (sem externally_connectable)", "v66_runtime")
    verify(res.get("fetch") == "bloqueado" and res.get("fetch_manifesto") == "bloqueado"
           and res.get("iframe") != "leu",
           "V6.6: a página não lê o popup nem o manifesto (sem web_accessible_resources)", "v66_popup_ilegivel")
    verify(v == {"user_login": "", "user_pass": ""}, "V6.6: postMessage pedindo preencher: nada acontece", "v66_postmessage")
    report["v66_detalhe"] = res

    # --- Phase 42: "Save new login" (V6.8) and theme (V6.9) ----------------------------
    phase_42_section(t, s)

    # --- locked vault --------------------------------------------------------------------
    verify(req({"v": 2, "op": "lock"}).get("type") == "ok", "cofre travado pelo socket")
    navigate(t, "http://localhost:8081/wp-login.php")
    pt, ps = open_popup(t, ".locked-body")
    g = evaluate(cdp, ps, GEOMETRY)
    verify(g["estadoTexto"] == "travado" and g["itens"] == [], "travado: o popup mostra o estado e nenhuma entrada", "travado")
    # Measurements of frame 17392 (360×343).
    locked_geo = g["travado"]
    expected = {"popup": [0, 0, 360, 343], "cabecalho": [0, 0, 360, 57]}
    diffs = [f"{k}: {g[k]} ≠ {e}" for k, e in expected.items() if g[k] != e]
    for k, e in {"avatar": [156, 81, 48, 48], "titulo": [24, 145, 312, 26], "texto": [24, 187, 312, 40],
                 "botao": [24, 243, 312, 44], "nota": [24, 303, 312, 16]}.items():
        if locked_geo[k] != e:
            diffs.append(f"{k}: {locked_geo[k]} ≠ {e}")
    verify(not diffs, "medidas do travado iguais às do frame 17392" + ("" if not diffs else ": " + "; ".join(diffs)), "medidas_17392")
    screenshot(ps, "cofre-travado-localhost-8081.png", 343)
    # V6.9: with the light theme of the system and of Chrome, it stays dark (D1).
    verify(theme_stays_dark(ps), "V6.9: cofre travado continua escuro com prefers-color-scheme: light", "v69_tema_travado")
    screenshot(ps, "cofre-travado-tema-claro.png", 343, EVIDENCE_POPUP)
    # V6.9: the button asks the daemon to open the app (D57); the test daemon without
    # WARD_DEV_APP answers "app unavailable", and the request shows up in its log.
    # The request to the host is spied on in the popup (test wrapper, with a pause to read).
    evaluate(cdp, ps, """(() => { const orig = chrome.runtime.sendNativeMessage.bind(chrome.runtime);
        window.__ops = []; chrome.runtime.sendNativeMessage = async (h, m) => { window.__ops.push(m && m.op);
        await new Promise((r) => setTimeout(r, 1500)); return orig(h, m); };
        document.querySelector('.locked-body .button').click(); return true; })()""")
    cdp.pump(0.5)
    ops = evaluate(cdp, ps, "window.__ops")
    verify(ops == ["open_app"] and closed(pt), f"V6.9: o botão do travado pede open_app ao daemon e o popup fecha ({ops})", "v69_botao_abre_app")
    unlock()

    log_console(cdp)
    # The whole temporary profile without secrets (nothing stored, V6.5).
    found_files = []
    for dir_path, _, names in os.walk(profile):
        for n in names:
            p = os.path.join(dir_path, n)
            try:
                if os.path.isfile(p) and has_secret(open(p, "rb").read()):
                    found_files.append(os.path.relpath(p, profile))
            except OSError:
                pass
    # Chrome's HTTP cache and form history keep what the site sent and what the user
    # submitted (the 8083 login); the extension itself stores nothing.
    from_extension = [a for a in found_files if ID in a or "Extension" in a or "Local Storage" in a or "IndexedDB" in a]
    verify(not from_extension, "V6.5: nada da extensão no perfil com segredo" + (f" ({from_extension})" if from_extension else ""), "v65_perfil")
    # Phase 44a: separates the dictionary of common passwords (zxcvbn; not a leak) from the rest.
    dic = zxcvbn_dictionary(profile)
    # Self-test of the filter: a vault secret planted in the middle of the zxcvbn list, a
    # dictionary pattern outside the zxcvbn file and a loose pattern do not pass as
    # dictionary.
    dic_list = sorted(t for t in dic if t.isalpha() and len(t) >= 4)[:40]
    middle = b",".join(dic_list[:20]) + b","
    tail = b"," + b",".join(dic_list[20:])
    non_dic = next(p for p in patterns if p not in dic)
    cc = os.path.join("Default", "Code Cache", "js", "x_0")
    self_test = [
        not dictionary_finding(cc, b"zxcvbn.min.js " + middle + non_dic + tail, dic)[0],
        not dictionary_finding(os.path.join("Default", "History"), middle + non_dic + tail, dic)[0],
    ]
    in_dic = [p for p in patterns if p in dic]
    for p_dic in in_dic:
        self_test += [
            dictionary_finding(cc, b"zxcvbn.min.js " + middle + p_dic + tail, dic)[0],
            not dictionary_finding(os.path.join("Default", "Code Cache", "js", "y_0"), middle + p_dic + tail, dic)[0],
            not dictionary_finding(cc, b"zxcvbn.min.js " + b"x" * 80 + p_dic + b"y" * 80, dic)[0],
            not dictionary_finding(os.path.join("Default", "Preferences"), middle + p_dic + tail, dic)[0],
        ]
    verify(len(dic_list) == 40 and all(self_test),
           f"Fase 44a: filtro do dicionário zxcvbn recusa segredo plantado ({len(self_test)} casos)",
           "f44a_filtro_dicionario_autoteste")
    from_dictionary = {}
    for a in list(found_files):
        try:
            yes, idx = dictionary_finding(a, open(os.path.join(profile, a), "rb").read(), dic)
        except OSError:
            continue
        if yes:
            from_dictionary[a] = idx
            found_files.remove(a)
    verify(not any(ID in a or "Extension" in a for a in from_dictionary),
           "V6.5: achado do dicionário zxcvbn fora de arquivo da extensão", "v65_dicionario_fora_da_extensao")
    report["perfil_dicionario_zxcvbn"] = {
        a: [{"linha_variacoes": i + 1, "bytes": len(patterns[i]), "so_digitos": patterns[i].isdigit()} for i in idx]
        for a, idx in sorted(from_dictionary.items())}
    for a, idx in sorted(from_dictionary.items()):
        print(f"info  {a}: lista de senhas comuns do zxcvbn (linha(s) {[i + 1 for i in idx]} de "
              ".dev/variations.txt; palavra do dicionário no meio da lista); não vem do cofre")
    report["perfil_com_padrao_fora_da_extensao"] = sorted(found_files)
    # Phase 42: the password of the new login does not stay in anything of the extension
    # either (the service worker holds it only in memory until saving).
    leaked = []
    for dir_path, _, file_names in os.walk(profile):
        for n in file_names:
            q = os.path.join(dir_path, n)
            rel = os.path.relpath(q, profile)
            if ID in rel or "Extension" in rel or "Local Storage" in rel or "IndexedDB" in rel or "Service Worker" in rel:
                try:
                    data_q = open(q, "rb").read()
                except OSError:
                    continue
                if any(x.encode() in data_q for x in new_passwords):
                    leaked.append(rel)
    verify(new_passwords and not leaked, "V6.5/V6.8: a senha do login novo não fica em nada da extensão no perfil"
           + (f" ({leaked})" if leaked else ""), "f42_perfil_sem_senha_nova")
    for a in found_files:
        # Diagnosis without the value: the readable text around it, with every pattern
        # replaced by ***.
        data = open(os.path.join(profile, a), "rb").read()
        for pattern in patterns:
            i = data.find(pattern)
            if i >= 0:
                excerpt = data[max(0, i - 300): i + len(pattern) + 300]
                for q in patterns:
                    excerpt = excerpt.replace(q, b"***")
                readable = "".join(chr(c) if 32 <= c < 127 else "." for c in excerpt)
                print(f"      {a}: padrão em {i}: {readable}")
                break
finally:
    try:
        cdp.cmd("Browser.close", deadline=10)
    except Exception:
        pass
    try:
        ch.wait(timeout=20)
    except subprocess.TimeoutExpired:
        ch.kill()
    console.close()

posts_after = posts_phish()
verify(posts_before is not None and posts_after == posts_before, "posts.log do phishing (8082) sem mudança", "posts_log")

# --- The host outside Chrome stays agent (the exception only holds with the installed Chrome as parent) ---
env = {"PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR, "HOME": WORK_DIR}


def frame(obj):
    b = json.dumps(obj).encode()
    return struct.pack("=I", len(b)) + b


stdin_data = frame({"v": 2, "op": "status"}) + frame(
    {"v": 2, "op": "fill", "data": {"entry": "acme-wp-prod", "origin": "http://localhost:8081"}})
p = subprocess.run([NM, f"chrome-extension://{ID}/"], input=stdin_data, capture_output=True, env=env, timeout=60)
verify(b'"type":"fill"' not in p.stdout and b"unauthorized" in p.stdout and not has_secret(p.stdout),
       "exceção de teste: o ward-nm aberto por outro processo continua agente", "excecao_so_chrome")
fake_file = os.path.join(WORK_DIR, "falso.txt")
with open(fake_file, "w") as f:
    f.write(json.dumps({"v": 2, "op": "fill", "data": {"entry": "acme-wp-prod", "origin": "http://localhost:8081"}}) + "\n")
p = subprocess.run([FAKE_CHROME, NM, fake_file], capture_output=True, env=env, timeout=60)
verify(b'"type":"fill"' not in p.stdout and b"unauthorized" in p.stdout and not has_secret(p.stdout),
       "exceção de teste: o ward-nm filho de um Chrome falso (outro exe) continua agente", "excecao_chrome_falso")
p = subprocess.run([NM, "chrome-extension://" + "a" * 32 + "/"], input=stdin_data, capture_output=True, env=env, timeout=60)
verify(p.returncode == 2 and p.stdout == b"", "ward-nm recusa outra extensão (id diferente do do Ward)", "id_outra_extensao")

os.makedirs(EVIDENCE, exist_ok=True)
with open(os.path.join(EVIDENCE, "qa.json"), "w", encoding="utf-8") as f:
    json.dump({k: v for k, v in report.items() if not k.startswith(("f42_", "v68_", "v69_", "medidas_17414", "medidas_18850", "cores_17414"))},
              f, ensure_ascii=False, indent=2, sort_keys=True)
    f.write("\n")
os.makedirs(EVIDENCE_POPUP, exist_ok=True)
with open(os.path.join(EVIDENCE_POPUP, "qa.json"), "w", encoding="utf-8") as f:
    json.dump({k: v for k, v in report.items() if k.startswith(("f42_", "v68_", "v69_", "medidas_17414", "medidas_18850", "cores_17414"))},
              f, ensure_ascii=False, indent=2, sort_keys=True)
    f.write("\n")
if os.environ.get("WARD_TESTE_MANTER_PERFIL") != "1":
    shutil.rmtree(profile, ignore_errors=True)
if failures:
    print(f"extension.py: {len(failures)} conferência(s) falharam", file=sys.stderr)
    sys.exit(1)
