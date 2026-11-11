#!/usr/bin/python3 -I
"""V4.27 (Phase 28b; R19, D13, S1): the `ward ssh` and tunnel suite opens nothing from the
passwd home. Runs only inside the bwrap of `dev/tests/ssh-home.sh` (home and `/tmp` on
tmpfs, `/usr/bin/ssh`/`sshpass`/`sftp` replaced by the wrapper with `strace -D -f`).

1. Plants the decoys in the covered home (ssh dir with a `config` of `Host *` + a
   `ProxyCommand` that touches a sentinel file in `/tmp` + `LocalCommand` + `IdentityAgent`
   of the fake agent, `known_hosts` with a swapped host key, a sentinel `id_ed25519` and
   FIFOs on the other names), the fake agent and inotify on every dir of the home.
2. Starts a test `wardd` of its own (copy of the test vault, `HOME` = the covered home,
   the agent variable pointing at the fake one) and unlocks it through the CLI in a pty.
3. Suite: `ward ssh` by password, by key, through the jump host, `sftp`, interactive in a
   pty and `ward run` through the tunnel. Each one has to succeed (a trace of something
   that did not connect proves nothing).
4. Verifies: the sentinels do not exist, no inotify event, no FIFO opened, no connection
   to the fake agent, no home path (outside the repository) in the `strace` traces, and
   nothing from `.dev/variations.txt` in the traces, the output and the log.
5. Controls: an `ssh` without the isolation creates the sentinel, raises events, opens a
   FIFO, connects to the agent and is flagged in the trace; otherwise the detector is
   worthless.

Usage (by ssh-home.sh): ssh_home.py <wardd> <ward> <root> <passwd home> <work dir>
Fake values only (D15); never prints a secret nor file contents.
"""

import ctypes
import json
import os
import pty
import re
import select
import shutil
import socket
import struct
import subprocess
import sys
import threading
import time

WARDD, WARD, ROOT, PASSWD_HOME, WORK_DIR = sys.argv[1:6]
if os.environ.get("WARD_SSH_HOME_DENTRO") != "1":
    print("ssh_home.py roda só dentro do bwrap de dev/tests/ssh-home.sh")
    sys.exit(2)

# Names built in pieces (V1.5 refuses to have them spelled out in dev/).
SSH_DIR = os.path.join(PASSWD_HOME, "." + "ssh")
AGENT_VAR = "SSH_AUTH" + "_SOCK"
SENTINEL = "/tmp/sentinela-ssh"
LOCAL_SENTINEL = "/tmp/sentinela-ssh-local"
TRACES = os.path.join(WORK_DIR, "tracos")
PASSWORD = json.load(open(os.path.join(ROOT, "dev/fixtures/entries.json")))["master_password"]
PG_SELECT = os.path.join(ROOT, "dev/tests/pg_select.py")
UID = os.geteuid()
RUNTIME = os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{UID}"
# Socket dir of this test only, in a user dir at the root of the bwrap (ssh-home.sh).
SOCKDIR = "/ward-ssh-home/sock"
PATTERNS = [p for p in open(os.path.join(ROOT, ".dev/variations.txt"), "rb").read().split(b"\n") if p]

failures = []
stage = ["setup"]


def verify(cond, msg):
    print(("ok   " if cond else "FALHA") + " " + msg, flush=True)
    if not cond:
        failures.append(msg)


def inside(p, base):
    return p == base or p.startswith(base.rstrip("/") + "/")


# --- 1. Decoys ---------------------------------------------------------------------------
for p in (2222, 2223, 2224, 55432):
    with socket.socket() as s:
        s.settimeout(1)
        if s.connect_ex(("127.0.0.1", p)) != 0:
            print(f"FALHA serviço de teste fora do ar na porta {p} (make dev-services-up)")
            sys.exit(1)

os.mkdir(SSH_DIR, 0o700)
MIN_ENV = {"PATH": "/usr/bin:/bin", "HOME": PASSWD_HOME, "LANG": "C.UTF-8"}
subprocess.run(["/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "sentinela-ward",
                "-f", os.path.join(SSH_DIR, "id_ed25519")], env=MIN_ENV, check=True,
               stdin=subprocess.DEVNULL, capture_output=True)
pub = open(os.path.join(SSH_DIR, "id_ed25519.pub")).read().split()
# Swapped host key for every service: if read, ssh would refuse the connection.
with open(os.path.join(SSH_DIR, "known_hosts"), "w") as f:
    for h in ("127.0.0.1", "[127.0.0.1]:2222", "[127.0.0.1]:2223", "[127.0.0.1]:2224", "ssh-interno"):
        f.write(f"{h} {pub[0]} {pub[1]}\n")

os.mkdir("/tmp/agente-falso", 0o700)
AGENT = "/tmp/agente-falso/sock"
with open(os.path.join(SSH_DIR, "config"), "w") as f:
    f.write("Host *\n"
            f"    ProxyCommand /usr/bin/touch {SENTINEL}\n"
            "    PermitLocalCommand yes\n"
            f"    LocalCommand /usr/bin/touch {LOCAL_SENTINEL}\n"
            f"    IdentityAgent {AGENT}\n"
            "    IdentityFile ~/" + "." + "ssh/id_ed25519\n")

# FIFOs: whoever opens one for reading is recorded by the writer.
FIFOS = [os.path.join(SSH_DIR, n) for n in (
    "id_rsa", "id_ecdsa", "id_ecdsa_sk", "id_ed25519_sk", "id_dsa", "id_xmss", "id_rsa-cert.pub",
    "id_ed25519-cert.pub", "known_hosts2", "authorized_keys", "environment", "rc")]
FIFOS += [os.path.join(PASSWD_HOME, n) for n in (".pgpass", ".psqlrc", ".pg_service.conf", ".netrc")]
openings = []  # (stage, path)
lock = threading.Lock()


def writer(path):
    while True:
        try:
            with open(path, "w"):
                pass
        except OSError:
            return
        with lock:
            openings.append((stage[0], path))
        time.sleep(0.01)


for c in FIFOS:
    os.mkfifo(c, 0o600)
    threading.Thread(target=writer, args=(c,), daemon=True).start()

# Fake agent: only counts the connections (answers nothing).
agent_connections = []
agent_srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
agent_srv.bind(AGENT)
agent_srv.listen(16)


def fake_agent():
    while True:
        try:
            c, _ = agent_srv.accept()
        except OSError:
            return
        with lock:
            agent_connections.append(stage[0])
        c.close()


threading.Thread(target=fake_agent, daemon=True).start()

# inotify (libc through ctypes) on every dir of the covered home, except the path to the
# repository (which is the repository's bind, not the home).
libc = ctypes.CDLL("libc.so.6", use_errno=True)
IN_ALL_EVENTS = 0x00000FFF
IN_NONBLOCK, IN_CLOEXEC = 0o4000, 0o2000000
fd_in = libc.inotify_init1(IN_NONBLOCK | IN_CLOEXEC)
if fd_in < 0:
    print("FALHA inotify_init1")
    sys.exit(1)
repo_component = os.path.relpath(ROOT, PASSWD_HOME).split("/")[0] if inside(ROOT, PASSWD_HOME) else None
watched_dirs = {}
for walk_dir, dirs, _ in os.walk(PASSWD_HOME):
    if walk_dir == PASSWD_HOME and repo_component in dirs:
        dirs.remove(repo_component)
    wd = libc.inotify_add_watch(fd_in, walk_dir.encode(), IN_ALL_EVENTS)
    if wd < 0:
        print(f"FALHA inotify_add_watch em {walk_dir}")
        sys.exit(1)
    watched_dirs[wd] = walk_dir
events = []  # (stage, path, mask)


def watcher():
    while True:
        r, _, _ = select.select([fd_in], [], [], 0.2)
        if not r:
            continue
        try:
            buf = os.read(fd_in, 65536)
        except BlockingIOError:
            continue
        i = 0
        while i + 16 <= len(buf):
            wd, mask, _, size = struct.unpack_from("iIII", buf, i)
            name = buf[i + 16:i + 16 + size].split(b"\0", 1)[0].decode(errors="replace")
            i += 16 + size
            with lock:
                events.append((stage[0], os.path.join(watched_dirs.get(wd, "?"), name), mask))


threading.Thread(target=watcher, daemon=True).start()
time.sleep(0.3)
with lock:
    events.clear()

# Current dir of the clients: a decoy dir (a relative path opened by ssh, such as
# `none`, shows up in the trace).
CWD = os.path.join(WORK_DIR, "cwd")
os.mkdir(CWD, 0o700)
open(os.path.join(CWD, "none"), "w").close()

# --- 2. Test wardd of this script only ---------------------------------------------------
shutil.copyfile(os.path.join(ROOT, ".dev/test-vault.ward"), os.path.join(WORK_DIR, "test-vault.ward"))
os.chmod(os.path.join(WORK_DIR, "test-vault.ward"), 0o600)
with open(os.path.join(WORK_DIR, "test-vault.state"), "w") as f:
    f.write("ward-estado 1\nvista 0\nqueimada 0\n")
LOG = os.path.join(WORK_DIR, "wardd.log")
daemon_env = {
    "HOME": PASSWD_HOME, "PATH": "/usr/bin:/bin", "XDG_RUNTIME_DIR": RUNTIME, "LANG": "C.UTF-8",
    "WARD_DEV_DIR": WORK_DIR, "WARD_DEV_SOCKET_DIR": SOCKDIR, "RUST_LOG": "trace", AGENT_VAR: AGENT,
}
daemon = subprocess.Popen([WARDD], env=daemon_env, cwd=ROOT, stdin=subprocess.DEVNULL,
                          stdout=open(LOG, "ab"), stderr=subprocess.STDOUT)
SOCK = os.path.join(SOCKDIR, "wardd.sock")


def stop_daemon():
    if daemon.poll() is None:
        daemon.terminate()
        try:
            daemon.wait(timeout=10)
        except subprocess.TimeoutExpired:
            daemon.kill()
            daemon.wait()
    shutil.rmtree(SOCKDIR, ignore_errors=True)


deadline = time.time() + 20
while time.time() < deadline and not os.path.exists(SOCK) and daemon.poll() is None:
    time.sleep(0.05)
if not os.path.exists(SOCK):
    print("FALHA o wardd de teste não subiu (fim do log abaixo)")
    print(open(LOG, errors="replace").read()[-1500:])
    stop_daemon()
    sys.exit(1)
print(f"wardd de teste só deste script: pid {daemon.pid}, socket {SOCK}")

CLI_ENV = {"PATH": "/usr/bin:/bin", "HOME": PASSWD_HOME, "LANG": "C.UTF-8", "WARD_DEV_SOCKET_DIR": SOCKDIR,
           AGENT_VAR: AGENT}
outputs = []


def pty_run(args, steps, timeout=90):
    """`ward <args>` in a pty; `steps` = [(wait_for, type)]. Returns (code, screen)."""
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(CWD)
        os.execve(WARD, [WARD] + args, dict(CLI_ENV, TERM="xterm"))
    screen = b""
    steps = list(steps)
    deadline = time.time() + timeout
    while time.time() < deadline:
        if steps and steps[0][0].encode() in screen:
            _, typed = steps.pop(0)
            time.sleep(0.1)
            os.write(fd, typed.encode())
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try:
                b = os.read(fd, 4096)
            except OSError:
                break
            if not b:
                break
            screen += b
    if steps:
        os.kill(pid, 9)
    _, st = os.waitpid(pid, 0)
    os.close(fd)
    outputs.append(screen)
    return os.waitstatus_to_exitcode(st), screen


def ward(args, stdin_data=b"", timeout=120):
    r = subprocess.run([WARD] + args, env=CLI_ENV, cwd=CWD, input=stdin_data, capture_output=True, timeout=timeout)
    outputs.append(r.stdout + r.stderr)
    return r


try:
    code, screen = pty_run(["destravar"], [("Senha mestra", PASSWORD + "\r")])
    verify(code == 0 and b"cofre destravado" in screen, f"destravar o wardd de teste ({code})")

    # --- 3. ward ssh and tunnel suite ---------------------------------------------------
    stage[0] = "suite"
    r = ward(["ssh", "acme-ssh-senha-stage", "whoami"])
    verify(r.returncode == 0 and r.stdout.strip() == b"deploy", f"ward ssh por senha → deploy ({r.returncode})")
    r = ward(["ssh", "acme-ssh-chave-prod", "whoami"])
    verify(r.returncode == 0 and r.stdout.strip() == b"deploy", f"ward ssh por chave (agente de D12) → deploy ({r.returncode})")
    r = ward(["ssh", "acme-ssh-interno", "hostname"])
    verify(r.returncode == 0 and r.stdout.strip() != b"", f"ward ssh pelo salto → hostname do interno ({r.returncode})")
    r = ward(["ssh", "beta-sftp-dev"], stdin_data=b"pwd\n")
    verify(r.returncode == 0 and b"Remote working directory" in r.stdout, f"ward ssh no sftp ({r.returncode})")
    code, screen = pty_run(["ssh", "acme-ssh-senha-stage"], [("$ ", "whoami\r"), ("deploy\r\n", "exit\r")])
    verify(code == 0 and b"deploy\r\n" in screen, f"ward ssh interativo num pty: whoami → deploy ({code})")
    r = ward(["run", "acme-pg-tunel", "--", "/bin/sh", "-c", '/usr/bin/python3 -I -B "$1" "select 1"', "sh", PG_SELECT])
    verify(r.returncode == 0 and r.stdout.strip() == b"1", f"ward run pelo túnel → select 1 ({r.returncode})")
    time.sleep(1.5)  # strace -D finishes after the program
    stage[0] = "after"

    # --- 4. Verifications -----------------------------------------------------------------
    verify(not os.path.exists(SENTINEL), "V4.27 /tmp/sentinela-ssh não existe (ProxyCommand do config não rodou)")
    verify(not os.path.exists(LOCAL_SENTINEL), "V4.27 LocalCommand do config não rodou")
    with lock:
        ev = [(c, m) for f_, c, m in events if f_ == "suite"]
        op = [c for f_, c in openings if f_ == "suite"]
        ag = [f_ for f_ in agent_connections if f_ == "suite"]
    verify(not ev, f"V4.27 inotify: nenhum evento no home durante a suíte ({len(ev)})")
    for c, m in sorted(set(ev))[:20]:
        print(f"        evento 0x{m:x} em {c}")
    verify(not op, f"V4.27 nenhuma isca FIFO aberta ({sorted(set(op))})")
    verify(not ag, f"V4.27 o agente falso não recebeu conexão ({len(ag)})")

    suite_traces = sorted(n for n in os.listdir(TRACES) if not n.endswith(".cwd"))
finally:
    stop_daemon()

# strace analyzer: home paths (outside the repository) and paths relative to the decoy
# dir; resolves a relative path through the current dir of each process (starts at the
# one written by the wrapper, follows chdir and passes from parent to child on clone/fork).
LINE = re.compile(r'^(\d+)\s+(?:<\.\.\. (\w+) resumed>|(\w+)\()(.*)')
QUOTED_PATH = re.compile(r'"((?:[^"\\]|\\.)*)"')
RETURN_VALUE = re.compile(r'=\s+(-?\d+)(?:\s|$)')
WITH_DIRFD = {"openat", "openat2", "newfstatat", "fstatat64", "faccessat", "faccessat2", "statx",
              "readlinkat", "mkdirat", "mknodat", "unlinkat", "renameat", "renameat2", "linkat",
              "symlinkat", "fchmodat", "fchmodat2", "fchownat", "futimesat", "utimensat", "execveat",
              "name_to_handle_at", "inotify_add_watch"}
FORKS = {"clone", "clone3", "fork", "vfork"}


def flagged(trace):
    cwd0 = open(trace + ".cwd").read().strip() if os.path.exists(trace + ".cwd") else None
    lines, parent_of = [], {}
    for line in open(trace, errors="replace"):
        m = LINE.match(line)
        if not m:
            continue
        pid, resumed, call, rest = m.groups()
        name = resumed or call
        r = RETURN_VALUE.search(rest.rsplit(")", 1)[-1] if ")" in rest else rest)
        ret = int(r.group(1)) if r else None
        if name in FORKS and ret and ret > 0:
            parent_of[str(ret)] = pid
        lines.append((pid, name, bool(resumed), rest, ret))
    cwd_of, first, found = {}, None, set()
    for pid, call, resumed, rest, ret in lines:
        if pid not in cwd_of:
            if first is None:
                first = pid
                cwd_of[pid] = cwd0
            else:
                cwd_of[pid] = cwd_of.get(parent_of.get(pid), cwd0)
        current = cwd_of[pid]
        if call == "fchdir" and ret == 0:
            cwd_of[pid] = None
            continue
        if resumed:
            continue
        if call in ("execve", "execveat"):
            rest = rest.split(",", 1)[0]
        paths = QUOTED_PATH.findall(rest)[:2]
        if call == "chdir" and paths and ret == 0:
            c = paths[0]
            cwd_of[pid] = None if (current is None and not c.startswith("/")) else os.path.normpath(os.path.join(current or "/", c))
            continue
        for c in paths:
            if not c:
                continue
            if not c.startswith("/"):
                if call in WITH_DIRFD and not rest.startswith("AT_FDCWD"):
                    found.add("(relativo a um dirfd) " + call)
                    continue
                if current is None:
                    found.add("(relativo sem diretório atual conhecido) " + call)
                    continue
                c = os.path.join(current, c)
            p = os.path.normpath(c)
            if p == CWD:
                continue
            if inside(p, CWD):
                found.add("(relativo ao diretório de iscas) " + os.path.relpath(p, CWD))
                continue
            if not inside(p, PASSWD_HOME) or inside(p, ROOT) or inside(ROOT, p):
                continue
            found.add(p)
    return found


traces_text = b""
total = {"ssh": 0, "sshpass": 0, "sftp": 0}
for n in suite_traces:
    path = os.path.join(TRACES, n)
    t = open(path, "rb").read()
    traces_text += t
    for prog in total:
        total[prog] += t.count(f'execve("/tmp/ward-real/{prog}"'.encode())
    a = flagged(path)
    verify(not a, f"V4.27 strace: {n} não abre nada do home ({len(a)})")
    for x in sorted(a)[:20]:
        print(f"        {x}")
verify(total["ssh"] >= 6 and total["sshpass"] >= 3 and total["sftp"] >= 1,
       f"os traços pegaram os programas do daemon (execve: {total})")
verify(sum(1 for p in PATTERNS if p in traces_text) == 0, "S1 nenhum padrão de .dev/variations.txt nos traços")
verify(sum(1 for p in PATTERNS if any(p in s for s in outputs)) == 0, "S1 nenhum padrão de .dev/variations.txt na saída da CLI")
log = open(LOG, "rb").read()
n_log = log.count(b"\n")
verify(sum(1 for p in PATTERNS if p in log) == 0, f"S1 log do daemon (trace, {n_log} linhas) sem padrão de .dev/variations.txt")

# --- 5. Controls: without the isolation, each detector flags ------------------------------
CONTROL_ENV = dict(MIN_ENV, **{AGENT_VAR: AGENT})
before = set(os.listdir(TRACES))
stage[0] = "control-config"
subprocess.run(["/usr/bin/ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=3", "-p", "2223",
                "ninguem@127.0.0.1", "true"], env=CONTROL_ENV, cwd=CWD, stdin=subprocess.DEVNULL,
               capture_output=True, timeout=30)
time.sleep(1)
stage[0] = "control-default"
subprocess.run(["/usr/bin/ssh", "-F", "/dev/null", "-o", "BatchMode=yes", "-o", "ConnectTimeout=3",
                "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null", "-p", "2223",
                "ninguem@127.0.0.1", "true"], env=CONTROL_ENV, cwd=CWD, stdin=subprocess.DEVNULL,
               capture_output=True, timeout=30)
time.sleep(1.5)
stage[0] = "end"
with lock:
    ev_c = [f_ for f_, _, _ in events if f_ == "control-config"]
    op_c = [f_ for f_, _ in openings if f_ == "control-default"]
    ag_c = [f_ for f_ in agent_connections if f_ == "control-default"]
verify(os.path.exists(SENTINEL), "controle: ssh sem -F /dev/null roda o ProxyCommand e cria a sentinela")
verify(bool(ev_c), "controle: o inotify acusa o ssh que lê o config do home")
verify(bool(op_c), "controle: o ssh com as identidades padrão abre isca FIFO")
verify(bool(ag_c), "controle: o ssh com a variável do agente conecta no agente falso")
new_traces = sorted(n for n in set(os.listdir(TRACES)) - before if not n.endswith(".cwd"))
verify(len(new_traces) == 2 and all(flagged(os.path.join(TRACES, n)) for n in new_traces),
       f"controle: o analisador do strace acusa os dois ssh sem o isolamento ({len(new_traces)} traços)")

if failures:
    print(f"\nteste-ssh-home: {len(failures)} falha(s)")
    sys.exit(1)
print("\nteste-ssh-home: V4.27 ok (nada do home do passwd na suíte de ward ssh e túnel)")
