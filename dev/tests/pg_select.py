#!/usr/bin/python3 -I
"""Minimal Postgres client for the tests (Phase 28; V4.29), standard library only: the
host has no `psql` and the container's `psql` (`psql_app`) does not work for the tunnel,
because the Ward bridge only accepts connections from the process group of `ward run`.

Reads `PGHOST`, `PGPORT`, `PGUSER`, `PGDATABASE` and `PGPASSWORD` from the environment
(like `psql`; it is what `ward run` delivers). Outside `ward run` (`real-home.sh`), the
password comes from the file named by the test password-file variable read in
`password()` (never in argv). SCRAM-SHA-256 authentication (Postgres 16 default) or
`trust`; no TLS (the bridge only talks to 127.0.0.1). Runs one query and prints each value
of the first column on a line, like `psql -tA`. Never reads anything from the home (no
`.pgpass`, no `.psqlrc`).

Usage: pg_select.py '<sql>'. Exits 0 if the query worked; 1 with an error line without a
value (the server text does not carry the password).
"""

import base64
import hashlib
import hmac
import os
import secrets
import socket
import struct
import sys


def fail(msg):
    print("pg_select: " + msg, file=sys.stderr)
    sys.exit(1)


def password():
    if "PGPASSWORD" in os.environ:
        return os.environ["PGPASSWORD"]
    path = os.environ.get("WARD_TESTE_PG_SENHA_ARQUIVO")
    if path:
        with open(path) as f:
            return f.read().strip()
    return ""


class Connection:
    def __init__(self, host, port):
        self.s = socket.create_connection((host, port), timeout=20)
        self.buf = b""

    def send(self, kind, body):
        self.s.sendall(kind + struct.pack("!I", len(body) + 4) + body)

    def read_exact(self, n):
        while len(self.buf) < n:
            d = self.s.recv(65536)
            if not d:
                fail("o servidor fechou a conexão")
            self.buf += d
        x, self.buf = self.buf[:n], self.buf[n:]
        return x

    def message(self):
        kind = self.read_exact(1)
        n = struct.unpack("!I", self.read_exact(4))[0]
        if n < 4 or n > 16 * 1024 * 1024:
            fail("mensagem de tamanho inválido")
        return kind, self.read_exact(n - 4)


def server_error(body):
    fields = {}
    for part in body.split(b"\0"):
        if part:
            fields[part[:1]] = part[1:].decode("utf-8", "replace")
    return fields.get(b"C", "?") + " " + fields.get(b"M", "")


def scram(c, mechanisms, pw):
    if b"SCRAM-SHA-256" not in mechanisms.split(b"\0"):
        fail("o servidor não oferece SCRAM-SHA-256")
    nonce = base64.b64encode(secrets.token_bytes(18)).decode()
    client_first = "n=,r=" + nonce
    initial = ("n,," + client_first).encode()
    c.send(b"p", b"SCRAM-SHA-256\0" + struct.pack("!I", len(initial)) + initial)
    kind, body = c.message()
    if kind == b"E":
        fail(server_error(body))
    if kind != b"R" or struct.unpack("!I", body[:4])[0] != 11:
        fail("resposta SCRAM inesperada")
    server_first = body[4:].decode()
    attrs = dict(x.split("=", 1) for x in server_first.split(","))
    if not attrs["r"].startswith(nonce):
        fail("nonce SCRAM do servidor inválido")
    salt = base64.b64decode(attrs["s"])
    salted = hashlib.pbkdf2_hmac("sha256", pw.encode(), salt, int(attrs["i"]))
    client_key = hmac.new(salted, b"Client Key", hashlib.sha256).digest()
    stored_key = hashlib.sha256(client_key).digest()
    without_proof = "c=biws,r=" + attrs["r"]
    auth = (client_first + "," + server_first + "," + without_proof).encode()
    signature = hmac.new(stored_key, auth, hashlib.sha256).digest()
    proof = bytes(a ^ b for a, b in zip(client_key, signature))
    final = (without_proof + ",p=" + base64.b64encode(proof).decode()).encode()
    c.send(b"p", final)
    kind, body = c.message()
    if kind == b"E":
        fail(server_error(body))
    if kind != b"R" or struct.unpack("!I", body[:4])[0] != 12:
        fail("fim do SCRAM inesperado")
    server_key = hmac.new(salted, b"Server Key", hashlib.sha256).digest()
    expected = base64.b64encode(hmac.new(server_key, auth, hashlib.sha256).digest()).decode()
    if not hmac.compare_digest(body[4:].decode(), "v=" + expected):
        fail("assinatura SCRAM do servidor não confere")


def main():
    if len(sys.argv) != 2:
        fail("uso: pg_select.py '<sql>'")
    host = os.environ.get("PGHOST", "127.0.0.1")
    port = int(os.environ.get("PGPORT", "5432"))
    user = os.environ.get("PGUSER", "")
    database = os.environ.get("PGDATABASE", user)
    c = Connection(host, port)
    params = b"user\0" + user.encode() + b"\0database\0" + database.encode() + b"\0\0"
    body = struct.pack("!I", 196608) + params
    c.s.sendall(struct.pack("!I", len(body) + 4) + body)
    while True:
        kind, body = c.message()
        if kind == b"E":
            fail(server_error(body))
        if kind == b"R":
            code = struct.unpack("!I", body[:4])[0]
            if code == 0:
                continue
            if code == 10:
                scram(c, body[4:], password())
                continue
            fail(f"método de autenticação {code} não suportado")
        if kind == b"Z":
            break
    c.send(b"Q", sys.argv[1].encode() + b"\0")
    ok = True
    while True:
        kind, body = c.message()
        if kind == b"D":
            n = struct.unpack("!H", body[:2])[0]
            if n:
                size = struct.unpack("!i", body[2:6])[0]
                print("" if size < 0 else body[6 : 6 + size].decode("utf-8", "replace"))
        elif kind == b"E":
            print("pg_select: " + server_error(body), file=sys.stderr)
            ok = False
        elif kind == b"Z":
            break
    c.send(b"X", b"")
    c.s.close()
    sys.exit(0 if ok else 1)


main()
