#!/usr/bin/python3 -I
"""Protocol v1 client of the upgrade tests (Phase 57; m2:V5.7): talks to a 0.1.4 `wardd`
inside the containers of `make test-install` and `make test-deb`, where the 0.2.0 client
(`wardd-test-client`, protocol v2) gets `unknown_version`.

It never builds a v1 frame itself: it sends the frame of a request line of
`dev/fixtures/legacy/protocol-v1.jsonl` (the 0.1.4 baseline, Phase 3; the unlock line
carries the base64 of the test master password of D15, fake) and compares the first reply
frame with the frame of a response line of the same file. Prints one line:

- `same` if the reply is the expected frame;
- `different <keys>` with the top-level keys of the reply (never its values);
- `connect: <errno>` or `bytes: 0` if the daemon refused or closed without replying.

Usage: legacy_client.py <socket> <protocol-v1.jsonl> <request line> <response line>
(1-based line numbers).
"""

import json
import os
import socket
import struct
import sys

MAX_FRAME = 1 << 20


def fixture_frame(path, number, kind):
    with open(path, encoding="utf-8") as f:
        lines = f.read().splitlines()
    if not 1 <= number <= len(lines):
        sys.exit(f"linha {number} fora de {path}")
    entry = json.loads(lines[number - 1])
    if entry.get("kind") != kind:
        sys.exit(f"a linha {number} de {path} não é {kind}")
    return entry["frame"]


def read_exact(conn, n):
    data = b""
    while len(data) < n:
        chunk = conn.recv(n - len(data))
        if not chunk:
            return None
        data += chunk
    return data


def main():
    if len(sys.argv) != 5:
        sys.exit("uso: legacy_client.py <socket> <protocol-v1.jsonl> <linha do pedido> <linha da resposta>")
    sock_path, fixture = sys.argv[1], sys.argv[2]
    request = fixture_frame(fixture, int(sys.argv[3]), "request")
    expected = fixture_frame(fixture, int(sys.argv[4]), "response")
    body = json.dumps(request, ensure_ascii=False, separators=(",", ":")).encode()
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
        conn.settimeout(10)
        try:
            conn.connect(sock_path)
        except OSError as e:
            print(f"connect: {e.errno}")
            return
        try:
            conn.sendall(struct.pack(">I", len(body)) + body)
            head = read_exact(conn, 4)
            size = struct.unpack(">I", head)[0] if head is not None else 0
            if size > MAX_FRAME:
                print("different (quadro grande demais)")
                return
            reply = read_exact(conn, size) if head is not None else None
        except OSError:
            reply = None
    if reply is None:
        print("bytes: 0")
        return
    try:
        parsed = json.loads(reply)
    except ValueError:
        print("different (resposta não é JSON)")
        return
    if parsed == expected:
        print("same")
    else:
        keys = sorted(parsed) if isinstance(parsed, dict) else type(parsed).__name__
        print(f"different {keys}")


if __name__ == "__main__":
    os.umask(0o077)
    main()
