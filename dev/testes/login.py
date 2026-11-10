"""`make teste-login` (Fase 43): `ward login` de ponta a ponta, no Chrome real.

Chamado por `dev/testes/login.sh`, que sobe o `wardd` de teste só dele com
`WARD_DEV_EXTENSAO=chrome`. Argumentos: raiz, pasta de trabalho (em `.dev/`), pasta do
socket, `ward-nm` de teste, Chrome, lista de variações, a extensão montada e a CLI `ward`
de teste. Nunca imprime a lista nem um valor dela; compara os valores enviados com a
fixture por sha256, sem mostrá-los.

O Chrome roda headless com `--remote-debugging-pipe` (sem porta TCP) e a extensão entra
por `Extensions.loadUnpacked`. O service worker dela abre o `ward-nm` (native messaging,
manifesto só no perfil temporário) e espera o pedido do daemon. Nada aqui clica no ícone:
o `ward login` age sozinho na aba ativa. As páginas hostis da origem certa
(`http://localhost:8081/ward-teste/...`) são servidas pelo próprio CDP
(`Fetch.fulfillRequest`), que também vê o que cada envio mandou.
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

RAIZ, TRAB, SOCKDIR, NM, CHROME, LISTA, EXT, WARD = sys.argv[1:9]
CAMINHO = os.path.join(SOCKDIR, "wardd.sock")
EVID = os.path.join(RAIZ, "docs", "mission", "evidence", "fase-43")
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
    print("login.py: lista de variações curta demais; rode 'make dev-variacoes'", file=sys.stderr)
    sys.exit(1)


def tem_segredo(dados):
    if isinstance(dados, str):
        dados = dados.encode()
    return any(p in dados for p in padroes)


fixtures = json.load(open(f"{RAIZ}/dev/fixtures/entradas.json", encoding="utf-8"))
ENTRADAS = {e["nome"]: e for e in fixtures["entradas"]}
SENHA_PROD = ENTRADAS["acme-wp-prod"]["segredos"]["senha"]
HASH_PROD = hashlib.sha256(SENHA_PROD.encode()).hexdigest()
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


# --- Fase 44c: a janela de teste (`wardd-aprovador-teste`) ------------------------------
APROVADOR_LOG = os.path.join(TRAB, "logs", "wardd-aprovador-teste.log")
APROVADOR_MODO = os.path.join(TRAB, "aprovador.modo")


def modo_janela(m):
    """O que a janela de teste responde ao próximo pedido (`confirmar`, `negar`…)."""
    with open(APROVADOR_MODO, "w") as f:
        f.write(m + "\n")


def pedidos_da_janela():
    """As linhas `pedido …` que a janela de teste viu, em ordem."""
    try:
        with open(APROVADOR_LOG, encoding="utf-8", errors="replace") as f:
            return [l.strip() for l in f if l.startswith("pedido ")]
    except OSError:
        return []


def posts_phish():
    p = subprocess.run([f"{RAIZ}/dev/servicos/compose.sh", "exec", "-T", "phish", "cat", "/registro/posts.log"],
                       capture_output=True, timeout=120)
    if p.returncode == 0:
        return p.stdout
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
        self.n += 1
        m = {"id": self.n, "method": metodo, "params": params}
        if sessao:
            m["sessionId"] = sessao
        self.ignorar.add(self.n)
        os.write(self.w, json.dumps(m).encode() + b"\0")

    def _ler(self, prazo):
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

    def bombear(self, segundos=0.2):
        fim = time.time() + segundos
        while True:
            j = self._ler(fim)
            if j is None:
                return
            self.eventos.append(j)


console = open(os.path.join(TRAB, "console.log"), "w", encoding="utf-8")
cli_log = open(os.path.join(TRAB, "cli.log"), "wb")


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


def avaliar(cdp, sessao, expr, prazo=30):
    r = cdp.cmd("Runtime.evaluate", {"expression": expr, "awaitPromise": True, "returnByValue": True}, sessao, prazo)
    if "exceptionDetails" in r:
        raise RuntimeError("avaliar: " + json.dumps(r["exceptionDetails"])[:300])
    return r.get("result", {}).get("value")


# --- páginas da origem certa servidas pelo CDP ----------------------------------------
def form(acao="/ward-teste/fim.html", metodo="post", extra_form="", extra_campos="", botao='<button type="submit">Acessar</button>'):
    return (
        f'<form id="f" action="{acao}" method="{metodo}" {extra_form}>'
        '<input type="text" name="log" id="user_login" autocomplete="username">'
        '<input type="password" name="pwd" id="user_pass" autocomplete="current-password">'
        f"{extra_campos}{botao}</form>"
    )


PAGINAS = {
    # O site recusa: o POST volta à mesma página de login (campos vazios).
    "recusa.html": "<h1>Login</h1>" + form(acao="/ward-teste/recusa.html"),
    # Página que não navega (o envio é tratado por script e cancelado): depois do envio
    # os campos ficam vazios.
    "nao-navega.html": "<h1>Login por script</h1>" + form() + '<script src="/ward-teste/nao-navega.js"></script>',
    "nao-navega.js": (
        "window.__envios = 0;"
        "document.getElementById('f').addEventListener('submit', (e) => { window.__envios += 1; e.preventDefault(); });"
    ),
    "fim.html": "<h1>Entrou</h1>",
    # Ataques de formulário na origem certa: nada pode ir para fora.
    "fora.html": "<h1>Destino fora</h1>" + form(acao="http://localhost:8082/wp-login.php"),
    "formaction.html": "<h1>formaction fora</h1>" + form(
        botao='<button type="submit" formaction="http://localhost:8082/wp-login.php">Acessar</button>'),
    "get.html": "<h1>GET</h1>" + form(metodo="get"),
    "alvo.html": "<h1>target</h1>" + form(extra_form='target="_blank"'),
    "duas.html": "<h1>Duas senhas</h1>" + form(extra_campos='<input type="password" name="pwd2" id="pwd2">'),
    # `action` real para fora, tapado por um campo `name="action"` que diz a origem certa.
    "mascarado.html": "<h1>action mascarado</h1>" + form(
        acao="http://localhost:8082/wp-login.php",
        extra_campos='<input type="hidden" name="action" value="/ward-teste/fim.html">'),
}
envios = []  # (página, sha256 da senha enviada ou None)


def servir_paginas(cdp, sessao):
    cdp.cmd("Fetch.enable", {"patterns": [{"urlPattern": "http://localhost:8081/ward-teste/*", "requestStage": "Request"}]}, sessao)


def atender_pausa(e):
    p = e["params"]
    nome = p["request"]["url"].split("?")[0].rsplit("/", 1)[-1]
    if p["request"].get("method") == "POST":
        corpo = p["request"].get("postData", "")
        try:
            pwd = parse_qs(corpo).get("pwd", [None])[0]
        except ValueError:
            pwd = None
        envios.append((nome, hashlib.sha256(pwd.encode()).hexdigest() if pwd else None))
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


# --- Chrome -------------------------------------------------------------------------
perfil = os.path.join(TRAB, "perfil")
home = os.path.join(TRAB, "home")
hosts = os.path.join(perfil, "NativeMessagingHosts")
os.makedirs(hosts)
os.makedirs(home)
# Sem o gerenciador de senhas do próprio Chrome: ele guardaria a senha enviada e a
# preencheria sozinho nas páginas seguintes, o que confundiria o "nada preenchido" (ver
# o handoff da Fase 43).
os.makedirs(os.path.join(perfil, "Default"))
with open(os.path.join(perfil, "Default", "Preferences"), "w") as f:
    json.dump({"credentials_enable_service": False, "profile": {"password_manager_enabled": False},
               "autofill": {"enabled": False, "profile_enabled": False}}, f)
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
     "--force-device-scale-factor=1", "--window-size=1280,800", "about:blank"],
    preexec_fn=prep, pass_fds=(3, 4), env=chrome_env, stdout=subprocess.DEVNULL, stderr=log_chrome)
os.close(r_cmd)
os.close(w_res)
cdp = Cdp(w_cmd, r_res)
cdp.ao_pausar = atender_pausa
paginas = {}


def abrir_aba(url):
    t = cdp.cmd("Target.createTarget", {"url": "about:blank"})["targetId"]
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
        try:
            pronto = avaliar(cdp, s, "document.readyState === 'complete' && location.href", prazo=5)
        except (RuntimeError, TimeoutError):
            pronto = False
        if pronto and pronto != "about:blank":
            return pronto
    raise TimeoutError(url)


def ward_login(entrada, t):
    """`ward login <entrada>` com a aba `t` ativa; bombeia o CDP enquanto a CLI roda."""
    cdp.cmd("Target.activateTarget", {"targetId": t})
    env = {"HOME": home, "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8", "WARD_DEV_SOCKET_DIR": SOCKDIR,
           "XDG_RUNTIME_DIR": os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")}
    p = subprocess.Popen([WARD, "login", entrada], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, env=env)
    fim = time.time() + 120
    while p.poll() is None and time.time() < fim:
        cdp.bombear(0.2)
    if p.poll() is None:
        p.kill()
    out, err = p.communicate()
    cli_log.write(b"$ ward login " + entrada.encode() + b"\n" + out + err)
    cli_log.flush()
    # A página seguinte termina de carregar.
    cdp.bombear(1.0)
    registrar_console(cdp)
    return p.returncode, out.decode(errors="replace"), err.decode(errors="replace")


def valores(s):
    try:
        return avaliar(cdp, s, "(() => { const u = document.getElementById('user_login'), p = document.getElementById('user_pass');"
                               " return {u: u ? u.value : null, p: p ? p.value : null, url: location.href}; })()", prazo=10)
    except (RuntimeError, TimeoutError):
        return None


def ward_nm_vivo():
    """Um `ward-nm` filho do Chrome (o service worker abriu a porta)."""
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
    confere(r.get("id") == ID, f"extensão carregada com o id fixo ({ID})")
    cdp.cmd("Target.setDiscoverTargets", {"discover": True})
    fim = time.time() + 20
    while time.time() < fim and not ward_nm_vivo():
        cdp.bombear(0.3)
    confere(ward_nm_vivo(), "o service worker abriu o ward-nm (filho direto do Chrome) e espera o pedido", "porta_aberta")

    # --- Fase 44c: o ataque `[::1]` da auditoria do M6 --------------------------------------
    # Um processo do uid escuta em [::1]:8081 (o WordPress está só em 127.0.0.1:8081). O
    # Chrome do usuário carrega o falso com a origem http://localhost:8081, a da entrada.
    # Sem a confirmação, nada sai; o pedido mostra a origem. Fica antes de toda outra
    # visita a localhost:8081: depois de uma conexão a 127.0.0.1, o Chrome reaproveita o
    # endereço e nem tenta o [::1].
    saida_falso = os.path.join(TRAB, "falso-v6.bin")
    falso = subprocess.Popen(["/usr/bin/python3", "-I", "-B", os.path.join(RAIZ, "dev", "testes", "falso_v6.py"),
                              "8081", saida_falso], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                             stderr=subprocess.DEVNULL)
    try:
        fim = time.time() + 10
        while time.time() < fim:
            try:
                socket.create_connection(("::1", 8081), 1).close()
                break
            except OSError:
                time.sleep(0.1)
        t, s = abrir_aba("http://localhost:8081/wp-login.php")
        titulo = avaliar(cdp, s, "document.title", prazo=10)
        confere(titulo == "falso-v6", f"ataque [::1]: a aba em localhost:8081 caiu no falso de [::1] (título {titulo!r})",
                "c44_v6_aba_no_falso")
        for m in ("negar",):
            modo_janela(m)
            vistos = len(pedidos_da_janela())
            codigo, out, err = ward_login("acme-wp-prod", t)
            novos = pedidos_da_janela()[vistos:]
            v = valores(s)
            confere(codigo != 0 and v is not None and not v["u"] and not v["p"],
                    f"ataque [::1], janela {m}: `ward login acme-wp-prod` sai ≠ 0 e nada é preenchido (saiu {codigo};"
                    f" {err.strip()[:80]})", f"c44_v6_{m}_nada")
            confere(novos == [f"pedido operacao=login entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao={m}"],
                    f"ataque [::1], janela {m}: o pedido mostra a origem ({novos})", f"c44_v6_{m}_origem")
        try:
            recebido = os.path.getsize(saida_falso)
        except FileNotFoundError:
            recebido = 0
        confere(recebido == 0, f"ataque [::1]: o falso não recebeu nenhum POST ({recebido} bytes)", "c44_v6_falso_vazio")
    finally:
        falso.terminate()
        falso.wait(timeout=10)
        modo_janela("confirmar")

    # --- V6.11 (lado do daemon): com aprovação e negado na janela, nada é preenchido ---------
    modo_janela("negar")
    t, s = abrir_aba("http://localhost:8081/wp-login.php")
    vistos = len(pedidos_da_janela())
    codigo, out, err = ward_login("acme-wp-dev", t)
    v = valores(s)
    confere(codigo != 0 and v is not None and not v["u"] and not v["p"],
            f"V6.11: acme-wp-dev (com aprovação) negado na janela: sai ≠ 0 e nada é preenchido (saiu {codigo};"
            f" {err.strip()[:100]}; campos {None if v is None else (bool(v['u']), bool(v['p']), v['url'][:60])})",
            "v611_sem_aprovacao")
    # 44b: a mensagem real do `ward login` negado (vai ao roteiro do usuário do M6).
    print(f"info  mensagem do `ward login` negado: {err.strip()[:120]!r}")
    relato["v611_mensagem_negado"] = err.strip()[:120]
    novos = pedidos_da_janela()[vistos:]
    confere(novos == ["pedido operacao=login entradas=acme-wp-dev origem=http://localhost:8081 confirmacao=0 decisao=negar"],
            f"Fase 44c: o pedido de acme-wp-dev (com aprovação) mostra a origem http e pede o PIN ({novos})",
            "c44_com_aprovacao_origem")
    modo_janela("confirmar")

    # --- V6.10: login de verdade no WordPress -----------------------------------------
    t, s = abrir_aba("http://localhost:8081/wp-login.php")
    vistos = len(pedidos_da_janela())
    codigo, out, err = ward_login("acme-wp-prod", t)
    confere(codigo == 0, f"V6.10: `ward login acme-wp-prod` sai 0 (saiu {codigo}: {err.strip()[:120]})", "v610_sai_0")
    novos = pedidos_da_janela()[vistos:]
    confere(novos == ["pedido operacao=login entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao=confirmar"],
            f"Fase 44c: acme-wp-prod (livre, http) só depois da confirmação da origem, sem PIN ({novos})",
            "c44_livre_confirmacao")
    confere(not tem_segredo(out + err), "V6.10: a saída da CLI não tem padrão de .dev/variacoes.txt", "v610_saida_sem_segredo")
    fim = time.time() + 15
    painel = False
    while time.time() < fim and not painel:
        cdp.bombear(0.3)
        try:
            painel = bool(avaliar(cdp, s, "location.pathname.startsWith('/wp-admin') && document.readyState === 'complete'"
                                          " && !!document.getElementById('wpadminbar')", prazo=5))
        except (RuntimeError, TimeoutError):
            painel = False
    confere(painel, "V6.10: a aba chega ao painel do WordPress logado (/wp-admin/ com a barra de admin)", "v610_painel")
    os.makedirs(EVID, exist_ok=True)
    try:
        png = cdp.cmd("Page.captureScreenshot", {"format": "png"}, s)["data"]
        with open(os.path.join(EVID, "v6-10-painel-wordpress.png"), "wb") as f:
            f.write(base64.b64decode(png))
    except (RuntimeError, TimeoutError):
        pass

    # --- V6.10: o site recusa; os campos ficam vazios ------------------------------------
    n_envios = len(envios)
    t, s = abrir_aba("http://localhost:8081/ward-teste/recusa.html")
    codigo, out, err = ward_login("acme-wp-prod", t)
    enviado = [h for (n, h) in envios[n_envios:] if n == "recusa.html"]
    confere(enviado == [HASH_PROD], "V6.10: o formulário foi enviado uma vez, à origem certa, com a senha da entrada", "v610_recusa_enviado")
    confere(codigo != 0 and "não aceitou o login" in err,
            f"V6.10: site que recusa o login: a CLI sai ≠ 0 dizendo isso (saiu {codigo})", "v610_recusa_sai")
    v = valores(s)
    confere(v is not None and v["u"] == "" and v["p"] == "", "V6.10: depois do envio recusado os campos estão vazios", "v610_recusa_vazios")

    t, s = abrir_aba("http://localhost:8081/ward-teste/nao-navega.html")
    codigo, out, err = ward_login("acme-wp-prod", t)
    v = valores(s)
    envios_script = avaliar(cdp, s, "window.__envios", prazo=10)
    confere(envios_script == 1, "página que trata o envio por script: o envio aconteceu (submit)", "nao_navega_enviou")
    confere(v is not None and v["u"] == "" and v["p"] == "",
            "página que não navega: depois do envio os campos ficam vazios (no mesmo documento)", "nao_navega_vazios")
    confere(codigo != 0, f"página que ainda pede a senha: a CLI sai ≠ 0 (saiu {codigo})", "nao_navega_sai")

    # --- V6.3: aba no phishing ------------------------------------------------------------
    t, s = abrir_aba("http://localhost:8082/wp-login.php")
    codigo, out, err = ward_login("acme-wp-prod", t)
    confere(codigo != 0 and "domínio não confere" in err,
            f"V6.3: aba no phishing (8082): sai ≠ 0 com \"domínio não confere\" (saiu {codigo})", "v63_dominio")
    v = valores(s)
    confere(v is not None and not v["u"] and not v["p"], "V6.3: nada preenchido na página do phishing", "v63_nada_preenchido")

    # --- V6.15: delta-redir (8084 → 8082) ---------------------------------------------------
    t, s = abrir_aba("http://localhost:8084/login")
    url_final = avaliar(cdp, s, "location.origin", prazo=10)
    confere(url_final == "http://localhost:8082", "V6.15: a aba do 8084 terminou no phishing (8082)")
    codigo, out, err = ward_login("delta-redir", t)
    confere(codigo != 0 and "domínio não confere" in err,
            f"V6.15: `ward login delta-redir` recusa por origem (saiu {codigo})", "v615_dominio")
    v = valores(s)
    confere(v is not None and not v["u"] and not v["p"], "V6.15: nada preenchido", "v615_nada_preenchido")

    # --- Ataques de formulário na origem certa ----------------------------------------------
    for pagina, rotulo in [
        ("fora.html", "action para o phishing"),
        ("formaction.html", "formaction do botão para o phishing"),
        ("get.html", "formulário GET"),
        ("alvo.html", "target=_blank"),
        ("duas.html", "dois campos de senha"),
        ("mascarado.html", "action para fora tapado por <input name=action>"),
    ]:
        n_envios = len(envios)
        t, s = abrir_aba(f"http://localhost:8081/ward-teste/{pagina}")
        codigo, out, err = ward_login("acme-wp-prod", t)
        v = valores(s)
        confere(codigo != 0 and "formulário de login seguro" in err,
                f"ataque {rotulo}: a CLI sai ≠ 0 (formulário recusado; saiu {codigo})", f"ataque_{pagina}")
        confere(v is not None and not v["u"] and not v["p"] and len(envios) == n_envios
                and v["url"].endswith(pagina),
                f"ataque {rotulo}: nada preenchido, nada enviado, a aba não saiu da página", f"ataque_{pagina}_nada")

    # --- Bloqueada e tipo errado ------------------------------------------------------------
    codigo, out, err = ward_login("gama-bloqueada", t)
    confere(codigo != 0 and "entrada inexistente" in err, "gama-bloqueada: \"entrada inexistente\" (V3.17)", "bloqueada")
    codigo, out, err = ward_login("acme-pg-dev", t)
    confere(codigo != 0, "acme-pg-dev (banco): recusado", "tipo_errado")

    # --- O phishing não recebeu nada -----------------------------------------------------------
    posts_depois = posts_phish()
    confere(posts_antes is not None and posts_depois == posts_antes, "posts.log do phishing (8082) sem mudança", "posts_log")

    # --- Nada da extensão guardado no perfil ----------------------------------------------------
    registrar_console(cdp)
    achados = []
    for raiz_p, _, nomes in os.walk(perfil):
        for n in nomes:
            caminho = os.path.join(raiz_p, n)
            rel = os.path.relpath(caminho, perfil)
            if ID not in rel and "Service Worker" not in rel and "Extension" not in rel:
                continue
            try:
                with open(caminho, "rb") as f:
                    if tem_segredo(f.read()):
                        achados.append(rel)
            except OSError:
                pass
    confere(not achados, f"perfil: nada da extensão com padrão de .dev/variacoes.txt ({len(achados)} arquivo(s))", "perfil_limpo")
finally:
    try:
        cdp.cmd("Browser.close", prazo=5)
    except Exception:
        pass
    try:
        ch.wait(timeout=10)
    except subprocess.TimeoutExpired:
        ch.kill()
    console.close()
    cli_log.close()
    os.makedirs(EVID, exist_ok=True)
    with open(os.path.join(EVID, "qa.json"), "w", encoding="utf-8") as f:
        json.dump(relato, f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write("\n")

if falhas:
    print(f"login.py: {len(falhas)} falha(s)", file=sys.stderr)
    sys.exit(1)
print("login.py: tudo certo")
