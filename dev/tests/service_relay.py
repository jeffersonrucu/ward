#!/usr/bin/python3 -IB
"""Phase 65a: the browser container (`dev/tests/browser-container.sh`) reaches the test
services without network.

The container runs with `--network none`. The test pages and the extension use
`http://localhost:8081`–`8084` as on the host, so:
- `host <dir>` (on the host, started by `browser-container.sh`): for each fixed port
  (8081–8084) listens on the Unix socket `<dir>/port-<n>.sock` and relays each connection
  to `127.0.0.1:<n>` (the test services of `dev/compose.yml`, only on 127.0.0.1); and on
  `<dir>/service.sock` answers the two fixed reads the suites make with
  `dev/services/compose.sh exec -T <service> cat <file>` (the phishing `posts.log` and
  WordPress's `zxcvbn.min.js`). Any other request is refused. The folder is 0700 (only the
  uid of the user, the same one that runs inside the container).
- `inside <dir>` (in the container, by `dev/container-browser/inside.sh`): listens on
  `127.0.0.1:<n>` (IPv4 only: `[::1]` stays free for the attacks of the suites) and
  relays each connection to `<dir>/port-<n>.sock`.
- `cat <service> <file>` (what the suites call instead of `compose.sh exec`): with
  `WARD_TEST_SERVICE_RELAY` (set only by `inside.sh`), asks `service.sock`; without it,
  runs `dev/services/compose.sh exec -T <service> cat <file>` itself. Exit code, stdout
  and stderr are those of the command.

Python standard library only. Never carries anything but the bytes of the connections
and of the two fixed files.
"""

from __future__ import annotations

import os
import socket
import struct
import subprocess
import sys
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PORTS = (8081, 8082, 8083, 8084)
READS = {
    ("phish", "/registro/posts.log"),
    ("wordpress", "/var/www/html/wp-includes/js/zxcvbn.min.js"),
}
MAX_REQUEST = 512


def pump(a: socket.socket, b: socket.socket) -> None:
    try:
        while True:
            d = a.recv(65536)
            if not d:
                break
            b.sendall(d)
    except OSError:
        pass
    finally:
        for s, how in ((b, socket.SHUT_WR), (a, socket.SHUT_RD)):
            try:
                s.shutdown(how)
            except OSError:
                pass


def bridge(client: socket.socket, connect) -> None:
    try:
        upstream = connect()
    except OSError:
        client.close()
        return
    t = threading.Thread(target=pump, args=(upstream, client), daemon=True)
    t.start()
    pump(client, upstream)
    t.join()
    client.close()
    upstream.close()


def serve(listener: socket.socket, handler) -> None:
    while True:
        c, _ = listener.accept()
        threading.Thread(target=handler, args=(c,), daemon=True).start()


def compose_cat(service: str, path: str) -> subprocess.CompletedProcess:
    return subprocess.run([str(ROOT / "dev" / "services" / "compose.sh"), "exec", "-T", service, "cat", path],
                          stdin=subprocess.DEVNULL, capture_output=True, timeout=120)


def answer_read(c: socket.socket) -> None:
    try:
        req = b""
        while not req.endswith(b"\n") and len(req) <= MAX_REQUEST:
            d = c.recv(MAX_REQUEST)
            if not d:
                break
            req += d
        parts = req.rstrip(b"\n").decode("ascii", "strict").split(" ")
        if len(parts) != 2 or (parts[0], parts[1]) not in READS:
            c.sendall(struct.pack(">iII", 2, 0, 9) + b"recusado\n")
            return
        p = compose_cat(parts[0], parts[1])
        c.sendall(struct.pack(">iII", p.returncode, len(p.stdout), len(p.stderr)) + p.stdout + p.stderr)
    except (OSError, UnicodeDecodeError, subprocess.SubprocessError):
        pass
    finally:
        c.close()


def unix_listener(path: Path) -> socket.socket:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.bind(str(path))
    os.chmod(path, 0o600)
    s.listen(64)
    return s


def host(d: Path) -> int:
    st = os.lstat(d)
    if not d.is_dir() or d.is_symlink() or st.st_uid != os.getuid() or st.st_mode & 0o077:
        print("service-relay: a pasta tem de ser 0700 e do usuário", file=sys.stderr)
        return 2
    for port in PORTS:
        lst = unix_listener(d / f"port-{port}.sock")
        threading.Thread(target=serve, args=(lst, lambda c, p=port: bridge(
            c, lambda: socket.create_connection(("127.0.0.1", p), 5))), daemon=True).start()
    lst = unix_listener(d / "service.sock")
    threading.Thread(target=serve, args=(lst, answer_read), daemon=True).start()
    (d / "host.ready").write_text("ok\n")
    threading.Event().wait()
    return 0


def inside(d: Path) -> int:
    for port in PORTS:
        lst = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        lst.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        lst.bind(("127.0.0.1", port))
        lst.listen(64)

        def to_unix(p=port):
            u = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            u.connect(str(d / f"port-{p}.sock"))
            return u
        threading.Thread(target=serve, args=(lst, lambda c, f=to_unix: bridge(c, f)), daemon=True).start()
    print("service-relay: pronto", flush=True)
    threading.Event().wait()
    return 0


def recv_exact(s: socket.socket, n: int) -> bytes:
    buf = b""
    while len(buf) < n:
        d = s.recv(n - len(buf))
        if not d:
            raise OSError("resposta cortada")
        buf += d
    return buf


def cat(service: str, path: str) -> int:
    relay = os.environ.get("WARD_TEST_SERVICE_RELAY")
    if not relay:
        p = compose_cat(service, path)
    else:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(150)
            s.connect(relay)
            s.sendall(f"{service} {path}\n".encode("ascii"))
            rc, n_out, n_err = struct.unpack(">iII", recv_exact(s, 12))
            out, err = recv_exact(s, n_out), recv_exact(s, n_err)
        p = subprocess.CompletedProcess([], rc, out, err)
    sys.stdout.buffer.write(p.stdout)
    sys.stderr.buffer.write(p.stderr)
    return p.returncode


def main(argv: list[str]) -> int:
    if len(argv) == 2 and argv[0] == "host":
        return host(Path(argv[1]))
    if len(argv) == 2 and argv[0] == "inside":
        return inside(Path(argv[1]))
    if len(argv) == 3 and argv[0] == "cat":
        return cat(argv[1], argv[2])
    print("uso: service_relay.py host <pasta> | inside <pasta> | cat <serviço> <arquivo>", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
