#!/usr/bin/env python3
"""Fake Ward test sites (Phase 12), standard library only.

    python3 -I -B server.py phish   # 8082: clone of the WordPress login screen;
                                      #       records EVERY POST in posts.log
    python3 -I -B server.py totp    # 8083: login with a code field, plus sign-up
    python3 -I -B server.py redir   # 8084: redirects everything to phish (8082)

Inputs are untrusted: body limited to 16 KiB, urlencoded forms only, fields with a
maximum size. The access log shows only method and path (no query, no body). Password
and code compared in constant time (hmac.compare_digest).
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import html
import json
import os
import re
import struct
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

BODY_LIMIT = 16 * 1024
FIELD_LIMIT = 256
NEW_ACCOUNTS_LIMIT = 100
RECORD = os.environ.get("WARD_RECORD", "/registro/posts.log")
ACCOUNTS = os.environ.get("WARD_ACCOUNTS", "/run/ward/totp-contas.json")
REDIR_TARGET = os.environ.get("WARD_REDIR_TARGET", "http://localhost:8082/wp-login.php")

STYLE = (
    "body{font-family:sans-serif;background:#f0f0f1;margin:0}"
    "main{width:320px;margin:8% auto;background:#fff;padding:26px 24px;"
    "border:1px solid #c3c4c7;border-radius:4px}"
    "label{display:block;margin-top:12px}input{width:100%;box-sizing:border-box;padding:6px}"
    "button{margin-top:16px;padding:6px 12px}.erro{color:#b32d2e}.ok{color:#007017}"
)


def page(title: str, body: str) -> bytes:
    return (
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">"
        f"<title>{html.escape(title)}</title><style>{STYLE}</style></head>"
        f"<body><main>{body}</main></body></html>"
    ).encode()


# --- TOTP (RFC 6238, SHA-1, 30 s, 6 digits) over the standard library hmac -----------
def totp(seed_b32: str, counter: int) -> str:
    key = base64.b32decode(seed_b32.upper() + "=" * (-len(seed_b32) % 8))
    mac = hmac.new(key, struct.pack(">Q", counter), hashlib.sha1).digest()
    offset = mac[-1] & 0x0F
    value = struct.unpack(">I", mac[offset : offset + 4])[0] & 0x7FFFFFFF
    return f"{value % 1_000_000:06d}"


def totp_matches(seed: str, code: str) -> bool:
    now = int(time.time()) // 30
    ok = False
    for step in (-1, 0, 1):  # tolerates one clock period on each side
        ok |= hmac.compare_digest(totp(seed, now + step).encode(), code.encode())
    return ok


class Base(BaseHTTPRequestHandler):
    server_version = "ward-teste"
    sys_version = ""
    protocol_version = "HTTP/1.1"
    timeout = 10  # a slow client does not hold the thread

    def log_message(self, fmt: str, *args) -> None:  # no query, no body
        url_path = urlsplit(self.path).path
        sys.stderr.write(f"{self.command} {url_path[:200]}\n")

    def respond(self, status: int, body: bytes, extra: dict[str, str] | None = None) -> None:
        self.send_response(status)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)

    def read_body(self) -> bytes | None:
        """POST body, or None if refused (already answered)."""
        try:
            size = int(self.headers.get("Content-Length", ""))
        except ValueError:
            self.respond(411, page("Erro", "<p>Content-Length obrigatório.</p>"))
            return None
        if size < 0 or size > BODY_LIMIT:
            self.close_connection = True
            self.respond(413, page("Erro", "<p>Corpo grande demais.</p>"))
            return None
        return self.rfile.read(size)

    def form(self, body: bytes) -> dict[str, str] | None:
        content_type = self.headers.get("Content-Type", "").split(";")[0].strip().lower()
        if content_type != "application/x-www-form-urlencoded":
            self.respond(415, page("Erro", "<p>Formulário inválido.</p>"))
            return None
        try:
            fields = parse_qs(body.decode("utf-8"), keep_blank_values=True, max_num_fields=20)
        except (UnicodeDecodeError, ValueError):
            self.respond(400, page("Erro", "<p>Formulário inválido.</p>"))
            return None
        out = {}
        for k, v in fields.items():
            if len(k) > 64 or len(v[0]) > FIELD_LIMIT:
                self.respond(400, page("Erro", "<p>Campo grande demais.</p>"))
                return None
            out[k] = v[0]
        return out


# --- phish (8082) ----------------------------------------------------------------------
RECORD_LOCK = threading.Lock()


def wp_screen(error: str = "") -> bytes:
    notice = f"<p class=\"erro\" id=\"login_error\">{html.escape(error)}</p>" if error else ""
    return page(
        "Acessar ‹ Acme — WordPress",
        "<h1>Acme</h1>" + notice +
        "<form name=\"loginform\" id=\"loginform\" action=\"/wp-login.php\" method=\"post\">"
        "<label for=\"user_login\">Nome de usuário ou endereço de e-mail</label>"
        "<input type=\"text\" name=\"log\" id=\"user_login\" autocomplete=\"username\">"
        "<label for=\"user_pass\">Senha</label>"
        "<input type=\"password\" name=\"pwd\" id=\"user_pass\" autocomplete=\"current-password\">"
        "<input type=\"hidden\" name=\"testcookie\" value=\"1\">"
        "<button type=\"submit\" name=\"wp-submit\" id=\"wp-submit\">Acessar</button>"
        "</form>",
    )


class Phish(Base):
    def do_GET(self) -> None:
        self.respond(200, wp_screen())

    def do_POST(self) -> None:
        body = self.read_body()
        if body is None:
            return
        line = {
            "quando": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
            "caminho": urlsplit(self.path).path[:200],
            "corpo": body.decode("utf-8", "replace"),
            "campos": {k: v[0] for k, v in parse_qs(body.decode("utf-8", "replace"), keep_blank_values=True, max_num_fields=50).items()},
        }
        with RECORD_LOCK:
            fd = os.open(RECORD, os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
            try:
                os.write(fd, (json.dumps(line, ensure_ascii=False) + "\n").encode())
            finally:
                os.close(fd)
        self.respond(200, wp_screen("Erro: a senha informada está incorreta."))


# --- totp (8083) -----------------------------------------------------------------------
VALID_USERNAME = re.compile(r"^[a-z0-9_.-]{1,64}$")
ACCOUNTS_LOCK = threading.Lock()
LOADED_ACCOUNTS: dict[str, dict[str, str]] = {}


def totp_screen(msg: str = "", css_class: str = "erro") -> bytes:
    notice = f"<p class=\"{css_class}\" id=\"mensagem\">{html.escape(msg)}</p>" if msg else ""
    return page(
        "Entrar — Portal de teste",
        "<h1>Portal de teste</h1>" + notice +
        "<form id=\"login\" action=\"/login\" method=\"post\">"
        "<label for=\"usuario\">Usuário</label>"
        "<input type=\"text\" name=\"usuario\" id=\"usuario\" autocomplete=\"username\">"
        "<label for=\"senha\">Senha</label>"
        "<input type=\"password\" name=\"senha\" id=\"senha\" autocomplete=\"current-password\">"
        "<label for=\"codigo\">Código de verificação</label>"
        "<input type=\"text\" name=\"codigo\" id=\"codigo\" inputmode=\"numeric\" "
        "autocomplete=\"one-time-code\" maxlength=\"6\">"
        "<button type=\"submit\">Entrar</button></form>"
        "<p><a href=\"/cadastro\">Criar conta</a></p>",
    )


def signup_screen(msg: str = "", css_class: str = "erro") -> bytes:
    notice = f"<p class=\"{css_class}\" id=\"mensagem\">{html.escape(msg)}</p>" if msg else ""
    return page(
        "Criar conta — Portal de teste",
        "<h1>Criar conta</h1>" + notice +
        "<form id=\"cadastro\" action=\"/cadastro\" method=\"post\">"
        "<label for=\"usuario\">Usuário</label>"
        "<input type=\"text\" name=\"usuario\" id=\"usuario\" autocomplete=\"username\">"
        "<label for=\"senha\">Senha</label>"
        "<input type=\"password\" name=\"senha\" id=\"senha\" autocomplete=\"new-password\">"
        "<button type=\"submit\">Criar conta</button></form>"
        "<p><a href=\"/login\">Entrar</a></p>",
    )


class Totp(Base):
    def do_GET(self) -> None:
        url_path = urlsplit(self.path).path
        if url_path in ("/", "/login"):
            self.respond(200, totp_screen())
        elif url_path == "/cadastro":
            self.respond(200, signup_screen())
        else:
            self.respond(404, page("Não encontrado", "<p>Não encontrado.</p>"))

    def do_POST(self) -> None:
        url_path = urlsplit(self.path).path
        if url_path not in ("/login", "/cadastro"):
            self.respond(404, page("Não encontrado", "<p>Não encontrado.</p>"))
            return
        body = self.read_body()
        if body is None:
            return
        fields = self.form(body)
        if fields is None:
            return
        username = fields.get("usuario", "")
        password = fields.get("senha", "")
        if url_path == "/cadastro":
            self.sign_up(username, password)
        else:
            self.log_in(username, password, fields.get("codigo", ""))

    def sign_up(self, username: str, password: str) -> None:
        if not VALID_USERNAME.match(username) or not (8 <= len(password) <= FIELD_LIMIT):
            self.respond(400, signup_screen("Usuário inválido ou senha com menos de 8 caracteres."))
            return
        with ACCOUNTS_LOCK:
            if username in LOADED_ACCOUNTS:
                self.respond(409, signup_screen("Usuário já existe."))
                return
            if len(LOADED_ACCOUNTS) >= NEW_ACCOUNTS_LIMIT:
                self.respond(429, signup_screen("Limite de contas de teste atingido."))
                return
            LOADED_ACCOUNTS[username] = {"senha": password, "totp_semente": ""}
        self.respond(201, signup_screen("Conta criada. Já pode entrar.", "ok"))

    def log_in(self, username: str, password: str, code: str) -> None:
        with ACCOUNTS_LOCK:
            account = LOADED_ACCOUNTS.get(username)
        # Always compares (even without an account) so timing does not reveal whether the user exists.
        password_ok = hmac.compare_digest(
            (account["senha"] if account else "\0" * 32).encode(), password.encode()
        )
        seed = account["totp_semente"] if account else ""
        code_ok = totp_matches(seed, code) if seed else True
        if account and password_ok and code_ok:
            self.respond(200, page("Painel — Portal de teste", f"<h1 id=\"painel\">Painel</h1><p>Olá, {html.escape(username)}.</p>"))
        else:
            self.respond(401, totp_screen("Usuário, senha ou código incorretos."))


# --- redir (8084) ----------------------------------------------------------------------
class Redir(Base):
    def redirect(self) -> None:
        self.respond(302, page("Redirecionando", "<p>Redirecionando…</p>"), {"Location": REDIR_TARGET})

    def do_GET(self) -> None:
        self.redirect()

    def do_POST(self) -> None:
        self.redirect()


def main() -> None:
    modes = {"phish": Phish, "totp": Totp, "redir": Redir}
    if len(sys.argv) != 2 or sys.argv[1] not in modes:
        sys.exit("uso: server.py phish|totp|redir")
    mode = sys.argv[1]
    if mode == "totp":
        with open(ACCOUNTS, encoding="utf-8") as f:
            for c in json.load(f):
                LOADED_ACCOUNTS[c["usuario"]] = {"senha": c["senha"], "totp_semente": c["totp_semente"]}
    port = int(os.environ.get("WARD_LISTEN_PORT", "8080"))
    server = ThreadingHTTPServer(("0.0.0.0", port), modes[mode])
    server.daemon_threads = True
    sys.stderr.write(f"ward-teste {mode} escutando na porta {port}\n")
    server.serve_forever()


if __name__ == "__main__":
    main()
