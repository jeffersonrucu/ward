#!/usr/bin/python3 -IB
"""Phase 39f (S2, S12; V5.4, V5.5): the app's copy in the graphical session, without disk.

Called by `dev/tests/clipboard.sh` (or `make test-clipboard`) with the binary of the
tests of `app/src-tauri`. Runs the ignored real-session test of the GTK clipboard module
(`TEST_NAME`) under `strace -f` and drives it through stdin. Only fake values, generated
on the spot; it never prints a value. Exits 1 if anything fails.

Watched folders (inotify, with the subfolders, and a scan at the end): `/tmp`, `/var/tmp`
and `$XDG_RUNTIME_DIR`. Before the run, the self-test: `wl-copy` (the old implementation)
writes the control value to `/tmp/wl-copy-buffer-*/` and the watcher must find it.

The test name and the line protocol on stdin (the commands and the replies) are the
interface with the Rust test; they change together with it.
"""

import ctypes
import ctypes.util
import os
import re
import secrets
import select
import shutil
import struct
import subprocess
import sys
import threading
import time

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUTPUT = os.path.join(ROOT, ".dev", "clipboard")
TIMEOUT_MS = 3000
TEST_NAME = "gtk_clipboard::tests::real_session"

UID = os.getuid()
RT = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{UID}")
FOLDERS = ["/tmp", "/var/tmp", RT]

failures = []


def fail(msg):
    failures.append(msg)
    print(f"FALHA: {msg}", flush=True)


def ok(msg):
    print(f"ok: {msg}", flush=True)


# --- file watcher (inotify) --------------------------------------------------------

IN_MODIFY = 0x2
IN_CLOSE_WRITE = 0x8
IN_MOVED_TO = 0x80
IN_CREATE = 0x100
IN_ISDIR = 0x40000000
IN_NONBLOCK = 0o4000
IN_CLOEXEC = 0o2000000
MASK = IN_MODIFY | IN_CLOSE_WRITE | IN_MOVED_TO | IN_CREATE
MAX_FILE = 1 << 20
MAX_FOLDERS = 20000


class Watcher:
    """Watches the folders and reads each written file, looking for the fake values."""

    def __init__(self, folders):
        self.libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
        self.fd = self.libc.inotify_init1(IN_NONBLOCK | IN_CLOEXEC)
        if self.fd < 0:
            raise OSError(ctypes.get_errno(), "inotify_init1")
        self.folders = {}
        self.values = set()
        self.found = []
        self.stop = threading.Event()
        self.lock = threading.Lock()
        for p in folders:
            self.watch_tree(p)
        self.thread = threading.Thread(target=self.loop, daemon=True)
        self.thread.start()

    def watch(self, folder):
        if len(self.folders) >= MAX_FOLDERS:
            return
        wd = self.libc.inotify_add_watch(self.fd, os.fsencode(folder), MASK)
        if wd >= 0:
            self.folders[wd] = folder

    def watch_tree(self, root):
        for current, subfolders, _ in os.walk(root, followlinks=False):
            self.watch(current)
            subfolders[:] = [s for s in subfolders if not os.path.islink(os.path.join(current, s))]

    def look_for(self, values):
        with self.lock:
            self.values |= {v.encode() for v in values}

    def check_file(self, path):
        try:
            st = os.lstat(path)
            if not os.path.isfile(path) or os.path.islink(path) or st.st_size > MAX_FILE:
                return
            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            try:
                data = os.read(fd, MAX_FILE)
            finally:
                os.close(fd)
        except OSError:
            return
        with self.lock:
            if any(v in data for v in self.values):
                self.found.append(path)

    def loop(self):
        while not self.stop.is_set():
            r, _, _ = select.select([self.fd], [], [], 0.1)
            if not r:
                continue
            try:
                buf = os.read(self.fd, 65536)
            except BlockingIOError:
                continue
            i = 0
            while i + 16 <= len(buf):
                wd, mask, _, size = struct.unpack_from("iIII", buf, i)
                name = buf[i + 16 : i + 16 + size].split(b"\0", 1)[0]
                i += 16 + size
                folder = self.folders.get(wd)
                if folder is None or not name:
                    continue
                path = os.path.join(folder, os.fsdecode(name))
                if mask & IN_ISDIR:
                    if mask & (IN_CREATE | IN_MOVED_TO):
                        self.watch_tree(path)
                    continue
                self.check_file(path)

    def close(self):
        self.stop.set()
        self.thread.join()
        os.close(self.fd)


def scan(values, since):
    """Files changed since `since` in the watched folders that contain a value."""
    found = []
    targets = [v.encode() for v in values]
    for root in FOLDERS:
        for current, subfolders, names in os.walk(root, followlinks=False):
            subfolders[:] = [s for s in subfolders if not os.path.islink(os.path.join(current, s))]
            for n in names:
                p = os.path.join(current, n)
                try:
                    st = os.lstat(p)
                    if not os.path.isfile(p) or os.path.islink(p) or st.st_size > MAX_FILE:
                        continue
                    if st.st_mtime < since:
                        continue
                    fd = os.open(p, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
                    try:
                        data = os.read(fd, MAX_FILE)
                    finally:
                        os.close(fd)
                except OSError:
                    continue
                if any(a in data for a in targets):
                    found.append(p)
    return found


# --- clipboard (the "user" side) ---------------------------------------------------

def reader_environment():
    env = {"PATH": "/usr/bin:/bin"}
    for v in ("WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"):
        if v in os.environ:
            env[v] = os.environ[v]
    return env


def paste(mime=None):
    """What `wl-paste` returns (bytes), or None if empty."""
    cmd = ["/usr/bin/wl-paste", "--no-newline"]
    if mime:
        cmd += ["--type", mime]
    try:
        r = subprocess.run(cmd, env=reader_environment(), capture_output=True, timeout=10)
    except subprocess.TimeoutExpired:
        return None
    return r.stdout if r.returncode == 0 else None


def mime_types():
    try:
        r = subprocess.run(["/usr/bin/wl-paste", "--list-types"], env=reader_environment(),
                           capture_output=True, timeout=10)
    except subprocess.TimeoutExpired:
        return []
    return r.stdout.decode(errors="replace").split() if r.returncode == 0 else []


def x11_types(hint_type):
    """Through X11 (the same backend as the test): the targets and the content of the hint."""
    code = (
        "import gi, sys\n"
        "gi.require_version('Gtk','3.0'); gi.require_version('Gdk','3.0')\n"
        "from gi.repository import Gtk, Gdk\n"
        "c = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)\n"
        "ok, targets = c.wait_for_targets()\n"
        "print(' '.join(a.name() for a in (targets or [])))\n"
        f"d = c.wait_for_contents(Gdk.Atom.intern({hint_type!r}, False))\n"
        "sys.stdout.write((d.get_data() or b'').decode('ascii','replace') if d else '')\n"
    )
    env = {"PATH": "/usr/bin:/bin", "GDK_BACKEND": "x11", "DISPLAY": os.environ["DISPLAY"],
           "GSETTINGS_BACKEND": "memory", "HOME": os.path.join(OUTPUT, "home")}
    if os.environ.get("XAUTHORITY"):
        env["XAUTHORITY"] = os.environ["XAUTHORITY"]
    try:
        r = subprocess.run(["/usr/bin/python3", "-I", "-B", "-c", code], env=env,
                           capture_output=True, timeout=20)
    except subprocess.TimeoutExpired:
        return [], ""
    lines = r.stdout.decode(errors="replace").split("\n", 1)
    return lines[0].split(), (lines[1] if len(lines) > 1 else "")


def copy_as_user(text):
    subprocess.run(["/usr/bin/wl-copy", "--", text], env=reader_environment(), check=True,
                   timeout=10)


def clear_clipboard():
    subprocess.run(["/usr/bin/wl-copy", "--clear"], env=reader_environment(), timeout=10)


# --- the app test ------------------------------------------------------------------

class AppTest:
    def __init__(self, binary, backend, log):
        home = os.path.join(OUTPUT, "home")
        os.makedirs(home, mode=0o700, exist_ok=True)
        env = {
            "PATH": "/usr/bin:/bin",
            "HOME": home,
            "XDG_RUNTIME_DIR": RT,
            "XDG_CONFIG_HOME": os.path.join(home, ".config"),
            "XDG_CACHE_HOME": os.path.join(home, ".cache"),
            "XDG_DATA_HOME": os.path.join(home, ".local", "share"),
            "GSETTINGS_BACKEND": "memory",
            "GDK_BACKEND": backend,
            "NO_AT_BRIDGE": "1",
            "LANG": "C.UTF-8",
        }
        if backend == "x11":
            env["DISPLAY"] = os.environ["DISPLAY"]
            if os.environ.get("XAUTHORITY"):
                env["XAUTHORITY"] = os.environ["XAUTHORITY"]
        else:
            env["WAYLAND_DISPLAY"] = os.environ["WAYLAND_DISPLAY"]
        cmd = ["/usr/bin/strace", "-f", "-qq", "-o", log,
               "-e", "trace=execve,execveat,open,openat,openat2,creat,rename,renameat,"
                     "renameat2,link,linkat,mknod,mknodat",
               binary, "--ignored", "--exact", TEST_NAME, "--nocapture", "--test-threads=1"]
        self.p = subprocess.Popen(cmd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.wait_reply("pronto")

    def wait_reply(self, expected=None, timeout=20):
        end = time.time() + timeout
        while time.time() < end:
            r, _, _ = select.select([self.p.stdout], [], [], max(0.0, end - time.time()))
            if not r:
                break
            line = self.p.stdout.readline()
            if not line:
                break
            # The 1st reply comes on the same line as libtest's "test … ...".
            mark = line.find("ward39f: ")
            if mark >= 0:
                reply = line[mark + len("ward39f: "):].strip()
                if expected is not None and reply != expected:
                    raise RuntimeError(f"teste respondeu {reply!r}, esperado {expected!r}")
                return reply
        raise RuntimeError("o teste não respondeu")

    def send(self, line, expected="ok"):
        self.p.stdin.write(line + "\n")
        self.p.stdin.flush()
        return self.wait_reply(expected)

    def quit(self):
        try:
            self.p.stdin.write("sair\n")
            self.p.stdin.flush()
            self.wait_reply("fim")
        finally:
            try:
                self.p.wait(timeout=20)
            except subprocess.TimeoutExpired:
                self.p.kill()
                self.p.wait()


WRITE = re.compile(r"O_WRONLY|O_RDWR|O_CREAT|O_TRUNC|O_APPEND|O_TMPFILE")


def check_strace(log, binary, name):
    execs, writes = [], []
    with open(log, encoding="utf-8", errors="replace") as f:
        for line in f:
            if "execve(" in line or "execveat(" in line:
                m = re.search(r'exec(?:ve|veat)\((?:\d+, )?"([^"]*)"', line)
                prog = m.group(1) if m else "?"
                if prog != binary:
                    execs.append(prog)
                continue
            m = re.search(r'\b(open|openat|openat2|creat)\((?:[A-Z_\d]+, )?"([^"]*)"(.*)', line)
            if m:
                call, path, rest = m.group(1), m.group(2), m.group(3)
                if "= -1" in rest:
                    continue
                inside = any(path == p or path.startswith(p.rstrip("/") + "/") for p in FOLDERS)
                if inside and (call == "creat" or WRITE.search(rest)):
                    writes.append(path)
                continue
            m = re.search(r'\b(?:rename|renameat2?|link|linkat|mknod|mknodat)\(.*"([^"]*)"', line)
            if m and " = 0" in line:
                path = m.group(1)
                if any(path.startswith(p.rstrip("/") + "/") for p in FOLDERS):
                    writes.append(path)
    if execs:
        fail(f"{name}: o processo do teste executou {sorted(set(execs))}")
    else:
        ok(f"{name}: nenhum programa executado (nada de wl-copy)")
    if writes:
        fail(f"{name}: abriu para escrita em pasta vigiada: {sorted(set(writes))}")
    else:
        ok(f"{name}: nenhum arquivo aberto para escrita em /tmp, /var/tmp nem {RT}")


def self_test(watcher):
    """The attack of the M5 audit must be found: wl-copy writes the value to /tmp."""
    control = "falso-39f-controle-" + secrets.token_hex(12)
    watcher.look_for([control])
    p = subprocess.Popen(["/usr/bin/wl-copy"], env=reader_environment(), stdin=subprocess.PIPE)
    p.communicate(control.encode(), timeout=10)
    end = time.time() + 10
    while time.time() < end and not watcher.found:
        time.sleep(0.1)
    clear_clipboard()
    if not watcher.found:
        fail("autoteste: o vigia não achou o valor que o wl-copy gravou em /tmp")
        return False
    ok("autoteste: o vigia acha o valor que o wl-copy grava em /tmp")
    with watcher.lock:
        watcher.found.clear()
        watcher.values.clear()
    return True


def x11_round(binary, watcher, values):
    log = os.path.join(OUTPUT, "strace-x11.log")
    v1, v2, v3, v4, v5 = values
    user_text = "texto-do-usuario-39f-" + secrets.token_hex(6)
    t = AppTest(binary, "x11", log)
    try:
        t.send(f"prazo {TIMEOUT_MS}")
        t.send(f"copiar {v1}")
        time.sleep(0.5)
        if paste() == v1.encode():
            ok("V5.4: o wl-paste devolve o valor copiado")
        else:
            fail("V5.4: o wl-paste não devolveu o valor copiado")
        targets, hint = x11_types("x-kde-passwordManagerHint")
        if "x-kde-passwordManagerHint" in targets and hint == "secret":
            ok("x-kde-passwordManagerHint = secret oferecido")
        else:
            fail(f"x-kde-passwordManagerHint ausente ou diferente (alvos {targets})")
        wl_types = mime_types()
        print(f"   tipos vistos pelo wl-paste: {' '.join(wl_types)}", flush=True)
        time.sleep(TIMEOUT_MS / 1000 + 1.5)
        if paste() == v1.encode():
            fail("V5.5: o valor continua depois do prazo")
        else:
            ok("V5.5: o valor sai depois do prazo")

        t.send(f"copiar {v2}")
        time.sleep(0.5)
        copy_as_user(user_text)
        time.sleep(TIMEOUT_MS / 1000 + 1.5)
        if paste() == user_text.encode():
            ok("V5.5: o texto que o usuário copiou depois continua lá")
        else:
            fail("V5.5: o texto que o usuário copiou depois foi apagado")

        t.send(f"copiar {v3}")
        time.sleep(0.5)
        before = paste() == v3.encode()
        t.send("limpar-fora")
        time.sleep(0.8)
        if before and paste() != v3.encode():
            ok("trava de fora (limpar fora da thread principal) tira o valor na hora")
        else:
            fail("trava de fora não tirou o valor (ou a cópia não chegou)")

        t.send(f"copiar-fora {v4}", expected="erro")
        time.sleep(0.5)
        if paste() == v4.encode():
            fail("copiar fora da thread principal chegou à área")
        else:
            ok("copiar fora da thread principal é recusado")

        t.send(f"copiar {v5}")
        time.sleep(0.5)
        present = paste() == v5.encode()
    finally:
        t.quit()
    time.sleep(1.0)
    if present and paste() != v5.encode():
        ok("ao sair do app o valor some (não fica guardado em lugar nenhum)")
    else:
        fail("o valor ficou na área de transferência depois de o app sair")
    check_strace(log, binary, "x11")


def wayland_round(binary, value):
    log = os.path.join(OUTPUT, "strace-wayland.log")
    t = AppTest(binary, "wayland", log)
    try:
        t.send(f"prazo {TIMEOUT_MS}")
        reply = t.send(f"copiar {value}", expected=None)
        time.sleep(0.8)
        pasted = paste() == value.encode()
        verdict = "aceitou" if pasted else "recusou (sem foco de teclado)"
        print(f"   wayland: copiar respondeu {reply}; compositor {verdict} a seleção",
              flush=True)
        t.send("limpar-fora")
        time.sleep(0.5)
        if pasted and paste() == value.encode():
            fail("wayland: limpar não tirou o valor")
    finally:
        t.quit()
    check_strace(log, binary, "wayland")


def main():
    if len(sys.argv) != 2 or not os.path.isabs(sys.argv[1]):
        print("uso: clipboard.py <binário absoluto dos testes do app>", file=sys.stderr)
        return 2
    binary = sys.argv[1]
    for v in ("WAYLAND_DISPLAY", "DISPLAY"):
        if not os.environ.get(v):
            print(f"area-transferencia: sem {v} (precisa da sessão gráfica)", file=sys.stderr)
            return 1
    if shutil.which("strace", path="/usr/bin:/bin") is None:
        print("area-transferencia: falta strace", file=sys.stderr)
        return 1
    if os.path.lexists(OUTPUT):
        shutil.rmtree(OUTPUT)
    os.makedirs(OUTPUT, mode=0o700)

    values = ["falso-39f-" + secrets.token_hex(12) for _ in range(6)]
    start = time.time()
    watcher = Watcher(FOLDERS)
    try:
        if not self_test(watcher):
            return 1
        watcher.look_for(values)
        x11_round(binary, watcher, values[:5])
        wayland_round(binary, values[5])
        time.sleep(0.5)
    finally:
        clear_clipboard()
        watcher.close()
    found = sorted(set(watcher.found) | set(scan(values, start - 1)))
    if found:
        fail(f"o valor apareceu em arquivo: {found}")
    else:
        ok(f"nenhum arquivo em /tmp, /var/tmp nem {RT} recebeu o valor (inotify e varredura)")
    for log in ("strace-x11.log", "strace-wayland.log"):
        p = os.path.join(OUTPUT, log)
        if os.path.exists(p):
            os.chmod(p, 0o600)
    if failures:
        print(f"area-transferencia: {len(failures)} falha(s)", flush=True)
        return 1
    print("area-transferencia: ok", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
