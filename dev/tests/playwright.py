#!/usr/bin/python3 -I
"""Verifications of `make test-playwright` (Phase 44). See `dev/tests/playwright.sh`.

Usage: playwright.py <ward> <root> <socket dir = rt> <work dir>. Fake values only (D15).
The Python Playwright (V6.12) comes from `PW_PYTHONPATH`.
"""

import json
import os
import pty
import select
import stat
import subprocess
import sys
import threading
import time

WARD, ROOT, RT, WORK_DIR = sys.argv[1:5]
PASSWORD = json.load(open(os.path.join(ROOT, "dev/fixtures/entries.json")))["master_password"]
PATTERNS = [l for l in open(os.path.join(ROOT, ".dev/variations.txt"), "rb").read().split(b"\n") if l]
failures = []


APPROVER_LOG = os.path.join(WORK_DIR, "logs", "wardd-aprovador-teste.log")
APPROVER_MODE = os.path.join(WORK_DIR, "aprovador.modo")


def set_window_mode(m):
    """Phase 44c: what the test window answers to the next request."""
    with open(APPROVER_MODE, "w") as f:
        f.write(m + "\n")


def window_requests():
    """The request lines the test window saw, in order."""
    try:
        with open(APPROVER_LOG, encoding="utf-8", errors="replace") as f:
            return [l.strip() for l in f if l.startswith("pedido ")]
    except OSError:
        return []


def verify(cond, msg):
    print(("ok   " if cond else "FALHA") + " " + msg, flush=True)
    if not cond:
        failures.append(msg)


def has_secret(b):
    return any(p in b for p in PATTERNS)


def environment():
    return {"PATH": "/usr/bin:/bin", "HOME": "/nao-existe", "LANG": "C.UTF-8",
            "WARD_DEV_SOCKET_DIR": RT}


def run_pty(args, typed=None, timeout=150):
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(WARD, [WARD] + args, environment())
    screen = b""
    seen = False
    deadline = time.time() + timeout
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            try:
                b = os.read(fd, 4096)
            except OSError:
                break
            if not b:
                break
            screen += b
            if typed is not None and not seen and b"Senha mestra" in screen:
                seen = True
                time.sleep(0.05)
                os.write(fd, typed.encode() + b"\r")
    _, st = os.waitpid(pid, 0)
    os.close(fd)
    return os.waitstatus_to_exitcode(st), screen.decode("utf-8", "replace")


def ward(args, timeout=200):
    """`ward <args>` without a terminal: (code, stdout, stderr)."""
    p = subprocess.run([WARD] + args, env=environment(), capture_output=True, timeout=timeout,
                       stdin=subprocess.DEVNULL)
    return p.returncode, p.stdout, p.stderr


def posts_log():
    p = subprocess.run([os.path.join(ROOT, "dev/services/compose.sh"), "exec", "-T", "phish",
                        "cat", "/registro/posts.log"], capture_output=True, timeout=60)
    if p.returncode == 0:
        return p.stdout
    # With the services just started, phish has not created `posts.log` yet (tmpfs):
    # nothing received, as in `dev/tests/login.py`. Any other error (container down) stays None.
    if b"No such file or directory" in p.stderr:
        return b""
    return None


def chrome_pids():
    """Processes with the `<rt>pw-*` profile in their argv."""
    mark = (RT + "/pw-").encode()
    found = {}
    for d in os.listdir("/proc"):
        if not d.isdigit():
            continue
        try:
            argv = open(f"/proc/{d}/cmdline", "rb").read()
        except OSError:
            continue
        if mark in argv:
            found[int(d)] = argv
    return found


def listening_inodes():
    inos = set()
    for path in ("/proc/net/tcp", "/proc/net/tcp6"):
        try:
            lines = open(path).read().splitlines()[1:]
        except OSError:
            continue
        for l in lines:
            c = l.split()
            if len(c) > 9 and c[3] == "0A":
                inos.add(c[9])
    return inos


def fds(pid):
    out = {}
    try:
        for f in os.listdir(f"/proc/{pid}/fd"):
            try:
                out[int(f)] = os.readlink(f"/proc/{pid}/fd/{f}")
            except OSError:
                pass
    except OSError:
        pass
    return out


class Watcher(threading.Thread):
    """While `ward playwright` runs: argv, ports, pipe and the temporary profile."""

    def __init__(self):
        super().__init__(daemon=True)
        self.stop = False
        self.argvs = {}
        self.listening = []
        self.profile_with_secret = []
        self.profiles = set()
        self.pipe_outside = []
        self.ss_chrome = False
        self.reads = 0

    def run(self):
        while not self.stop:
            procs = chrome_pids()
            listen_inodes = listening_inodes()
            for pid, argv in procs.items():
                self.argvs[pid] = argv
                for f, target in fds(pid).items():
                    if target.startswith("socket:[") and target[8:-1] in listen_inodes:
                        self.listening.append((pid, f))
            # The main Chrome has fds 3 and 4 (the pipe); no other readable process
            # (besides the main Chrome itself) has the same pipes.
            main_procs = [p for p, a in procs.items() if b"--remote-debugging-pipe" in a
                         and b"--type=" not in a]
            for p in main_procs:
                f = fds(p)
                pipes = {f.get(3), f.get(4)} - {None}
                if not pipes:
                    continue
                for other in os.listdir("/proc"):
                    if not other.isdigit() or int(other) == p:
                        continue
                    if pipes & set(fds(int(other)).values()):
                        self.pipe_outside.append(int(other))
            if procs:
                s = subprocess.run(["ss", "-ltnp"], capture_output=True, text=True)
                if "chrome" in s.stdout:
                    self.ss_chrome = True
            for name in os.listdir(RT):
                if not name.startswith("pw-"):
                    continue
                self.profiles.add(name)
                for dirpath, _, files in os.walk(os.path.join(RT, name)):
                    for a in files:
                        c = os.path.join(dirpath, a)
                        try:
                            if os.path.islink(c) or not os.path.isfile(c):
                                continue
                            with open(c, "rb") as h:
                                b = h.read(32 * 1024 * 1024)
                            self.reads += 1
                            if has_secret(b):
                                self.profile_with_secret.append(c)
                        except OSError:
                            pass
            time.sleep(0.05)


# --- unlock -------------------------------------------------------------------------
code, screen = run_pty(["destravar"], typed=PASSWORD, timeout=120)
verify(code == 0, f"destravar o cofre de teste (saiu {code})")
log_before = posts_log()
verify(log_before is not None, "posts.log do phish legível (serviços de pé)")

# --- V6.12 / V6.13 / V6.14 -------------------------------------------------------------
watcher = Watcher()
watcher.start()
t0 = time.time()
code, out, err = ward(["playwright", "acme-wp-prod"])
dur = time.time() - t0
watcher.stop = True
# Phase 44c (D9): free level on an http origin, so only after the origin is confirmed.
verify(window_requests() == ["pedido operacao=playwright entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao=confirmar"],
         f"Fase 44c: acme-wp-prod (livre, http) só depois da confirmação da origem, sem PIN ({window_requests()})")
watcher.join()
expected = os.path.join(RT, "playwright", "acme-wp-prod.json")
verify(code == 0, f"V6.12 ward playwright acme-wp-prod sai 0 (saiu {code}, {dur:.1f} s; erro: {err.decode(errors='replace').strip()})")
verify(out.decode().strip() == expected, f"V6.12 imprime só o caminho {expected}")
verify(not has_secret(out + err), "V6.13 saída e erro sem padrão do cofre")
state_json = b""
if os.path.exists(expected):
    st = os.lstat(expected)
    verify(stat.S_ISREG(st.st_mode) and stat.S_IMODE(st.st_mode) == 0o600, "V6.12 storage state 0600")
    state_json = open(expected, "rb").read()
    verify(not has_secret(state_json), "V6.13 storage state sem a senha nem variação")
    try:
        j = json.loads(state_json)
        names = [c["name"] for c in j["cookies"]]
        verify(any(n.startswith("wordpress_logged_in") for n in names), "V6.12 cookie de sessão do WordPress no storage state")
    except (ValueError, KeyError) as e:
        verify(False, f"V6.12 storage state em JSON do Playwright ({type(e).__name__})")
verify(len(watcher.argvs) > 0, f"V6.14 o Chrome foi visto rodando ({len(watcher.argvs)} processos)")
verify(any(b"--remote-debugging-pipe" in a for a in watcher.argvs.values()), "V6.14 Chrome com --remote-debugging-pipe")
verify(not any(b"remote-debugging-port" in a for a in watcher.argvs.values()), "V6.14 nenhum --remote-debugging-port")
verify(not any(has_secret(a) for a in watcher.argvs.values()), "V6.14 nenhum argv do Chrome com segredo")
verify(not watcher.listening, f"V6.14 nenhum processo do Chrome escutando em TCP ({watcher.listening})")
verify(not watcher.ss_chrome, "V6.14 ss -ltnp não mostra o Chrome")
verify(not watcher.pipe_outside, f"V6.14 o pipe de depuração só no Chrome principal ({watcher.pipe_outside})")
verify(watcher.reads > 0, f"V6.13 perfil temporário lido durante a sessão ({watcher.reads} leituras, {len(watcher.profiles)} perfil)")
verify(not watcher.profile_with_secret, f"V6.13 perfil temporário sem a senha ({watcher.profile_with_secret[:3]})")
verify(not [n for n in os.listdir(RT) if n.startswith("pw-")], "V6.13 o perfil temporário sumiu no fim")
verify(not chrome_pids(), "V6.13 nenhum processo do Chrome sobrou")

# The user's Playwright test, with the storage state.
pw = os.environ.get("PW_PYTHONPATH", "")
if not state_json:
    verify(False, "V6.12 Playwright do Python: sem storage state para testar")
elif pw:
    script = (
        "import sys; sys.path.insert(0, sys.argv[1])\n"
        "from playwright.sync_api import sync_playwright\n"
        "with sync_playwright() as p:\n"
        "    b = p.chromium.launch(channel='chrome', headless=True)\n"
        "    c = b.new_context(storage_state=sys.argv[2])\n"
        "    pg = c.new_page()\n"
        "    pg.goto('http://localhost:8081/wp-admin/')\n"
        "    print(pg.url)\n"
        "    print('ADMINBAR' if pg.locator('#wpadminbar').count() > 0 else 'SEM')\n"
        "    b.close()\n"
    )
    # Short (Chrome's Unix socket lives in TMPDIR) and outside the user's profile.
    tmp = f"/run/user/{os.geteuid()}/ward-pwpy-{os.getpid()}"
    os.makedirs(tmp, mode=0o700, exist_ok=True)
    env = {"PATH": "/usr/bin:/bin", "HOME": tmp, "TMPDIR": tmp, "LANG": "C.UTF-8",
           "PLAYWRIGHT_BROWSERS_PATH": os.path.join(os.path.expanduser("~"), ".cache/ms-playwright")}
    p = subprocess.run(["/usr/bin/python3", "-I", "-c", script, pw, expected], env=env,
                       capture_output=True, text=True, timeout=120)
    subprocess.run(["rm", "-rf", "--", tmp], check=False)
    lines = p.stdout.split()
    verify(p.returncode == 0 and len(lines) == 2 and "/wp-admin/" in lines[0]
             and "wp-login.php" not in lines[0] and lines[1] == "ADMINBAR",
             f"V6.12 Playwright em Python com o storage_state abre /wp-admin/ logado ({p.stdout.strip()!r} {p.stderr.strip()[-300:]!r})")
elif os.environ.get("NO_PLAYWRIGHT_PYTHON") == "1":
    print("pulo V6.12 (Playwright do Python): NO_PLAYWRIGHT_PYTHON=1")
else:
    verify(False, "V6.12 Playwright do Python: defina PW_PYTHONPATH (ou NO_PLAYWRIGHT_PYTHON=1)")

# --- V6.15: redirect to the phishing site ---------------------------------------------
code, out, err = ward(["playwright", "delta-redir"])
e = err.decode(errors="replace")
verify(code != 0 and "origem" in e, f"V6.15 delta-redir recusado por origem (saiu {code}: {e.strip()})")
verify(not os.path.exists(os.path.join(RT, "playwright", "delta-redir.json")), "V6.15 nenhum storage state do delta-redir")
log_after = posts_log()
verify(log_before is not None and log_after == log_before, "V6.15 posts.log do phish não mudou")
verify(log_after is not None and not has_secret(log_after), "V6.15 posts.log sem padrão do cofre")
verify(not chrome_pids() and not [n for n in os.listdir(RT) if n.startswith("pw-")],
         "V6.15 Chrome e perfil sumiram depois da recusa")

# --- Phase 44c: the `[::1]` attack of the M6 audit ----------------------------------------
# A process of the uid listens on [::1]:8081 (WordPress is only on 127.0.0.1:8081).
FAKE_OUTPUT = os.path.join(WORK_DIR, "falso-v6.bin")
fake = subprocess.Popen(["/usr/bin/python3", "-I", "-B", os.path.join(ROOT, "dev", "tests", "fake_v6.py"),
                          "8081", FAKE_OUTPUT], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL)
try:
    import socket
    deadline = time.time() + 10
    up = False
    while time.time() < deadline and not up:
        try:
            socket.create_connection(("::1", 8081), 1).close()
            up = True
        except OSError:
            time.sleep(0.1)
    verify(up, "ataque [::1]: o falso escuta em [::1]:8081")
    # Denied in the window: Chrome does not even open and nothing leaves.
    set_window_mode("negar")
    seen_count = len(window_requests())
    code, out, err = ward(["playwright", "acme-wp-prod"], timeout=60)
    new_requests = window_requests()[seen_count:]
    verify(code != 0 and "negado" in err.decode(errors="replace") and not out,
             f"ataque [::1], janela negada: `ward playwright acme-wp-prod` sai ≠ 0 com \"negado\" (saiu {code})")
    verify(new_requests == ["pedido operacao=playwright entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao=negar"],
             f"ataque [::1], janela negada: o pedido mostra a origem ({new_requests})")
    # Confirmed: `--host-resolver-rules` takes `localhost` to 127.0.0.1 (the real
    # WordPress); the fake one on [::1] receives nothing.
    set_window_mode("confirmar")
    code, out, err = ward(["playwright", "acme-wp-prod"])
    verify(code == 0 and out.decode(errors="replace").strip() == expected,
             f"ataque [::1], confirmado: o login vai ao WordPress de 127.0.0.1 (saiu {code}: {err.decode(errors='replace').strip()[:120]})")
    try:
        received = os.path.getsize(FAKE_OUTPUT)
    except FileNotFoundError:
        received = 0
    verify(received == 0, f"ataque [::1]: o falso de [::1]:8081 não recebeu nenhum POST ({received} bytes)")
finally:
    fake.terminate()
    fake.wait(timeout=10)
    set_window_mode("confirmar")

# --- Refusals without opening the browser ----------------------------------------------
for entry, expected_text in (("gama-bloqueada", "entrada inexistente"), ("nao-existe", "entrada inexistente"),
                       ("acme-pg-dev", "tipo sem entrega")):
    code, out, err = ward(["playwright", entry], timeout=30)
    verify(code != 0 and expected_text in err.decode(errors="replace") and not out,
             f"{entry}: recusado com '{expected_text}' (saiu {code})")

# --- V6.13: locking deletes the storage states -----------------------------------------
code, _, _ = ward(["travar"])
verify(code == 0 and not os.path.exists(expected), "V6.13 ward travar apaga o storage state")
verify(not os.listdir(os.path.join(RT, "playwright")) if os.path.isdir(os.path.join(RT, "playwright")) else True,
         "V6.13 <rt>playwright/ vazia depois de travar")

if failures:
    print(f"{len(failures)} falha(s)")
    sys.exit(1)
print("playwright.py: tudo ok")
