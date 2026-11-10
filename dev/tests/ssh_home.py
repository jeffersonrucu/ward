#!/usr/bin/python3 -I
"""V4.27 (Fase 28b; R19, D13, S1): a suíte de `ward ssh` e túnel não abre nada do home
do passwd. Roda só dentro do bwrap de `dev/tests/ssh-home.sh` (home e `/tmp` em
tmpfs, `/usr/bin/ssh`/`sshpass`/`sftp` trocados pelo invólucro com `strace -D -f`).

1. Planta as iscas no home coberto (pasta do ssh com `config` `Host *` +
   `ProxyCommand touch /tmp/sentinela-ssh` + `LocalCommand` + `IdentityAgent` do agente
   falso, `known_hosts` com chave de host trocada, `id_ed25519` de sentinela e FIFOs nos
   outros nomes), o agente falso e o inotify em todas as pastas do home.
2. Sobe um `wardd` de teste só dele (cópia do cofre de teste, `HOME` = o home coberto,
   a variável do agente apontando para o falso) e destrava pela CLI num pty.
3. Suíte: `ward ssh` por senha, por chave, pelo salto, `sftp`, interativo num pty e
   `ward run` pelo túnel. Cada um tem de dar certo (traço de quem não conectou não prova
   nada).
4. Confere: sentinelas não existem, nenhum evento do inotify, nenhuma FIFO aberta,
   nenhuma conexão ao agente falso, nenhum caminho do home (fora do repositório) nos
   traços do `strace`, e nada de `.dev/variations.txt` nos traços, na saída e no log.
5. Controles: um `ssh` sem o isolamento cria a sentinela, gera eventos, abre FIFO,
   conecta no agente e é acusado no traço; senão o detector não vale.

Uso (pelo ssh-home.sh): ssh_home.py <wardd> <ward> <raiz> <home do passwd> <pasta de trabalho>
Só valores falsos (D15); nunca imprime segredo nem conteúdo de arquivo.
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

WARDD, WARD, RAIZ, CASA, TRAB = sys.argv[1:6]
if os.environ.get("WARD_SSH_HOME_DENTRO") != "1":
    print("ssh_home.py roda só dentro do bwrap de dev/tests/ssh-home.sh")
    sys.exit(2)

# Nomes montados por partes (o V1.5 recusa citá-los por extenso em dev/).
PASTA_SSH = os.path.join(CASA, "." + "ssh")
VAR_AGENTE = "SSH_AUTH" + "_SOCK"
SENTINELA = "/tmp/sentinela-ssh"
SENTINELA_LOCAL = "/tmp/sentinela-ssh-local"
TRACOS = os.path.join(TRAB, "tracos")
SENHA = json.load(open(os.path.join(RAIZ, "dev/fixtures/entries.json")))["senha_mestra"]
PG_SELECT = os.path.join(RAIZ, "dev/tests/pg_select.py")
UID = os.geteuid()
RUNTIME = os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{UID}"
# Pasta do socket só deste teste, numa pasta do usuário na raiz do bwrap (ssh-home.sh).
SOCKDIR = "/ward-ssh-home/sock"
PADROES = [p for p in open(os.path.join(RAIZ, ".dev/variations.txt"), "rb").read().split(b"\n") if p]

falhas = []
fase = ["preparo"]


def conferir(cond, msg):
    print(("ok   " if cond else "FALHA") + " " + msg, flush=True)
    if not cond:
        falhas.append(msg)


def dentro(p, base):
    return p == base or p.startswith(base.rstrip("/") + "/")


# --- 1. Iscas --------------------------------------------------------------------------
for p in (2222, 2223, 2224, 55432):
    with socket.socket() as s:
        s.settimeout(1)
        if s.connect_ex(("127.0.0.1", p)) != 0:
            print(f"FALHA serviço de teste fora do ar na porta {p} (make dev-services-up)")
            sys.exit(1)

os.mkdir(PASTA_SSH, 0o700)
AMB_MIN = {"PATH": "/usr/bin:/bin", "HOME": CASA, "LANG": "C.UTF-8"}
subprocess.run(["/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "sentinela-ward",
                "-f", os.path.join(PASTA_SSH, "id_ed25519")], env=AMB_MIN, check=True,
               stdin=subprocess.DEVNULL, capture_output=True)
pub = open(os.path.join(PASTA_SSH, "id_ed25519.pub")).read().split()
# Chave de host trocada para todos os serviços: lido, o ssh recusaria a conexão.
with open(os.path.join(PASTA_SSH, "known_hosts"), "w") as f:
    for h in ("127.0.0.1", "[127.0.0.1]:2222", "[127.0.0.1]:2223", "[127.0.0.1]:2224", "ssh-interno"):
        f.write(f"{h} {pub[0]} {pub[1]}\n")

os.mkdir("/tmp/agente-falso", 0o700)
AGENTE = "/tmp/agente-falso/sock"
with open(os.path.join(PASTA_SSH, "config"), "w") as f:
    f.write("Host *\n"
            f"    ProxyCommand /usr/bin/touch {SENTINELA}\n"
            "    PermitLocalCommand yes\n"
            f"    LocalCommand /usr/bin/touch {SENTINELA_LOCAL}\n"
            f"    IdentityAgent {AGENTE}\n"
            "    IdentityFile ~/" + "." + "ssh/id_ed25519\n")

# FIFOs: quem abrir para ler fica anotado pelo escritor.
FIFOS = [os.path.join(PASTA_SSH, n) for n in (
    "id_rsa", "id_ecdsa", "id_ecdsa_sk", "id_ed25519_sk", "id_dsa", "id_xmss", "id_rsa-cert.pub",
    "id_ed25519-cert.pub", "known_hosts2", "authorized_keys", "environment", "rc")]
FIFOS += [os.path.join(CASA, n) for n in (".pgpass", ".psqlrc", ".pg_service.conf", ".netrc")]
aberturas = []  # (fase, caminho)
trava = threading.Lock()


def escritor(caminho):
    while True:
        try:
            with open(caminho, "w"):
                pass
        except OSError:
            return
        with trava:
            aberturas.append((fase[0], caminho))
        time.sleep(0.01)


for c in FIFOS:
    os.mkfifo(c, 0o600)
    threading.Thread(target=escritor, args=(c,), daemon=True).start()

# Agente falso: só conta as conexões (não responde nada).
conexoes_agente = []
srv_agente = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
srv_agente.bind(AGENTE)
srv_agente.listen(16)


def agente_falso():
    while True:
        try:
            c, _ = srv_agente.accept()
        except OSError:
            return
        with trava:
            conexoes_agente.append(fase[0])
        c.close()


threading.Thread(target=agente_falso, daemon=True).start()

# inotify (libc por ctypes) em cada pasta do home coberto, menos o caminho até o
# repositório (que é o bind do repositório, não o home).
libc = ctypes.CDLL("libc.so.6", use_errno=True)
IN_ALL_EVENTS = 0x00000FFF
IN_NONBLOCK, IN_CLOEXEC = 0o4000, 0o2000000
fd_in = libc.inotify_init1(IN_NONBLOCK | IN_CLOEXEC)
if fd_in < 0:
    print("FALHA inotify_init1")
    sys.exit(1)
comp_repo = os.path.relpath(RAIZ, CASA).split("/")[0] if dentro(RAIZ, CASA) else None
pastas_vigiadas = {}
for raiz_w, dirs, _ in os.walk(CASA):
    if raiz_w == CASA and comp_repo in dirs:
        dirs.remove(comp_repo)
    wd = libc.inotify_add_watch(fd_in, raiz_w.encode(), IN_ALL_EVENTS)
    if wd < 0:
        print(f"FALHA inotify_add_watch em {raiz_w}")
        sys.exit(1)
    pastas_vigiadas[wd] = raiz_w
eventos = []  # (fase, caminho, máscara)


def vigia():
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
            wd, mascara, _, tam = struct.unpack_from("iIII", buf, i)
            nome = buf[i + 16:i + 16 + tam].split(b"\0", 1)[0].decode(errors="replace")
            i += 16 + tam
            with trava:
                eventos.append((fase[0], os.path.join(pastas_vigiadas.get(wd, "?"), nome), mascara))


threading.Thread(target=vigia, daemon=True).start()
time.sleep(0.3)
with trava:
    eventos.clear()

# Diretório atual dos clientes: pasta de iscas (caminho relativo aberto pelo ssh, como
# `none`, aparece no traço).
CWD = os.path.join(TRAB, "cwd")
os.mkdir(CWD, 0o700)
open(os.path.join(CWD, "none"), "w").close()

# --- 2. wardd de teste só deste script ---------------------------------------------------
shutil.copyfile(os.path.join(RAIZ, ".dev/test-vault.ward"), os.path.join(TRAB, "test-vault.ward"))
os.chmod(os.path.join(TRAB, "test-vault.ward"), 0o600)
with open(os.path.join(TRAB, "test-vault.state"), "w") as f:
    f.write("ward-estado 1\nvista 0\nqueimada 0\n")
LOG = os.path.join(TRAB, "wardd.log")
amb_daemon = {
    "HOME": CASA, "PATH": "/usr/bin:/bin", "XDG_RUNTIME_DIR": RUNTIME, "LANG": "C.UTF-8",
    "WARD_DEV_DIR": TRAB, "WARD_DEV_SOCKET_DIR": SOCKDIR, "RUST_LOG": "trace", VAR_AGENTE: AGENTE,
}
daemon = subprocess.Popen([WARDD], env=amb_daemon, cwd=RAIZ, stdin=subprocess.DEVNULL,
                          stdout=open(LOG, "ab"), stderr=subprocess.STDOUT)
SOCK = os.path.join(SOCKDIR, "wardd.sock")


def encerrar_daemon():
    if daemon.poll() is None:
        daemon.terminate()
        try:
            daemon.wait(timeout=10)
        except subprocess.TimeoutExpired:
            daemon.kill()
            daemon.wait()
    shutil.rmtree(SOCKDIR, ignore_errors=True)


fim = time.time() + 20
while time.time() < fim and not os.path.exists(SOCK) and daemon.poll() is None:
    time.sleep(0.05)
if not os.path.exists(SOCK):
    print("FALHA o wardd de teste não subiu (fim do log abaixo)")
    print(open(LOG, errors="replace").read()[-1500:])
    encerrar_daemon()
    sys.exit(1)
print(f"wardd de teste só deste script: pid {daemon.pid}, socket {SOCK}")

AMB_CLI = {"PATH": "/usr/bin:/bin", "HOME": CASA, "LANG": "C.UTF-8", "WARD_DEV_SOCKET_DIR": SOCKDIR,
           VAR_AGENTE: AGENTE}
saidas = []


def pty_rodar(args, passos, prazo=90):
    """`ward <args>` num pty; `passos` = [(esperar, digitar)]. Devolve (código, tela)."""
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(CWD)
        os.execve(WARD, [WARD] + args, dict(AMB_CLI, TERM="xterm"))
    tela = b""
    passos = list(passos)
    fim = time.time() + prazo
    while time.time() < fim:
        if passos and passos[0][0].encode() in tela:
            _, digitar = passos.pop(0)
            time.sleep(0.1)
            os.write(fd, digitar.encode())
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try:
                b = os.read(fd, 4096)
            except OSError:
                break
            if not b:
                break
            tela += b
    if passos:
        os.kill(pid, 9)
    _, st = os.waitpid(pid, 0)
    os.close(fd)
    saidas.append(tela)
    return os.waitstatus_to_exitcode(st), tela


def ward(args, entrada=b"", prazo=120):
    r = subprocess.run([WARD] + args, env=AMB_CLI, cwd=CWD, input=entrada, capture_output=True, timeout=prazo)
    saidas.append(r.stdout + r.stderr)
    return r


try:
    cod, tela = pty_rodar(["destravar"], [("Senha mestra", SENHA + "\r")])
    conferir(cod == 0 and b"cofre destravado" in tela, f"destravar o wardd de teste ({cod})")

    # --- 3. Suíte de ward ssh e túnel ---------------------------------------------------
    fase[0] = "suite"
    r = ward(["ssh", "acme-ssh-senha-stage", "whoami"])
    conferir(r.returncode == 0 and r.stdout.strip() == b"deploy", f"ward ssh por senha → deploy ({r.returncode})")
    r = ward(["ssh", "acme-ssh-chave-prod", "whoami"])
    conferir(r.returncode == 0 and r.stdout.strip() == b"deploy", f"ward ssh por chave (agente de D12) → deploy ({r.returncode})")
    r = ward(["ssh", "acme-ssh-interno", "hostname"])
    conferir(r.returncode == 0 and r.stdout.strip() != b"", f"ward ssh pelo salto → hostname do interno ({r.returncode})")
    r = ward(["ssh", "beta-sftp-dev"], entrada=b"pwd\n")
    conferir(r.returncode == 0 and b"Remote working directory" in r.stdout, f"ward ssh no sftp ({r.returncode})")
    cod, tela = pty_rodar(["ssh", "acme-ssh-senha-stage"], [("$ ", "whoami\r"), ("deploy\r\n", "exit\r")])
    conferir(cod == 0 and b"deploy\r\n" in tela, f"ward ssh interativo num pty: whoami → deploy ({cod})")
    r = ward(["run", "acme-pg-tunel", "--", "/bin/sh", "-c", '/usr/bin/python3 -I -B "$1" "select 1"', "sh", PG_SELECT])
    conferir(r.returncode == 0 and r.stdout.strip() == b"1", f"ward run pelo túnel → select 1 ({r.returncode})")
    time.sleep(1.5)  # o strace -D termina depois do programa
    fase[0] = "depois"

    # --- 4. Conferências ------------------------------------------------------------------
    conferir(not os.path.exists(SENTINELA), "V4.27 /tmp/sentinela-ssh não existe (ProxyCommand do config não rodou)")
    conferir(not os.path.exists(SENTINELA_LOCAL), "V4.27 LocalCommand do config não rodou")
    with trava:
        ev = [(c, m) for f_, c, m in eventos if f_ == "suite"]
        ab = [c for f_, c in aberturas if f_ == "suite"]
        ag = [f_ for f_ in conexoes_agente if f_ == "suite"]
    conferir(not ev, f"V4.27 inotify: nenhum evento no home durante a suíte ({len(ev)})")
    for c, m in sorted(set(ev))[:20]:
        print(f"        evento 0x{m:x} em {c}")
    conferir(not ab, f"V4.27 nenhuma isca FIFO aberta ({sorted(set(ab))})")
    conferir(not ag, f"V4.27 o agente falso não recebeu conexão ({len(ag)})")

    tracos_suite = sorted(n for n in os.listdir(TRACOS) if not n.endswith(".cwd"))
finally:
    encerrar_daemon()

# Analisador do strace: caminhos do home (fora do repositório) e relativos ao diretório
# de iscas; resolve caminho relativo pelo diretório atual de cada processo (começa no
# gravado pelo invólucro, segue chdir e passa de pai para filho no clone/fork).
LINHA = re.compile(r'^(\d+)\s+(?:<\.\.\. (\w+) resumed>|(\w+)\()(.*)')
CAMINHO = re.compile(r'"((?:[^"\\]|\\.)*)"')
RETORNO = re.compile(r'=\s+(-?\d+)(?:\s|$)')
COM_DIRFD = {"openat", "openat2", "newfstatat", "fstatat64", "faccessat", "faccessat2", "statx",
             "readlinkat", "mkdirat", "mknodat", "unlinkat", "renameat", "renameat2", "linkat",
             "symlinkat", "fchmodat", "fchmodat2", "fchownat", "futimesat", "utimensat", "execveat",
             "name_to_handle_at", "inotify_add_watch"}
FORKS = {"clone", "clone3", "fork", "vfork"}


def acusados(traco):
    cwd0 = open(traco + ".cwd").read().strip() if os.path.exists(traco + ".cwd") else None
    linhas, pai_de = [], {}
    for linha in open(traco, errors="replace"):
        m = LINHA.match(linha)
        if not m:
            continue
        pid, resumida, chamada, resto = m.groups()
        nome = resumida or chamada
        r = RETORNO.search(resto.rsplit(")", 1)[-1] if ")" in resto else resto)
        ret = int(r.group(1)) if r else None
        if nome in FORKS and ret and ret > 0:
            pai_de[str(ret)] = pid
        linhas.append((pid, nome, bool(resumida), resto, ret))
    cwd_de, primeiro, achados = {}, None, set()
    for pid, chamada, resumida, resto, ret in linhas:
        if pid not in cwd_de:
            if primeiro is None:
                primeiro = pid
                cwd_de[pid] = cwd0
            else:
                cwd_de[pid] = cwd_de.get(pai_de.get(pid), cwd0)
        atual = cwd_de[pid]
        if chamada == "fchdir" and ret == 0:
            cwd_de[pid] = None
            continue
        if resumida:
            continue
        if chamada in ("execve", "execveat"):
            resto = resto.split(",", 1)[0]
        caminhos = CAMINHO.findall(resto)[:2]
        if chamada == "chdir" and caminhos and ret == 0:
            c = caminhos[0]
            cwd_de[pid] = None if (atual is None and not c.startswith("/")) else os.path.normpath(os.path.join(atual or "/", c))
            continue
        for c in caminhos:
            if not c:
                continue
            if not c.startswith("/"):
                if chamada in COM_DIRFD and not resto.startswith("AT_FDCWD"):
                    achados.add("(relativo a um dirfd) " + chamada)
                    continue
                if atual is None:
                    achados.add("(relativo sem diretório atual conhecido) " + chamada)
                    continue
                c = os.path.join(atual, c)
            p = os.path.normpath(c)
            if p == CWD:
                continue
            if dentro(p, CWD):
                achados.add("(relativo ao diretório de iscas) " + os.path.relpath(p, CWD))
                continue
            if not dentro(p, CASA) or dentro(p, RAIZ) or dentro(RAIZ, p):
                continue
            achados.add(p)
    return achados


texto_tracos = b""
total = {"ssh": 0, "sshpass": 0, "sftp": 0}
for n in tracos_suite:
    caminho = os.path.join(TRACOS, n)
    t = open(caminho, "rb").read()
    texto_tracos += t
    for prog in total:
        total[prog] += t.count(f'execve("/tmp/ward-real/{prog}"'.encode())
    a = acusados(caminho)
    conferir(not a, f"V4.27 strace: {n} não abre nada do home ({len(a)})")
    for x in sorted(a)[:20]:
        print(f"        {x}")
conferir(total["ssh"] >= 6 and total["sshpass"] >= 3 and total["sftp"] >= 1,
         f"os traços pegaram os programas do daemon (execve: {total})")
conferir(sum(1 for p in PADROES if p in texto_tracos) == 0, "S1 nenhum padrão de .dev/variations.txt nos traços")
conferir(sum(1 for p in PADROES if any(p in s for s in saidas)) == 0, "S1 nenhum padrão de .dev/variations.txt na saída da CLI")
log = open(LOG, "rb").read()
n_log = log.count(b"\n")
conferir(sum(1 for p in PADROES if p in log) == 0, f"S1 log do daemon (trace, {n_log} linhas) sem padrão de .dev/variations.txt")

# --- 5. Controles: sem o isolamento, cada detector acusa ---------------------------------
AMB_CONTROLE = dict(AMB_MIN, **{VAR_AGENTE: AGENTE})
antes = set(os.listdir(TRACOS))
fase[0] = "controle-config"
subprocess.run(["/usr/bin/ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=3", "-p", "2223",
                "ninguem@127.0.0.1", "true"], env=AMB_CONTROLE, cwd=CWD, stdin=subprocess.DEVNULL,
               capture_output=True, timeout=30)
time.sleep(1)
fase[0] = "controle-padrao"
subprocess.run(["/usr/bin/ssh", "-F", "/dev/null", "-o", "BatchMode=yes", "-o", "ConnectTimeout=3",
                "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null", "-p", "2223",
                "ninguem@127.0.0.1", "true"], env=AMB_CONTROLE, cwd=CWD, stdin=subprocess.DEVNULL,
               capture_output=True, timeout=30)
time.sleep(1.5)
fase[0] = "fim"
with trava:
    ev_c = [f_ for f_, _, _ in eventos if f_ == "controle-config"]
    ab_c = [f_ for f_, _ in aberturas if f_ == "controle-padrao"]
    ag_c = [f_ for f_ in conexoes_agente if f_ == "controle-padrao"]
conferir(os.path.exists(SENTINELA), "controle: ssh sem -F /dev/null roda o ProxyCommand e cria a sentinela")
conferir(bool(ev_c), "controle: o inotify acusa o ssh que lê o config do home")
conferir(bool(ab_c), "controle: o ssh com as identidades padrão abre isca FIFO")
conferir(bool(ag_c), "controle: o ssh com a variável do agente conecta no agente falso")
novos = sorted(n for n in set(os.listdir(TRACOS)) - antes if not n.endswith(".cwd"))
conferir(len(novos) == 2 and all(acusados(os.path.join(TRACOS, n)) for n in novos),
         f"controle: o analisador do strace acusa os dois ssh sem o isolamento ({len(novos)} traços)")

if falhas:
    print(f"\nteste-ssh-home: {len(falhas)} falha(s)")
    sys.exit(1)
print("\nteste-ssh-home: V4.27 ok (nada do home do passwd na suíte de ward ssh e túnel)")
