#!/usr/bin/python3 -IB
"""Phase 64 (mission 2, Battery D) helper: talks to a test `wardd` 0.2.0 (protocol v2)
or 0.1.4 (protocol v1) on its own socket. Never prints a secret: every revealed value
becomes its sha256 inside this process, and the password and the PIN come from 0600
files written by the harness (fake values of D15).

Without arguments it is the app (WARD_DEV_APP of the test daemon; the daemon gives the
app role to the process it opened): reads `$WARD_DEV_DIR/f64-step` (`dump`, `pin` or
`restore <version>`), `$WARD_DEV_DIR/f64-proto` (1 or 2), `f64-pw`, `f64-pin`, and
writes `$WARD_DEV_DIR/f64-out-<step>.json` (0600, by rename).

Subcommands (as agent, from the harness):
  agent <1|2> <socket> <status|lock|list|open_app|unlock <pwfile>>
      prints `<type> <code>` of the reply (never a value);
  compare <dump 0.1.4> <dump 0.2.0> [--same-proto]
      prints one line per difference and `iguais <n>` at the end; exits 1 if any;
  pty <out> <timeout> <pwfile|-> -- <argv...>
      runs argv in a pty; with a pwfile, types the password only after the prompt
      "Senha mestra"; writes the screen to <out> with the password scrubbed; prints
      `exit=<n> prompt=<0|1>`.
"""

import base64
import hashlib
import hmac
import json
import os
import pty
import select
import socket
import struct
import sys
import time

LIMIT = 1 << 20

V1_OPS = {
    "status": "estado", "unlock": "destravar", "lock": "travar", "list": "lista",
    "detail": "detalhe", "reveal": "revelar", "totp": "totp", "config": "configuracao",
    "backups": "backups", "change_pin": "trocar_pin", "restore_backup": "restaurar_backup",
    "open_app": "abrir_app",
}
V1_KEYS = {
    "entry": "entrada", "field": "campo", "extra": "extra", "password": "senha",
    "current": "atual", "new": "novo", "version": "versao", "pin": "pin",
}
# Positional tables of the two versions (0.1.4 resposta.rs / pedido.rs; 0.2.0
# response.rs / request.rs).
TEXT_V1 = ["url", "usuario", "motor", "host", "porta", "base", "tunel", "salto", "variavel",
           "access_key_id", "regiao", "role_arn", "seguranca", "protocolo"]
TEXT_V2 = ["url", "username", "engine", "host", "port", "db_name", "tunnel", "jump", "variable",
           "access_key_id", "region", "role_arn", "security", "protocol"]
SECRET_V1 = ["senha", "totp_semente", "chave_privada", "passphrase", "token",
             "secret_access_key", "session_token", "texto"]
SECRET_V2 = ["password", "totp_seed", "private_key", "passphrase", "token",
             "secret_access_key", "session_token", "text"]
# m2:DT4 (types and levels), written by hand like crates/ward-core/src/wire_v1.rs.
NAMES_V4_V5 = {
    "login": "login", "banco": "database", "servidor": "server", "api": "api", "aws": "aws",
    "ftp-sftp": "ftp-sftp", "smtp": "smtp", "nota-segura": "secure-note",
    "generico": "generic", "bloqueado": "blocked", "com-aprovacao": "approval",
    "livre": "free",
}
SECURITY_V4_V5 = {"nenhuma": "none", "starttls": "starttls", "tls": "tls"}


def read_exact(s, n):
    parts = []
    while n > 0:
        b = s.recv(n)
        if not b:
            raise ConnectionError("fechou")
        parts.append(b)
        n -= len(b)
    return b"".join(parts)


class Client:
    def __init__(self, proto, path):
        self.proto, self.path = proto, path

    def call(self, op, data=None):
        data = data or {}
        if self.proto == 1:
            frame = {"v": 1, "op": V1_OPS[op], "dados": {V1_KEYS.get(k, k): v for k, v in data.items()}}
        else:
            frame = {"v": 2, "op": op, "data": data}
        body = json.dumps(frame, ensure_ascii=False, separators=(",", ":")).encode()
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(60)
            s.connect(self.path)
            s.sendall(struct.pack(">I", len(body)) + body)
            (n,) = struct.unpack(">I", read_exact(s, 4))
            if n == 0 or n > LIMIT:
                raise ConnectionError("quadro fora do limite")
            reply = json.loads(read_exact(s, n))
        if reply.get("v") == 1:
            return reply.get("tipo"), reply.get("dados") or {}
        return reply.get("type"), reply.get("data") or {}


def code_of(kind, data):
    if kind in ("error", "erro"):
        return data.get("code") or data.get("codigo") or "?"
    return "-"


def b64(text):
    return base64.b64encode(text.encode()).decode()


def sha(b):
    return hashlib.sha256(b).hexdigest()


def totp_local(seed_b32, t):
    key = base64.b32decode(seed_b32.upper() + "=" * (-len(seed_b32) % 8))
    msg = struct.pack(">Q", int(t) // 30)
    h = hmac.new(key, msg, hashlib.sha1).digest()
    o = h[-1] & 15
    return "%06d" % ((struct.unpack(">I", h[o:o + 4])[0] & 0x7FFFFFFF) % 1000000)


def norm_detail(proto, d):
    """The detail in the v2 vocabulary (the 0.1.4 one goes through the m2:DT4 table)."""
    if proto == 2:
        out = {k: d.get(k) for k in ("name", "type", "group", "environment", "project", "tags", "level")}
        out["fields"] = dict(d.get("fields") or {})
        out["secrets"] = list(d.get("secrets") or [])
        out["extras"] = [{"name": x.get("name"), "secret": x.get("secret"), "value": x.get("value")}
                         for x in d.get("extras") or []]
        out["attachments"] = [{"name": a.get("name"), "size": a.get("size")} for a in d.get("attachments") or []]
        out["created_at"], out["modified_at"] = d.get("created_at"), d.get("modified_at")
        return out
    out = {
        "name": d.get("nome"), "type": NAMES_V4_V5.get(d.get("tipo"), "?" + str(d.get("tipo"))),
        "group": d.get("grupo"), "environment": d.get("ambiente"), "project": d.get("projeto"),
        "tags": d.get("tags"), "level": NAMES_V4_V5.get(d.get("nivel"), "?" + str(d.get("nivel"))),
    }
    fields = {}
    for k, v in (d.get("campos") or {}).items():
        k2 = TEXT_V2[TEXT_V1.index(k)] if k in TEXT_V1 else "?" + k
        if k2 == "security":
            v = SECURITY_V4_V5.get(v, "?" + v)
        fields[k2] = v
    out["fields"] = fields
    out["secrets"] = [SECRET_V2[SECRET_V1.index(s)] if s in SECRET_V1 else "?" + s for s in d.get("segredos") or []]
    out["extras"] = [{"name": x.get("nome"), "secret": x.get("segredo"), "value": x.get("valor")}
                     for x in d.get("extras") or []]
    out["attachments"] = [{"name": a.get("nome"), "size": a.get("tamanho")} for a in d.get("anexos") or []]
    out["created_at"], out["modified_at"] = d.get("criada_em"), d.get("alterada_em")
    return out


def app_main():
    dev = os.environ["WARD_DEV_DIR"]
    sockdir = os.environ["WARD_DEV_SOCKET_DIR"]
    rd = lambda n: open(os.path.join(dev, n)).read().strip()
    step = rd("f64-step")
    proto = int(rd("f64-proto"))
    pw, pin = rd("f64-pw"), rd("f64-pin")
    c = Client(proto, os.path.join(sockdir, "wardd.sock"))
    out = {"step": step, "proto": proto}
    kind, data = c.call("status")
    out["role"] = data.get("role") or data.get("papel")
    out["locked_before"] = data.get("locked", data.get("travado"))
    if step == "dump":
        kind, data = c.call("unlock", {"password": b64(pw)})
        out["unlock"] = [kind, code_of(kind, data)]
        kind, data = c.call("list")
        items = data.get("items") or data.get("itens") or []
        names = sorted(i.get("name") or i.get("nome") for i in items)
        out["list"] = names
        entries = {}
        for n in names:
            kind, d = c.call("detail", {"entry": n})
            if kind not in ("detail", "detalhe"):
                entries[n] = {"error": code_of(kind, d)}
                continue
            e = norm_detail(proto, d)
            raw_secrets = d.get("secrets") or d.get("segredos") or []
            hashes = {}
            seed = None
            for s_raw, s in zip(raw_secrets, e["secrets"]):
                kind, v = c.call("reveal", {"entry": n, "field": s_raw})
                if kind in ("secret", "segredo"):
                    val = base64.b64decode(v.get("value") or v.get("valor") or "")
                    hashes[s] = sha(val)
                    if s == "totp_seed":
                        seed = val.decode()
                    del val
                else:
                    hashes[s] = "erro:" + code_of(kind, v)
            for x in e["extras"]:
                if x["secret"]:
                    kind, v = c.call("reveal", {"entry": n, "extra": x["name"]})
                    x["value_sha256"] = (sha(base64.b64decode(v.get("value") or v.get("valor") or ""))
                                         if kind in ("secret", "segredo") else "erro:" + code_of(kind, v))
            e["secret_sha256"] = hashes
            if seed is not None:
                for _ in range(3):
                    t0 = time.time()
                    kind, v = c.call("totp", {"entry": n})
                    t1 = time.time()
                    if int(t0) // 30 == int(t1) // 30:
                        break
                code = v.get("code") or v.get("codigo") or ""
                e["totp"] = {"period": int(t0) // 30, "code_sha256": sha((code + "@" + str(int(t0) // 30)).encode()),
                             "matches_seed": hmac.compare_digest(code, totp_local(seed, t0)),
                             "kind": kind}
                seed = None
            entries[n] = e
        out["entries"] = entries
        kind, data = c.call("config")
        if proto == 1:
            data = {"lock_minutes": data.get("minutos_trava"), "external_backup": data.get("backup_externo"),
                    "local_backup": data.get("backup_local")}
        out["config"] = [kind, data]
        kind, data = c.call("backups")
        vs = data.get("versions") or data.get("versoes") or []
        out["backups"] = [[v.get("name") or v.get("nome"), v.get("verified", v.get("conferida")),
                           v.get("out_of_order", v.get("fora_de_ordem"))] for v in vs]
    elif step == "pin":
        kind, data = c.call("unlock", {"password": b64(pw)})
        out["unlock"] = [kind, code_of(kind, data)]
        wrong = "".join(str((int(ch) + 1) % 10) for ch in pin)
        kind, data = c.call("change_pin", {"current": b64(wrong), "new": b64(pin)})
        out["pin_wrong"] = [kind, code_of(kind, data)]
        kind, data = c.call("change_pin", {"current": b64(pin), "new": b64(pin)})
        out["pin_right"] = [kind, code_of(kind, data)]
    elif step.startswith("restore "):
        version = step.split(" ", 1)[1]
        kind, data = c.call("unlock", {"password": b64(pw)})
        out["unlock"] = [kind, code_of(kind, data)]
        kind, data = c.call("restore_backup", {"version": version, "password": b64(pw), "pin": b64(pin)})
        out["restore"] = [kind, code_of(kind, data)]
        kind, data = c.call("backups")
        vs = data.get("versions") or data.get("versoes") or []
        out["backups"] = [[v.get("name") or v.get("nome"), v.get("verified", v.get("conferida")),
                           v.get("out_of_order", v.get("fora_de_ordem"))] for v in vs]
    elif step.startswith("rotate "):
        kind, data = c.call("unlock", {"password": b64(pw)})
        out["unlock"] = [kind, code_of(kind, data)]
        codes = []
        for i in range(int(step.split(" ", 1)[1])):
            kind, data = c.call("lock_timeout", {"minutes": 31 + i})
            codes.append(code_of(kind, data) if kind == "error" else kind)
            time.sleep(1.05)
        out["writes"] = codes
        kind, data = c.call("lock_timeout", {"minutes": 240})
        out["restore_minutes"] = [kind, code_of(kind, data)]
    elif step == "pinonly":
        kind, data = c.call("change_pin", {"current": b64(pin), "new": b64(pin)})
        out["pin_right"] = [kind, code_of(kind, data)]
    elif step == "backups":
        kind, data = c.call("unlock", {"password": b64(pw)})
        out["unlock"] = [kind, code_of(kind, data)]
        kind, data = c.call("backups")
        vs = data.get("versions") or data.get("versoes") or []
        out["backups"] = [[v.get("name") or v.get("nome"), v.get("verified", v.get("conferida")),
                           v.get("out_of_order", v.get("fora_de_ordem"))] for v in vs]
    del pw, pin
    final = os.path.join(dev, "f64-out-" + step.split(" ")[0] + ".json")
    tmp = final + ".tmp"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as f:
        json.dump(out, f, indent=1, sort_keys=True, ensure_ascii=False)
    os.rename(tmp, final)


def agent_main(args):
    proto, path, op = int(args[0]), args[1], args[2]
    c = Client(proto, path)
    try:
        if op == "unlock":
            pw = open(args[3]).read().strip()
            kind, data = c.call("unlock", {"password": b64(pw)})
            del pw
        else:
            kind, data = c.call(op)
    except (OSError, ValueError, ConnectionError) as e:
        print(f"sem-resposta {type(e).__name__}")
        return 1
    extra = ""
    if op == "status":
        extra = " locked=%s role=%s" % (data.get("locked", data.get("travado")), data.get("role") or data.get("papel"))
    if op == "list" and kind in ("list", "lista"):
        extra = " n=%d" % len(data.get("items") or data.get("itens") or [])
    msg = ""
    if kind in ("error", "erro"):
        msg = " msg=" + json.dumps(data.get("message") or data.get("mensagem") or "", ensure_ascii=False)
    print(f"{kind} {code_of(kind, data)}{extra}{msg}")
    return 0


def compare_main(args):
    a = json.load(open(args[0]))
    b = json.load(open(args[1]))
    diffs = []
    if a.get("list") != b.get("list"):
        diffs.append("lista de nomes diferente: so-a=%s so-b=%s" % (
            sorted(set(a["list"]) - set(b["list"])), sorted(set(b["list"]) - set(a["list"]))))
    n = 0
    for name in sorted(set(a.get("entries", {})) & set(b.get("entries", {}))):
        ea, eb = a["entries"][name], b["entries"][name]
        for key in sorted(set(ea) | set(eb)):
            if key == "totp":
                ta, tb = ea.get("totp"), eb.get("totp")
                if not ta or not tb or not ta["matches_seed"] or not tb["matches_seed"]:
                    diffs.append(f"{name}: totp ausente ou fora da semente ({bool(ta)}/{bool(tb)})")
                elif ta["period"] == tb["period"] and ta["code_sha256"] != tb["code_sha256"]:
                    diffs.append(f"{name}: totp diferente no mesmo período")
                continue
            if ea.get(key) != eb.get(key):
                diffs.append(f"{name}: {key} diferente")
        n += 1
    for k in ("config",):
        if (a.get(k) or [None, None])[1] != (b.get(k) or [None, None])[1]:
            diffs.append(f"{k} diferente: {a.get(k)} × {b.get(k)}")
    for d in diffs:
        print("DIFERENTE " + d)
    print(f"comparadas {n} entradas; diferenças {len(diffs)}")
    return 1 if diffs else 0


def pty_main(args):
    out, timeout, pwfile = args[0], float(args[1]), args[2]
    assert args[3] == "--"
    argv = args[4:]
    env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "LANG", "WARD_DEV_SOCKET_DIR", "WARD_DEV_DIR", "XDG_RUNTIME_DIR")}
    pw = open(pwfile).read().strip() if pwfile != "-" else None
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(argv[0], argv, env)
    screen, prompt, end = b"", False, time.time() + timeout
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            try:
                b = os.read(fd, 4096)
            except OSError:
                break
            if not b:
                break
            screen += b
            if not prompt and b"Senha mestra" in screen:
                prompt = True
                if pw is not None:
                    time.sleep(0.1)
                    os.write(fd, pw.encode() + b"\r")
    else:
        os.kill(pid, 9)
    _, st = os.waitpid(pid, 0)
    os.close(fd)
    text = screen.decode("utf-8", "replace")
    if pw:
        text = text.replace(pw, "<senha-falsa>")
    with open(out, "w") as f:
        f.write(text)
    print(f"exit={os.waitstatus_to_exitcode(st)} prompt={int(prompt)}")
    return 0


if __name__ == "__main__":
    if len(sys.argv) == 1:
        app_main()
    elif sys.argv[1] == "agent":
        sys.exit(agent_main(sys.argv[2:]))
    elif sys.argv[1] == "compare":
        sys.exit(compare_main(sys.argv[2:]))
    elif sys.argv[1] == "pty":
        sys.exit(pty_main(sys.argv[2:]))
    else:
        sys.exit("uso: f64.py [agent|compare|pty] ...")
