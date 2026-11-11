#!/usr/bin/python3 -IB
"""Asks the test daemon for `abrir_app` (Phase 23a, D63). Used by `make app-bridge`.

The request has no field: the daemon opens only the `WARD_DEV_APP` of its configuration,
with the fixed environment; nothing from here reaches the opened process. This client is
an agent (the app role only holds for the process the launcher opened) and sees no secret.

Usage: python3 -I -B dev/open-app.py <socket of the test daemon>
Exits 0 with `ok`; 1 with the error code of the daemon (or of the connection).
"""

import json
import socket
import struct
import sys

LIMIT = 1 << 20


def read_exact(s, n):
    parts = []
    while n > 0:
        b = s.recv(n)
        if not b:
            raise ConnectionError("o daemon fechou a conexão")
        parts.append(b)
        n -= len(b)
    return b"".join(parts)


def main():
    if len(sys.argv) != 2 or not sys.argv[1].startswith("/"):
        print("uso: open-app.py <socket absoluto>", file=sys.stderr)
        return 2
    body = json.dumps({"v": 1, "op": "abrir_app", "dados": {}}).encode()
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(30)
            s.connect(sys.argv[1])
            s.sendall(struct.pack(">I", len(body)) + body)
            (n,) = struct.unpack(">I", read_exact(s, 4))
            if n == 0 or n > LIMIT:
                raise ConnectionError("quadro fora do limite")
            reply = json.loads(read_exact(s, n))
    except (OSError, ValueError) as e:
        print(f"abrir_app: sem resposta do daemon de teste ({type(e).__name__})", file=sys.stderr)
        return 1
    kind = reply.get("tipo")
    if kind == "ok":
        print("abrir_app: ok")
        return 0
    code = (reply.get("dados") or {}).get("codigo", "?")
    print(f"abrir_app: o daemon respondeu {kind} {code}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
