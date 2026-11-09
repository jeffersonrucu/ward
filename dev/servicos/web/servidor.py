#!/usr/bin/env python3
"""Sites falsos de teste do Ward (Fase 12), só biblioteca padrão.

    python3 -I -B servidor.py phish   # 8082: clone da tela de login do WordPress;
                                      #       grava TODO POST em /registro/posts.log
    python3 -I -B servidor.py totp    # 8083: login com campo de código e cadastro novo
    python3 -I -B servidor.py redir   # 8084: redireciona tudo para o phish (8082)

Entradas são não confiáveis: corpo limitado a 16 KiB, só formulário urlencoded,
campos com tamanho máximo. O log de acesso mostra só método e caminho (sem query,
sem corpo). Senha e código comparados em tempo constante (hmac.compare_digest).
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

LIMITE_CORPO = 16 * 1024
LIMITE_CAMPO = 256
LIMITE_CONTAS_NOVAS = 100
REGISTRO = os.environ.get("WARD_REGISTRO", "/registro/posts.log")
CONTAS = os.environ.get("WARD_CONTAS", "/run/ward/totp-contas.json")
DESTINO_REDIR = os.environ.get("WARD_DESTINO_REDIR", "http://localhost:8082/wp-login.php")

ESTILO = (
    "body{font-family:sans-serif;background:#f0f0f1;margin:0}"
    "main{width:320px;margin:8% auto;background:#fff;padding:26px 24px;"
    "border:1px solid #c3c4c7;border-radius:4px}"
    "label{display:block;margin-top:12px}input{width:100%;box-sizing:border-box;padding:6px}"
    "button{margin-top:16px;padding:6px 12px}.erro{color:#b32d2e}.ok{color:#007017}"
)


def pagina(titulo: str, corpo: str) -> bytes:
    return (
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">"
        f"<title>{html.escape(titulo)}</title><style>{ESTILO}</style></head>"
        f"<body><main>{corpo}</main></body></html>"
    ).encode()


# --- TOTP (RFC 6238, SHA-1, 30 s, 6 dígitos) sobre o hmac da biblioteca padrão -------
def totp(semente_b32: str, contador: int) -> str:
    chave = base64.b32decode(semente_b32.upper() + "=" * (-len(semente_b32) % 8))
    mac = hmac.new(chave, struct.pack(">Q", contador), hashlib.sha1).digest()
    deslocamento = mac[-1] & 0x0F
    valor = struct.unpack(">I", mac[deslocamento : deslocamento + 4])[0] & 0x7FFFFFFF
    return f"{valor % 1_000_000:06d}"


def totp_confere(semente: str, codigo: str) -> bool:
    agora = int(time.time()) // 30
    ok = False
    for passo in (-1, 0, 1):  # tolera um período de relógio para cada lado
        ok |= hmac.compare_digest(totp(semente, agora + passo).encode(), codigo.encode())
    return ok


class Base(BaseHTTPRequestHandler):
    server_version = "ward-teste"
    sys_version = ""
    protocol_version = "HTTP/1.1"
    timeout = 10  # cliente lento não segura a thread

    def log_message(self, formato: str, *args) -> None:  # sem query nem corpo
        caminho = urlsplit(self.path).path
        sys.stderr.write(f"{self.command} {caminho[:200]}\n")

    def responder(self, status: int, corpo: bytes, extra: dict[str, str] | None = None) -> None:
        self.send_response(status)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(corpo)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(corpo)

    def ler_corpo(self) -> bytes | None:
        """Corpo do POST, ou None se recusado (já respondido)."""
        try:
            tamanho = int(self.headers.get("Content-Length", ""))
        except ValueError:
            self.responder(411, pagina("Erro", "<p>Content-Length obrigatório.</p>"))
            return None
        if tamanho < 0 or tamanho > LIMITE_CORPO:
            self.close_connection = True
            self.responder(413, pagina("Erro", "<p>Corpo grande demais.</p>"))
            return None
        return self.rfile.read(tamanho)

    def formulario(self, corpo: bytes) -> dict[str, str] | None:
        tipo = self.headers.get("Content-Type", "").split(";")[0].strip().lower()
        if tipo != "application/x-www-form-urlencoded":
            self.responder(415, pagina("Erro", "<p>Formulário inválido.</p>"))
            return None
        try:
            campos = parse_qs(corpo.decode("utf-8"), keep_blank_values=True, max_num_fields=20)
        except (UnicodeDecodeError, ValueError):
            self.responder(400, pagina("Erro", "<p>Formulário inválido.</p>"))
            return None
        saida = {}
        for k, v in campos.items():
            if len(k) > 64 or len(v[0]) > LIMITE_CAMPO:
                self.responder(400, pagina("Erro", "<p>Campo grande demais.</p>"))
                return None
            saida[k] = v[0]
        return saida


# --- phish (8082) ----------------------------------------------------------------------
TRAVA_REGISTRO = threading.Lock()


def tela_wp(erro: str = "") -> bytes:
    aviso = f"<p class=\"erro\" id=\"login_error\">{html.escape(erro)}</p>" if erro else ""
    return pagina(
        "Acessar ‹ Acme — WordPress",
        "<h1>Acme</h1>" + aviso +
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
        self.responder(200, tela_wp())

    def do_POST(self) -> None:
        corpo = self.ler_corpo()
        if corpo is None:
            return
        linha = {
            "quando": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
            "caminho": urlsplit(self.path).path[:200],
            "corpo": corpo.decode("utf-8", "replace"),
            "campos": {k: v[0] for k, v in parse_qs(corpo.decode("utf-8", "replace"), keep_blank_values=True, max_num_fields=50).items()},
        }
        with TRAVA_REGISTRO:
            fd = os.open(REGISTRO, os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
            try:
                os.write(fd, (json.dumps(linha, ensure_ascii=False) + "\n").encode())
            finally:
                os.close(fd)
        self.responder(200, tela_wp("Erro: a senha informada está incorreta."))


# --- totp (8083) -----------------------------------------------------------------------
USUARIO_VALIDO = re.compile(r"^[a-z0-9_.-]{1,64}$")
TRAVA_CONTAS = threading.Lock()
CONTAS_CARREGADAS: dict[str, dict[str, str]] = {}


def tela_totp(msg: str = "", classe: str = "erro") -> bytes:
    aviso = f"<p class=\"{classe}\" id=\"mensagem\">{html.escape(msg)}</p>" if msg else ""
    return pagina(
        "Entrar — Portal de teste",
        "<h1>Portal de teste</h1>" + aviso +
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


def tela_cadastro(msg: str = "", classe: str = "erro") -> bytes:
    aviso = f"<p class=\"{classe}\" id=\"mensagem\">{html.escape(msg)}</p>" if msg else ""
    return pagina(
        "Criar conta — Portal de teste",
        "<h1>Criar conta</h1>" + aviso +
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
        caminho = urlsplit(self.path).path
        if caminho in ("/", "/login"):
            self.responder(200, tela_totp())
        elif caminho == "/cadastro":
            self.responder(200, tela_cadastro())
        else:
            self.responder(404, pagina("Não encontrado", "<p>Não encontrado.</p>"))

    def do_POST(self) -> None:
        caminho = urlsplit(self.path).path
        if caminho not in ("/login", "/cadastro"):
            self.responder(404, pagina("Não encontrado", "<p>Não encontrado.</p>"))
            return
        corpo = self.ler_corpo()
        if corpo is None:
            return
        campos = self.formulario(corpo)
        if campos is None:
            return
        usuario = campos.get("usuario", "")
        senha = campos.get("senha", "")
        if caminho == "/cadastro":
            self.cadastrar(usuario, senha)
        else:
            self.entrar(usuario, senha, campos.get("codigo", ""))

    def cadastrar(self, usuario: str, senha: str) -> None:
        if not USUARIO_VALIDO.match(usuario) or not (8 <= len(senha) <= LIMITE_CAMPO):
            self.responder(400, tela_cadastro("Usuário inválido ou senha com menos de 8 caracteres."))
            return
        with TRAVA_CONTAS:
            if usuario in CONTAS_CARREGADAS:
                self.responder(409, tela_cadastro("Usuário já existe."))
                return
            if len(CONTAS_CARREGADAS) >= LIMITE_CONTAS_NOVAS:
                self.responder(429, tela_cadastro("Limite de contas de teste atingido."))
                return
            CONTAS_CARREGADAS[usuario] = {"senha": senha, "totp_semente": ""}
        self.responder(201, tela_cadastro("Conta criada. Já pode entrar.", "ok"))

    def entrar(self, usuario: str, senha: str, codigo: str) -> None:
        with TRAVA_CONTAS:
            conta = CONTAS_CARREGADAS.get(usuario)
        # Compara sempre (mesmo sem conta) para não revelar por tempo se o usuário existe.
        senha_ok = hmac.compare_digest(
            (conta["senha"] if conta else "\0" * 32).encode(), senha.encode()
        )
        semente = conta["totp_semente"] if conta else ""
        codigo_ok = totp_confere(semente, codigo) if semente else True
        if conta and senha_ok and codigo_ok:
            self.responder(200, pagina("Painel — Portal de teste", f"<h1 id=\"painel\">Painel</h1><p>Olá, {html.escape(usuario)}.</p>"))
        else:
            self.responder(401, tela_totp("Usuário, senha ou código incorretos."))


# --- redir (8084) ----------------------------------------------------------------------
class Redir(Base):
    def redirecionar(self) -> None:
        self.responder(302, pagina("Redirecionando", "<p>Redirecionando…</p>"), {"Location": DESTINO_REDIR})

    def do_GET(self) -> None:
        self.redirecionar()

    def do_POST(self) -> None:
        self.redirecionar()


def main() -> None:
    modos = {"phish": Phish, "totp": Totp, "redir": Redir}
    if len(sys.argv) != 2 or sys.argv[1] not in modos:
        sys.exit("uso: servidor.py phish|totp|redir")
    modo = sys.argv[1]
    if modo == "totp":
        with open(CONTAS, encoding="utf-8") as f:
            for c in json.load(f):
                CONTAS_CARREGADAS[c["usuario"]] = {"senha": c["senha"], "totp_semente": c["totp_semente"]}
    porta = int(os.environ.get("WARD_PORTA", "8080"))
    servidor = ThreadingHTTPServer(("0.0.0.0", porta), modos[modo])
    servidor.daemon_threads = True
    sys.stderr.write(f"ward-teste {modo} escutando na porta {porta}\n")
    servidor.serve_forever()


if __name__ == "__main__":
    main()
