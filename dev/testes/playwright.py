#!/usr/bin/python3 -I
"""Conferências do `make teste-playwright` (Fase 44). Ver `dev/testes/playwright.sh`.

Uso: playwright.py <ward> <raiz> <pasta do socket = rt> <pasta de trabalho>. Só valores
falsos (D15). O Playwright do Python (V6.12) vem de `PW_PYTHONPATH`.
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

WARD, RAIZ, RT, TRAB = sys.argv[1:5]
SENHA = json.load(open(os.path.join(RAIZ, "dev/fixtures/entradas.json")))["senha_mestra"]
PADROES = [l for l in open(os.path.join(RAIZ, ".dev/variacoes.txt"), "rb").read().split(b"\n") if l]
falhas = []


APROVADOR_LOG = os.path.join(TRAB, "logs", "wardd-aprovador-teste.log")
APROVADOR_MODO = os.path.join(TRAB, "aprovador.modo")


def modo_janela(m):
    """Fase 44c: o que a janela de teste responde ao próximo pedido."""
    with open(APROVADOR_MODO, "w") as f:
        f.write(m + "\n")


def pedidos_da_janela():
    """As linhas `pedido …` que a janela de teste viu, em ordem."""
    try:
        with open(APROVADOR_LOG, encoding="utf-8", errors="replace") as f:
            return [l.strip() for l in f if l.startswith("pedido ")]
    except OSError:
        return []


def conferir(cond, msg):
    print(("ok   " if cond else "FALHA") + " " + msg, flush=True)
    if not cond:
        falhas.append(msg)


def tem_segredo(b):
    return any(p in b for p in PADROES)


def ambiente():
    return {"PATH": "/usr/bin:/bin", "HOME": "/nao-existe", "LANG": "C.UTF-8",
            "WARD_DEV_SOCKET_DIR": RT}


def rodar_pty(args, digitar=None, prazo=150):
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(WARD, [WARD] + args, ambiente())
    tela = b""
    viu = False
    fim = time.time() + prazo
    while time.time() < fim:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            try:
                b = os.read(fd, 4096)
            except OSError:
                break
            if not b:
                break
            tela += b
            if digitar is not None and not viu and b"Senha mestra" in tela:
                viu = True
                time.sleep(0.05)
                os.write(fd, digitar.encode() + b"\r")
    _, st = os.waitpid(pid, 0)
    os.close(fd)
    return os.waitstatus_to_exitcode(st), tela.decode("utf-8", "replace")


def ward(args, prazo=200):
    """`ward <args>` sem terminal: (código, stdout, stderr)."""
    p = subprocess.run([WARD] + args, env=ambiente(), capture_output=True, timeout=prazo,
                       stdin=subprocess.DEVNULL)
    return p.returncode, p.stdout, p.stderr


def posts_log():
    p = subprocess.run([os.path.join(RAIZ, "dev/servicos/compose.sh"), "exec", "-T", "phish",
                        "cat", "/registro/posts.log"], capture_output=True, timeout=60)
    if p.returncode == 0:
        return p.stdout
    # Com os serviços recém-subidos o phish ainda não criou o `posts.log` (tmpfs): nada
    # recebido, como em `dev/testes/login.py`. Outro erro (contêiner fora) continua None.
    if b"No such file or directory" in p.stderr:
        return b""
    return None


def pids_do_chrome():
    """Processos com o perfil `<rt>pw-*` no argv."""
    marca = (RT + "/pw-").encode()
    achados = {}
    for d in os.listdir("/proc"):
        if not d.isdigit():
            continue
        try:
            argv = open(f"/proc/{d}/cmdline", "rb").read()
        except OSError:
            continue
        if marca in argv:
            achados[int(d)] = argv
    return achados


def inodes_escutando():
    inos = set()
    for arq in ("/proc/net/tcp", "/proc/net/tcp6"):
        try:
            linhas = open(arq).read().splitlines()[1:]
        except OSError:
            continue
        for l in linhas:
            c = l.split()
            if len(c) > 9 and c[3] == "0A":
                inos.add(c[9])
    return inos


def fds(pid):
    saida = {}
    try:
        for f in os.listdir(f"/proc/{pid}/fd"):
            try:
                saida[int(f)] = os.readlink(f"/proc/{pid}/fd/{f}")
            except OSError:
                pass
    except OSError:
        pass
    return saida


class Vigia(threading.Thread):
    """Enquanto o `ward playwright` roda: argv, portas, pipe e o perfil temporário."""

    def __init__(self):
        super().__init__(daemon=True)
        self.parar = False
        self.argvs = {}
        self.escutando = []
        self.perfil_com_segredo = []
        self.perfis = set()
        self.pipe_fora = []
        self.ss_chrome = False
        self.leituras = 0

    def run(self):
        while not self.parar:
            procs = pids_do_chrome()
            escuta = inodes_escutando()
            for pid, argv in procs.items():
                self.argvs[pid] = argv
                for f, alvo in fds(pid).items():
                    if alvo.startswith("socket:[") and alvo[8:-1] in escuta:
                        self.escutando.append((pid, f))
            # O Chrome principal tem os fds 3 e 4 (o pipe); nenhum outro processo legível
            # (fora o próprio Chrome principal) tem os mesmos pipes.
            principal = [p for p, a in procs.items() if b"--remote-debugging-pipe" in a
                         and b"--type=" not in a]
            for p in principal:
                f = fds(p)
                pipes = {f.get(3), f.get(4)} - {None}
                if not pipes:
                    continue
                for outro in os.listdir("/proc"):
                    if not outro.isdigit() or int(outro) == p:
                        continue
                    if pipes & set(fds(int(outro)).values()):
                        self.pipe_fora.append(int(outro))
            if procs:
                s = subprocess.run(["ss", "-ltnp"], capture_output=True, text=True)
                if "chrome" in s.stdout:
                    self.ss_chrome = True
            for nome in os.listdir(RT):
                if not nome.startswith("pw-"):
                    continue
                self.perfis.add(nome)
                for raiz, _, arqs in os.walk(os.path.join(RT, nome)):
                    for a in arqs:
                        c = os.path.join(raiz, a)
                        try:
                            if os.path.islink(c) or not os.path.isfile(c):
                                continue
                            with open(c, "rb") as h:
                                b = h.read(32 * 1024 * 1024)
                            self.leituras += 1
                            if tem_segredo(b):
                                self.perfil_com_segredo.append(c)
                        except OSError:
                            pass
            time.sleep(0.05)


# --- destravar -----------------------------------------------------------------------
cod, tela = rodar_pty(["destravar"], digitar=SENHA, prazo=120)
conferir(cod == 0, f"destravar o cofre de teste (saiu {cod})")
log_antes = posts_log()
conferir(log_antes is not None, "posts.log do phish legível (serviços de pé)")

# --- V6.12 / V6.13 / V6.14 -------------------------------------------------------------
vigia = Vigia()
vigia.start()
t0 = time.time()
cod, out, err = ward(["playwright", "acme-wp-prod"])
dur = time.time() - t0
vigia.parar = True
# Fase 44c (D9): livre numa origem http, então só depois da confirmação da origem.
conferir(pedidos_da_janela() == ["pedido operacao=playwright entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao=confirmar"],
         f"Fase 44c: acme-wp-prod (livre, http) só depois da confirmação da origem, sem PIN ({pedidos_da_janela()})")
vigia.join()
esperado = os.path.join(RT, "playwright", "acme-wp-prod.json")
conferir(cod == 0, f"V6.12 ward playwright acme-wp-prod sai 0 (saiu {cod}, {dur:.1f} s; erro: {err.decode(errors='replace').strip()})")
conferir(out.decode().strip() == esperado, f"V6.12 imprime só o caminho {esperado}")
conferir(not tem_segredo(out + err), "V6.13 saída e erro sem padrão do cofre")
estado = b""
if os.path.exists(esperado):
    st = os.lstat(esperado)
    conferir(stat.S_ISREG(st.st_mode) and stat.S_IMODE(st.st_mode) == 0o600, "V6.12 storage state 0600")
    estado = open(esperado, "rb").read()
    conferir(not tem_segredo(estado), "V6.13 storage state sem a senha nem variação")
    try:
        j = json.loads(estado)
        nomes = [c["name"] for c in j["cookies"]]
        conferir(any(n.startswith("wordpress_logged_in") for n in nomes), "V6.12 cookie de sessão do WordPress no storage state")
    except (ValueError, KeyError) as e:
        conferir(False, f"V6.12 storage state em JSON do Playwright ({type(e).__name__})")
conferir(len(vigia.argvs) > 0, f"V6.14 o Chrome foi visto rodando ({len(vigia.argvs)} processos)")
conferir(any(b"--remote-debugging-pipe" in a for a in vigia.argvs.values()), "V6.14 Chrome com --remote-debugging-pipe")
conferir(not any(b"remote-debugging-port" in a for a in vigia.argvs.values()), "V6.14 nenhum --remote-debugging-port")
conferir(not any(tem_segredo(a) for a in vigia.argvs.values()), "V6.14 nenhum argv do Chrome com segredo")
conferir(not vigia.escutando, f"V6.14 nenhum processo do Chrome escutando em TCP ({vigia.escutando})")
conferir(not vigia.ss_chrome, "V6.14 ss -ltnp não mostra o Chrome")
conferir(not vigia.pipe_fora, f"V6.14 o pipe de depuração só no Chrome principal ({vigia.pipe_fora})")
conferir(vigia.leituras > 0, f"V6.13 perfil temporário lido durante a sessão ({vigia.leituras} leituras, {len(vigia.perfis)} perfil)")
conferir(not vigia.perfil_com_segredo, f"V6.13 perfil temporário sem a senha ({vigia.perfil_com_segredo[:3]})")
conferir(not [n for n in os.listdir(RT) if n.startswith("pw-")], "V6.13 o perfil temporário sumiu no fim")
conferir(not pids_do_chrome(), "V6.13 nenhum processo do Chrome sobrou")

# O teste Playwright do usuário, com o storage state.
pw = os.environ.get("PW_PYTHONPATH", "")
if not estado:
    conferir(False, "V6.12 Playwright do Python: sem storage state para testar")
elif pw:
    roteiro = (
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
    # Curto (o socket Unix do Chrome fica no TMPDIR) e fora do perfil do usuário.
    tmp = f"/run/user/{os.geteuid()}/ward-pwpy-{os.getpid()}"
    os.makedirs(tmp, mode=0o700, exist_ok=True)
    env = {"PATH": "/usr/bin:/bin", "HOME": tmp, "TMPDIR": tmp, "LANG": "C.UTF-8",
           "PLAYWRIGHT_BROWSERS_PATH": os.path.join(os.path.expanduser("~"), ".cache/ms-playwright")}
    p = subprocess.run(["/usr/bin/python3", "-I", "-c", roteiro, pw, esperado], env=env,
                       capture_output=True, text=True, timeout=120)
    subprocess.run(["rm", "-rf", "--", tmp], check=False)
    linhas = p.stdout.split()
    conferir(p.returncode == 0 and len(linhas) == 2 and "/wp-admin/" in linhas[0]
             and "wp-login.php" not in linhas[0] and linhas[1] == "ADMINBAR",
             f"V6.12 Playwright em Python com o storage_state abre /wp-admin/ logado ({p.stdout.strip()!r} {p.stderr.strip()[-300:]!r})")
elif os.environ.get("SEM_PLAYWRIGHT_PYTHON") == "1":
    print("pulo V6.12 (Playwright do Python): SEM_PLAYWRIGHT_PYTHON=1")
else:
    conferir(False, "V6.12 Playwright do Python: defina PW_PYTHONPATH (ou SEM_PLAYWRIGHT_PYTHON=1)")

# --- V6.15: redirecionamento para o phishing -------------------------------------------
cod, out, err = ward(["playwright", "delta-redir"])
e = err.decode(errors="replace")
conferir(cod != 0 and "origem" in e, f"V6.15 delta-redir recusado por origem (saiu {cod}: {e.strip()})")
conferir(not os.path.exists(os.path.join(RT, "playwright", "delta-redir.json")), "V6.15 nenhum storage state do delta-redir")
log_depois = posts_log()
conferir(log_antes is not None and log_depois == log_antes, "V6.15 posts.log do phish não mudou")
conferir(log_depois is not None and not tem_segredo(log_depois), "V6.15 posts.log sem padrão do cofre")
conferir(not pids_do_chrome() and not [n for n in os.listdir(RT) if n.startswith("pw-")],
         "V6.15 Chrome e perfil sumiram depois da recusa")

# --- Fase 44c: o ataque `[::1]` da auditoria do M6 -----------------------------------------
# Um processo do uid escuta em [::1]:8081 (o WordPress está só em 127.0.0.1:8081).
SAIDA_FALSO = os.path.join(TRAB, "falso-v6.bin")
falso = subprocess.Popen(["/usr/bin/python3", "-I", "-B", os.path.join(RAIZ, "dev", "testes", "falso_v6.py"),
                          "8081", SAIDA_FALSO], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL)
try:
    import socket
    fim = time.time() + 10
    no_ar = False
    while time.time() < fim and not no_ar:
        try:
            socket.create_connection(("::1", 8081), 1).close()
            no_ar = True
        except OSError:
            time.sleep(0.1)
    conferir(no_ar, "ataque [::1]: o falso escuta em [::1]:8081")
    # Negado na janela: o Chrome nem abre e nada sai.
    modo_janela("negar")
    vistos = len(pedidos_da_janela())
    cod, out, err = ward(["playwright", "acme-wp-prod"], prazo=60)
    novos = pedidos_da_janela()[vistos:]
    conferir(cod != 0 and "negado" in err.decode(errors="replace") and not out,
             f"ataque [::1], janela negada: `ward playwright acme-wp-prod` sai ≠ 0 com \"negado\" (saiu {cod})")
    conferir(novos == ["pedido operacao=playwright entradas=acme-wp-prod origem=http://localhost:8081 confirmacao=1 decisao=negar"],
             f"ataque [::1], janela negada: o pedido mostra a origem ({novos})")
    # Confirmado: o `--host-resolver-rules` leva `localhost` a 127.0.0.1 (o WordPress
    # verdadeiro); o falso de [::1] não recebe nada.
    modo_janela("confirmar")
    cod, out, err = ward(["playwright", "acme-wp-prod"])
    conferir(cod == 0 and out.decode(errors="replace").strip() == esperado,
             f"ataque [::1], confirmado: o login vai ao WordPress de 127.0.0.1 (saiu {cod}: {err.decode(errors='replace').strip()[:120]})")
    try:
        recebido = os.path.getsize(SAIDA_FALSO)
    except FileNotFoundError:
        recebido = 0
    conferir(recebido == 0, f"ataque [::1]: o falso de [::1]:8081 não recebeu nenhum POST ({recebido} bytes)")
finally:
    falso.terminate()
    falso.wait(timeout=10)
    modo_janela("confirmar")

# --- Recusas sem abrir o navegador -----------------------------------------------------
for entrada, texto in (("gama-bloqueada", "entrada inexistente"), ("nao-existe", "entrada inexistente"),
                       ("acme-pg-dev", "tipo sem entrega")):
    cod, out, err = ward(["playwright", entrada], prazo=30)
    conferir(cod != 0 and texto in err.decode(errors="replace") and not out,
             f"{entrada}: recusado com '{texto}' (saiu {cod})")

# --- V6.13: travar apaga os storage states ---------------------------------------------
cod, _, _ = ward(["travar"])
conferir(cod == 0 and not os.path.exists(esperado), "V6.13 ward travar apaga o storage state")
conferir(not os.listdir(os.path.join(RT, "playwright")) if os.path.isdir(os.path.join(RT, "playwright")) else True,
         "V6.13 <rt>playwright/ vazia depois de travar")

if falhas:
    print(f"{len(falhas)} falha(s)")
    sys.exit(1)
print("playwright.py: tudo ok")
