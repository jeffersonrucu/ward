"""`make teste-extensao` (Fase 41): a extensão no Chrome real, pelo CDP no pipe.

Chamado por `dev/testes/extensao.sh`, que sobe o `wardd` de teste só dele com
`WARD_DEV_EXTENSAO=chrome`. Argumentos: raiz, pasta de trabalho (em `.dev/`), pasta do
socket, `ward-nm` de teste, `chrome-falso`, Chrome, lista de variações e a extensão
montada. Nunca imprime a lista nem um valor dela; compara os valores preenchidos com a
fixture sem mostrá-los.

O Chrome roda headless com `--remote-debugging-pipe` (sem porta TCP) e
`--enable-unsafe-extension-debugging`: a extensão entra por `Extensions.loadUnpacked`
(o `--load-extension` não vale no Chrome de marca 137+), e o clique no ícone é o
`Extensions.triggerAction`, que dá o `activeTab` como o clique de verdade. As páginas
hostis da origem certa (`http://localhost:8081/ward-teste/...`) são servidas pelo
próprio CDP (`Fetch.fulfillRequest`): para o Chrome, são documentos dessa origem.
"""

import base64
import json
import secrets
import os
import shutil
import socket
import struct
import subprocess
import sys
import time

RAIZ, TRAB, SOCKDIR, NM, FALSO, CHROME, LISTA, EXT, WARD = sys.argv[1:10]
CAMINHO = os.path.join(SOCKDIR, "wardd.sock")
EVID = os.path.join(RAIZ, "docs", "mission", "evidence", "fase-41")
EVID42 = os.path.join(RAIZ, "docs", "mission", "evidence", "fase-42")
HOST = "br.ward.nm"
falhas = []
relato = {}


def falhou(msg):
    print(f"FALHA {msg}", file=sys.stderr)
    falhas.append(msg)


def ok(msg):
    print(f"ok    {msg}")


def confere(cond, msg, chave=None):
    if chave:
        relato[chave] = bool(cond)
    if cond:
        ok(msg)
    else:
        falhou(msg)
    return cond


padroes = [l for l in open(LISTA, "rb").read().split(b"\n") if l]
if len(padroes) < 10:
    print("extensao.py: lista de variações curta demais; rode 'make dev-variacoes'", file=sys.stderr)
    sys.exit(1)


def tem_segredo(dados):
    if isinstance(dados, str):
        dados = dados.encode()
    return any(p in dados for p in padroes)


fixtures = json.load(open(f"{RAIZ}/dev/fixtures/entradas.json", encoding="utf-8"))
ENTRADAS = {e["nome"]: e for e in fixtures["entradas"]}
ID = None
with open(os.path.join(RAIZ, "crates", "ward-nm", "src", "lib.rs"), encoding="utf-8") as f:
    for linha in f:
        if linha.startswith("pub const ID_EXTENSAO"):
            ID = linha.split('"')[1]
assert ID and len(ID) == 32


# --- socket do daemon ---------------------------------------------------------------
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


def destravar():
    senha = base64.b64encode(fixtures["senha_mestra"].encode()).decode()
    return req({"v": 1, "op": "destravar", "dados": {"senha": senha}}).get("tipo") == "ok"


def posts_phish():
    p = subprocess.run([f"{RAIZ}/dev/servicos/compose.sh", "exec", "-T", "phish", "cat", "/registro/posts.log"],
                       capture_output=True, timeout=120)
    if p.returncode == 0:
        return p.stdout
    # Serviço recém-subido (tmpfs limpo): o phishing ainda não recebeu nenhum POST.
    if b"No such file or directory" in p.stderr:
        return b""
    return None


# --- CDP no pipe ----------------------------------------------------------------------
class Cdp:
    def __init__(self, fd_escrita, fd_leitura):
        self.w = fd_escrita
        self.r = fd_leitura
        self.n = 0
        self.buf = b""
        self.eventos = []
        self.ao_pausar = None
        self.ignorar = set()

    def enviar(self, metodo, params, sessao=None):
        """Comando sem esperar a resposta (para atender o Fetch no meio de outro)."""
        self.n += 1
        m = {"id": self.n, "method": metodo, "params": params}
        if sessao:
            m["sessionId"] = sessao
        self.ignorar.add(self.n)
        os.write(self.w, json.dumps(m).encode() + b"\0")

    def _ler(self, prazo):
        import select
        while b"\0" not in self.buf:
            falta = prazo - time.time()
            if falta <= 0:
                return None
            pronto, _, _ = select.select([self.r], [], [], falta)
            if not pronto:
                return None
            d = os.read(self.r, 1 << 20)
            if not d:
                raise RuntimeError("o Chrome fechou o pipe")
            self.buf += d
        msg, _, self.buf = self.buf.partition(b"\0")
        j = json.loads(msg)
        if j.get("method") == "Fetch.requestPaused" and self.ao_pausar:
            self.ao_pausar(j)
            return self._ler(prazo)
        if j.get("id") in self.ignorar:
            self.ignorar.discard(j["id"])
            return self._ler(prazo)
        return j

    def cmd(self, metodo, params=None, sessao=None, prazo=30):
        self.n += 1
        meu = self.n
        m = {"id": meu, "method": metodo, "params": params or {}}
        if sessao:
            m["sessionId"] = sessao
        os.write(self.w, json.dumps(m).encode() + b"\0")
        fim = time.time() + prazo
        while True:
            j = self._ler(fim)
            if j is None:
                raise TimeoutError(metodo)
            if j.get("id") == meu:
                if "error" in j:
                    raise RuntimeError(f"{metodo}: {j['error'].get('message')}")
                return j.get("result", {})
            self.eventos.append(j)

    def esperar(self, cond, prazo=20):
        fim = time.time() + prazo
        while True:
            for i, e in enumerate(self.eventos):
                if cond(e):
                    return self.eventos.pop(i)
            j = self._ler(fim)
            if j is None:
                return None
            self.eventos.append(j)

    def bombear(self, segundos=0.2):
        fim = time.time() + segundos
        while True:
            j = self._ler(fim)
            if j is None:
                return
            self.eventos.append(j)


console = open(os.path.join(TRAB, "console.log"), "w", encoding="utf-8")


def registrar_console(cdp):
    resto = []
    for e in cdp.eventos:
        m = e.get("method")
        if m == "Runtime.consoleAPICalled":
            args = e["params"].get("args", [])
            console.write(" ".join(str(a.get("value", a.get("description", ""))) for a in args) + "\n")
        elif m == "Log.entryAdded":
            console.write(str(e["params"]["entry"].get("text", "")) + "\n")
        elif m == "Runtime.exceptionThrown":
            console.write(json.dumps(e["params"].get("exceptionDetails", {}))[:2000] + "\n")
        else:
            resto.append(e)
    cdp.eventos = resto
    console.flush()


def avaliar(cdp, sessao, expr, prazo=30, contexto=None):
    p = {"expression": expr, "awaitPromise": True, "returnByValue": True}
    if contexto is not None:
        p["contextId"] = contexto
    r = cdp.cmd("Runtime.evaluate", p, sessao, prazo)
    if "exceptionDetails" in r:
        raise RuntimeError("avaliar: " + json.dumps(r["exceptionDetails"])[:300])
    return r.get("result", {}).get("value")


# --- páginas hostis da origem certa (servidas pelo CDP) --------------------------------
FORM = (
    '<form id="f" action="/wp-login.php" method="post">'
    '<input type="text" name="log" id="user_login" autocomplete="username">'
    '<input type="password" name="pwd" id="user_pass" autocomplete="current-password">'
    '<button type="submit">Acessar</button></form>'
)
PAGINAS = {
    # V6.4: iframe do phishing dentro da origem certa.
    "iframe.html": "<h1>Origem certa com iframe</h1>" + FORM
    + '<iframe id="isca" src="http://localhost:8082/wp-login.php" width="400" height="400"></iframe>',
    # V6.4: só o iframe do phishing tem formulário; o documento principal, nenhum.
    "iframe-sem-campo.html": "<h1>Origem certa sem formulário</h1>"
    + '<iframe id="isca" src="http://localhost:8082/wp-login.php" width="400" height="400"></iframe>',
    # DOM clobbering: nomes que tapam os métodos do document e do form, mais os
    # protótipos do mundo da página trocados (a extensão usa os do mundo isolado).
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
    # Campo de senha escondido: as iscas vêm antes do campo de verdade.
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
    # Só campos escondidos: nada é preenchido.
    "so-escondido.html": (
        '<form><input type="text" id="isca_user" style="opacity:0">'
        '<input type="password" id="isca_pw" style="display:none"></form>'
    ),
    # Página que tenta falar com a extensão e ler o popup.
    "mensagens.html": "<h1>Mensagens</h1>" + FORM + '<script src="/ward-teste/mensagens.js"></script>',
    "mensagens.js": (
        "window.__res = {};"
        f"const ID = '{ID}';"
        "window.__res.runtime = typeof chrome === 'undefined' || !chrome.runtime ? 'sem chrome.runtime' : typeof chrome.runtime.sendMessage;"
        "try { chrome.runtime.sendMessage(ID, {v:1, op:'preencher', dados:{entrada:'acme-wp-prod', origem: location.origin}},"
        " (r) => { window.__res.resposta = r === undefined ? 'nada' : 'algo'; }); window.__res.envio = 'chamou'; }"
        " catch (e) { window.__res.envio = 'erro'; }"
        "window.postMessage({v:1, op:'preencher', dados:{entrada:'acme-wp-prod'}}, '*');"
        "window.postMessage({tipo:'ward', entrada:'acme-wp-prod'}, '*');"
        "fetch('chrome-extension://' + ID + '/popup.html').then(() => { window.__res.fetch = 'leu'; }, () => { window.__res.fetch = 'bloqueado'; });"
        "fetch('chrome-extension://' + ID + '/manifest.json').then(() => { window.__res.fetch_manifesto = 'leu'; }, () => { window.__res.fetch_manifesto = 'bloqueado'; });"
        "const f = document.createElement('iframe'); f.src = 'chrome-extension://' + ID + '/popup.html';"
        "f.onload = () => { try { window.__res.iframe = f.contentDocument ? 'leu' : 'sem documento'; } catch (e) { window.__res.iframe = 'bloqueado'; } };"
        "document.body.append(f);"
    ),
}


# Fase 42: envio forjado pelo script da página (sem ativação do usuário) e formulário
# só num iframe de outra origem.
PAGINAS.update({
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


def servir_paginas(cdp, sessao):
    cdp.cmd("Fetch.enable", {"patterns": [{"urlPattern": "http://localhost:8081/ward-teste/*", "requestStage": "Request"}]}, sessao)


def atender_pausa(e):
    p = e["params"]
    nome = p["request"]["url"].rsplit("/", 1)[-1]
    corpo = PAGINAS.get(nome)
    if corpo is None:
        cdp.enviar("Fetch.failRequest", {"requestId": p["requestId"], "errorReason": "Failed"}, e.get("sessionId"))
        return
    tipo = "text/javascript" if nome.endswith(".js") else "text/html; charset=utf-8"
    if not nome.endswith(".js"):
        corpo = f'<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><title>{nome}</title></head><body>{corpo}</body></html>'
    cdp.enviar("Fetch.fulfillRequest", {
        "requestId": p["requestId"], "responseCode": 200,
        "responseHeaders": [{"name": "Content-Type", "value": tipo}],
        "body": base64.b64encode(corpo.encode()).decode()}, e.get("sessionId"))


def atender_fetch(cdp):
    pass


# --- Chrome -------------------------------------------------------------------------
perfil = os.path.join(TRAB, "perfil")
home = os.path.join(TRAB, "home")
hosts = os.path.join(perfil, "NativeMessagingHosts")
os.makedirs(hosts)
os.makedirs(home)
with open(os.path.join(hosts, HOST + ".json"), "w") as f:
    json.dump({"name": HOST, "description": "Ward (teste)", "path": NM, "type": "stdio",
               "allowed_origins": [f"chrome-extension://{ID}/"]}, f)

if not destravar():
    falhou("não destravou o cofre de teste (cópia)")
    sys.exit(1)
ok("destravou o cofre de teste (cópia) pelo socket")
posts_antes = posts_phish()
confere(posts_antes is not None, "posts.log do phishing lido antes do teste")

r_cmd, w_cmd = os.pipe()
r_res, w_res = os.pipe()


def prep():
    os.dup2(r_cmd, 3)
    os.dup2(w_res, 4)


chrome_env = {"HOME": home, "PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR, "LANG": "pt_BR.UTF-8"}
log_chrome = open(os.path.join(TRAB, "chrome.log"), "w")
ch = subprocess.Popen(
    [CHROME, "--headless=new", f"--user-data-dir={perfil}", "--remote-debugging-pipe",
     "--enable-unsafe-extension-debugging", "--no-first-run", "--no-default-browser-check",
     "--disable-gpu", "--disable-sync", "--disable-background-networking", "--lang=pt-BR",
     "--force-device-scale-factor=1", "--window-size=1280,800",
     # As páginas servidas pelo CDP não têm IP (para o Chrome, são "públicas") e o Local
     # Network Access barraria o iframe do phishing em localhost:8082 (V6.4). Não é ele
     # que protege o preenchimento: o teste quer o iframe carregado.
     "--disable-features=LocalNetworkAccessChecks", "about:blank"],
    preexec_fn=prep, pass_fds=(3, 4), env=chrome_env, stdout=subprocess.DEVNULL, stderr=log_chrome)
os.close(r_cmd)
os.close(w_res)
cdp = Cdp(w_cmd, r_res)
cdp.ao_pausar = atender_pausa

GEOMETRIA = """(() => {
  const r = (s) => { const e = document.querySelector(s); if (!e) return null;
    const b = e.getBoundingClientRect(); return [Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)]; };
  const todos = (s) => [...document.querySelectorAll(s)].map((e) => { const b = e.getBoundingClientRect();
    return [Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)]; });
  const cor = (s, p) => { const e = document.querySelector(s); return e ? getComputedStyle(e)[p] : null; };
  return { popup: r('#popup'), cabecalho: r('.cabecalho'), simbolo: r('.cabecalho .simbolo'), estado: r('.estado'),
    origem: r('.origem'), rotulo: r('.corpo > p.secundario'), itens: todos('.item'), avatares: todos('.item .avatar'),
    badges: todos('.item .badge-ambiente'), tipos: todos('.item .badge-tipo'), nota: r('.corpo > p.nota:last-child'),
    rodape: r('.rodape'), botao: r('.rodape .botao'),
    travado: { avatar: r('.avatar-grande'), titulo: r('.titulo-travado'), texto: r('.texto-travado'),
      botao: r('.corpo-travado .botao'), nota: r('.corpo-travado .nota') },
    fonte: document.fonts.check('500 14px Inter') && document.fonts.check('13px "JetBrains Mono"'),
    cores: { fundo: cor('body', 'backgroundColor'), item0: cor('.item', 'backgroundColor'),
      estado: cor('.estado', 'color'), nome: cor('.item-nome', 'color'), detalhe: cor('.item-detalhe', 'color'),
      stage: cor('.badge-ambiente', 'color') },
    textos: [...document.querySelectorAll('.item')].map((b) => ({ nome: b.querySelector('.item-nome').textContent,
      amb: b.querySelector('.badge-ambiente').textContent, det: b.querySelector('.item-detalhe').textContent })),
    origemTexto: (document.querySelector('.origem-valor') || {}).textContent || null,
    rotuloTexto: (document.querySelector('.corpo > p.secundario') || {}).textContent || null,
    notaTexto: (document.querySelector('.nota') || {}).textContent || null,
    estadoTexto: (document.querySelector('.estado') || {}).textContent || null,
    html: document.documentElement.outerHTML };
})()"""

paginas = {}  # targetId -> sessionId
popups_vistos = set()
abas = {}  # targetId da página -> targetId do alvo "tab" (o do triggerAction)


def alvos_tab():
    return {a["targetId"] for a in cdp.cmd("Target.getTargets", {"filter": [{"type": "tab"}]})["targetInfos"]}


def abrir_aba(url):
    antes = alvos_tab()
    t = cdp.cmd("Target.createTarget", {"url": "about:blank"})["targetId"]
    novas = alvos_tab() - antes
    if len(novas) != 1:
        raise RuntimeError("não achei o alvo tab da aba nova")
    abas[t] = novas.pop()
    s = cdp.cmd("Target.attachToTarget", {"targetId": t, "flatten": True})["sessionId"]
    cdp.cmd("Page.enable", {}, s)
    cdp.cmd("Runtime.enable", {}, s)
    cdp.cmd("Log.enable", {}, s)
    servir_paginas(cdp, s)
    paginas[t] = s
    navegar(t, url)
    return t, s


def navegar(t, url):
    s = paginas[t]
    cdp.cmd("Page.navigate", {"url": url}, s)
    fim = time.time() + 20
    while time.time() < fim:
        cdp.bombear(0.2)
        atender_fetch(cdp)
        try:
            pronto = avaliar(cdp, s, "document.readyState === 'complete' && location.href", prazo=5)
        except (RuntimeError, TimeoutError):
            pronto = False
        if pronto and pronto != "about:blank":
            return pronto
    raise TimeoutError(url)


def ativar(t):
    cdp.cmd("Target.activateTarget", {"targetId": t})


def abrir_popup(t, esperar=".item, .corpo-travado, .nota"):
    """O clique no ícone (`Extensions.triggerAction`): abre o popup e dá o activeTab."""
    ativar(t)
    cdp.cmd("Extensions.triggerAction", {"id": ID, "targetId": abas[t]})
    url_popup = f"chrome-extension://{ID}/popup.html"
    pt = None
    fim = time.time() + 20
    while pt is None and time.time() < fim:
        for a in cdp.cmd("Target.getTargets")["targetInfos"]:
            if a["url"].startswith(url_popup) and a["targetId"] not in popups_vistos:
                pt = a["targetId"]
                break
        else:
            cdp.bombear(0.2)
    if pt is None:
        raise TimeoutError("o popup não abriu")
    popups_vistos.add(pt)
    ps = cdp.cmd("Target.attachToTarget", {"targetId": pt, "flatten": True})["sessionId"]
    cdp.cmd("Runtime.enable", {}, ps)
    cdp.cmd("Log.enable", {}, ps)
    cdp.cmd("Page.enable", {}, ps)
    fim = time.time() + 20
    while time.time() < fim:
        try:
            if avaliar(cdp, ps, f"document.fonts.status === 'loaded' && !!document.querySelector({json.dumps(esperar)})", prazo=5):
                break
        except (RuntimeError, TimeoutError):
            pass
        cdp.bombear(0.1)
    return pt, ps


def fechou(pt, prazo=10):
    fim = time.time() + prazo
    while time.time() < fim:
        alvos = cdp.cmd("Target.getTargets")["targetInfos"]
        if not any(a["targetId"] == pt for a in alvos):
            return True
        cdp.bombear(0.2)
    return False


def fechar_popup(pt):
    try:
        cdp.cmd("Target.closeTarget", {"targetId": pt})
    except RuntimeError:
        pass


def captura(ps, nome, altura, pasta=EVID, alem=True, y=0):
    os.makedirs(pasta, exist_ok=True)
    img = cdp.cmd("Page.captureScreenshot", {"format": "png", "clip": {"x": 0, "y": y, "width": 360, "height": altura, "scale": 1},
                                             "captureBeyondViewport": alem}, ps)["data"]
    with open(os.path.join(pasta, nome), "wb") as f:
        f.write(base64.b64decode(img))
    ok(f"screenshot {nome}")


def valores(s, ids, contexto=None):
    js = "(() => { const v = {}; for (const id of %s) { const e = document.getElementById(id); v[id] = e ? e.value : null; } return v; })()" % json.dumps(ids)
    return avaliar(cdp, s, js, contexto=contexto)


def clicar_entrada(ps, nome):
    avaliar(cdp, ps, f"document.querySelector('[data-entrada=\"{nome}\"]').click(), true")


SENHA = {n: e["segredos"]["senha"] for n, e in ENTRADAS.items() if e["tipo"] == "login"}
USUARIO = {n: e["campos"]["usuario"] for n, e in ENTRADAS.items() if e["tipo"] == "login"}


def esperar_valor(s, campo, prazo=10):
    fim = time.time() + prazo
    while time.time() < fim:
        v = valores(s, [campo]).get(campo)
        if v:
            return v
        cdp.bombear(0.2)
    return ""


def preencher_forcado(ps, origem_aba, origem_pedida, entrada):
    """Pelo contexto do popup: `preencher` direto no `ward-nm` (como um content script que
    forçasse a mensagem). Devolve só o tipo/código, nunca valores."""
    js = """(async () => { const r = await chrome.runtime.sendNativeMessage(%s, {v: 1, op: 'preencher',
      dados: {entrada: %s, origem: %s}}); return r ? (r.tipo === 'erro' ? 'erro:' + r.dados.codigo : r.tipo) : 'nada'; })()""" % (
        json.dumps(HOST), json.dumps(entrada), json.dumps(origem_pedida))
    return avaliar(cdp, ps, js)


# --- Fase 42 (V6.8, V6.9) ---------------------------------------------------------------
URL_POPUP = f"chrome-extension://{ID}/popup.html"


def tema_continua_escuro(ps):
    """V6.9 (D1): com `prefers-color-scheme: light` (o tema claro do sistema e do Chrome),
    o popup continua com as cores escuras e não troca nada."""
    antes = avaliar(cdp, ps, "[getComputedStyle(document.body).backgroundColor, getComputedStyle(document.body).color,"
                    " getComputedStyle(document.documentElement).colorScheme]")
    cdp.cmd("Emulation.setEmulatedMedia", {"features": [{"name": "prefers-color-scheme", "value": "light"}]}, ps)
    cdp.bombear(0.3)
    claro = avaliar(cdp, ps, "[matchMedia('(prefers-color-scheme: light)').matches, getComputedStyle(document.body).backgroundColor,"
                    " getComputedStyle(document.body).color, getComputedStyle(document.documentElement).colorScheme]")
    return claro[0] is True and claro[1:] == antes and antes[0] == "rgb(16, 17, 20)" and antes[1] == "rgb(247, 248, 250)"


def sem_prender(ps, sim):
    """Cabeçalho e rodapé sem `sticky` (pelo CSSOM, que a CSP deixa): o layout fica o
    do frame 18850, com o conteúdo inteiro sem rolagem."""
    v = "static" if sim else ""
    avaliar(cdp, ps, f"document.querySelector('.cabecalho').style.position = '{v}';"
            f" document.querySelector('.rodape-salvar').style.position = '{v}'; true")
    cdp.bombear(0.2)


def popup_novo(prazo=10):
    """Um popup da extensão que ainda não foi visto (aberto pelo `chrome.action.openPopup`
    do service worker, sem clique no ícone)."""
    fim = time.time() + prazo
    while time.time() < fim:
        for a in cdp.cmd("Target.getTargets")["targetInfos"]:
            if a["url"].startswith(URL_POPUP) and a["targetId"] not in popups_vistos:
                return a["targetId"]
        cdp.bombear(0.2)
    return None


def anexar_popup(pt, esperar):
    popups_vistos.add(pt)
    ps = cdp.cmd("Target.attachToTarget", {"targetId": pt, "flatten": True})["sessionId"]
    cdp.cmd("Runtime.enable", {}, ps)
    cdp.cmd("Log.enable", {}, ps)
    cdp.cmd("Page.enable", {}, ps)
    fim = time.time() + 20
    while time.time() < fim:
        try:
            if avaliar(cdp, ps, f"document.fonts.status === 'loaded' && !!document.querySelector({json.dumps(esperar)})", prazo=5):
                break
        except (RuntimeError, TimeoutError):
            pass
        cdp.bombear(0.1)
    return ps


def digitar(s, campo, valor):
    """Foco no campo e texto pelo `Input.insertText` (entrada do usuário, confiável)."""
    avaliar(cdp, s, f"document.getElementById({json.dumps(campo)}).focus(), true")
    cdp.cmd("Input.insertText", {"text": valor}, s)


def clicar(s, seletor, contexto=None):
    """Clique de mouse de verdade (`Input.dispatchMouseEvent`) no centro do elemento:
    dá a ativação do usuário à página."""
    r = avaliar(cdp, s, f"(() => {{ const b = document.querySelector({json.dumps(seletor)}).getBoundingClientRect();"
                " return [b.x + b.width / 2, b.y + b.height / 2]; })()", contexto=contexto)
    for tipo in ("mousePressed", "mouseReleased"):
        cdp.cmd("Input.dispatchMouseEvent", {"type": tipo, "x": r[0], "y": r[1], "button": "left", "clickCount": 1}, s)


def esperar_carga(s, cond, prazo=15):
    fim = time.time() + prazo
    while time.time() < fim:
        cdp.bombear(0.3)
        try:
            if avaliar(cdp, s, cond, prazo=5):
                return True
        except (RuntimeError, TimeoutError):
            pass
    return False


def pendente_do_popup(ps):
    """O que o service worker devolve ao popup sobre o pendente da aba ativa."""
    return avaliar(cdp, ps, """(async () => { const [a] = await chrome.tabs.query({active: true, currentWindow: true});
        try { const r = await chrome.runtime.sendMessage({t: 'pendente', aba: a.id}); return r === undefined ? null : r; }
        catch (e) { return 'erro'; } })()""")


def contexto_do_content_script(s):
    """O mundo isolado da extensão no quadro principal da aba (onde roda o `envio.js`)."""
    cdp.bombear(0.3)
    principal = cdp.cmd("Page.getFrameTree", {}, s)["frameTree"]["frame"]["id"]
    achado = None
    for e in cdp.eventos:
        if e.get("method") == "Runtime.executionContextCreated" and e.get("sessionId") == s:
            c = e["params"]["context"]
            aux = c.get("auxData", {})
            if c.get("origin", "").startswith(f"chrome-extension://{ID}") and aux.get("frameId") == principal \
                    and aux.get("type") == "isolated":
                achado = c["id"]
    return achado


GEOMETRIA_SALVAR = """(() => {
  const r = (e) => { if (!e) return null; const b = e.getBoundingClientRect();
    return [Math.round(b.x), Math.round(b.y + window.scrollY), Math.round(b.width), Math.round(b.height)]; };
  const q = (s) => document.querySelector(s);
  const campo = (id) => document.getElementById(id) ? document.getElementById(id).closest('.campo') : null;
  const cor = (e, p) => e ? getComputedStyle(e)[p] : null;
  return { vh: window.innerHeight, cabecalho: r(q('.cabecalho')), introducao: r(q('.introducao')),
    titulo: r(q('.titulo-salvar')), nome: r(campo('nome')), usuario: r(campo('usuario')),
    senha: r(q('.entrada-mascara').closest('.campo')), entradaNome: r(q('#nome')), entradaSenha: r(q('.entrada-mascara')),
    linha: r(q('.linha-dupla')), tipo: r(campo('tipo')), ambiente: r(campo('ambiente')), grupo: r(campo('grupo')),
    nivel: r(q('.nivel-inicial')), rodape: r(q('.rodape-salvar')), botoes: [...document.querySelectorAll('.rodape-salvar .botao')].map(r),
    textos: { titulo: q('.titulo-salvar').textContent, intro: q('.texto-salvar').textContent,
      nome: q('#nome').value, usuario: q('#usuario').value, grupo: q('#grupo').value, ambiente: q('#ambiente').value,
      tipo: q('#tipo').value, tipos: [...q('#tipo').options].map((o) => o.value), mascara: q('.mascara').textContent,
      nivel: q('.nivel-titulo').textContent, botoes: [...document.querySelectorAll('.rodape-salvar .botao')].map((b) => b.textContent) },
    cores: { fundo: cor(document.body, 'backgroundColor'), entrada: cor(q('#nome'), 'backgroundColor'),
      borda: cor(q('#nome'), 'borderTopColor'), nivel: cor(q('.nivel-inicial'), 'backgroundColor'),
      primario: cor(q('.botao.primario'), 'backgroundColor'), intro: cor(q('.texto-salvar'), 'color') },
    fonte: document.fonts.check('600 18px Inter') && document.fonts.check('500 13px Inter'),
    html: document.documentElement.outerHTML };
})()"""


def secao_fase_42(t, s):
    usuario = "novo." + secrets.token_hex(4)
    # Senha falsa gerada na hora (nunca escrita no repositório nem impressa).
    senha = "falso-" + secrets.token_urlsafe(18)
    novos.append(senha)

    # 1. Envio forjado pelo script da página (requestSubmit sem ativação): nada.
    navegar(t, "http://localhost:8081/ward-teste/envio-forjado.html")
    esperar_carga(s, "location.pathname === '/ward-teste/fim.html'")
    confere(popup_novo(3) is None, "ataque: requestSubmit do script da página (sem ativação do usuário) não oferece salvar", "f42_forjado_sem_popup")
    pt, ps = abrir_popup(t)
    confere(pendente_do_popup(ps) is None and avaliar(cdp, ps, "!document.querySelector('.popup-salvar')"),
            "ataque: nada pendente depois do envio forjado", "f42_forjado_sem_pendente")
    fechar_popup(pt)

    # 2. Login que o cofre já tem (acme-totp-dev), enviado pelo usuário: nada a salvar.
    navegar(t, "http://localhost:8083/login")
    ativar(t)
    digitar(s, "usuario", USUARIO["acme-totp-dev"])
    digitar(s, "senha", SENHA["acme-totp-dev"])
    clicar(s, "#login button")
    esperar_carga(s, "document.readyState === 'complete' && location.pathname === '/login' && !!document.getElementById('mensagem')")
    confere(popup_novo(3) is None, "login já cadastrado (acme-totp-dev): enviar não oferece salvar", "f42_conhecido_sem_popup")

    # 3. Formulário só num iframe de outra origem: o content script não roda nele.
    navegar(t, "http://localhost:8081/ward-teste/iframe-cadastro.html")
    cdp.bombear(1.0)
    filhos = cdp.cmd("Page.getFrameTree", {}, s)["frameTree"].get("childFrames", [])
    if filhos:
        ctx = cdp.cmd("Page.createIsolatedWorld", {"frameId": filhos[0]["frame"]["id"], "worldName": "conferencia"}, s)["executionContextId"]
        ativar(t)
        for campo, valor in (("usuario", "iframe." + secrets.token_hex(3)), ("senha", "falso-iframe-" + secrets.token_hex(6))):
            avaliar(cdp, s, f"document.getElementById({json.dumps(campo)}).focus(), true", contexto=ctx)
            cdp.cmd("Input.insertText", {"text": valor}, s)
        b = avaliar(cdp, s, "(() => { const b = document.querySelector('#cadastro button').getBoundingClientRect(); return [b.x + b.width/2, b.y + b.height/2]; })()", contexto=ctx)
        for tipo in ("mousePressed", "mouseReleased"):
            cdp.cmd("Input.dispatchMouseEvent", {"type": tipo, "x": b[0], "y": b[1] + 80, "button": "left", "clickCount": 1}, s)
        cdp.bombear(2.0)
    confere(filhos and popup_novo(3) is None, "ataque: envio dentro do iframe (outra origem) não oferece salvar", "f42_iframe_sem_popup")

    # 4. Cofre travado no envio: o login é descartado na hora (nada fica no service worker).
    confere(req({"v": 1, "op": "travar"}).get("tipo") == "ok", "cofre travado antes do envio")
    navegar(t, "http://localhost:8083/cadastro")
    ativar(t)
    digitar(s, "usuario", "travado." + secrets.token_hex(3))
    digitar(s, "senha", "falso-travado-" + secrets.token_hex(6))
    clicar(s, "#cadastro button")
    esperar_carga(s, "!!document.getElementById('mensagem')")
    confere(popup_novo(3) is None, "cofre travado no envio: não oferece salvar", "f42_travado_sem_popup")
    destravar()
    pt, ps = abrir_popup(t)
    confere(pendente_do_popup(ps) is None and avaliar(cdp, ps, "!document.querySelector('.popup-salvar')"),
            "cofre travado no envio: nada ficou pendente para depois do destravar", "f42_travado_descartado")
    # V6.9: sugestões com tema claro continuam escuras.
    confere(tema_continua_escuro(ps), "V6.9: sugestões do domínio continuam escuras com prefers-color-scheme: light", "v69_tema_sugestoes")
    fechar_popup(pt)

    # 5. V6.8: cadastro novo em localhost:8083, enviado pelo usuário.
    navegar(t, "http://localhost:8083/cadastro")
    ativar(t)
    digitar(s, "usuario", usuario)
    digitar(s, "senha", senha)
    clicar(s, "#cadastro button")
    pt = popup_novo(15)
    sozinho = pt is not None
    relato["f42_popup_abre_sozinho"] = sozinho
    print(f"      popup aberto pelo service worker (openPopup): {sozinho}")
    esperar_carga(s, "!!document.getElementById('mensagem')")
    conta = avaliar(cdp, s, "document.getElementById('mensagem').textContent")
    confere(conta == "Conta criada. Já pode entrar.", "o cadastro no localhost:8083 foi aceito pelo site")
    if pt is None:
        pt, ps = abrir_popup(t, ".popup-salvar")
    else:
        ps = anexar_popup(pt, ".popup-salvar")
    confere(sozinho, "V6.8: enviar o login novo abre o popup \"Salvar login novo\" sozinho", "v68_oferece")
    # A janela do popup do Chrome vai até 600 de altura (o frame 17414); no headless ela
    # nasce menor (não dá para mudar pelo CDP): o rodapé fica preso embaixo dela.
    g = avaliar(cdp, ps, GEOMETRIA_SALVAR)
    tx = g["textos"]
    confere(tx["titulo"] == "Salvar este login no Ward?" and tx["intro"] ==
            "O formulário de localhost:8083 foi enviado com um login que ainda não está no cofre.",
            "V6.8: título e texto do frame 17414 com o host da aba", "v68_textos")
    confere(tx["usuario"] == usuario and tx["nome"] == "localhost-8083" and tx["grupo"] == "localhost"
            and tx["ambiente"] == "dev" and tx["tipos"] == ["login"] and tx["tipo"] == "login",
            f"V6.8: pede tipo, grupo e ambiente, com sugestões do host ({tx['nome']}, {tx['grupo']}, {tx['ambiente']})", "v68_pede_campos")
    confere(tx["mascara"] == "••••••••••••" and senha not in g["html"] and not tem_segredo(g["html"]),
            "V6.8: a senha não chega ao popup (máscara fixa, DOM sem o valor)", "v68_dom_sem_senha")
    pend = pendente_do_popup(ps)
    confere(isinstance(pend, dict) and sorted(pend) == ["origem", "usuario"] and pend["origem"] == "http://localhost:8083"
            and senha not in json.dumps(pend), "V6.8: o service worker dá ao popup só a origem e o usuário", "v68_pendente_sem_senha")
    # Medidas do frame 17414 (360×600; corpo rola) e do 18850 (conteúdo inteiro, 806).
    esperado = {"cabecalho": [0, 0, 360, 57], "introducao": [16, 73, 328, 66], "nome": [16, 155, 328, 70],
                "entradaNome": [16, 181, 328, 44], "usuario": [16, 241, 328, 70], "senha": [16, 327, 328, 110],
                "entradaSenha": [16, 353, 328, 44], "linha": [16, 453, 328, 70], "tipo": [16, 453, 160, 70],
                "ambiente": [184, 453, 160, 70], "grupo": [16, 539, 328, 70], "nivel": [16, 625, 328, 88]}
    difs = [f"{k}: {g[k]} ≠ {v}" for k, v in esperado.items() if g[k] != v]
    # O rodapé fica preso embaixo da janela do popup (até 600); com a janela maior que o
    # conteúdo, fica depois do corpo, como no 18850 (y 729).
    y_rodape = min(g["vh"], 806) - 77
    if g["rodape"] != [0, y_rodape, 360, 77]:
        difs.append(f"rodape: {g['rodape']} ≠ {[0, y_rodape, 360, 77]} (janela {g['vh']})")
    if g["botoes"] != [[16, y_rodape + 17, 158, 44], [186, y_rodape + 17, 158, 44]]:
        difs.append(f"botoes: {g['botoes']}")
    confere(not difs, "medidas do popup iguais às dos frames 17414/18850 (Extensão / Salvar login novo)"
            + ("" if not difs else ": " + "; ".join(difs)), "medidas_17414")
    confere(g["cores"] == {"fundo": "rgb(16, 17, 20)", "entrada": "rgb(22, 24, 29)", "borda": "rgb(36, 40, 47)",
                           "nivel": "rgb(28, 31, 37)", "primario": "rgb(74, 108, 240)", "intro": "rgb(154, 161, 174)"}
            and g["fonte"], f"cores e fontes iguais às do FigCraft ({g['cores']})", "cores_17414")
    relato["f42_janela_popup"] = g["vh"]
    captura(ps, "salvar-login-novo-localhost-8083.png", g["vh"], EVID42, alem=False)
    # 18850: sem prender cabeçalho e rodapé, o rodapé vem depois do corpo (y 729).
    sem_prender(ps, True)
    g2 = avaliar(cdp, ps, GEOMETRIA_SALVAR)
    confere(g2["rodape"] == [0, 729, 360, 77] and g2["nivel"] == [16, 625, 328, 88]
            and avaliar(cdp, ps, "document.documentElement.scrollHeight") == 806,
            f"medidas do conteúdo inteiro iguais às do frame 18850 (360×806; rodapé {g2['rodape']})", "medidas_18850")
    sem_prender(ps, False)
    # O popup do headless não captura além da janela (repete o topo): a evidência do
    # resto do corpo é a janela rolada até o fim, com cabeçalho e rodapé presos.
    avaliar(cdp, ps, "window.scrollTo(0, document.documentElement.scrollHeight), true")
    cdp.bombear(0.3)
    captura(ps, "salvar-login-novo-rolado.png", g["vh"], EVID42, alem=False,
            y=avaliar(cdp, ps, "Math.round(window.scrollY)"))
    avaliar(cdp, ps, "window.scrollTo(0, 0), true")
    confere(tema_continua_escuro(ps), "V6.9: salvar login novo continua escuro com prefers-color-scheme: light", "v69_tema_salvar")
    captura(ps, "salvar-login-novo-tema-claro.png", g["vh"], EVID42, alem=False)

    # 6. Ataque: o content script (mundo isolado da aba) pede o pendente ou salva por
    # conta própria; o service worker não responde nada a quem tem aba.
    ctx = contexto_do_content_script(s)
    if ctx is not None:
        r = avaliar(cdp, s, """(async () => { const out = {};
            for (const m of [{t: 'pendente', aba: 0}, {t: 'salvar', aba: 0, nome: 'invasor', usuario: 'x', grupo: 'g', ambiente: 'dev'},
                             {t: 'descartar', aba: 0}]) {
              try { const r = await chrome.runtime.sendMessage(m); out[m.t] = r === undefined ? 'nada' : JSON.stringify(r); }
              catch (e) { out[m.t] = 'sem resposta'; } }
            return out; })()""", contexto=ctx)
    else:
        r = None
    confere(r is not None and all(v in ("nada", "sem resposta") for v in r.values()),
            f"ataque: mensagens do content script (pendente/salvar/descartar) ficam sem resposta ({r})", "f42_cs_sem_resposta")
    confere(pendente_do_popup(ps) is not None, "o pendente continua lá depois do ataque do content script (só o popup descarta)")

    # 7. Nome inválido: o daemon recusa e o popup mostra o motivo no campo.
    avaliar(cdp, ps, "(() => { const n = document.getElementById('nome'); n.value = 'Nome Inválido'; return true; })()")
    avaliar(cdp, ps, "document.querySelector('.botao.primario').click(), true")
    cdp.bombear(1.5)
    erro_nome = avaliar(cdp, ps, "(() => { const c = document.getElementById('nome').closest('.campo');"
                        " return c.classList.contains('com-erro') ? c.querySelector('.ajuda').textContent : ''; })()")
    confere(erro_nome.startswith("use letras minúsculas"), f"nome inválido: o motivo do daemon aparece no campo ({erro_nome})", "f42_erro_validacao")
    captura(ps, "salvar-login-novo-erro-nome.png", g["vh"], EVID42, alem=False)

    # 8. Salvar no Ward (nome, grupo e ambiente do usuário).
    avaliar(cdp, ps, """(() => { document.getElementById('nome').value = 'localhost-8083-novo';
        document.getElementById('grupo').value = 'teste-fase-42'; document.getElementById('ambiente').value = 'dev'; return true; })()""")
    avaliar(cdp, ps, "document.querySelector('.botao.primario').click(), true")
    confere(fechou(pt), "V6.8: \"Salvar no Ward\" grava e o popup fecha", "v68_salvou")
    p = subprocess.run([WARD, "lista", "--json"], capture_output=True, timeout=60,
                       env={"PATH": "/usr/bin:/bin", "HOME": TRAB, "WARD_DEV_SOCKET_DIR": SOCKDIR, "LANG": "C.UTF-8"})
    try:
        itens = json.loads(p.stdout)
        itens = itens.get("entradas", itens.get("itens", itens)) if isinstance(itens, dict) else itens
        item = next((i for i in itens if i.get("nome") == "localhost-8083-novo"), None)
    except (ValueError, AttributeError):
        item = None
    confere(p.returncode == 0 and item is not None and item.get("grupo") == "teste-fase-42" and item.get("ambiente") == "dev"
            and item.get("tipo") == "login" and "nivel" not in item and senha.encode() not in p.stdout,
            f"V6.8: a entrada aparece no `ward lista --json` (login, grupo e ambiente escolhidos; sem nível, D50) ({item})", "v68_ward_lista")
    # Nível "com aprovação": o uso pelo agente pede aprovação (sem o app no daemon de
    # teste, "app indisponível"); uma entrada livre seguiria sem perguntar.
    r = req({"v": 1, "op": "login", "dados": {"entrada": "localhost-8083-novo"}})
    codigo = (r.get("dados") or {}).get("codigo")
    confere(codigo == "aprovacao_indisponivel", f"V6.8: nível inicial \"com aprovação\" (o uso pelo agente pede aprovação: {codigo})", "v68_com_aprovacao")
    # O preenchimento da entrada nova devolve a senha enviada (comparada sem imprimir).
    navegar(t, "http://localhost:8083/login")
    pt, ps = abrir_popup(t)
    nomes = [x["nome"] for x in avaliar(cdp, ps, GEOMETRIA)["textos"]]
    confere(nomes == ["acme-totp-dev", "localhost-8083-novo"], f"a entrada nova é sugerida em localhost:8083 ({nomes})", "f42_sugerida")
    clicar_entrada(ps, "localhost-8083-novo")
    fechou(pt)
    v = valores(s, ["usuario", "senha"])
    confere(v["usuario"] == usuario and v["senha"] == senha, "a entrada nova preenche o usuário e a senha enviados", "f42_preenche")
    avaliar(cdp, s, "document.getElementById('usuario').value = ''; document.getElementById('senha').value = ''; true")

    # 9. "Agora não" descarta o pendente.
    navegar(t, "http://localhost:8083/cadastro")
    ativar(t)
    digitar(s, "usuario", "agora.nao." + secrets.token_hex(3))
    digitar(s, "senha", "falso-agora-nao-" + secrets.token_hex(6))
    clicar(s, "#cadastro button")
    pt = popup_novo(15)
    ps = anexar_popup(pt, ".popup-salvar") if pt else None
    if ps:
        avaliar(cdp, ps, "document.querySelector('.botao.secundario-botao').click(), true")
    confere(pt is not None and fechou(pt), "\"Agora não\" fecha o popup", "f42_agora_nao")
    pt, ps = abrir_popup(t)
    confere(pendente_do_popup(ps) is None, "\"Agora não\": nada fica pendente", "f42_agora_nao_descarta")
    fechar_popup(pt)


novos = []


try:
    # Extensão montada, pelo pipe, com o id fixo do manifesto.
    r = cdp.cmd("Extensions.loadUnpacked", {"path": EXT})
    confere(r.get("id") == ID, f"extensão carregada com o id fixo do manifesto ({ID})", "id_fixo")
    cdp.cmd("Target.setDiscoverTargets", {"discover": True})

    # --- V6.7: localhost:8081 ---------------------------------------------------------
    t, s = abrir_aba("http://localhost:8081/wp-login.php")
    pt, ps = abrir_popup(t)
    g = avaliar(cdp, ps, GEOMETRIA)
    nomes = [x["nome"] for x in g["textos"]]
    confere(nomes == ["acme-wp-dev", "acme-wp-prod"], "V6.7: em localhost:8081 aparecem acme-wp-dev e acme-wp-prod (só elas)", "v67_sugestoes_8081")
    confere(g["origemTexto"] == "http://localhost:8081", "popup mostra a origem da aba", "origem_mostrada")
    confere(g["rotuloTexto"] == "2 entradas para esta origem" and g["estadoTexto"] == "destravado",
            "rótulo da lista e estado do cabeçalho", "rotulo_estado")
    com_totp = lambda n: USUARIO[n] + (" · com TOTP" if ENTRADAS[n]["segredos"].get("totp_semente") else "")
    confere([x["amb"] for x in g["textos"]] == ["dev", "prod"] and g["textos"][0]["det"] == com_totp("acme-wp-dev")
            and g["textos"][1]["det"] == com_totp("acme-wp-prod"),
            f"ambiente e detalhe (usuário · com TOTP) de cada entrada: {[(x['amb'], x['det']) for x in g['textos']]}", "detalhes")
    confere(g["fonte"], "Inter e JetBrains Mono carregadas do pacote", "fontes")
    # Medidas do frame 17332 (360×494): cabeçalho 57; corpo com padding 16 e gap 12;
    # origem 328×70; rótulo em y 155; itens 328×76 em y 185 e 265 (gap 4); avatar 40;
    # nota 328×48 em y 353; rodapé em y 417 (77); botão 328×44 em (16, 434).
    esperado = {"popup": [0, 0, 360, 494], "cabecalho": [0, 0, 360, 57], "simbolo": [16, 16, 24, 24],
                "estado": [257, 18, 87, 20], "origem": [16, 73, 328, 70], "rotulo": [16, 155, 173, 18],
                "itens": [[16, 185, 328, 76], [16, 265, 328, 76]], "avatares": [[32, 203, 40, 40], [32, 283, 40, 40]],
                "nota": [16, 353, 328, 48], "rodape": [0, 417, 360, 77], "botao": [16, 434, 328, 44]}
    difs = []
    for k, v in esperado.items():
        real = g[k]
        # A largura de um texto que abraça o conteúdo depende do desenho da fonte: da
        # pílula de estado confere a borda direita (344), y e altura; do rótulo, x, y e altura.
        if k == "estado" and real:
            real = [v[0], real[1], v[2], real[3]] if real[0] + real[2] == v[0] + v[2] else real
        elif k == "rotulo" and real:
            real = [real[0], real[1], v[2], real[3]]
        if real != v:
            difs.append(f"{k}: {real} ≠ {v}")
    confere(not difs, "medidas do popup iguais às do frame 17332 (Extensão / Sugestões do domínio)" + ("" if not difs else ": " + "; ".join(difs)), "medidas_17332")
    confere(g["cores"]["fundo"] == "rgb(16, 17, 20)" and g["cores"]["nome"] == "rgb(247, 248, 250)"
            and g["cores"]["detalhe"] == "rgb(154, 161, 174)" and g["cores"]["estado"] == "rgb(76, 199, 126)",
            "cores do fundo, nome, detalhe e estado iguais às do FigCraft", "cores_17332")
    confere(not tem_segredo(g["html"]), "V6.5: DOM do popup sem segredo", "v65_dom_popup")
    arm = avaliar(cdp, ps, """(async () => ({ storage: typeof chrome.storage, local: localStorage.length,
        sessao: sessionStorage.length, idb: (await indexedDB.databases()).length,
        cookies: document.cookie }))()""")
    confere(arm == {"storage": "undefined", "local": 0, "sessao": 0, "idb": 0, "cookies": ""},
            "V6.5: sem chrome.storage (sem a permissão) e nada no armazenamento do popup", "v65_armazenamento")
    # Hover no primeiro item, como no frame (estado hover do Item de lista).
    cdp.cmd("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": 100, "y": 220}, ps)
    cdp.bombear(0.3)
    item0 = avaliar(cdp, ps, "getComputedStyle(document.querySelector('.item')).backgroundColor")
    confere(item0 == "rgb(36, 40, 47)", "hover do item com a superfície elevada (#24282F)", "hover")
    captura(ps, "sugestoes-do-dominio-localhost-8081.png", 494)
    # Clicar em acme-wp-prod preenche usuário e senha (e o popup fecha).
    clicar_entrada(ps, "acme-wp-prod")
    confere(fechou(pt), "o popup fecha depois de preencher (o contexto com a resposta some)", "popup_fecha")
    v = valores(s, ["user_login", "user_pass"])
    confere(v["user_login"] == USUARIO["acme-wp-prod"] and v["user_pass"] == SENHA["acme-wp-prod"],
            "V6.7: clicar em acme-wp-prod preenche usuário e senha em localhost:8081", "v67_preenche_8081")
    registrar_console(cdp)

    # --- V6.7: localhost:8083 com TOTP -------------------------------------------------
    navegar(t, "http://localhost:8083/login")
    pt, ps = abrir_popup(t)
    g = avaliar(cdp, ps, GEOMETRIA)
    confere([x["nome"] for x in g["textos"]] == ["acme-totp-dev"], "V6.7: em localhost:8083 só acme-totp-dev", "v67_sugestoes_8083")
    captura(ps, "sugestoes-do-dominio-localhost-8083.png", g["popup"][3])
    clicar_entrada(ps, "acme-totp-dev")
    fechou(pt)
    v = valores(s, ["usuario", "senha", "codigo"])
    confere(v["usuario"] == USUARIO["acme-totp-dev"] and v["senha"] == SENHA["acme-totp-dev"]
            and len(v["codigo"] or "") == 6 and v["codigo"].isdigit(),
            "V6.7: acme-totp-dev preenche usuário, senha e TOTP", "v67_totp")
    avaliar(cdp, s, "document.getElementById('login').requestSubmit(), true")
    fim = time.time() + 15
    entrou = False
    while time.time() < fim and not entrou:
        cdp.bombear(0.3)
        try:
            entrou = avaliar(cdp, s, "!!document.getElementById('painel')", prazo=5)
        except (RuntimeError, TimeoutError):
            entrou = False
    confere(entrou, "V6.7: o login no localhost:8083 entra (Painel)", "v67_login_entra")

    # --- V6.2: 127.0.0.1:8081 ----------------------------------------------------------
    navegar(t, "http://127.0.0.1:8081/wp-login.php")
    pt, ps = abrir_popup(t)
    g = avaliar(cdp, ps, GEOMETRIA)
    confere(g["itens"] == [] and g["rotuloTexto"] == "Nenhuma entrada para esta origem",
            "V6.2: em 127.0.0.1:8081, acme-wp-* não é sugerida", "v62_nao_sugere")
    captura(ps, "sugestoes-do-dominio-sem-entrada-127.png", g["popup"][3])
    confere(preencher_forcado(ps, "", "http://127.0.0.1:8081", "acme-wp-dev") == "erro:origem_diferente",
            "V6.2: preencher forçado para a aba 127.0.0.1:8081: o daemon recusa (origem_diferente)", "v62_daemon_recusa")
    # A outra camada: a injeção confere a origem do documento antes de escrever.
    res = avaliar(cdp, ps, """(async () => { const m = await import('./pagina.js');
        const [aba] = await chrome.tabs.query({active: true, currentWindow: true});
        const [r] = await chrome.scripting.executeScript({target: {tabId: aba.id, frameIds: [0]}, world: 'ISOLATED',
          func: m.preencherNaPagina, args: ['http://localhost:8081', 'x', btoa('nao-e-segredo'), '']});
        return r.result; })()""")
    v = valores(s, ["user_login", "user_pass"])
    confere(res == {"ok": False, "motivo": "origem"} and v == {"user_login": "", "user_pass": ""},
            "injeção com origem esperada diferente da do documento: nada escrito", "injecao_confere_origem")
    fechar_popup(pt)

    # --- phishing (8082) e redir (8084) ------------------------------------------------
    navegar(t, "http://localhost:8082/wp-login.php")
    pt, ps = abrir_popup(t)
    g = avaliar(cdp, ps, GEOMETRIA)
    confere(g["itens"] == [], "phishing localhost:8082: nada sugerido", "phish_nada")
    confere(preencher_forcado(ps, "", "http://localhost:8082", "acme-wp-prod") == "erro:origem_diferente",
            "phishing: preencher acme-wp-prod forçado com a origem da aba: recusado", "phish_recusa")
    fechar_popup(pt)
    final = navegar(t, "http://localhost:8084/login")
    confere(final.startswith("http://localhost:8082/"), "redir 8084 leva a aba ao phishing 8082")
    pt, ps = abrir_popup(t)
    g = avaliar(cdp, ps, GEOMETRIA)
    confere(g["itens"] == [] and g["origemTexto"] == "http://localhost:8082",
            "redir: o popup usa a origem final (8082), delta-redir não é sugerida", "redir_nada")
    confere(preencher_forcado(ps, "", "http://localhost:8082", "delta-redir") == "erro:origem_diferente",
            "redir: preencher delta-redir com a origem final: recusado (D9)", "redir_recusa")
    fechar_popup(pt)

    # --- aba que navega com o popup aberto ---------------------------------------------
    navegar(t, "http://localhost:8081/wp-login.php")
    pt, ps = abrir_popup(t)
    navegar(t, "http://localhost:8082/wp-login.php")
    clicar_entrada(ps, "acme-wp-prod")
    cdp.bombear(1.5)
    nota = avaliar(cdp, ps, "document.querySelector('.nota.erro') ? document.querySelector('.nota.erro').textContent : ''")
    v = valores(s, ["user_login", "user_pass"])
    confere(nota.startswith("A aba mudou") and v == {"user_login": "", "user_pass": ""},
            "aba que navegou para o phishing com o popup aberto: nada preenchido", "aba_navegou")
    fechar_popup(pt)

    # --- V6.4: iframe do phishing na origem certa ---------------------------------------
    cdp.cmd("Network.enable", {}, s)
    navegar(t, "http://localhost:8081/ward-teste/iframe.html")
    cdp.bombear(1.0)
    for e in cdp.eventos:
        if e.get("method") == "Network.loadingFailed":
            print("      falhou carga:", e["params"].get("errorText"), e["params"].get("blockedReason"), e["params"].get("corsErrorStatus"))
    pt, ps = abrir_popup(t)
    clicar_entrada(ps, "acme-wp-prod")
    fechou(pt)
    v = valores(s, ["user_login", "user_pass"])
    arvore = cdp.cmd("Page.getFrameTree", {}, s)["frameTree"]
    filhos = arvore.get("childFrames", [])
    print("      quadros:", json.dumps([{k: f["frame"].get(k) for k in ("url", "unreachableUrl", "securityOrigin")} for f in filhos]))
    ctx = None
    if filhos:
        ctx = cdp.cmd("Page.createIsolatedWorld", {"frameId": filhos[0]["frame"]["id"], "worldName": "conferencia"}, s)["executionContextId"]
    vi = valores(s, ["user_login", "user_pass"], contexto=ctx) if ctx else None
    url_iframe = avaliar(cdp, s, "location.href", contexto=ctx) if ctx else None
    confere(url_iframe == "http://localhost:8082/wp-login.php", f"V6.4: o iframe é o phishing carregado ({url_iframe})")
    confere(v["user_login"] == USUARIO["acme-wp-prod"] and v["user_pass"] == SENHA["acme-wp-prod"],
            "V6.4: o documento da origem certa recebe usuário e senha", "v64_principal")
    confere(vi == {"user_login": "", "user_pass": ""},
            f"V6.4: o iframe do phishing (localhost:8082) não recebe nada (iframe: {None if vi is None else {k: (None if x is None else bool(x)) for k, x in vi.items()}}, quadros filhos: {len(filhos)})",
            "v64_iframe_vazio")

    navegar(t, "http://localhost:8081/ward-teste/iframe-sem-campo.html")
    cdp.bombear(1.0)
    pt, ps = abrir_popup(t)
    clicar_entrada(ps, "acme-wp-prod")
    cdp.bombear(1.5)
    nota = avaliar(cdp, ps, "document.querySelector('.nota.erro') ? document.querySelector('.nota.erro').textContent : ''")
    filhos = cdp.cmd("Page.getFrameTree", {}, s)["frameTree"].get("childFrames", [])
    ctx = cdp.cmd("Page.createIsolatedWorld", {"frameId": filhos[0]["frame"]["id"], "worldName": "conferencia"}, s)["executionContextId"] if filhos else None
    vi = valores(s, ["user_login", "user_pass"], contexto=ctx) if ctx else None
    confere(nota.startswith("Não achei um campo de senha visível") and vi == {"user_login": "", "user_pass": ""},
            "V6.4: formulário só no iframe do phishing: nada preenchido", "v64_so_iframe")
    fechar_popup(pt)

    # --- DOM clobbering -----------------------------------------------------------------
    navegar(t, "http://localhost:8081/ward-teste/clobbering.html")
    pt, ps = abrir_popup(t)
    clicar_entrada(ps, "acme-wp-prod")
    fechou(pt)
    v = valores(s, ["user_login", "user_pass", "isca_pw"])
    chamadas = avaliar(cdp, s, "window.__chamadas.filter((c) => c !== 'get')")
    confere(v["user_login"] == USUARIO["acme-wp-prod"] and v["user_pass"] == SENHA["acme-wp-prod"] and v["isca_pw"] == "",
            "DOM clobbering: preenche os campos certos, a isca fica vazia", "clobbering_campos")
    confere(chamadas == [], "DOM clobbering: os protótipos trocados pela página não são chamados", "clobbering_prototipos")

    # --- campo de senha escondido -------------------------------------------------------
    navegar(t, "http://localhost:8081/ward-teste/escondido.html")
    pt, ps = abrir_popup(t)
    clicar_entrada(ps, "acme-wp-prod")
    fechou(pt)
    iscas = ["isca_user", "isca_none", "isca_vis", "isca_opac", "isca_pai", "isca_1px", "isca_fora",
             "isca_clip", "isca_coberta", "isca_desab"]
    v = valores(s, iscas + ["user_login", "user_pass"])
    cheias = [i for i in iscas if v[i]]
    confere(not cheias, "campo escondido: nenhuma das 10 iscas recebe valor" + (f" ({cheias})" if cheias else ""), "escondido_iscas")
    confere(v["user_login"] == USUARIO["acme-wp-prod"] and v["user_pass"] == SENHA["acme-wp-prod"],
            "campo escondido: só os campos visíveis recebem", "escondido_visiveis")
    navegar(t, "http://localhost:8081/ward-teste/so-escondido.html")
    pt, ps = abrir_popup(t)
    clicar_entrada(ps, "acme-wp-prod")
    cdp.bombear(1.5)
    nota = avaliar(cdp, ps, "document.querySelector('.nota.erro') ? document.querySelector('.nota.erro').textContent : ''")
    v = valores(s, ["isca_user", "isca_pw"])
    confere(nota.startswith("Não achei um campo de senha visível") and v == {"isca_user": "", "isca_pw": ""},
            "só campos escondidos: nada preenchido e o popup avisa", "so_escondido")
    captura(ps, "sugestoes-do-dominio-sem-campo.png", avaliar(cdp, ps, "Math.round(document.getElementById('popup').getBoundingClientRect().height)"))
    fechar_popup(pt)

    # --- V6.6: página que fala com a extensão ------------------------------------------
    navegar(t, "http://localhost:8081/ward-teste/mensagens.html")
    cdp.bombear(2.0)
    res = avaliar(cdp, s, "window.__res")
    v = valores(s, ["user_login", "user_pass"])
    confere(res.get("runtime") == "sem chrome.runtime" or res.get("resposta") in (None, "nada"),
            "V6.6: chrome.runtime.sendMessage(<id>) da página não chega a nada (sem externally_connectable)", "v66_runtime")
    confere(res.get("fetch") == "bloqueado" and res.get("fetch_manifesto") == "bloqueado"
            and res.get("iframe") != "leu",
            "V6.6: a página não lê o popup nem o manifesto (sem web_accessible_resources)", "v66_popup_ilegivel")
    confere(v == {"user_login": "", "user_pass": ""}, "V6.6: postMessage pedindo preencher: nada acontece", "v66_postmessage")
    relato["v66_detalhe"] = res

    # --- Fase 42: "Salvar login novo" (V6.8) e tema (V6.9) -------------------------------
    secao_fase_42(t, s)

    # --- cofre travado -------------------------------------------------------------------
    confere(req({"v": 1, "op": "travar"}).get("tipo") == "ok", "cofre travado pelo socket")
    navegar(t, "http://localhost:8081/wp-login.php")
    pt, ps = abrir_popup(t, ".corpo-travado")
    g = avaliar(cdp, ps, GEOMETRIA)
    confere(g["estadoTexto"] == "travado" and g["itens"] == [], "travado: o popup mostra o estado e nenhuma entrada", "travado")
    # Medidas do frame 17392 (360×343).
    tv = g["travado"]
    esperado = {"popup": [0, 0, 360, 343], "cabecalho": [0, 0, 360, 57]}
    difs = [f"{k}: {g[k]} ≠ {e}" for k, e in esperado.items() if g[k] != e]
    for k, e in {"avatar": [156, 81, 48, 48], "titulo": [24, 145, 312, 26], "texto": [24, 187, 312, 40],
                 "botao": [24, 243, 312, 44], "nota": [24, 303, 312, 16]}.items():
        if tv[k] != e:
            difs.append(f"{k}: {tv[k]} ≠ {e}")
    confere(not difs, "medidas do travado iguais às do frame 17392" + ("" if not difs else ": " + "; ".join(difs)), "medidas_17392")
    captura(ps, "cofre-travado-localhost-8081.png", 343)
    # V6.9: com o tema claro do sistema e do Chrome, continua escuro (D1).
    confere(tema_continua_escuro(ps), "V6.9: cofre travado continua escuro com prefers-color-scheme: light", "v69_tema_travado")
    captura(ps, "cofre-travado-tema-claro.png", 343, EVID42)
    # V6.9: o botão pede ao daemon para abrir o app (D57); o daemon de teste sem
    # WARD_DEV_APP responde "app indisponível", e o pedido aparece no log dele.
    # O pedido ao host é espiado no popup (embrulho de teste, com uma pausa para ler).
    avaliar(cdp, ps, """(() => { const orig = chrome.runtime.sendNativeMessage.bind(chrome.runtime);
        window.__ops = []; chrome.runtime.sendNativeMessage = async (h, m) => { window.__ops.push(m && m.op);
        await new Promise((r) => setTimeout(r, 1500)); return orig(h, m); };
        document.querySelector('.corpo-travado .botao').click(); return true; })()""")
    cdp.bombear(0.5)
    ops = avaliar(cdp, ps, "window.__ops")
    confere(ops == ["abrir_app"] and fechou(pt), f"V6.9: o botão do travado pede abrir_app ao daemon e o popup fecha ({ops})", "v69_botao_abre_app")
    destravar()

    registrar_console(cdp)
    # O perfil temporário inteiro sem segredo (nada guardado, V6.5).
    achou = []
    for raiz_p, _, nomes in os.walk(perfil):
        for n in nomes:
            p = os.path.join(raiz_p, n)
            try:
                if os.path.isfile(p) and tem_segredo(open(p, "rb").read()):
                    achou.append(os.path.relpath(p, perfil))
            except OSError:
                pass
    # Cache HTTP e histórico de formulário do Chrome guardam o que o site mandou e o que
    # o usuário enviou (o login do 8083); a extensão em si não grava nada.
    da_extensao = [a for a in achou if ID in a or "Extension" in a or "Local Storage" in a or "IndexedDB" in a]
    confere(not da_extensao, "V6.5: nada da extensão no perfil com segredo" + (f" ({da_extensao})" if da_extensao else ""), "v65_perfil")
    relato["perfil_com_padrao_fora_da_extensao"] = sorted(achou)
    # Fase 42: a senha do login novo também não fica em nada da extensão (o service
    # worker a tem só em memória até salvar).
    novas = []
    for raiz_p, _, nomes_p in os.walk(perfil):
        for n in nomes_p:
            q = os.path.join(raiz_p, n)
            rel = os.path.relpath(q, perfil)
            if ID in rel or "Extension" in rel or "Local Storage" in rel or "IndexedDB" in rel or "Service Worker" in rel:
                try:
                    dados_q = open(q, "rb").read()
                except OSError:
                    continue
                if any(x.encode() in dados_q for x in novos):
                    novas.append(rel)
    confere(novos and not novas, "V6.5/V6.8: a senha do login novo não fica em nada da extensão no perfil"
            + (f" ({novas})" if novas else ""), "f42_perfil_sem_senha_nova")
    for a in achou:
        # Diagnóstico sem valor: o texto legível em volta, com todo padrão trocado por ***.
        dados = open(os.path.join(perfil, a), "rb").read()
        for padrao in padroes:
            i = dados.find(padrao)
            if i >= 0:
                trecho = dados[max(0, i - 300): i + len(padrao) + 300]
                for q in padroes:
                    trecho = trecho.replace(q, b"***")
                legivel = "".join(chr(c) if 32 <= c < 127 else "." for c in trecho)
                print(f"      {a}: padrão em {i}: {legivel}")
                break
finally:
    try:
        cdp.cmd("Browser.close", prazo=10)
    except Exception:
        pass
    try:
        ch.wait(timeout=20)
    except subprocess.TimeoutExpired:
        ch.kill()
    console.close()

posts_depois = posts_phish()
confere(posts_antes is not None and posts_depois == posts_antes, "posts.log do phishing (8082) sem mudança", "posts_log")

# --- O host fora do Chrome fica agente (a exceção só vale com o Chrome instalado de pai) ---
env = {"PATH": "/usr/bin:/bin", "WARD_DEV_SOCKET_DIR": SOCKDIR, "HOME": TRAB}


def quadro(obj):
    b = json.dumps(obj).encode()
    return struct.pack("=I", len(b)) + b


entrada = quadro({"v": 1, "op": "estado"}) + quadro(
    {"v": 1, "op": "preencher", "dados": {"entrada": "acme-wp-prod", "origem": "http://localhost:8081"}})
p = subprocess.run([NM, f"chrome-extension://{ID}/"], input=entrada, capture_output=True, env=env, timeout=60)
confere(b"preenchimento" not in p.stdout and b"nao_autorizado" in p.stdout and not tem_segredo(p.stdout),
        "exceção de teste: o ward-nm aberto por outro processo continua agente", "excecao_so_chrome")
arq = os.path.join(TRAB, "falso.txt")
with open(arq, "w") as f:
    f.write(json.dumps({"v": 1, "op": "preencher", "dados": {"entrada": "acme-wp-prod", "origem": "http://localhost:8081"}}) + "\n")
p = subprocess.run([FALSO, NM, arq], capture_output=True, env=env, timeout=60)
confere(b"preenchimento" not in p.stdout and b"nao_autorizado" in p.stdout and not tem_segredo(p.stdout),
        "exceção de teste: o ward-nm filho de um Chrome falso (outro exe) continua agente", "excecao_chrome_falso")
p = subprocess.run([NM, "chrome-extension://" + "a" * 32 + "/"], input=entrada, capture_output=True, env=env, timeout=60)
confere(p.returncode == 2 and p.stdout == b"", "ward-nm recusa outra extensão (id diferente do do Ward)", "id_outra_extensao")

os.makedirs(EVID, exist_ok=True)
with open(os.path.join(EVID, "qa.json"), "w", encoding="utf-8") as f:
    json.dump({k: v for k, v in relato.items() if not k.startswith(("f42_", "v68_", "v69_", "medidas_17414", "medidas_18850", "cores_17414"))},
              f, ensure_ascii=False, indent=2, sort_keys=True)
    f.write("\n")
os.makedirs(EVID42, exist_ok=True)
with open(os.path.join(EVID42, "qa.json"), "w", encoding="utf-8") as f:
    json.dump({k: v for k, v in relato.items() if k.startswith(("f42_", "v68_", "v69_", "medidas_17414", "medidas_18850", "cores_17414"))},
              f, ensure_ascii=False, indent=2, sort_keys=True)
    f.write("\n")
if os.environ.get("WARD_TESTE_MANTER_PERFIL") != "1":
    shutil.rmtree(perfil, ignore_errors=True)
if falhas:
    print(f"extensao.py: {len(falhas)} conferência(s) falharam", file=sys.stderr)
    sys.exit(1)
