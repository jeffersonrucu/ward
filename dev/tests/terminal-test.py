#!/usr/bin/python3 -IB
""""Terminal" of the test bridge (Phase 39c; D3, V5.24). Development only.

The bridge opened by the test daemon builds the Connect button with the same terminal
type as the app (`app/core/src/terminal.rs`): this script plays the part of
`x-terminal-emulator`, receives `-e <repo>/target/debug/ward ssh --token-fd <n> <entry>`
and the inherited fd `<n>` (the token goes only in the socket; never in argv nor in the
environment). It opens no window: it runs the command in a pty, waits for the server
prompt and sends `echo WARD-E2E-$(id -un); exit`.

The mode comes from the mode file in `$WARD_DEV_DIR` (one word; QA writes it before
clicking Connect; without the file, `normal`). The mode words are the ones `main()`
accepts, written by `dev/tests/app_e2e.py`:
- normal: runs the command as it came (what the real terminal does);
- reuse: reads the token from the fd, looks for it in `/proc/*/cmdline` and
  `/proc/*/environ` (must find 0), uses it once (connects without approval) and again
  (the level applies: the approval window opens and QA denies);
- expired: reads the token, waits 31 s and uses it (the level applies);
- other entry: reads the token of the jump entry and uses it on the internal entry with
  approval, which jumps through that same jump entry (the token covers neither another
  entry nor its jump: the level applies, the window opens).

Records in the test terminal log under `$WARD_DEV_DIR/logs/` (0600) one line per use
(use number, entry, exit code, connected, denied) and, in the reuse mode, how many
`/proc` files had the token out of how many were read. Never writes the token nor the
server output.
"""

import os
import re
import select
import socket
import subprocess
import sys
import time

TIMEOUT = 150
MARK = b"WARD-E2E-"


def die(msg):
    print(f"terminal-teste: {msg}", file=sys.stderr)
    sys.exit(2)


def dev_and_ward():
    dev = os.environ.get("WARD_DEV_DIR", "")
    if not dev.startswith("/") or not dev.endswith("/.dev") or "/../" in dev or "/./" in dev:
        die("WARD_DEV_DIR fora da forma <repositório>/.dev")
    a = sys.argv[1:]
    if len(a) != 6 or a[0] != "-e" or a[2:4] != ["ssh", "--token-fd"]:
        die("uso: terminal-test.py -e <ward> ssh --token-fd <n> <entrada>")
    ward = a[1]
    if ward != os.path.dirname(dev) + "/target/debug/ward":
        die("o ward não é o do repositório")
    if not re.fullmatch(r"[0-9]{1,7}", a[4]) or not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", a[5]):
        die("fd ou entrada fora da forma")
    return dev, ward, int(a[4]), a[5]


def record(dev, line):
    os.makedirs(dev + "/logs", mode=0o700, exist_ok=True)
    fd = os.open(dev + "/logs/terminal-teste.log", os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "a") as f:
        f.write(f"{time.strftime('%H:%M:%S')} {line}\n")


def environment():
    # What the app's terminal type passed (environment from scratch): nothing more.
    return dict(os.environ)


def run(argv, inherited=()):
    """Runs `argv` in a pty; sends the test command after the prompt. Returns
    (code, connected, denied)."""
    master, slave = os.openpty()
    p = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave, pass_fds=inherited,
                         env=environment(), cwd="/", start_new_session=True)
    os.close(slave)
    for fd in inherited:
        os.close(fd)
    output = bytearray()
    sent = False
    deadline = time.monotonic() + TIMEOUT
    while time.monotonic() < deadline:
        r, _, _ = select.select([master], [], [], 0.2)
        if r:
            try:
                b = os.read(master, 4096)
            except OSError:
                b = b""
            if not b:
                break
            output += b
        # busybox's ash asks for the cursor position (ESC[6n) after the prompt: the
        # terminal answers, and the prompt is searched without the escape sequences.
        if r and b"\x1b[6n" in b:
            os.write(master, b"\x1b[1;1R")
        screen_tail = re.sub(rb"\x1b\[[0-9;?]*[A-Za-z]", b"", bytes(output[-128:]))
        if not sent and re.search(rb"[$#] ?$", screen_tail.rstrip(b"\r\n ") + b" "):
            os.write(master, b"echo " + MARK + b"$(id -un); exit\n")
            sent = True
        if p.poll() is not None and not r:
            break
    try:
        code = p.wait(timeout=10)
    except subprocess.TimeoutExpired:
        p.kill()
        code = p.wait()
    os.close(master)
    text = bytes(output)
    # The pty output (prompt, banner, `ward ssh` error) goes to .dev/logs (0600), which the
    # log scan of `make test-app-e2e` checks against .dev/variations.txt.
    dev = os.environ["WARD_DEV_DIR"]
    fd = os.open(dev + "/logs/terminal-teste.saida", os.O_WRONLY | os.O_CREAT | os.O_APPEND | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "ab") as f:
        f.write(b"--- " + " ".join(argv[1:3] + argv[4:]).encode() + b"\n" + text[-2000:] + b"\n")
    connected = re.search(MARK + rb"[a-z]", text) is not None
    denied = b"negad" in text
    return code, connected, denied


def yes_no(flag):
    return "sim" if flag else "nao"


def channel(token):
    a, b = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
    a.sendall(token)
    a.close()
    return b.detach()


def read_token(fd):
    parts = []
    while True:
        b = os.read(fd, 128)
        if not b:
            break
        parts.append(b)
    os.close(fd)
    return b"".join(parts)


def scan_proc(token):
    found = read_count = 0
    for pid in os.listdir("/proc"):
        if not pid.isdigit() or int(pid) == os.getpid():
            continue
        for name in ("cmdline", "environ"):
            try:
                with open(f"/proc/{pid}/{name}", "rb") as f:
                    data = f.read()
            except OSError:
                continue
            read_count += 1
            if token in data:
                found += 1
    return found, read_count


def main():
    dev, ward, fd, entry = dev_and_ward()
    try:
        with open(dev + "/terminal-teste.modo") as f:
            mode = f.read().strip() or "normal"
    except FileNotFoundError:
        mode = "normal"
    if mode not in ("normal", "reuso", "expirado", "outra-entrada"):
        die("modo desconhecido")
    record(dev, f"abriu modo={mode} entrada={entry}")
    if mode == "normal":
        c, conn, den = run([ward, "ssh", "--token-fd", str(fd), entry], (fd,))
        record(dev, f"uso=1 entrada={entry} saida={c} conectou={yes_no(conn)} negado={yes_no(den)}")
        return 0
    token = read_token(fd)
    uses = []
    if mode == "reuso":
        found, read_count = scan_proc(token)
        record(dev, f"proc={found}/{read_count}")
        uses = [(entry, 0), (entry, 0)]
    elif mode == "expirado":
        uses = [(entry, 31)]
    else:
        uses = [("acme-ssh-interno-aprov", 0)]
    for i, (e, delay) in enumerate(uses, 1):
        time.sleep(delay)
        n = channel(token)
        c, conn, den = run([ward, "ssh", "--token-fd", str(n), e], (n,))
        record(dev, f"uso={i} entrada={e} saida={c} conectou={yes_no(conn)} negado={yes_no(den)}")
    token = b"\0" * len(token)
    return 0


if __name__ == "__main__":
    sys.exit(main())
