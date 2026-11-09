#!/usr/bin/python3 -IB
"""Pede `abrir_app` ao daemon de teste (Fase 23a, D63). Uso do `make app-ponte`.

O pedido não tem campo: o daemon abre só o `WARD_DEV_APP` da configuração dele, com o
ambiente fixo; nada daqui chega ao processo aberto. Este cliente é agente (o papel app
só vale para o processo que o lançador abriu) e não vê segredo nenhum.

Uso: python3 -I -B dev/abrir-app.py <socket do daemon de teste>
Sai 0 com `ok`; 1 com o código de erro do daemon (ou da conexão).
"""

import json
import socket
import struct
import sys

LIMITE = 1 << 20


def ler_exato(s, n):
    partes = []
    while n > 0:
        b = s.recv(n)
        if not b:
            raise ConnectionError("o daemon fechou a conexão")
        partes.append(b)
        n -= len(b)
    return b"".join(partes)


def main():
    if len(sys.argv) != 2 or not sys.argv[1].startswith("/"):
        print("uso: abrir-app.py <socket absoluto>", file=sys.stderr)
        return 2
    corpo = json.dumps({"v": 1, "op": "abrir_app", "dados": {}}).encode()
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(30)
            s.connect(sys.argv[1])
            s.sendall(struct.pack(">I", len(corpo)) + corpo)
            (n,) = struct.unpack(">I", ler_exato(s, 4))
            if n == 0 or n > LIMITE:
                raise ConnectionError("quadro fora do limite")
            resposta = json.loads(ler_exato(s, n))
    except (OSError, ValueError) as e:
        print(f"abrir_app: sem resposta do daemon de teste ({type(e).__name__})", file=sys.stderr)
        return 1
    tipo = resposta.get("tipo")
    if tipo == "ok":
        print("abrir_app: ok")
        return 0
    codigo = (resposta.get("dados") or {}).get("codigo", "?")
    print(f"abrir_app: o daemon respondeu {tipo} {codigo}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
