"""Chrome falso dos testes do `ward playwright` (Fase 44): fala o DevTools Protocol pelo
pipe de depuração (lê os comandos no fd 3, escreve no fd 4, mensagens terminadas em
`\\0`), como o `--remote-debugging-pipe` do Chrome, e nunca abre rede.

Uso: chrome_falso.py <pasta> [argv do Chrome…]. O comportamento vem de `<pasta>/modo`:

- `ok`: navega, recebe o preenchimento, "envia" o POST (com a senha no `postData`, como
  o Chrome manda no `Fetch.requestPaused`) e devolve um cookie de sessão;
- `redir`: a URL da entrada redireciona para `http://localhost:8082/wp-login.php`;
- `mente`: a árvore de quadros diz que a página é de outra origem;
- `pos_outra`: o envio do formulário vai para outra origem;
- `cookie_senha` / `ls_senha`: o site guarda a senha (base64) num cookie ou no
  `localStorage`;
- `form_outra`: o código de preencher responde que o formulário envia para outra origem;
- `morre`: sai logo depois do primeiro comando;
- `grande`: manda 5 MiB sem `\\0`;
- `lento`: espera 1,5 s antes de responder os cookies.

Registra em `<pasta>/registro` (uma linha JSON por evento): argv, nomes do ambiente,
fds abertos e o alvo de 0–2, e, do preenchimento, só o sha256 de cada argumento (nunca
o valor).
"""

import hashlib
import json
import os
import sys
import time
from urllib.parse import urlsplit

PASTA = sys.argv[1]
MODO = open(os.path.join(PASTA, "modo")).read().strip()


def registrar(**kw):
    with open(os.path.join(PASTA, "registro"), "a") as f:
        f.write(json.dumps(kw) + "\n")


def origem(url):
    u = urlsplit(url)
    porta = f":{u.port}" if u.port and u.port != {"http": 80, "https": 443}.get(u.scheme) else ""
    return f"{u.scheme}://{u.hostname}{porta}"


fds = sorted(int(f) for f in os.listdir("/proc/self/fd"))
registrar(
    evento="inicio",
    argv=sys.argv[2:],
    ambiente=sorted(os.environ.keys()),
    home=os.environ.get("HOME", ""),
    cwd=os.getcwd(),
    fds=fds,
    alvos={str(i): os.readlink(f"/proc/self/fd/{i}") for i in (0, 1, 2)},
)

buf = b""


def ler():
    global buf
    while b"\0" not in buf:
        d = os.read(3, 65536)
        if not d:
            sys.exit(0)
        buf += d
    m, buf = buf.split(b"\0", 1)
    return json.loads(m)


def escrever(m):
    os.write(4, json.dumps(m).encode() + b"\0")


def responder(cmd, result):
    m = {"id": cmd["id"], "result": result}
    if "sessionId" in cmd:
        m["sessionId"] = cmd["sessionId"]
    escrever(m)


def evento(metodo, params):
    escrever({"method": metodo, "params": params, "sessionId": "S1"})


pendentes = []
contador = [0]


def pausar(url, metodo="GET", corpo=None):
    """Manda o `Fetch.requestPaused` de um documento do quadro principal e espera a
    decisão do daemon. Devolve True se ele mandou seguir."""
    contador[0] += 1
    rid = f"r{contador[0]}"
    req = {"url": url, "method": metodo, "headers": {}}
    if corpo is not None:
        req["postData"] = corpo
        req["hasPostData"] = True
    evento("Fetch.requestPaused", {"requestId": rid, "request": req, "frameId": "T1",
                                   "resourceType": "Document", "extra": [1.5, -2, 3e10]})
    while True:
        c = ler()
        if c.get("method") in ("Fetch.continueRequest", "Fetch.failRequest") and \
                c.get("params", {}).get("requestId") == rid:
            responder(c, {})
            seguiu = c["method"] == "Fetch.continueRequest"
            registrar(evento="decisao", url=url, seguiu=seguiu)
            return seguiu
        pendentes.append(c)


url_atual = "about:blank"
usuario_senha = {}


def proximo():
    if pendentes:
        return pendentes.pop(0)
    return ler()


while True:
    c = proximo()
    m = c.get("method")
    p = c.get("params", {})
    registrar(evento="comando", metodo=m)
    if MODO == "morre":
        sys.exit(3)
    if MODO == "grande":
        os.write(4, b"x" * (5 * 1024 * 1024))
        time.sleep(5)
        sys.exit(0)
    if m == "Target.createBrowserContext":
        responder(c, {"browserContextId": "C1"})
    elif m == "Target.createTarget":
        responder(c, {"targetId": "T1"})
    elif m == "Target.attachToTarget":
        responder(c, {"sessionId": "S1"})
    elif m == "Page.getFrameTree":
        url = url_atual
        if MODO == "mente" and url != "about:blank":
            url = "http://localhost:8082/wp-login.php"
        responder(c, {"frameTree": {"frame": {"id": "T1", "url": url, "securityOrigin": origem(url) if url != "about:blank" else "://"}}})
    elif m == "Page.navigate":
        url = p["url"]
        if not pausar(url):
            responder(c, {"frameId": "T1", "errorText": "net::ERR_BLOCKED_BY_CLIENT"})
            continue
        if MODO == "redir":
            destino = "http://localhost:8082/wp-login.php"
            if not pausar(destino):
                responder(c, {"frameId": "T1", "errorText": "net::ERR_BLOCKED_BY_CLIENT"})
                continue
            url = destino
        url_atual = url
        responder(c, {"frameId": "T1", "loaderId": "L1"})
        evento("Page.loadEventFired", {"timestamp": 1.5})
    elif m == "Page.createIsolatedWorld":
        responder(c, {"executionContextId": 7})
    elif m == "Runtime.callFunctionOn":
        decl = p.get("functionDeclaration", "")
        args = [a.get("value") for a in p.get("arguments", [])]
        if "usuario, senha" in decl:
            registrar(evento="preencher",
                      sha256=[hashlib.sha256(a.encode()).hexdigest() if a is not None else None for a in args],
                      origem=args[3] if len(args) > 3 else None,
                      contexto=p.get("executionContextId"))
            usuario_senha["senha"] = args[1]
            if MODO == "form_outra":
                responder(c, {"result": {"type": "string", "value": "origem"}})
                continue
            alvo = "http://localhost:8082/wp-login.php" if MODO == "pos_outra" else url_atual
            responder(c, {"result": {"type": "string", "value": "ok"}})
            if pausar(alvo, "POST", f"log=admin&pwd={args[1]}"):
                url_atual = alvo
                evento("Page.loadEventFired", {"timestamp": 2.5})
        else:
            ls = []
            if MODO == "ls_senha":
                import base64
                ls = [["tok", base64.b64encode(usuario_senha.get("senha", "").encode()).decode()]]
            responder(c, {"result": {"type": "string", "value": json.dumps({"senha": False, "ls": ls})}})
    elif m == "Storage.getCookies":
        if MODO == "lento":
            time.sleep(1.5)
        valor = "token-de-sessao-falso"
        if MODO == "cookie_senha":
            import base64
            valor = base64.b64encode(usuario_senha.get("senha", "").encode()).decode()
        responder(c, {"cookies": [{"name": "sessao", "value": valor, "domain": "clientex.invalid",
                                   "path": "/", "expires": -1, "size": 10, "httpOnly": True,
                                   "secure": True, "session": True, "priority": "Medium"}]})
    else:
        responder(c, {})
