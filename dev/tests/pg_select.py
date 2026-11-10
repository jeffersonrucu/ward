#!/usr/bin/python3 -I
"""Cliente mínimo do Postgres para os testes (Fase 28; V4.29), só com a biblioteca
padrão: o host não tem `psql` e o `psql` do contêiner (`psql_app`) não serve para o
túnel, porque a ponte do Ward só aceita conexões do grupo de processos do `ward run`.

Lê `PGHOST`, `PGPORT`, `PGUSER`, `PGDATABASE` e `PGPASSWORD` do ambiente (como o
`psql`; é o que o `ward run` entrega). Fora do `ward run` (o `real-home.sh`), a senha
vem do arquivo em `WARD_TESTE_PG_SENHA_ARQUIVO` (nunca em argv). Autenticação
SCRAM-SHA-256 (a padrão do Postgres 16) ou `trust`; sem TLS (a ponte só fala com
127.0.0.1). Roda uma consulta e imprime cada valor da primeira coluna numa linha, como
`psql -tA`. Nunca lê nada do home (sem `.pgpass`, sem `.psqlrc`).

Uso: pg_select.py '<sql>'. Sai 0 se a consulta deu certo; 1 com uma linha de erro sem
valor (o texto do servidor não traz a senha).
"""

import base64
import hashlib
import hmac
import os
import secrets
import socket
import struct
import sys


def falhar(msg):
    print("pg_select: " + msg, file=sys.stderr)
    sys.exit(1)


def senha():
    if "PGPASSWORD" in os.environ:
        return os.environ["PGPASSWORD"]
    arq = os.environ.get("WARD_TESTE_PG_SENHA_ARQUIVO")
    if arq:
        with open(arq) as f:
            return f.read().strip()
    return ""


class Conexao:
    def __init__(self, host, porta):
        self.s = socket.create_connection((host, porta), timeout=20)
        self.buf = b""

    def mandar(self, tipo, corpo):
        self.s.sendall(tipo + struct.pack("!I", len(corpo) + 4) + corpo)

    def ler_exato(self, n):
        while len(self.buf) < n:
            d = self.s.recv(65536)
            if not d:
                falhar("o servidor fechou a conexão")
            self.buf += d
        x, self.buf = self.buf[:n], self.buf[n:]
        return x

    def mensagem(self):
        tipo = self.ler_exato(1)
        n = struct.unpack("!I", self.ler_exato(4))[0]
        if n < 4 or n > 16 * 1024 * 1024:
            falhar("mensagem de tamanho inválido")
        return tipo, self.ler_exato(n - 4)


def erro_do_servidor(corpo):
    campos = {}
    for parte in corpo.split(b"\0"):
        if parte:
            campos[parte[:1]] = parte[1:].decode("utf-8", "replace")
    return campos.get(b"C", "?") + " " + campos.get(b"M", "")


def scram(c, mecanismos, pw):
    if b"SCRAM-SHA-256" not in mecanismos.split(b"\0"):
        falhar("o servidor não oferece SCRAM-SHA-256")
    nonce = base64.b64encode(secrets.token_bytes(18)).decode()
    primeira = "n=,r=" + nonce
    inicial = ("n,," + primeira).encode()
    c.mandar(b"p", b"SCRAM-SHA-256\0" + struct.pack("!I", len(inicial)) + inicial)
    tipo, corpo = c.mensagem()
    if tipo == b"E":
        falhar(erro_do_servidor(corpo))
    if tipo != b"R" or struct.unpack("!I", corpo[:4])[0] != 11:
        falhar("resposta SCRAM inesperada")
    servidor = corpo[4:].decode()
    attrs = dict(x.split("=", 1) for x in servidor.split(","))
    if not attrs["r"].startswith(nonce):
        falhar("nonce SCRAM do servidor inválido")
    sal = base64.b64decode(attrs["s"])
    salgada = hashlib.pbkdf2_hmac("sha256", pw.encode(), sal, int(attrs["i"]))
    chave_cliente = hmac.new(salgada, b"Client Key", hashlib.sha256).digest()
    guardada = hashlib.sha256(chave_cliente).digest()
    sem_prova = "c=biws,r=" + attrs["r"]
    auth = (primeira + "," + servidor + "," + sem_prova).encode()
    assinatura = hmac.new(guardada, auth, hashlib.sha256).digest()
    prova = bytes(a ^ b for a, b in zip(chave_cliente, assinatura))
    final = (sem_prova + ",p=" + base64.b64encode(prova).decode()).encode()
    c.mandar(b"p", final)
    tipo, corpo = c.mensagem()
    if tipo == b"E":
        falhar(erro_do_servidor(corpo))
    if tipo != b"R" or struct.unpack("!I", corpo[:4])[0] != 12:
        falhar("fim do SCRAM inesperado")
    chave_servidor = hmac.new(salgada, b"Server Key", hashlib.sha256).digest()
    esperada = base64.b64encode(hmac.new(chave_servidor, auth, hashlib.sha256).digest()).decode()
    if not hmac.compare_digest(corpo[4:].decode(), "v=" + esperada):
        falhar("assinatura SCRAM do servidor não confere")


def principal():
    if len(sys.argv) != 2:
        falhar("uso: pg_select.py '<sql>'")
    host = os.environ.get("PGHOST", "127.0.0.1")
    porta = int(os.environ.get("PGPORT", "5432"))
    usuario = os.environ.get("PGUSER", "")
    base = os.environ.get("PGDATABASE", usuario)
    c = Conexao(host, porta)
    params = b"user\0" + usuario.encode() + b"\0database\0" + base.encode() + b"\0\0"
    corpo = struct.pack("!I", 196608) + params
    c.s.sendall(struct.pack("!I", len(corpo) + 4) + corpo)
    while True:
        tipo, corpo = c.mensagem()
        if tipo == b"E":
            falhar(erro_do_servidor(corpo))
        if tipo == b"R":
            codigo = struct.unpack("!I", corpo[:4])[0]
            if codigo == 0:
                continue
            if codigo == 10:
                scram(c, corpo[4:], senha())
                continue
            falhar(f"método de autenticação {codigo} não suportado")
        if tipo == b"Z":
            break
    c.mandar(b"Q", sys.argv[1].encode() + b"\0")
    ok = True
    while True:
        tipo, corpo = c.mensagem()
        if tipo == b"D":
            n = struct.unpack("!H", corpo[:2])[0]
            if n:
                tam = struct.unpack("!i", corpo[2:6])[0]
                print("" if tam < 0 else corpo[6 : 6 + tam].decode("utf-8", "replace"))
        elif tipo == b"E":
            print("pg_select: " + erro_do_servidor(corpo), file=sys.stderr)
            ok = False
        elif tipo == b"Z":
            break
    c.mandar(b"X", b"")
    c.s.close()
    sys.exit(0 if ok else 1)


principal()
