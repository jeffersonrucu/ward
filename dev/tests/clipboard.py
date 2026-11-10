#!/usr/bin/python3 -IB
"""Fase 39f (S2, S12; V5.4, V5.5): o "copiar" do app na sessão gráfica, sem disco.

Chamado por `dev/tests/clipboard.sh` (ou `make test-clipboard`) com
o binário dos testes do `app/src-tauri`. Roda o teste ignorado
`area_gtk::testes::sessao_real` sob `strace -f` e o guia pela entrada padrão. Só
valores falsos, gerados na hora; nunca imprime um valor. Sai 1 se algo falhar.

Pastas vigiadas (inotify, com as subpastas, e varredura no fim): `/tmp`, `/var/tmp` e
`$XDG_RUNTIME_DIR`. Antes da rodada, o autoteste: o `wl-copy` (a implementação antiga)
grava o valor de controle em `/tmp/wl-copy-buffer-*/` e o vigia tem de achar.
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

RAIZ = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
SAIDA = os.path.join(RAIZ, ".dev", "clipboard")
PRAZO_MS = 3000
TESTE = "area_gtk::testes::sessao_real"

UID = os.getuid()
RT = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{UID}")
PASTAS = ["/tmp", "/var/tmp", RT]

falhas = []


def falha(msg):
    falhas.append(msg)
    print(f"FALHA: {msg}", flush=True)


def ok(msg):
    print(f"ok: {msg}", flush=True)


# --- vigia de arquivos (inotify) ---------------------------------------------------

IN_MODIFY = 0x2
IN_CLOSE_WRITE = 0x8
IN_MOVED_TO = 0x80
IN_CREATE = 0x100
IN_ISDIR = 0x40000000
IN_NONBLOCK = 0o4000
IN_CLOEXEC = 0o2000000
MASCARA = IN_MODIFY | IN_CLOSE_WRITE | IN_MOVED_TO | IN_CREATE
MAX_ARQUIVO = 1 << 20
MAX_PASTAS = 20000


class Vigia:
    """Vigia as pastas e lê cada arquivo escrito, procurando os valores falsos."""

    def __init__(self, pastas):
        self.libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
        self.fd = self.libc.inotify_init1(IN_NONBLOCK | IN_CLOEXEC)
        if self.fd < 0:
            raise OSError(ctypes.get_errno(), "inotify_init1")
        self.pastas = {}
        self.valores = set()
        self.achados = []
        self.parar = threading.Event()
        self.trava = threading.Lock()
        for p in pastas:
            self.vigiar_arvore(p)
        self.thread = threading.Thread(target=self.laco, daemon=True)
        self.thread.start()

    def vigiar(self, pasta):
        if len(self.pastas) >= MAX_PASTAS:
            return
        wd = self.libc.inotify_add_watch(self.fd, os.fsencode(pasta), MASCARA)
        if wd >= 0:
            self.pastas[wd] = pasta

    def vigiar_arvore(self, raiz):
        for atual, subpastas, _ in os.walk(raiz, followlinks=False):
            self.vigiar(atual)
            subpastas[:] = [s for s in subpastas if not os.path.islink(os.path.join(atual, s))]

    def procurar(self, valores):
        with self.trava:
            self.valores |= {v.encode() for v in valores}

    def conferir_arquivo(self, caminho):
        try:
            st = os.lstat(caminho)
            if not os.path.isfile(caminho) or os.path.islink(caminho) or st.st_size > MAX_ARQUIVO:
                return
            fd = os.open(caminho, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            try:
                dados = os.read(fd, MAX_ARQUIVO)
            finally:
                os.close(fd)
        except OSError:
            return
        with self.trava:
            if any(v in dados for v in self.valores):
                self.achados.append(caminho)

    def laco(self):
        while not self.parar.is_set():
            r, _, _ = select.select([self.fd], [], [], 0.1)
            if not r:
                continue
            try:
                buf = os.read(self.fd, 65536)
            except BlockingIOError:
                continue
            i = 0
            while i + 16 <= len(buf):
                wd, mascara, _, tam = struct.unpack_from("iIII", buf, i)
                nome = buf[i + 16 : i + 16 + tam].split(b"\0", 1)[0]
                i += 16 + tam
                pasta = self.pastas.get(wd)
                if pasta is None or not nome:
                    continue
                caminho = os.path.join(pasta, os.fsdecode(nome))
                if mascara & IN_ISDIR:
                    if mascara & (IN_CREATE | IN_MOVED_TO):
                        self.vigiar_arvore(caminho)
                    continue
                self.conferir_arquivo(caminho)

    def fechar(self):
        self.parar.set()
        self.thread.join()
        os.close(self.fd)


def varrer(valores, desde):
    """Arquivos alterados desde `desde` nas pastas vigiadas que contêm um valor."""
    achados = []
    alvos = [v.encode() for v in valores]
    for raiz in PASTAS:
        for atual, subpastas, nomes in os.walk(raiz, followlinks=False):
            subpastas[:] = [s for s in subpastas if not os.path.islink(os.path.join(atual, s))]
            for n in nomes:
                p = os.path.join(atual, n)
                try:
                    st = os.lstat(p)
                    if not os.path.isfile(p) or os.path.islink(p) or st.st_size > MAX_ARQUIVO:
                        continue
                    if st.st_mtime < desde:
                        continue
                    fd = os.open(p, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
                    try:
                        dados = os.read(fd, MAX_ARQUIVO)
                    finally:
                        os.close(fd)
                except OSError:
                    continue
                if any(a in dados for a in alvos):
                    achados.append(p)
    return achados


# --- área de transferência (o lado do "usuário") ----------------------------------

def ambiente_leitor():
    env = {"PATH": "/usr/bin:/bin"}
    for v in ("WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"):
        if v in os.environ:
            env[v] = os.environ[v]
    return env


def colar(tipo=None):
    """O que o `wl-paste` devolve (bytes) ou None se vazia."""
    cmd = ["/usr/bin/wl-paste", "--no-newline"]
    if tipo:
        cmd += ["--type", tipo]
    try:
        r = subprocess.run(cmd, env=ambiente_leitor(), capture_output=True, timeout=10)
    except subprocess.TimeoutExpired:
        return None
    return r.stdout if r.returncode == 0 else None


def tipos():
    try:
        r = subprocess.run(["/usr/bin/wl-paste", "--list-types"], env=ambiente_leitor(),
                           capture_output=True, timeout=10)
    except subprocess.TimeoutExpired:
        return []
    return r.stdout.decode(errors="replace").split() if r.returncode == 0 else []


def tipos_x11(tipo_dica):
    """Pelo X11 (o mesmo backend do teste): os alvos e o conteúdo da dica."""
    codigo = (
        "import gi, sys\n"
        "gi.require_version('Gtk','3.0'); gi.require_version('Gdk','3.0')\n"
        "from gi.repository import Gtk, Gdk\n"
        "c = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)\n"
        "ok, alvos = c.wait_for_targets()\n"
        "print(' '.join(a.name() for a in (alvos or [])))\n"
        f"d = c.wait_for_contents(Gdk.Atom.intern({tipo_dica!r}, False))\n"
        "sys.stdout.write((d.get_data() or b'').decode('ascii','replace') if d else '')\n"
    )
    env = {"PATH": "/usr/bin:/bin", "GDK_BACKEND": "x11", "DISPLAY": os.environ["DISPLAY"],
           "GSETTINGS_BACKEND": "memory", "HOME": os.path.join(SAIDA, "home")}
    if os.environ.get("XAUTHORITY"):
        env["XAUTHORITY"] = os.environ["XAUTHORITY"]
    try:
        r = subprocess.run(["/usr/bin/python3", "-I", "-B", "-c", codigo], env=env,
                           capture_output=True, timeout=20)
    except subprocess.TimeoutExpired:
        return [], ""
    linhas = r.stdout.decode(errors="replace").split("\n", 1)
    return linhas[0].split(), (linhas[1] if len(linhas) > 1 else "")


def copiar_como_usuario(texto):
    subprocess.run(["/usr/bin/wl-copy", "--", texto], env=ambiente_leitor(), check=True,
                   timeout=10)


def limpar_area():
    subprocess.run(["/usr/bin/wl-copy", "--clear"], env=ambiente_leitor(), timeout=10)


# --- o teste do app ----------------------------------------------------------------

class Teste:
    def __init__(self, binario, backend, log):
        home = os.path.join(SAIDA, "home")
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
               binario, "--ignored", "--exact", TESTE, "--nocapture", "--test-threads=1"]
        self.p = subprocess.Popen(cmd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.esperar("pronto")

    def esperar(self, esperado=None, prazo=20):
        fim = time.time() + prazo
        while time.time() < fim:
            r, _, _ = select.select([self.p.stdout], [], [], max(0.0, fim - time.time()))
            if not r:
                break
            linha = self.p.stdout.readline()
            if not linha:
                break
            # A 1ª resposta vem na mesma linha do "test … ..." do libtest.
            marca = linha.find("ward39f: ")
            if marca >= 0:
                resposta = linha[marca + len("ward39f: "):].strip()
                if esperado is not None and resposta != esperado:
                    raise RuntimeError(f"teste respondeu {resposta!r}, esperado {esperado!r}")
                return resposta
        raise RuntimeError("o teste não respondeu")

    def pedir(self, linha, esperado="ok"):
        self.p.stdin.write(linha + "\n")
        self.p.stdin.flush()
        return self.esperar(esperado)

    def sair(self):
        try:
            self.p.stdin.write("sair\n")
            self.p.stdin.flush()
            self.esperar("fim")
        finally:
            try:
                self.p.wait(timeout=20)
            except subprocess.TimeoutExpired:
                self.p.kill()
                self.p.wait()


ESCRITA = re.compile(r"O_WRONLY|O_RDWR|O_CREAT|O_TRUNC|O_APPEND|O_TMPFILE")


def conferir_strace(log, binario, nome):
    execs, escritas = [], []
    with open(log, encoding="utf-8", errors="replace") as f:
        for linha in f:
            if "execve(" in linha or "execveat(" in linha:
                m = re.search(r'exec(?:ve|veat)\((?:\d+, )?"([^"]*)"', linha)
                prog = m.group(1) if m else "?"
                if prog != binario:
                    execs.append(prog)
                continue
            m = re.search(r'\b(open|openat|openat2|creat)\((?:[A-Z_\d]+, )?"([^"]*)"(.*)', linha)
            if m:
                chamada, caminho, resto = m.group(1), m.group(2), m.group(3)
                if "= -1" in resto:
                    continue
                dentro = any(caminho == p or caminho.startswith(p.rstrip("/") + "/") for p in PASTAS)
                if dentro and (chamada == "creat" or ESCRITA.search(resto)):
                    escritas.append(caminho)
                continue
            m = re.search(r'\b(?:rename|renameat2?|link|linkat|mknod|mknodat)\(.*"([^"]*)"', linha)
            if m and " = 0" in linha:
                caminho = m.group(1)
                if any(caminho.startswith(p.rstrip("/") + "/") for p in PASTAS):
                    escritas.append(caminho)
    if execs:
        falha(f"{nome}: o processo do teste executou {sorted(set(execs))}")
    else:
        ok(f"{nome}: nenhum programa executado (nada de wl-copy)")
    if escritas:
        falha(f"{nome}: abriu para escrita em pasta vigiada: {sorted(set(escritas))}")
    else:
        ok(f"{nome}: nenhum arquivo aberto para escrita em /tmp, /var/tmp nem {RT}")


def autoteste(vigia):
    """O ataque da auditoria do M5 tem de ser achado: o wl-copy grava o valor em /tmp."""
    controle = "falso-39f-controle-" + secrets.token_hex(12)
    vigia.procurar([controle])
    p = subprocess.Popen(["/usr/bin/wl-copy"], env=ambiente_leitor(), stdin=subprocess.PIPE)
    p.communicate(controle.encode(), timeout=10)
    fim = time.time() + 10
    while time.time() < fim and not vigia.achados:
        time.sleep(0.1)
    limpar_area()
    if not vigia.achados:
        falha("autoteste: o vigia não achou o valor que o wl-copy gravou em /tmp")
        return False
    ok("autoteste: o vigia acha o valor que o wl-copy grava em /tmp")
    with vigia.trava:
        vigia.achados.clear()
        vigia.valores.clear()
    return True


def rodada_x11(binario, vigia, valores):
    log = os.path.join(SAIDA, "strace-x11.log")
    v1, v2, v3, v4, v5 = valores
    usuario = "texto-do-usuario-39f-" + secrets.token_hex(6)
    t = Teste(binario, "x11", log)
    try:
        t.pedir(f"prazo {PRAZO_MS}")
        t.pedir(f"copiar {v1}")
        time.sleep(0.5)
        if colar() == v1.encode():
            ok("V5.4: o wl-paste devolve o valor copiado")
        else:
            falha("V5.4: o wl-paste não devolveu o valor copiado")
        alvos, dica = tipos_x11("x-kde-passwordManagerHint")
        if "x-kde-passwordManagerHint" in alvos and dica == "secret":
            ok("x-kde-passwordManagerHint = secret oferecido")
        else:
            falha(f"x-kde-passwordManagerHint ausente ou diferente (alvos {alvos})")
        tipos_wl = tipos()
        print(f"   tipos vistos pelo wl-paste: {' '.join(tipos_wl)}", flush=True)
        time.sleep(PRAZO_MS / 1000 + 1.5)
        if colar() == v1.encode():
            falha("V5.5: o valor continua depois do prazo")
        else:
            ok("V5.5: o valor sai depois do prazo")

        t.pedir(f"copiar {v2}")
        time.sleep(0.5)
        copiar_como_usuario(usuario)
        time.sleep(PRAZO_MS / 1000 + 1.5)
        if colar() == usuario.encode():
            ok("V5.5: o texto que o usuário copiou depois continua lá")
        else:
            falha("V5.5: o texto que o usuário copiou depois foi apagado")

        t.pedir(f"copiar {v3}")
        time.sleep(0.5)
        antes = colar() == v3.encode()
        t.pedir("limpar-fora")
        time.sleep(0.8)
        if antes and colar() != v3.encode():
            ok("trava de fora (limpar fora da thread principal) tira o valor na hora")
        else:
            falha("trava de fora não tirou o valor (ou a cópia não chegou)")

        t.pedir(f"copiar-fora {v4}", esperado="erro")
        time.sleep(0.5)
        if colar() == v4.encode():
            falha("copiar fora da thread principal chegou à área")
        else:
            ok("copiar fora da thread principal é recusado")

        t.pedir(f"copiar {v5}")
        time.sleep(0.5)
        presente = colar() == v5.encode()
    finally:
        t.sair()
    time.sleep(1.0)
    if presente and colar() != v5.encode():
        ok("ao sair do app o valor some (não fica guardado em lugar nenhum)")
    else:
        falha("o valor ficou na área de transferência depois de o app sair")
    conferir_strace(log, binario, "x11")


def rodada_wayland(binario, valor):
    log = os.path.join(SAIDA, "strace-wayland.log")
    t = Teste(binario, "wayland", log)
    try:
        t.pedir(f"prazo {PRAZO_MS}")
        resposta = t.pedir(f"copiar {valor}", esperado=None)
        time.sleep(0.8)
        colado = colar() == valor.encode()
        print(f"   wayland: copiar respondeu {resposta}; compositor "
              f"{'aceitou' if colado else 'recusou (sem foco de teclado)'} a seleção",
              flush=True)
        t.pedir("limpar-fora")
        time.sleep(0.5)
        if colado and colar() == valor.encode():
            falha("wayland: limpar não tirou o valor")
    finally:
        t.sair()
    conferir_strace(log, binario, "wayland")


def main():
    if len(sys.argv) != 2 or not os.path.isabs(sys.argv[1]):
        print("uso: clipboard.py <binário absoluto dos testes do app>", file=sys.stderr)
        return 2
    binario = sys.argv[1]
    for v in ("WAYLAND_DISPLAY", "DISPLAY"):
        if not os.environ.get(v):
            print(f"area-transferencia: sem {v} (precisa da sessão gráfica)", file=sys.stderr)
            return 1
    if shutil.which("strace", path="/usr/bin:/bin") is None:
        print("area-transferencia: falta strace", file=sys.stderr)
        return 1
    if os.path.lexists(SAIDA):
        shutil.rmtree(SAIDA)
    os.makedirs(SAIDA, mode=0o700)

    valores = ["falso-39f-" + secrets.token_hex(12) for _ in range(6)]
    inicio = time.time()
    vigia = Vigia(PASTAS)
    try:
        if not autoteste(vigia):
            return 1
        vigia.procurar(valores)
        rodada_x11(binario, vigia, valores[:5])
        rodada_wayland(binario, valores[5])
        time.sleep(0.5)
    finally:
        limpar_area()
        vigia.fechar()
    achados = sorted(set(vigia.achados) | set(varrer(valores, inicio - 1)))
    if achados:
        falha(f"o valor apareceu em arquivo: {achados}")
    else:
        ok(f"nenhum arquivo em /tmp, /var/tmp nem {RT} recebeu o valor (inotify e varredura)")
    for log in ("strace-x11.log", "strace-wayland.log"):
        p = os.path.join(SAIDA, log)
        if os.path.exists(p):
            os.chmod(p, 0o600)
    if falhas:
        print(f"area-transferencia: {len(falhas)} falha(s)", flush=True)
        return 1
    print("area-transferencia: ok", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
