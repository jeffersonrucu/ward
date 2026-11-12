"""Fake Chrome of the `ward playwright` tests (Phase 44): speaks the DevTools Protocol
over the debugging pipe (reads the commands on fd 3, writes on fd 4, messages ended by
`\\0`), like Chrome's `--remote-debugging-pipe`, and never opens the network.

Usage: fake_chrome.py <folder> [Chrome argv...]. The behaviour comes from the mode file
in the folder (the names are the `MODE` comparisons below):

- `ok`: navigates, receives the fill, "sends" the POST (with the password in
  `postData`, as Chrome sends it in `Fetch.requestPaused`) and returns a session cookie;
- `redir`: the entry URL redirects to `http://localhost:8082/wp-login.php`;
- lying mode: the frame tree says the page is from another origin;
- post-elsewhere mode: the form submission goes to another origin;
- cookie and `localStorage` modes: the site keeps the password (base64) in a cookie
  or in `localStorage`;
- form-elsewhere mode: the fill code answers that the form submits to another origin;
- dying mode: exits right after the first command;
- large mode: sends 5 MiB without `\\0`;
- slow mode: waits 1.5 s before answering the cookies.

Records in the record file of the folder (one JSON line per event): argv, environment
names, open fds and the target of 0-2, and, from the fill, only the sha256 of each
argument (never the value).
"""

import hashlib
import json
import os
import sys
import time
from urllib.parse import urlsplit

FOLDER = sys.argv[1]
MODE = open(os.path.join(FOLDER, "modo")).read().strip()


def record(fields):
    with open(os.path.join(FOLDER, "registro"), "a") as f:
        f.write(json.dumps(fields) + "\n")


def origin(url):
    u = urlsplit(url)
    port = f":{u.port}" if u.port and u.port != {"http": 80, "https": 443}.get(u.scheme) else ""
    return f"{u.scheme}://{u.hostname}{port}"


fds = sorted(int(f) for f in os.listdir("/proc/self/fd"))
record({
    "evento": "inicio",
    "argv": sys.argv[2:],
    "ambiente": sorted(os.environ.keys()),
    "home": os.environ.get("HOME", ""),
    "cwd": os.getcwd(),
    "fds": fds,
    "alvos": {str(i): os.readlink(f"/proc/self/fd/{i}") for i in (0, 1, 2)},
})

buf = b""


def read_message():
    global buf
    while b"\0" not in buf:
        d = os.read(3, 65536)
        if not d:
            sys.exit(0)
        buf += d
    m, buf = buf.split(b"\0", 1)
    return json.loads(m)


def write_message(m):
    os.write(4, json.dumps(m).encode() + b"\0")


def reply(cmd, result):
    m = {"id": cmd["id"], "result": result}
    if "sessionId" in cmd:
        m["sessionId"] = cmd["sessionId"]
    write_message(m)


def event(method, params):
    write_message({"method": method, "params": params, "sessionId": "S1"})


pending = []
counter = [0]


def pause(url, method="GET", body=None):
    """Sends the `Fetch.requestPaused` of a main-frame document and waits for the
    daemon's decision. Returns True if it said to go on."""
    counter[0] += 1
    rid = f"r{counter[0]}"
    req = {"url": url, "method": method, "headers": {}}
    if body is not None:
        req["postData"] = body
        req["hasPostData"] = True
    event("Fetch.requestPaused", {"requestId": rid, "request": req, "frameId": "T1",
                                  "resourceType": "Document", "extra": [1.5, -2, 3e10]})
    while True:
        c = read_message()
        if c.get("method") in ("Fetch.continueRequest", "Fetch.failRequest") and \
                c.get("params", {}).get("requestId") == rid:
            reply(c, {})
            went_on = c["method"] == "Fetch.continueRequest"
            record({"evento": "decisao", "url": url, "seguiu": went_on})
            return went_on
        pending.append(c)


current_url = "about:blank"
credentials = {}


def next_command():
    if pending:
        return pending.pop(0)
    return read_message()


while True:
    c = next_command()
    m = c.get("method")
    p = c.get("params", {})
    record({"evento": "comando", "metodo": m})
    if MODE == "morre":
        sys.exit(3)
    if MODE == "grande":
        os.write(4, b"x" * (5 * 1024 * 1024))
        time.sleep(5)
        sys.exit(0)
    if m == "Target.createBrowserContext":
        reply(c, {"browserContextId": "C1"})
    elif m == "Target.createTarget":
        reply(c, {"targetId": "T1"})
    elif m == "Target.attachToTarget":
        reply(c, {"sessionId": "S1"})
    elif m == "Page.getFrameTree":
        url = current_url
        if MODE == "mente" and url != "about:blank":
            url = "http://localhost:8082/wp-login.php"
        reply(c, {"frameTree": {"frame": {"id": "T1", "url": url, "securityOrigin": origin(url) if url != "about:blank" else "://"}}})
    elif m == "Page.navigate":
        url = p["url"]
        if not pause(url):
            reply(c, {"frameId": "T1", "errorText": "net::ERR_BLOCKED_BY_CLIENT"})
            continue
        if MODE == "redir":
            target = "http://localhost:8082/wp-login.php"
            if not pause(target):
                reply(c, {"frameId": "T1", "errorText": "net::ERR_BLOCKED_BY_CLIENT"})
                continue
            url = target
        current_url = url
        reply(c, {"frameId": "T1", "loaderId": "L1"})
        event("Page.loadEventFired", {"timestamp": 1.5})
    elif m == "Page.createIsolatedWorld":
        reply(c, {"executionContextId": 7})
    elif m == "Runtime.callFunctionOn":
        decl = p.get("functionDeclaration", "")
        args = [a.get("value") for a in p.get("arguments", [])]
        # The parameter list of the daemon's fill script (a literal of `playwright.rs`).
        if "usuario, senha" in decl:
            record({"evento": "fill",
                    "sha256": [hashlib.sha256(a.encode()).hexdigest() if a is not None else None for a in args],
                    "origem": args[3] if len(args) > 3 else None,
                    "contexto": p.get("executionContextId")})
            credentials["senha"] = args[1]
            if MODE == "form_outra":
                reply(c, {"result": {"type": "string", "value": "origem"}})
                continue
            target = "http://localhost:8082/wp-login.php" if MODE == "pos_outra" else current_url
            reply(c, {"result": {"type": "string", "value": "ok"}})
            if pause(target, "POST", f"log=admin&pwd={args[1]}"):
                current_url = target
                event("Page.loadEventFired", {"timestamp": 2.5})
        else:
            ls = []
            if MODE == "ls_senha":
                import base64
                ls = [["tok", base64.b64encode(credentials.get("senha", "").encode()).decode()]]
            reply(c, {"result": {"type": "string", "value": json.dumps({"senha": False, "ls": ls})}})
    elif m == "Storage.getCookies":
        if MODE == "lento":
            time.sleep(1.5)
        value = "token-de-sessao-falso"
        if MODE == "cookie_senha":
            import base64
            value = base64.b64encode(credentials.get("senha", "").encode()).decode()
        reply(c, {"cookies": [{"name": "sessao", "value": value, "domain": "clientex.invalid",
                               "path": "/", "expires": -1, "size": 10, "httpOnly": True,
                               "secure": True, "session": True, "priority": "Medium"}]})
    else:
        reply(c, {})
