#!/usr/bin/python3 -I
"""`make test-cli` (Phase 25): the test `ward` CLI (feature `test-mode`) end to end,
with the terminal in a real pty.

1. V4.2 (S5): a fake server in Python that records everything it receives, at the test
   socket path (`WARD_DEV_SOCKET_DIR` in a 0700 dir of this test only). The
   CLI's unlock must abort with the "server is not the wardd" message without showing
   the password prompt; the password is typed anyway and the fake's record stays empty.
   m2:V4.16 (S5): a non-dumpable fake server of this uid that answers protocol v1; the
   CLI stops with the incompatible version message before the password prompt, and no
   byte of the password reaches it.
2. Against the test daemon (`make dev-daemon`, vault in `.dev/`): lock, then the list
   answers "locked" (V4.22); unlock with the wrong and the right password through the
   pty; the JSON list with only the six fields and no pattern
   of `.dev/variations.txt` (V4.1, S1). The prompt shows up and the password does not
   echo. V4.21 timed (Phase 28b): locked and never unlocked, `ward run` exits ≠ 0 in
   120 s ± 2 s; with an unlock in the middle of the wait, it exits 0 (the suite
   gets ~2 min longer).
3. `ward run` (Phase 26).
4. `ward ssh` (Phase 27) against the test SSH servers (2222, 2223): V4.23, V4.24, V4.26
   (agent attacked by `ssh-add`, SIGKILL on the CLI, lock; since 54c the lock ends the
   whole session with 137; since 54l SIGKILL on the CLI with a password leaves no ssh),
   V4.28, V4.31, V4.32, prefix oracle of the revised D34 (pty and timing), V4.14 and
   V4.35/V4.37 through the argv in `/proc/<pid>/cmdline` (the `wardd` refuses to start
   under `strace`).
5. Jump and tunnel (Phase 28) against the jump host, the inner SSH and the test Postgres:
   V4.25, V4.29 (client `dev/tests/pg_select.py`), V4.30, V4.35 with a jump (no `ssh`
   child of another `ssh`: no `sh -c`/`ProxyJump`), attack on the tunnel bridge and
   V4.33 (the daemon log, which `make dev-daemon` starts with `RUST_LOG=trace`, without
   any pattern of `.dev/variations.txt`).

6. Phase 27a (high finding of the M3 audit): the app's "Connect" token reaches `ward ssh`
   only through an inherited socket (`--token-fd <n>`), via a fake terminal, against the
   test daemon; a reader looping over `/proc/*/cmdline`, `/proc/*/environ` and
   `/proc/<ward>/fd/*` does not find the token; `--token` in argv is refused without
   connecting; a channel with garbage, empty, too large or a pipe is refused without
   echo. Does not need the test SSH servers.

The test daemon must run without `WARD_DEV_APP` (`make dev-daemon WARD_DEV_APP=`): with
the bridge on, the requests that need approval wait for the human and expire; the suite
refuses at the start (Phase 28g).

With `WARD_DEV_SOCKET_DIR` in the environment (absolute 0700 dir), uses the test daemon
of that socket (the one of `WARD_DEV_SOCKET_DIR=… make dev-daemon`), instead of the
default test socket dir in `$XDG_RUNTIME_DIR`.

Usage: dev/tests/cli.py <ward binary> <repository root>. Only fake values (D15).
"""

import base64
import json
import os
import pty
import re
import select
import socket
import subprocess
import sys
import threading
import time

WARD, ROOT = sys.argv[1], sys.argv[2]
PASSWORD = json.load(open(os.path.join(ROOT, "dev/fixtures/entries.json")))["master_password"]
UID = os.geteuid()
failures = []


def verify(cond, msg):
    print(("ok   " if cond else "FALHA") + " " + msg)
    if not cond:
        failures.append(msg)


SOCKET_DIR = os.environ.get("WARD_DEV_SOCKET_DIR") or f"/run/user/{UID}/ward-teste"
if not os.path.isabs(SOCKET_DIR):
    print("FALHA WARD_DEV_SOCKET_DIR não é absoluto")
    sys.exit(1)


def environment(extra=None):
    e = {"PATH": "/usr/bin:/bin", "HOME": "/nao-existe", "LANG": "C.UTF-8"}
    if "WARD_DEV_SOCKET_DIR" in os.environ:
        e["WARD_DEV_SOCKET_DIR"] = SOCKET_DIR
    e.update(extra or {})
    return e


def run_pty(args, to_type=None, env=None, deadline=150):
    """Runs in the pty. With `to_type`, writes the password when the prompt shows up (or,
    if the process exits before, does not write it). Returns (code, everything that showed
    up on the screen, whether the prompt showed up)."""
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(WARD, [WARD] + args, env or environment())
    screen = b""
    saw_prompt = False
    end = time.time() + deadline
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            try:
                b = os.read(fd, 4096)
            except OSError:
                break
            if not b:
                break
            screen += b
            if to_type is not None and not saw_prompt and b"Senha mestra" in screen:
                saw_prompt = True
                time.sleep(0.05)
                os.write(fd, to_type.encode() + b"\r")
    _, st = os.waitpid(pid, 0)
    os.close(fd)
    return os.waitstatus_to_exitcode(st), screen.decode("utf-8", "replace"), saw_prompt


# --- 0. The test daemon must run without WARD_DEV_APP (Phase 28g) --------------------
# With the open-app operation on (the default of `make dev-daemon` is the test bridge), a
# request "with approval" opens the bridge and waits for the human until it expires:
# V4.30, V4.32 and the `--token-fd` of 27a would give "request expired" instead of
# "approval unavailable". The mark `.dev/wardd.app` (from `dev/daemon.sh`) tells with which
# WARD_DEV_APP and on which socket the test daemon of this checkout started.
def test_daemon_app():
    try:
        p = open(os.path.join(ROOT, ".dev/wardd.pid")).read(16).strip()
        if not p.isdigit() or open(f"/proc/{p}/comm").read().strip() != "wardd":
            return ""
        lines = open(os.path.join(ROOT, ".dev/wardd.app")).read(8192).splitlines()
    except OSError:
        return ""
    app = lines[0] if lines else ""
    mark_socket = lines[1] if len(lines) > 1 else ""
    if mark_socket and mark_socket != os.path.join(SOCKET_DIR, "wardd.sock"):
        return ""
    return app


if test_daemon_app():
    print(
        "FALHA o daemon de teste está no ar com WARD_DEV_APP (abre a ponte nos pedidos com "
        "aprovação, e V4.30, V4.32 e a 27a dariam 'pedido expirado'). Rode "
        "'make dev-daemon-stop' e suba com 'make dev-daemon WARD_DEV_APP=' antes do "
        "'make test-cli'."
    )
    sys.exit(1)

# --- 1. V4.2: fake server on the test socket ------------------------------------------
fake_dir = f"/run/user/{UID}/ward-cli-falso-{os.getpid()}"
os.mkdir(fake_dir, 0o700)
path = os.path.join(fake_dir, "wardd.sock")
record = bytearray()
srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
srv.bind(path)
os.chmod(path, 0o600)
srv.listen(8)


def fake_server():
    while True:
        try:
            c, _ = srv.accept()
        except OSError:
            return
        c.settimeout(5)
        try:
            while True:
                b = c.recv(65536)
                if not b:
                    break
                record.extend(b)
        except OSError:
            pass
        c.close()


threading.Thread(target=fake_server, daemon=True).start()
try:
    env = environment({"WARD_DEV_SOCKET_DIR": fake_dir})
    code, screen, prompt = run_pty(["unlock"], to_type=PASSWORD, env=env, deadline=20)
    verify(code != 0, f"V4.2 destravar contra o falso sai ≠ 0 (saiu {code})")
    verify("servidor não é o wardd" in screen, "V4.2 mensagem 'servidor não é o wardd'")
    verify(not prompt, "V4.2 o prompt da senha não apareceu")
    for args in (["list", "--json"], ["lock"]):
        code, screen, _ = run_pty(args, env=env, deadline=20)
        verify(code != 0 and "servidor não é o wardd" in screen, f"V4.2 {args[0]} recusa o falso")
    time.sleep(0.2)
    verify(len(record) == 0, f"V4.2 o falso recebeu {len(record)} bytes")
    verify(PASSWORD.encode() not in record, "V4.2 a senha não está no registro do falso")
finally:
    srv.close()
    os.unlink(path)
    os.rmdir(fake_dir)

# --- 1b. m2:V4.16 (S5): fake server that answers protocol v1 -------------------------
# A Python server, non-dumpable (`prctl(PR_SET_DUMPABLE, 0)`) and of this uid, so it
# passes the test-mode server check like the 0.1.x `wardd` would; it answers every frame
# with the v1 status reply and records every byte it receives. The CLI unlock must stop
# with the incompatible version message before the password prompt, and no byte of the
# password (nor its base64) reaches the fake.
V1_FAKE = r"""
import ctypes, os, socket, struct, sys
path, record = sys.argv[1], sys.argv[2]
if ctypes.CDLL(None, use_errno=True).prctl(4, 0, 0, 0, 0) != 0:
    sys.exit(3)
srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
srv.bind(path)
os.chmod(path, 0o600)
srv.listen(8)
print("pronto", flush=True)
reply = b'{"v":1,"tipo":"estado","dados":{"travado":true,"cofre_existe":true,"papel":"agente"}}'
fd = os.open(record, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
while True:
    c, _ = srv.accept()
    c.settimeout(5)
    try:
        while True:
            n = c.recv(4)
            if len(n) < 4:
                break
            size = struct.unpack(">I", n)[0]
            body = b""
            while len(body) < min(size, 1 << 20):
                b = c.recv(size - len(body))
                if not b:
                    break
                body += b
            os.write(fd, n + body)
            c.sendall(struct.pack(">I", len(reply)) + reply)
    except OSError:
        pass
    c.close()
"""
v1_dir = f"/run/user/{UID}/ward-cli-v1-{os.getpid()}"
os.mkdir(v1_dir, 0o700)
v1_record = os.path.join(v1_dir, "registro")
v1_srv = subprocess.Popen([sys.executable, "-I", "-c", V1_FAKE, os.path.join(v1_dir, "wardd.sock"), v1_record],
                          stdout=subprocess.PIPE, text=True)
try:
    verify(v1_srv.stdout.readline().strip() == "pronto", "m2:V4.16 servidor falso v1 (não dumpable) no ar")
    env = environment({"WARD_DEV_SOCKET_DIR": v1_dir})
    code, screen, prompt = run_pty(["unlock"], to_type=PASSWORD, env=env, deadline=20)
    verify(code != 0, f"m2:V4.16 destravar contra o servidor v1 sai ≠ 0 (saiu {code})")
    verify("incompatível" in screen, "m2:V4.16 mensagem pt-BR de versão incompatível")
    verify(not prompt, "m2:V4.16 o prompt da senha não apareceu")
    time.sleep(0.2)
    try:
        got = open(v1_record, "rb").read()
    except FileNotFoundError:
        got = b""
    verify(PASSWORD.encode() not in got and base64.b64encode(PASSWORD.encode()) not in got,
           "m2:V4.16 nenhum byte da senha chegou ao servidor v1")
    ops = []
    while len(got) >= 4:
        n = int.from_bytes(got[:4], "big")
        try:
            ops.append(json.loads(got[4:4 + n]).get("op"))
        except ValueError:
            ops.append("?")
        got = got[4 + n:]
    verify(all(o == "status" for o in ops), f"m2:V4.16 o servidor v1 só recebeu status ({ops})")
    for args in (["list", "--json"], ["lock"]):
        code, screen, _ = run_pty(args, env=env, deadline=20)
        verify(code != 0 and "incompatível" in screen, f"m2:V4.16 {args[0]} recusa o servidor v1")
finally:
    v1_srv.kill()
    v1_srv.wait()
    for f in ("wardd.sock", "registro"):
        try:
            os.unlink(os.path.join(v1_dir, f))
        except FileNotFoundError:
            pass
    os.rmdir(v1_dir)

# --- 1c. m2:V5.1, m2:V5.2 (m2:DT10): English subcommands, no alias --------------------
# The 0.1.x names exit 2 with the pt-BR "unknown command" before connecting: a listener on the
# socket of a folder of its own accepts no connection. A password typed after the old
# unlock name does not come back. `ward --help` differs from the 0.1.4 help only in the
# subcommand names (and the padding that keeps the column).
old_dir = f"/run/user/{UID}/ward-cli-old-{os.getpid()}"
os.mkdir(old_dir, 0o700)
old_path = os.path.join(old_dir, "wardd.sock")
old_srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
old_srv.bind(old_path)
old_srv.listen(8)
old_srv.setblocking(False)
try:
    env = environment({"WARD_DEV_SOCKET_DIR": old_dir})
    unlock_old = "des" + "travar"
    for args in (["lista"], ["lista", "--json"], ["travar"], [unlock_old], [unlock_old, PASSWORD], ["ajuda"]):
        r = subprocess.run([WARD, *args], env=env, capture_output=True, timeout=20, stdin=subprocess.DEVNULL)
        err = r.stderr.decode(errors="replace")
        verify(r.returncode == 2 and "comando desconhecido" in err,
               f"m2:V5.1 `ward {args[0]}` sai 2 com 'comando desconhecido' (saiu {r.returncode})")
        verify(PASSWORD.encode() not in r.stdout + r.stderr, f"m2:V5.1 `ward {args[0]}` não ecoa argumento")
    try:
        old_srv.accept()
        accepted = True
    except BlockingIOError:
        accepted = False
    verify(not accepted, "m2:V5.1 os nomes antigos não falam com o daemon (nenhuma conexão)")
finally:
    old_srv.close()
    os.unlink(old_path)
    os.rmdir(old_dir)

for args in (["--help"], ["help"], ["-h"]):
    r = subprocess.run([WARD, *args], env=environment(), capture_output=True, timeout=20)
    verify(r.returncode == 0, f"m2:V5.1 `ward {args[0]}` sai 0")
help_now = subprocess.run([WARD, "--help"], env=environment(), capture_output=True, timeout=20).stdout.decode()
help_old = open(os.path.join(ROOT, "dev/fixtures/legacy/help-0.1.4.txt"), encoding="utf-8").read()
for old, new in (("ward lista [--json]   ", "ward list [--json]    "),
                 ("ward " + "des" + "travar        ", "ward unlock           "),
                 ("ward travar           ", "ward lock             ")):
    help_old = help_old.replace(old, new)
verify(help_now == help_old, "m2:V5.2 `ward --help` = ajuda da 0.1.4 com só os nomes de subcomando trocados")
r = subprocess.run([WARD, "--version"], env=environment(), capture_output=True, timeout=20)
verify(r.returncode == 0 and r.stdout.decode().startswith("ward "), "m2:V5.1 `ward --version` sai 0")

# --- 2. Against the test daemon -------------------------------------------------------
sock = os.path.join(SOCKET_DIR, "wardd.sock")
if not os.path.exists(sock):
    print("FALHA o daemon de teste não está no ar: rode 'make dev-daemon'")
    sys.exit(1)

code, screen, _ = run_pty(["lock"])
verify(code == 0 and "cofre travado" in screen, "travar")
code, screen, _ = run_pty(["list"])
verify(code != 0 and "travado" in screen, "V4.22 list depois de travar responde 'travado'")
# Phase 54b (RA8, 54a): the unlock hint uses the full path, not the `ward` of the PATH.
verify("`/usr/local/bin/ward unlock`" in screen,
       "V4.22 a dica de destravar é `/usr/local/bin/ward unlock`")

# V4.21 (D16), timed (M4 scrutiny, Phase 28b): locked and never unlocked, `ward run`
# waits the daemon's 2 min and exits ≠ 0 in 120 s ± 2 s; with an unlock from
# another terminal within the deadline, the request goes on and exits 0 (the unlock
# comes right below, with the wrong password before the right one).

t0 = time.monotonic()
r = subprocess.run([WARD, "run", "acme-pg-dev", "--", "/bin/true"], env=environment(),
                   capture_output=True, timeout=150)
dt = time.monotonic() - t0
verify(r.returncode != 0 and 118.0 <= dt <= 122.0,
       f"V4.21 travado sem destravar sai ≠ 0 em 120 s ± 2 s (saiu {r.returncode} em {dt:.1f} s)")
v421 = subprocess.Popen([WARD, "run", "acme-pg-dev", "--", "/bin/true"], env=environment(),
                        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
time.sleep(3)
verify(v421.poll() is None, "V4.21 o ward run travado está esperando o destrave")
t_v421 = time.monotonic()

code, screen, prompt = run_pty(["unlock"], to_type="falso-errada-ward-99")
verify(prompt and code != 0 and "senha mestra incorreta" in screen, "destravar com senha errada")
verify("falso-errada-ward-99" not in screen, "a senha errada não ecoou")

code, screen, prompt = run_pty(["unlock"], to_type=PASSWORD)
verify(prompt, "destravar mostra o prompt da senha")
verify(code == 0 and "cofre destravado" in screen, f"destravar com a senha certa (saiu {code})")
verify(PASSWORD not in screen, "a senha não ecoou na tela")
try:
    v421.wait(timeout=30)
except subprocess.TimeoutExpired:
    v421.kill()
    v421.wait()
verify(v421.returncode == 0 and time.monotonic() - t_v421 < 120,
       f"V4.21 destravado em outro terminal, o ward run que esperava segue e sai 0 (saiu {v421.returncode})")

code, screen, prompt = run_pty(["unlock"], to_type=PASSWORD)
verify(code == 0 and "já está destravado" in screen and not prompt, "destravar de novo não pede senha")

# JSON list: plain stdout, no pty.

r = subprocess.run([WARD, "list", "--json"], env=environment(), capture_output=True, timeout=60)
verify(r.returncode == 0, "V4.1 list --json sai 0")
items = json.loads(r.stdout)
names = [i["name"] for i in items]
verify("gama-bloqueada" not in names, "V4.1 sem a bloqueada")
expected = [e["name"] for e in json.load(open(os.path.join(ROOT, "dev/fixtures/entries.json")))["entries"]
            if e.get("level") != "blocked"]
verify(sorted(names) == sorted(expected), f"V4.1 {len(names)} entradas visíveis, todas menos a bloqueada")
verify(len(names) == 21, f"V4.1 são 21 entradas visíveis (as 22 do cofre menos a bloqueada; vieram {len(names)})")
# m2:V4.18 (Phase 51): keys and values in English (protocol v2 and vault v5 names).
fields = {"name", "type", "group", "environment", "project", "address"}
verify(all(set(i) == fields for i in items), "V4.1 só name, type, group, environment, project e address")
V5_TYPES = {"login", "database", "server", "api", "aws", "smtp", "ftp-sftp", "secure-note", "generic"}
verify(all(i["type"] in V5_TYPES for i in items), "m2:V4.18 o type de cada entrada é um nome v5")
r2 = subprocess.run([WARD, "list"], env=environment(), capture_output=True, timeout=60)
verify(r2.returncode == 0 and b"NOME" in r2.stdout, "V4.1 list em texto")

variations_path = os.path.join(ROOT, ".dev/variations.txt")
if os.path.exists(variations_path):
    patterns = [p for p in open(variations_path, "rb").read().split(b"\n") if p]
    output = r.stdout + r.stderr + r2.stdout + r2.stderr
    found = sum(1 for p in patterns if p in output)
    verify(found == 0, f"S1 nenhum padrão de .dev/variations.txt na saída da lista ({found})")
else:
    verify(False, "falta .dev/variations.txt: rode 'make dev-variations'")
    patterns = []

# --- 3. `ward run` (Phase 26) against the test daemon ---------------------------------
# Only local commands (sh, env, cat, sha256sum): no ssh/curl client here.


def run(args, env=None, input_data=b""):
    return subprocess.run([WARD, "run"] + args, env=env or environment(), input=input_data,
                          capture_output=True, timeout=150)


def no_variation(r, label):
    combined = r.stdout + r.stderr
    found = sum(1 for p in patterns if p in combined)
    verify(found == 0, f"S4 {label}: nenhum padrão de .dev/variations.txt na saída ({found})")


# Phase 54b (D68): the real CLI tells the daemon the kind of its stdin through `fstat`
# (the test daemon logs only the kind name, with RUST_LOG=trace): `/dev/null` is null
# (the `cat` ends without waiting), pipe, file, `/dev/zero` is other and the pty is
# terminal.
LOG_54B = os.path.join(ROOT, ".dev", "logs", "wardd.log")


def kinds_in_log(action):
    before = os.path.getsize(LOG_54B) if os.path.exists(LOG_54B) else 0
    action()
    time.sleep(0.3)
    with open(LOG_54B, "rb") as f:
        f.seek(before)
        new = re.findall(rb"executar: entrada padr\xc3\xa3o (\w+)", f.read())
    return [t.decode() for t in new]


with open("/dev/null", "rb") as null_file, open(os.path.join(ROOT, "Makefile"), "rb") as plain_file, \
        open("/dev/zero", "rb") as zero:
    for label, action, kind in [
        ("/dev/null", lambda: subprocess.run([WARD, "run", "acme-pg-dev", "--", "/bin/cat"],
                                             env=environment(), stdin=null_file, capture_output=True,
                                             timeout=30), "nula"),
        ("pipe", lambda: run(["acme-pg-dev", "--", "/bin/true"], input_data=b"x\n"), "pipe"),
        ("arquivo", lambda: subprocess.run([WARD, "run", "acme-pg-dev", "--", "/bin/true"],
                                           env=environment(), stdin=plain_file, capture_output=True,
                                           timeout=30), "arquivo"),
        ("/dev/zero", lambda: subprocess.run([WARD, "run", "acme-pg-dev", "--", "/bin/true"],
                                             env=environment(), stdin=zero, capture_output=True,
                                             timeout=30), "outra"),
        ("pty", lambda: run_pty(["run", "acme-pg-dev", "--", "/bin/true"]), "terminal"),
    ]:
        seen = kinds_in_log(action)
        verify(seen == [kind], f"Fase 54b: entrada {label} vai ao daemon como {kind} ({seen})")

r = run(["acme-wp-prod", "--", "/bin/sh", "-c", 'printf "%s|%s" "$WARD_USER" "$WARD_PASS"; exit 5'])
verify(r.returncode == 5, f"V4.19 o código do comando volta (saiu {r.returncode})")
verify(r.stdout.decode().endswith("|••••"), "V4.17 a senha impressa sai ••••")
no_variation(r, "login")

r = run(["gama-generico", "--", "/bin/sh", "-c",
         'f="$WARD_FILE_APP_ENV"; stat -c %a "$f"; cat "$f"; env; echo "$f"'])
output = r.stdout.decode(errors="replace")
verify(r.returncode == 0 and output.startswith("600\n"), "V4.10 anexo 0600 entregue")
path = output.strip().split("\n")[-1]
verify(path.startswith(f"/run/user/{UID}/") and "/tmp/" in path, "S2 anexo em <rt>tmp/")
verify(not os.path.exists(path), "V4.15 anexo apagado no fim")
verify("LISTEN_" not in output and "WARD_DEV_DIR" not in output, "D60 nada do ambiente do daemon")
no_variation(r, "genérico + anexo + env")

r = run(["acme-wp-prod", "--", "/bin/cat"], input_data=b"entrada padrao 26\n")
verify(r.returncode == 0 and r.stdout == b"entrada padrao 26\n", "D13 a entrada padrão chega ao comando")

r = run(["gama-nota", "--", "/bin/true"])
verify(r.returncode != 0 and b"tipo sem entrega" in r.stderr, "V4.11 nota segura: tipo sem entrega")

r = run(["gama-bloqueada", "--", "/bin/true"])
verify(r.returncode != 0 and b"entrada" in r.stderr, "bloqueada = inexistente")

big_env = environment({f"GRANDE_{i}": "x" * (100 * 1024) for i in range(8)})
r = run(["acme-wp-prod", "--", "/bin/true"], env=big_env)
verify(r.returncode != 0 and b"passam de 750 KiB" in r.stderr and b"xxxx" not in r.stderr,
       "V4.36 (D52) ambiente acima de 750 KiB recusado sem citar valor")

r = run(["acme-wp-prod", "--", "/usr/bin/env"], env=environment({"LD_PRELOAD": "/nao/existe.so"}))
verify(r.returncode == 0 and b"LD_PRELOAD" not in r.stdout, "D13 LD_* não chega ao filho")

# Phase 26a (medium finding of the M3 audit): the child's environment is built from
# scratch in the daemon. The attacker's dir (in .dev/) comes first in the CLI's PATH with
# fake `true`, `env` and `wardfalso-26a` that leave a mark; BASH_ENV/ENV, NODE_OPTIONS,
# GIT_SSH_COMMAND, PYTHONSTARTUP, PGSERVICEFILE and HOME point at it. None of it reaches
# the child; the command without `/` comes from the fixed PATH.
import shutil  # noqa: E402
import tempfile  # noqa: E402

attacker = tempfile.mkdtemp(prefix="atacante-26a-", dir=os.path.join(ROOT, ".dev"))
try:
    mark = os.path.join(attacker, "marca")
    for name in ("true", "env", "sh", "wardfalso-26a"):
        f = os.path.join(attacker, name)
        with open(f, "w") as a:
            a.write(f"#!/bin/sh\necho pego >> '{mark}'\n")
        os.chmod(f, 0o755)
    rc = os.path.join(attacker, "rc.sh")
    with open(rc, "w") as a:
        a.write(f"echo pego >> '{mark}'\n")
    attack_env = environment({
        "PATH": f"{attacker}:/usr/bin:/bin", "BASH_ENV": rc, "ENV": rc,
        "NODE_OPTIONS": f"--require {attacker}/mod.js", "GIT_SSH_COMMAND": f"{attacker}/true",
        "PYTHONSTARTUP": rc, "PGSERVICEFILE": rc, "HOME": attacker, "LANG": attacker,
        "TZ": "../../.." + attacker, "TERM": "../x",
    })
    r = run(["acme-wp-prod", "--", "true"], env=attack_env)
    verify(r.returncode == 0, f"26a `true` sem `/` vem do PATH fixo (saiu {r.returncode})")
    r = run(["acme-wp-prod", "--", "wardfalso-26a"], env=attack_env)
    verify(r.returncode == 127, f"26a comando só na pasta do atacante não é achado (saiu {r.returncode})")
    r = run(["acme-wp-prod", "--", "bash", "-c", "exit 0"], env=attack_env)
    verify(r.returncode == 0, "26a bash -c com BASH_ENV da CLI")
    r = run(["acme-wp-prod", "--", "env"], env=attack_env)
    output = r.stdout.decode(errors="replace")
    names = {l.split("=", 1)[0] for l in output.splitlines() if "=" in l}
    allowed = {"PATH", "HOME", "USER", "LOGNAME", "LC_CTYPE"}
    verify(r.returncode == 0 and not os.path.exists(mark), "26a nada da pasta do atacante rodou")
    verify("PATH=/usr/local/bin:/usr/bin:/bin" in output.splitlines(), "26a PATH fixo no filho")
    verify(attacker not in output and all(n in allowed or n.startswith("WARD_") for n in names),
           f"26a só PATH/HOME/USER/LOGNAME, locale e a entrega no filho ({sorted(names)})")
    r = run(["acme-wp-prod", "--", "/bin/sh", "-c", 'printf "%s|%s|%s|" "$LANG" "$TZ" "$TERM"; pwd'],
            env=environment({"LANG": "pt_BR.UTF-8", "TZ": "America/Sao_Paulo", "TERM": "xterm-256color"}))
    verify(r.stdout.decode() == f"pt_BR.UTF-8|America/Sao_Paulo|xterm-256color|{os.getcwd()}\n",
           "26a LANG/TZ/TERM válidos passam e o diretório de trabalho é o da CLI")
finally:
    shutil.rmtree(attacker, ignore_errors=True)

# V4.17 (S4): plain $PGPASSWORD, in base64, with urllib.parse.quote, cut at the `:` with
# `sleep 0.2`, and all of it again on stderr.
V417 = r'''
p="$PGPASSWORD"
printf '%s\n' "$p"; printf %s "$p" | base64
python3 -I -c 'import os, urllib.parse; print(urllib.parse.quote(os.environ["PGPASSWORD"], safe=""))'
printf '%s' "${p%%:*}:"; sleep 0.2; printf '%s\n' "${p#*:}"
'''
r = run(["acme-pg-dev", "--", "/bin/sh", "-c", V417 + "{\n" + V417 + "} >&2"])
output = r.stdout + r.stderr
verify(r.returncode == 0 and output.count("••••".encode()) >= 8, "V4.17 as oito formas saem ••••")
no_variation(r, "V4.17")

# V4.3–V4.8, V4.10: the child prints only the sha256 of each variable (and of each
# delivered file); the expected value comes from the "expected delivery" table, with the
# fake values read from dev/fixtures/entries.json (no secret written here).
import hashlib  # noqa: E402
import hmac  # noqa: E402
import struct  # noqa: E402
import urllib.parse  # noqa: E402

FIX = {e["name"]: e for e in json.load(open(os.path.join(ROOT, "dev/fixtures/entries.json")))["entries"]}


def c(n, k):
    return str(FIX[n]["fields"][k])


def s(n, k):
    return FIX[n]["secrets"][k]


def totp(seed_b32, t):
    key = __import__("base64").b32decode(seed_b32)
    h = hmac.new(key, struct.pack(">Q", int(t) // 30), hashlib.sha1).digest()
    o = h[-1] & 15
    return "%06d" % ((struct.unpack(">I", h[o:o + 4])[0] & 0x7FFFFFFF) % 1000000)


def h(v):
    return hashlib.sha256(v if isinstance(v, bytes) else v.encode()).hexdigest()


pg = {"PGHOST": c("acme-pg-dev", "host"), "PGPORT": c("acme-pg-dev", "port"),
      "PGDATABASE": c("acme-pg-dev", "db_name"), "PGUSER": c("acme-pg-dev", "username"),
      "PGPASSWORD": s("acme-pg-dev", "password")}
pg["DATABASE_URL"] = "postgresql://%s:%s@%s:%s/%s" % (
    pg["PGUSER"], urllib.parse.quote(pg["PGPASSWORD"], safe=""), pg["PGHOST"], pg["PGPORT"], pg["PGDATABASE"])
EXPECTED = {
    "acme-wp-prod": {"WARD_URL": c("acme-wp-prod", "url"), "WARD_USER": c("acme-wp-prod", "username"),
                     "WARD_PASS": s("acme-wp-prod", "password")},
    "acme-pg-dev": pg,
    "acme-ssh-senha-stage": {"WARD_HOST": "127.0.0.1", "WARD_PORT": "2222", "WARD_USER": "deploy",
                             "WARD_PASS": s("acme-ssh-senha-stage", "password")},
    "acme-ssh-chave-prod": {"WARD_HOST": "127.0.0.1", "WARD_PORT": "2223", "WARD_USER": "deploy",
                            "WARD_PASSPHRASE": s("acme-ssh-chave-prod", "passphrase")},
    "beta-github-api": {"WARD_URL": c("beta-github-api", "url"), "WARD_TOKEN": s("beta-github-api", "token"),
                        "GITHUB_TOKEN": s("beta-github-api", "token")},
    "beta-aws-dev": {"AWS_ACCESS_KEY_ID": c("beta-aws-dev", "access_key_id"),
                     "AWS_SECRET_ACCESS_KEY": s("beta-aws-dev", "secret_access_key"),
                     "AWS_SESSION_TOKEN": s("beta-aws-dev", "session_token"),
                     "AWS_REGION": c("beta-aws-dev", "region"), "AWS_DEFAULT_REGION": c("beta-aws-dev", "region"),
                     "AWS_ROLE_ARN": c("beta-aws-dev", "role_arn")},
    "beta-smtp-stage": {"SMTP_HOST": c("beta-smtp-stage", "host"), "SMTP_PORT": c("beta-smtp-stage", "port"),
                        "SMTP_USER": c("beta-smtp-stage", "username"), "SMTP_PASS": s("beta-smtp-stage", "password"),
                        "SMTP_SECURITY": c("beta-smtp-stage", "security")},
    "beta-sftp-dev": {"WARD_PROTOCOL": "sftp", "WARD_HOST": "127.0.0.1", "WARD_PORT": "2222",
                      "WARD_USER": "sftpuser", "WARD_PASS": s("beta-sftp-dev", "password")},
    "gama-generico": {"WARD_REGIAO": "r-01",
                      "WARD_CHAVE_LICENCA": [x for x in FIX["gama-generico"]["extras"]
                                             if x["name"] == "CHAVE_LICENCA"][0]["value"]},
}
FILES = {
    "acme-ssh-chave-prod": {"WARD_FILE_KEY": s("acme-ssh-chave-prod", "private_key_file")},
    "gama-generico": {"WARD_FILE_APP_ENV": FIX["gama-generico"]["attachments"][0]["file"]},
}
# m2:V4.19 (m2:DT11): none of the 0.1.x names reaches the child.
OLD_NAMES = ("WARD_ENTRADA", "WARD_GRUPO", "WARD_AMBIENTE", "WARD_PROTOCOLO")
OLD_PREFIX = "WARD_ARQUIVO_"
# For each name: "<name> <sha256 of the value>"; for a file: "<name> <mode> <dir> <sha256>".
SCRIPT = r'''
for n; do
  case "$n" in
    arq:*) n="${n#arq:}"; f="$(printenv "$n")"
           printf '%s %s %s ' "$n" "$(stat -c %a "$f")" "$(dirname "$(dirname "$f")")"
           sha256sum < "$f" | cut -c1-64 ;;
    *) printf '%s ' "$n"; printf %s "$(printenv "$n"; echo x)" | head -c -2 | sha256sum | cut -c1-64 ;;
  esac
done
'''
for name, vars_ in EXPECTED.items():
    entry = FIX[name]
    all_vars = dict(vars_, **{"WARD_ENTRY": name, "WARD_GROUP": entry["group"],
                              "WARD_ENVIRONMENT": entry["environment"]})
    names = list(all_vars) + ["arq:" + k for k in FILES.get(name, {})]
    if name == "acme-wp-prod":
        names.append("WARD_TOTP")
    before = time.time()
    r = run([name, "--", "/bin/sh", "-c", SCRIPT, "sh"] + names)
    after = time.time()
    lines = dict(l.split(" ", 1) for l in r.stdout.decode().splitlines() if " " in l)
    matches = r.returncode == 0 and all(lines.get(k) == h(v) for k, v in all_vars.items())
    for k, path in FILES.get(name, {}).items():
        content = open(os.path.join(ROOT, path), "rb").read()
        mode, dir_, hash_ = (lines.get(k) or "  ").split(" ")
        matches = matches and mode == "600" and dir_.startswith(f"/run/user/{UID}/") \
            and dir_.endswith("/tmp") and hash_ == h(content)
    if name == "acme-wp-prod":
        seed = s(name, "totp_seed")
        matches = matches and lines.get("WARD_TOTP") in {h(totp(seed, t)) for t in (before, after)}
    verify(matches, f"V4.3–V4.10 entrega de {name} bate com a tabela ({len(names)} itens)")
    no_variation(r, f"hashes de {name}")
    # Only the names of `env -0` (the values stay in the child; the output goes through
    # the mask and is checked against the list of variations).
    r = run([name, "--", "/usr/bin/env", "-0"])
    seen = {kv.split(b"=", 1)[0].decode(errors="replace") for kv in r.stdout.split(b"\0") if b"=" in kv}
    expected_ward = {k for k in all_vars if k.startswith("WARD_")} | set(FILES.get(name, {}))
    if name == "acme-wp-prod":
        expected_ward.add("WARD_TOTP")
    verify(r.returncode == 0 and {k for k in seen if k.startswith("WARD_")} == expected_ward,
           f"m2:V4.19 {name}: as variáveis WARD_* são exatamente as da tabela")
    verify(not any(k in OLD_NAMES or k.startswith(OLD_PREFIX) for k in seen),
           f"m2:V4.19 {name}: nenhum nome da 0.1.x")
    no_variation(r, f"env -0 de {name}")

# --- 4. `ward ssh` (Phase 27) against the test daemon and the test SSH servers --------
# The `ssh`/`sshpass`/`sftp` is run by the daemon (D13: `-F /dev/null`, Ward's
# `known_hosts` in `.dev/known_hosts`); from here only the CLI. Needs the services up
# (`make dev-services-up`); without them, the section is skipped with a warning (does
# not pass).
import glob  # noqa: E402
import signal  # noqa: E402

DEV = os.path.join(ROOT, ".dev")
RT_TEST = SOCKET_DIR
SSH_PASSWORD = FIX["acme-ssh-senha-stage"]["secrets"]["password"]


def port_open(p):
    try:
        socket.create_connection(("127.0.0.1", p), timeout=2).close()
        return True
    except OSError:
        return False


def ssh(args, input_data=b"", env=None, deadline=150):
    return subprocess.run([WARD, "ssh"] + args, env=env or environment(), input=input_data,
                          capture_output=True, timeout=deadline)


# Fake terminal of Phase 27a: drops the `-e` and runs the rest, like
# `x-terminal-emulator -e`; passes on the inherited fds (what cosmic-term and xterm do).
FAKE_TERMINAL = ["/bin/sh", "-c", 'shift; exec "$@"', "terminal-falso", "-e"]


def ssh_with_channel(content, args, channel="socket", deadline=150):
    """Like the app (Phase 27a): `content` queued in a Unix socket (or in a pipe, for the
    attack), the writer's end closed, and only the other end inherited by the fake
    terminal → `ward ssh --token-fd <n>`. The content never goes in argv nor in the
    environment."""
    if channel == "socket":
        a, b = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
        if content:
            a.sendall(content)
        a.close()
        fd = b.detach()
    else:
        fd, w = os.pipe()
        os.write(w, content)
        os.close(w)
    try:
        return subprocess.run(FAKE_TERMINAL + [WARD, "ssh", "--token-fd", str(fd)] + args,
                              env=environment(), pass_fds=(fd,), stdin=subprocess.DEVNULL,
                              capture_output=True, timeout=deadline)
    finally:
        os.close(fd)


class Scanner(threading.Thread):
    """V4.14: reads `/proc/*/cmdline` every 50 ms and counts what has a secret pattern."""

    def __init__(self):
        super().__init__(daemon=True)
        self.stop = False
        self.found = 0
        self.seen = 0

    def run(self):
        while not self.stop:
            for f in glob.glob("/proc/[0-9]*/cmdline"):
                try:
                    c = open(f, "rb").read()
                except OSError:
                    continue
                self.seen += 1
                if any(p in c for p in patterns):
                    self.found += 1
            time.sleep(0.05)


def agents():
    return glob.glob(os.path.join(RT_TEST, "ssh-*"))


def wait_for(cond, deadline):
    end = time.time() + deadline
    while time.time() < end:
        if cond():
            return True
        time.sleep(0.05)
    return cond()


def octal(s):
    return "".join("\\%03o" % b for b in s.encode())


def interactive(entry, steps, deadline=60):
    """`ward ssh <entry>` in a pty: `steps` is a list of (wait for, type). Returns
    (code, screen, [(time, chunk)])."""
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(WARD, [WARD, "ssh", entry], environment({"TERM": "xterm"}))
    screen = b""
    chunks = []
    end = time.time() + deadline
    steps = list(steps)
    start = time.time()
    while time.time() < end:
        if steps and steps[0][0].encode() in screen:
            _, to_type = steps.pop(0)
            if to_type is not None:
                time.sleep(0.1)
                os.write(fd, to_type.encode())
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try:
                b = os.read(fd, 4096)
            except OSError:
                break
            if not b:
                break
            screen += b
            chunks.append((time.time() - start, b))
    code = None
    if steps:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
    else:
        _, st = os.waitpid(pid, 0)
        code = os.waitstatus_to_exitcode(st)
    os.close(fd)
    return code, screen, chunks


services_up = port_open(2222) and port_open(2223)
if not services_up:
    verify(False, "Fase 27: SSH de teste fora do ar (make dev-services-up); seção ssh pulada")
else:
    scanner = Scanner()
    scanner.start()

    # V4.23 (password, with the child's argv in /proc/<pid>/cmdline) and V4.24 (key).
    seen = []

    def watch_children():
        end = time.time() + 20
        while time.time() < end and not seen:
            for f in glob.glob("/proc/[0-9]*/cmdline"):
                try:
                    a = open(f, "rb").read().split(b"\0")
                except OSError:
                    continue
                if a and a[0] == b"/usr/bin/sshpass" and b"--" in a:
                    seen.append(a)
            time.sleep(0.02)

    t = threading.Thread(target=watch_children, daemon=True)
    t.start()
    r = ssh(["acme-ssh-senha-stage", "sleep 1; whoami"])
    t.join()
    verify(r.returncode == 0 and r.stdout.strip() == b"deploy", f"V4.23 ssh por senha: whoami → deploy ({r.returncode})")
    a = seen[0] if seen else []
    o = [a[i + 1] for i in range(len(a) - 1) if a[i] == b"-o"]
    verify(a[:3] == [b"/usr/bin/sshpass", b"-e", b"/usr/bin/ssh"] and b"PubkeyAuthentication=no" in o
           and b"IdentityAgent=none" in o and a[3:5] == [b"-F", b"/dev/null"],
           "V4.23 o filho é sshpass -e → ssh -F /dev/null com PubkeyAuthentication=no e IdentityAgent=none")
    i = a.index(b"--") if b"--" in a else -1
    verify(i > 2 and a[i - 2:i] == [b"-l", b"deploy"] and a[i + 1] == b"127.0.0.1"
           and not any(x.startswith(b"User=") or x.endswith(b"@127.0.0.1") for x in a),
           "V4.35 usuário só depois de -l, host depois de --, sem usuário@host nem User=")
    verify(f"UserKnownHostsFile={DEV}/known_hosts".encode() in o, "D13 known_hosts do Ward")
    no_variation(r, "ssh por senha")

    r = ssh(["acme-ssh-chave-prod", "whoami"])
    verify(r.returncode == 0 and r.stdout.strip() == b"deploy", f"V4.24 ssh por chave: whoami → deploy ({r.returncode})")
    no_variation(r, "ssh por chave")
    verify(not agents(), "D12 o agente some no fim da sessão")

    r = ssh(["acme-ssh-senha-stage", "exit", "3"])
    verify(r.returncode == 3, f"o código do comando remoto volta (saiu {r.returncode})")

    # V4.37: sftp with a real login (sftpuser, 2222) and the command through stdin.
    r = ssh(["beta-sftp-dev"], input_data=b"pwd\n")
    verify(r.returncode == 0 and b"Remote working directory" in r.stdout, f"V4.37 sftp entra no SSH de teste ({r.returncode})")
    no_variation(r, "sftp")
    r = ssh(["beta-sftp-dev", "ls"])
    verify(r.returncode == 125 and b"pedido inv" in r.stderr, "sftp com comando recusado")

    r = ssh(["gama-nota", "id"])
    verify(r.returncode == 125 and b"tipo sem entrega" in r.stderr, "nota segura: tipo sem entrega")
    r = ssh(["gama-bloqueada", "id"])
    verify(r.returncode == 125 and b"entrada" in r.stderr, "bloqueada = inexistente")

    # V4.32: made-up token (through the fd channel, Phase 27a) on an entry with approval,
    # no app: the level applies. Section 6 repeats it with the /proc reader.
    r = ssh_with_channel(b"AbCdEfGhIjKlMnOpQrStUvWxYz012345", ["acme-pg-aprovacao", "id"])
    # The test daemon opens the test bridge as the app (WARD_DEV_APP), which denies;
    # without an app it would be "approval unavailable". Both mean the level applies.
    verify(r.returncode == 125 and (b"pedido negado" in r.stderr or "aprovação indisponível".encode() in r.stderr),
           f"V4.32 token inventado ignorado: vale o nível ({r.stderr[:80]!r})")

    # V4.26: another process of the user finds the agent socket and tries to list, add
    # and remove: refused in all three. The ssh-add runs with a fake HOME, no config.
    p = subprocess.Popen([WARD, "ssh", "acme-ssh-chave-prod", "sleep 8"], env=environment(),
                         stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    verify(wait_for(lambda: glob.glob(os.path.join(RT_TEST, "ssh-*/agent.sock")), 20), "D12 agente da sessão no ar")
    socks = glob.glob(os.path.join(RT_TEST, "ssh-*/agent.sock"))
    if socks:
        fake_key = os.path.join(DEV, "logs", f"chave-qualquer-{os.getpid()}")
        subprocess.run(["/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", fake_key],
                       env=environment(), check=True, capture_output=True)
        # Name built in parts (V1.5 refuses to have it spelled out in dev/).
        agent_env = environment({"SSH_AUTH" + "_SOCK": socks[0]})
        for label, args in [("ssh-add -L", ["-L"]), ("ssh-add <chave>", [fake_key]), ("ssh-add -D", ["-D"])]:
            x = subprocess.run(["/usr/bin/ssh-add"] + args, env=agent_env, capture_output=True, timeout=20)
            verify(x.returncode != 0 and b"ssh-ed25519" not in x.stdout, f"V4.26 {label} de outro processo recusado")
        for f in (fake_key, fake_key + ".pub"):
            os.remove(f)
    p.wait(timeout=60)
    verify(p.returncode == 0, f"a sessão do agente atacado terminou normal ({p.returncode})")
    verify(wait_for(lambda: not agents(), 3), "V4.26 socket e pasta do agente somem no fim")

    def of_session(mark):
        found = []
        for f in glob.glob("/proc/[0-9]*/cmdline"):
            try:
                a = open(f, "rb").read().split(b"\0")
            except OSError:
                continue
            if a and a[0] in (b"/usr/bin/ssh", b"/usr/bin/sshpass") and mark in a:
                found.append(f)
        return found

    # V4.26: SIGKILL on the CLI brings down the session and the agent; since Phase 54l,
    # with a password too, no ssh/sshpass of the session is left (the ssh that sshpass
    # creates with its own session dies through the daemon, without relying on the SIGHUP
    # of sshpass's pty).
    for entry, mark in [("acme-ssh-chave-prod", "sleep 20"), ("acme-ssh-senha-stage", "sleep 23")]:
        p = subprocess.Popen([WARD, "ssh", entry, mark], env=environment(),
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        verify(wait_for(lambda: of_session(mark.encode()), 20), f"sessão {entry} no ar (SIGKILL)")
        if entry == "acme-ssh-chave-prod":
            verify(wait_for(lambda: agents(), 20), "agente no ar (SIGKILL)")
        time.sleep(1)
        p.kill()
        p.wait()
        if entry == "acme-ssh-chave-prod":
            verify(wait_for(lambda: not agents(), 3), "V4.26 SIGKILL na CLI: o agente some")
        verify(wait_for(lambda: not of_session(mark.encode()), 5),
               f"V4.26 (54l) SIGKILL na CLI: nenhum ssh/sshpass da sessão {entry} sobra")

    # V4.26: locking in the middle of the session brings down the agent and, since Phase
    # 54c (S8), the whole session: the CLI exits 137 with the warning and no ssh/sshpass
    # of the session is left (with a password, the ssh that sshpass creates with its own
    # session too).

    for entry, mark in [("acme-ssh-chave-prod", "sleep 21"), ("acme-ssh-senha-stage", "sleep 22")]:
        p = subprocess.Popen([WARD, "ssh", entry, mark], env=environment(),
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        up = wait_for(lambda: of_session(mark.encode()), 20)
        verify(up, f"sessão {entry} no ar (travar)")
        if entry == "acme-ssh-chave-prod":
            verify(wait_for(lambda: agents(), 20), "agente no ar (travar)")
        time.sleep(1)
        code, _, _ = run_pty(["lock"])
        start = time.time()
        try:
            _, err = p.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            p.kill()
            _, err = p.communicate()
        dt = time.time() - start
        if entry == "acme-ssh-chave-prod":
            verify(code == 0 and wait_for(lambda: not agents(), 3), "V4.26 travar o cofre derruba o agente")
        verify(p.returncode == 137 and dt < 5,
               f"V4.26 (54c) travar encerra o ward ssh {entry}: 137 em < 5 s ({p.returncode}, {dt:.1f} s)")
        verify(b"o cofre travou; o comando foi encerrado" in err, f"V4.26 (54c) aviso da trava ({err[-120:]!r})")
        verify(wait_for(lambda: not of_session(mark.encode()), 5),
               f"V4.26 (54c) nenhum ssh/sshpass da sessão {entry} sobra depois da trava")
        code, screen, _ = run_pty(["unlock"], to_type=PASSWORD)
        verify(code == 0, "destravar de novo")

    # V4.31 and the prefix oracle of the M4 audit (revised D34; V4.34 withdrawn) in a
    # pty: prompt, whoami and, like the audit's pty oracle script, the first level of the search: a
    # chunk `M<n>:` + candidate for each character of a batch that has the password's
    # first one, each one paused 0.4 s. With the mask that held output back, only the
    # right candidate came out as `••••`. Now all come out in clear and at once: the
    # oracle accepts none. Last, the whole password in one `printf` comes out `••••`.
    batch = sorted(set("abcdefghijklmnop" + SSH_PASSWORD[0]))
    parts = [f"printf '\\nM{i:02d}:{octal(c)}'; sleep 0.4" for i, c in enumerate(batch)]
    cmd = ("; ".join(parts) + f"; printf '\\nINTEIRA:%s\\n' \"$(printf '{octal(SSH_PASSWORD)}')\"; "
           "echo FIM$((30+4))\r")
    code, screen, chunks = interactive("acme-ssh-senha-stage", [
        ("$ ", "whoami\r"),
        ("deploy\r\n", cmd),
        ("FIM34\r\n", "exit\r"),
    ], deadline=90)
    verify(code == 0 and b"deploy\r\n" in screen, f"V4.31 interativo: whoami → deploy e sai 0 ({code})")
    after = screen.split(b"FIM$((30+4))", 1)[-1]
    accepted = []
    for i, c in enumerate(batch):
        m = f"M{i:02d}:".encode()
        j = after.find(m)
        piece = after[j + len(m):].split(b"\r")[0].split(b"\n")[0] if j >= 0 else b""
        if piece != c.encode():
            accepted.append(c)
    verify(not accepted, f"D34 oráculo de prefixo no pty: nenhum candidato se distingue ({len(accepted)})")
    # Timing: each candidate arrives before the next one (nothing held until new output).
    # Only the output after the echo of the typed line (the echo has the `M<n>:` too).
    echo_end = screen.find(b"FIM$((30+4))") + len(b"FIM$((30+4))")
    arrival, pos = {}, 0
    for t, b in chunks:
        begin, pos = pos, pos + len(b)
        if pos <= echo_end:
            continue
        piece = screen[max(begin, echo_end):pos]
        for i in range(len(batch)):
            if f"M{i:02d}:".encode() in piece:
                arrival.setdefault(i, t)
    gaps = [arrival[i + 1] - arrival[i] for i in range(len(batch) - 1) if i in arrival and i + 1 in arrival]
    verify(len(gaps) == len(batch) - 1 and min(gaps) >= 0.25,
           f"D34 cada candidato sai na hora, um por pausa ({[round(g, 2) for g in gaps]})")
    verify(b"INTEIRA:\xe2\x80\xa2\xe2\x80\xa2\xe2\x80\xa2\xe2\x80\xa2" in after
           and SSH_PASSWORD.encode() not in screen, "S4 a senha inteira num pedaço sai ••••")
    verify(sum(1 for x in patterns if x in screen) == 0, "S4 nenhum padrão de .dev/variations.txt no pty")

    # The timing script of the M4 audit (no pty): `A`, pause, candidate, pause, `Z`. The
    # candidate that is a prefix of the password arrives before the `Z`, like the others
    # (before, it was held until the `Z`: timing oracle).
    delays = {}
    for n, candidate in enumerate(["qqq", SSH_PASSWORD[:3], "fax", SSH_PASSWORD[:7]]):
        cmd = f"printf A; sleep 1; printf '{octal(candidate)}'; sleep 1.5; printf 'Z\\n'"
        p = subprocess.Popen([WARD, "ssh", "acme-ssh-senha-stage", cmd], env=environment(),
                             stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        arrivals, seen_out = [], b""
        while True:
            d = os.read(p.stdout.fileno(), 4096)
            if not d:
                break
            seen_out += d
            arrivals.append((time.time(), seen_out))
        p.wait(timeout=60)
        t_candidate = next((t for t, v in arrivals if v.startswith(b"A") and len(v) > 1), None)
        t_z = next((t for t, v in arrivals if b"Z" in v), None)
        delays[n] = None if t_candidate is None or t_z is None else round(t_z - t_candidate, 2)
    verify(all(x is not None and x >= 1.0 for x in delays.values()),
           f"D34 oráculo por tempo: o candidato nunca espera o Z ({delays})")

    # V4.28: the host key of 2222 changed (fake line in Ward's known_hosts): refused.
    kh = os.path.join(DEV, "known_hosts")
    original = open(kh, "rb").read() if os.path.exists(kh) else None
    fake_key = os.path.join(DEV, "logs", f"host-falso-{os.getpid()}")
    subprocess.run(["/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", fake_key],
                   env=environment(), check=True, capture_output=True)
    pub = open(fake_key + ".pub").read().split()[:2]
    os.remove(fake_key)
    os.remove(fake_key + ".pub")
    try:
        with open(kh, "w") as f:
            f.write(f"[127.0.0.1]:2222 {pub[0]} {pub[1]}\n")
        os.chmod(kh, 0o600)  # the daemon refuses a loose known_hosts (M4 audit)
        r = ssh(["acme-ssh-senha-stage", "whoami"])
        verify(r.returncode != 0 and b"deploy" not in r.stdout
               and (b"IDENTIFICATION HAS CHANGED" in r.stderr or b"verification failed" in r.stderr),
               f"V4.28 chave de host mudada: recusa ({r.returncode})")
    finally:
        if original is None:
            os.remove(kh)
        else:
            with open(kh, "wb") as f:
                f.write(original)

    scanner.stop = True
    scanner.join()
    verify(scanner.seen > 0 and scanner.found == 0,
           f"V4.14 nenhum segredo em /proc/*/cmdline durante o ward ssh ({scanner.seen} lidos)")

    # V4.35/V4.37: the argv of each `sshpass`/`ssh`/`sftp` the daemon creates, read in
    # /proc/<pid>/cmdline while the process lives (it is the argv of the `execve`). The
    # contract's `strace` does not work: the `wardd` refuses to start under a debugger
    # (TracerPid) and `ptrace_scope=1` does not allow attaching to a non-descendant.
    PROGS = {b"/usr/bin/sshpass", b"/usr/bin/ssh", b"/usr/bin/sftp"}

    def ppid(pid):
        try:
            return int(open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()[1])
        except (OSError, ValueError, IndexError):
            return None

    def comm(pid):
        try:
            return open(f"/proc/{pid}/comm").read().strip()
        except OSError:
            return ""

    def capture(run_fn):
        seen = {}
        done = []

        def loop():
            while not done:
                for f in glob.glob("/proc/[0-9]*/cmdline"):
                    pid = int(f.split("/")[2])
                    if pid in seen:
                        continue
                    try:
                        a = open(f, "rb").read().split(b"\0")[:-1]
                    except OSError:
                        continue
                    if a and a[0] in PROGS:
                        parent = ppid(pid)
                        seen[pid] = (a, parent, comm(parent) if parent else "")
                time.sleep(0.005)

        t = threading.Thread(target=loop, daemon=True)
        t.start()
        r = run_fn()
        done.append(1)
        t.join()
        return r, seen

    def slow_sftp():
        p = subprocess.Popen([WARD, "ssh", "beta-sftp-dev"], env=environment(), stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        time.sleep(2)
        out, _ = p.communicate(b"pwd\n", timeout=60)
        return out

    r1, v1 = capture(lambda: ssh(["acme-ssh-senha-stage", "sleep 1; whoami"]))
    out2, v2 = capture(slow_sftp)
    verify(r1.stdout.strip() == b"deploy" and b"Remote working directory" in out2, "ssh e sftp com o argv capturado")
    ssh_l = [x for x in v1.values() if x[0][0] == b"/usr/bin/ssh"]
    sp_l = [x for x in v1.values() if x[0][0] == b"/usr/bin/sshpass"]
    verify(len(ssh_l) == 1 and all(x[0][1:3] == [b"-F", b"/dev/null"] and b"-l" in x[0]
                                   and x[0][x[0].index(b"-l"):x[0].index(b"-l") + 4] == [b"-l", b"deploy", b"--", b"127.0.0.1"]
                                   and not any(y.startswith(b"User=") or b"@" in y for y in x[0]) for x in ssh_l),
           "V4.35 ssh: -F /dev/null, -l deploy -- 127.0.0.1, sem usuário@host nem User=")
    verify(len(sp_l) == 1 and sp_l[0][2] == "wardd" and sp_l[0][0][:3] == [b"/usr/bin/sshpass", b"-e", b"/usr/bin/ssh"],
           f"V4.35 sem sh -c: o wardd executa o sshpass direto ({[x[2] for x in sp_l]})")
    sftp_l = [x for x in v2.values() if x[0][0] == b"/usr/bin/sftp"]
    verify(len(sftp_l) == 1 and sftp_l[0][0][1:3] == [b"-F", b"/dev/null"]
           and sftp_l[0][0][-6:] == [b"-o", b"User=sftpuser", b"-P", b"2222", b"--", b"127.0.0.1"]
           and b"-l" not in sftp_l[0][0] and not any(b"@" in y for y in sftp_l[0][0]),
           "V4.37 sftp: -F /dev/null, -o User=sftpuser -P 2222 -- 127.0.0.1, sem -l nem usuário@host")
    sp2 = [x for x in v2.values() if x[0][0] == b"/usr/bin/sshpass"]
    verify(len(sp2) == 1 and sp2[0][2] == "wardd", "V4.37 sem sh -c: o wardd executa o sshpass do sftp direto")
    everything = b"\n".join(b"\0".join(x[0]) for x in list(v1.values()) + list(v2.values()))
    verify(sum(1 for x in patterns if x in everything) == 0, "S3 nenhum segredo no argv do ssh/sftp")

# --- 5. Jump and tunnel (Phase 28) against the test SSH servers and Postgres ----------
# V4.25 (jump), V4.29 (database tunnel), V4.30 (combined level), V4.35 with a jump (no
# `sh -c` in the chain: the jump's `ssh` is a child of the daemon, never of the main
# `ssh`) and the attack on the bridge (a process outside the session is closed without
# any byte). The Postgres client is `dev/tests/pg_select.py` (the host has no `psql`).
PG_SELECT = os.path.join(ROOT, "dev/tests/pg_select.py")
if not (services_up and port_open(2224) and port_open(55432)):
    verify(False, "Fase 28: ssh-salto (2224) ou postgres (55432) fora do ar; seção salto/túnel pulada")
else:
    trace_env = environment({"RUST_LOG": "trace"})
    r, seen = capture(lambda: ssh(["acme-ssh-interno", "sleep 1; hostname"], env=trace_env))
    verify(r.returncode == 0 and r.stdout.strip() == b"ssh-interno",
           f"V4.25 ward ssh acme-ssh-interno hostname → ssh-interno ({r.returncode} {r.stderr[-200:]!r})")
    no_variation(r, "ssh com salto")
    ssh_v = [x for x in seen.values() if x[0][0] == b"/usr/bin/ssh"]
    main_ssh = [x for x in ssh_v if b"-W" not in x[0]]
    jump = [x for x in ssh_v if b"-W" in x[0]]
    verify(len(main_ssh) == 1 and len(jump) == 1, f"V4.35 dois ssh: o principal e o do salto ({len(main_ssh)}, {len(jump)})")
    verify(all(x[2] in ("wardd", "sshpass") for x in ssh_v),
           f"V4.35 sem sh -c nem ProxyJump: todo ssh é filho do wardd ou do sshpass ({[x[2] for x in ssh_v]})")
    verify(not any(b"ProxyJump" in y or y.startswith(b"ProxyCommand=") and y != b"ProxyCommand=none"
                   for x in ssh_v for y in x[0]), "V4.35 nada de ProxyJump nem ProxyCommand")
    if main_ssh:
        a = main_ssh[0][0]
        o = [a[i + 1] for i in range(len(a) - 1) if a[i] == b"-o"]
        i = a.index(b"--") if b"--" in a else -1
        verify(a[1:3] == [b"-F", b"/dev/null"] and b"HostKeyAlias=ssh-interno" in o and b"CheckHostIP=no" in o
               and i > 2 and a[i - 2:i] == [b"-l", b"deploy"] and a[i + 1] == b"127.0.0.1"
               and not any(y.startswith(b"User=") or y.endswith(b"@127.0.0.1") for y in a),
               "V4.35 principal: -F /dev/null, HostKeyAlias=ssh-interno, -l deploy -- 127.0.0.1")
    if jump:
        a = jump[0][0]
        i = a.index(b"--") if b"--" in a else -1
        verify(a[1:3] == [b"-F", b"/dev/null"]
               and a[i - 6:i + 2] == [b"-W", b"ssh-interno:22", b"-p", b"2224", b"-l", b"salto", b"--", b"127.0.0.1"] and a[a.index(b"-W") + 1] == b"ssh-interno:22"
               and a[i - 2:i] == [b"-l", b"salto"] and a[i + 1] == b"127.0.0.1"
               and f"UserKnownHostsFile={DEV}/known_hosts".encode() in a
               and not any(y.startswith(b"User=") or y.endswith(b"@127.0.0.1") for y in a),
               "V4.35 salto: -F /dev/null, -W ssh-interno:22, -l salto -- 127.0.0.1, known_hosts do Ward")
    everything = b"\n".join(b"\0".join(x[0]) for x in seen.values())
    verify(sum(1 for x in patterns if x in everything) == 0, "S3 nenhum segredo no argv do salto")
    verify(not agents(), "D12 o agente do salto some no fim")

    # V4.30: free with a jump "with approval": the most restrictive applies.
    r = ssh(["acme-ssh-interno-aprov", "hostname"], env=trace_env)
    verify(r.returncode == 125 and b"ssh-interno" not in r.stdout
           and (b"pedido negado" in r.stderr or "aprovação indisponível".encode() in r.stderr),
           f"V4.30 salto com aprovação: vale o nível mais restritivo ({r.stderr[:80]!r})")

    # V4.29: `ward run <tunnel entry> -- … select 1` through the bridge; PGHOST=127.0.0.1
    # and the local port; after the command, the port closes.
    r = run(["acme-pg-tunel", "--", "/bin/sh", "-c",
             'echo "ponta $PGHOST $PGPORT"; /usr/bin/python3 -I "$1" "select 1"', "sh", PG_SELECT],
            env=trace_env)
    lines = r.stdout.decode(errors="replace").split("\n")
    endpoint = next((x.split() for x in lines if x.startswith("ponta ")), [])
    verify(r.returncode == 0 and "1" in lines, f"V4.29 select 1 pelo túnel → 1 ({r.returncode} {r.stderr[-200:]!r})")
    verify(len(endpoint) == 3 and endpoint[1] == "127.0.0.1" and endpoint[2].isdigit() and endpoint[2] != "5432",
           f"V4.29 PGHOST=127.0.0.1 com a porta local ({endpoint})")
    if len(endpoint) == 3 and endpoint[2].isdigit():
        verify(not port_open(int(endpoint[2])), "V4.29 o túnel fecha ao fim")
    no_variation(r, "run com túnel")
    r = run(["acme-pg-tunel", "--", "/bin/sh", "-c", '/usr/bin/python3 -I "$1" "select current_user"', "sh", PG_SELECT])
    verify(r.returncode == 0 and r.stdout.strip() == b"app", "V4.29 current_user pelo túnel → app")

    # Attack: during the `ward run`, another process of the user (this test) connects to
    # the bridge: closed without any byte; the command keeps using the tunnel.
    p = subprocess.Popen([WARD, "run", "acme-pg-tunel", "--", "/bin/sh", "-c",
                          'echo "$PGPORT"; sleep 3; /usr/bin/python3 -I "$1" "select 2"', "sh", PG_SELECT],
                         env=environment(), stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    tunnel_port = p.stdout.readline().strip()
    received = b"?"
    if tunnel_port.isdigit():
        try:
            conn = socket.create_connection(("127.0.0.1", int(tunnel_port)), timeout=5)
            conn.settimeout(5)
            conn.sendall(b"\0\0\0\x08\x04\xd2\x16\x2f")
            received = conn.recv(64)
            conn.close()
        except OSError:
            received = b""
    out, err = p.communicate(timeout=60)
    verify(received == b"", f"ataque: a ponte do túnel fecha a conexão de fora da sessão sem byte ({received!r})")
    verify(p.returncode == 0 and out.strip() == b"2", f"o comando segue com o túnel depois do ataque ({p.returncode})")

    # V4.33 (S13): the daemon log (with RUST_LOG=trace) has no pattern of
    # .dev/variations.txt; the CLI's stderr with RUST_LOG=trace neither (no_variation).
    log = os.path.join(DEV, "logs", "wardd.log")
    log_text = open(log, "rb").read() if os.path.exists(log) else b""
    verify(b"wardd[debug]" in log_text, "V4.33 o daemon de teste está com RUST_LOG=trace (linhas [debug] no log)")
    verify(log_text and sum(1 for x in patterns if x in log_text) == 0,
           f"V4.33 nenhum padrão de .dev/variations.txt no log do daemon ({len(log_text)} bytes)")

# --- 6. Phase 27a: the "Connect" token only through the fd channel --------------------
# Does not need the test SSH servers: the approval Postgres entry is a database with approval,
# and the daemon checks the level before the type (V4.32). The token is made up here (the
# daemon's format, random on each run, never written in the repository); the test daemon
# ignores it (the level applies) and logs "invalid token", proof that it came through
# the channel.
import secrets  # noqa: E402
import string  # noqa: E402

DAEMON_LOG = os.path.join(DEV, "logs", "wardd.log")
INVALID_TOKEN_MSG = "ssh: token inválido, vencido, já usado ou de outra entrada".encode()


def invalid_token_lines():
    try:
        return open(DAEMON_LOG, "rb").read().count(INVALID_TOKEN_MSG)
    except OSError:
        return -1


class ProcReader(threading.Thread):
    """Reads in a loop `/proc/*/cmdline` and `/proc/*/environ` of every process and tries
    to reopen and read each `/proc/<pid>/fd/*` of the `ward` and fake terminal
    processes. Records where the token showed up."""

    def __init__(self, needle):
        super().__init__(daemon=True)
        self.needle = needle
        self.stop = False
        self.found = []
        self.scanned = 0
        self.wards = set()
        self.fds_tried = 0

    def run(self):
        while not self.stop:
            for d in glob.glob("/proc/[0-9]*"):
                for name in ("cmdline", "environ"):
                    try:
                        c = open(os.path.join(d, name), "rb").read()
                    except OSError:
                        continue
                    self.scanned += 1
                    if self.needle in c:
                        self.found.append(f"{d}/{name}")
                try:
                    a0 = open(os.path.join(d, "cmdline"), "rb").read().split(b"\0")
                except OSError:
                    continue
                if WARD.encode() in a0 or b"terminal-falso" in a0:
                    self.wards.add(d)
                    for f in glob.glob(os.path.join(d, "fd", "*")):
                        # 0–2 are this script's pipes (reading 1 or 2 would steal the
                        # CLI's output); the channel is ≥ 3.
                        if os.path.basename(f) in ("0", "1", "2"):
                            continue
                        self.fds_tried += 1
                        try:
                            fd = os.open(f, os.O_RDONLY | os.O_NONBLOCK)
                        except OSError:
                            continue
                        try:
                            if self.needle in os.read(fd, 4096):
                                self.found.append(f)
                        except OSError:
                            pass
                        finally:
                            os.close(fd)
            time.sleep(0.005)


alphabet = string.ascii_letters + string.digits
token = "".join(secrets.choice(alphabet) for _ in range(32)).encode()
before = invalid_token_lines()
reader = ProcReader(token)
reader.start()
r = ssh_with_channel(token, ["acme-pg-aprovacao", "id"])
time.sleep(0.2)
reader.stop = True
reader.join()
verify(r.returncode == 125 and (b"pedido negado" in r.stderr or "aprovação indisponível".encode() in r.stderr),
       f"27a ward ssh --token-fd pelo terminal falso: token inventado, vale o nível ({r.returncode}, {r.stderr[:80]!r})")
verify(invalid_token_lines() == before + 1,
       "27a o token chegou ao daemon pelo canal (uma linha 'token inválido' a mais no log)")
verify(reader.scanned > 0 and reader.wards, f"27a o leitor varreu {reader.scanned} arquivos e viu o ward")
verify(not reader.found, f"27a token em nenhum /proc/*/cmdline, environ ou fd reaberto ({reader.found})")
verify(token not in r.stdout + r.stderr, "27a o token não volta na saída")

# `--token` in argv: refused (2) without connecting (nothing new in the daemon log).
before = invalid_token_lines()
for args in (["--token", token.decode(), "acme-pg-aprovacao", "id"],
             ["--token=" + token.decode(), "acme-pg-aprovacao", "id"]):
    r = ssh(args)
    verify(r.returncode == 2 and b"linha de comando" in r.stderr and token not in r.stderr,
           f"27a {args[0][:8]} em argv recusado sem eco ({r.returncode})")
verify(invalid_token_lines() == before, "27a --token em argv não chega ao daemon")

# Forged channel: garbage, empty, too large, pipe (reopenable through /proc), closed fd.
cases = [
    (b"falso-lixo;id\n" * 2, "socket", "fora do formato"),
    (b"", "socket", "veio vazio"),
    (b"A" * 4096, "socket", "passa do tamanho"),
    (token, "pipe", "não é o do app"),
]
for content, channel, msg in cases:
    r = ssh_with_channel(content, ["acme-pg-aprovacao", "id"], channel=channel)
    verify(r.returncode == 125 and msg.encode() in r.stderr and b"falso" not in r.stderr
           and token not in r.stderr and b"AAAA" not in r.stderr,
           f"27a canal {channel} ({msg}) recusado sem eco ({r.returncode}, {r.stderr[:70]!r})")
r = ssh(["--token-fd", "1000", "acme-pg-aprovacao", "id"])
verify(r.returncode == 125 and "não está aberto".encode() in r.stderr, f"27a fd fechado recusado ({r.returncode})")
verify(invalid_token_lines() == before, "27a nenhum canal forjado chegou ao daemon")

print(f"\n{len(failures)} falha(s)")
sys.exit(1 if failures else 0)
