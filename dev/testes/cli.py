#!/usr/bin/python3 -I
"""`make teste-cli` (Fase 25): a CLI `ward` de teste (feature `modo-teste`) de ponta a
ponta, com o terminal num pty de verdade.

1. V4.2 (S5): um servidor falso em Python que grava tudo o que recebe, no caminho do
   socket de teste (`WARD_DEV_SOCKET_DIR` numa pasta 0700 só deste teste). `ward
   destravar` precisa abortar com "servidor não é o wardd" sem mostrar o prompt da
   senha; a senha é digitada assim mesmo e o registro do falso fica vazio.
2. Contra o daemon de teste (`make dev-daemon`, cofre de `.dev/`): `ward travar`,
   `ward lista` responde "travado" (V4.22); `ward destravar` com senha errada e certa
   pelo pty; `ward lista --json` só com os seis campos e sem nenhum padrão de
   `.dev/variacoes.txt` (V4.1, S1). O prompt aparece e a senha não ecoa.
3. `ward run` (Fase 26).
4. `ward ssh` (Fase 27) contra os SSH de teste (2222, 2223): V4.23, V4.24, V4.26 (agente
   atacado por `ssh-add`, SIGKILL na CLI, travar), V4.28, V4.31, V4.32, V4.34 (pty),
   V4.14 e V4.35/V4.37 pelo argv em `/proc/<pid>/cmdline` (o `wardd` recusa subir sob
   `strace`).
5. Salto e túnel (Fase 28) contra o `ssh-salto`/`ssh-interno` e o Postgres de teste:
   V4.25, V4.29 (cliente `dev/testes/pg_select.py`), V4.30, V4.35 com salto (nenhum
   `ssh` filho de outro `ssh`: sem `sh -c`/`ProxyJump`), ataque à ponte do túnel e
   V4.33 (log do daemon, que o `make dev-daemon` sobe com `RUST_LOG=trace`, sem nenhum
   padrão de `.dev/variacoes.txt`).

6. Fase 27a (achado alto da auditoria do M3): o token do "Conectar" chega ao `ward ssh`
   só por um socket herdado (`--token-fd <n>`), por um terminal falso, contra o daemon
   de teste; um leitor em laço de `/proc/*/cmdline`, `/proc/*/environ` e
   `/proc/<ward>/fd/*` não acha o token; `--token` em argv é recusado sem conectar; canal
   com lixo, vazio, grande ou pipe é recusado sem eco. Não precisa dos SSH de teste.

Com `WARD_DEV_SOCKET_DIR` no ambiente (pasta absoluta 0700), usa o daemon de teste
desse socket (o de `WARD_DEV_SOCKET_DIR=… make dev-daemon`), no lugar de
`$XDG_RUNTIME_DIR/ward-teste`.

Uso: dev/testes/cli.py <binário ward> <raiz do repositório>. Só valores falsos (D15).
"""

import json
import os
import pty
import select
import socket
import sys
import threading
import time

WARD, RAIZ = sys.argv[1], sys.argv[2]
SENHA = json.load(open(os.path.join(RAIZ, "dev/fixtures/entradas.json")))["senha_mestra"]
UID = os.geteuid()
falhas = []


def conferir(cond, msg):
    print(("ok   " if cond else "FALHA") + " " + msg)
    if not cond:
        falhas.append(msg)


PASTA_SOCKET = os.environ.get("WARD_DEV_SOCKET_DIR") or f"/run/user/{UID}/ward-teste"
if not os.path.isabs(PASTA_SOCKET):
    print("FALHA WARD_DEV_SOCKET_DIR não é absoluto")
    sys.exit(1)


def ambiente(extra=None):
    e = {"PATH": "/usr/bin:/bin", "HOME": "/nao-existe", "LANG": "C.UTF-8"}
    if "WARD_DEV_SOCKET_DIR" in os.environ:
        e["WARD_DEV_SOCKET_DIR"] = PASTA_SOCKET
    e.update(extra or {})
    return e


def rodar_pty(args, digitar=None, env=None, prazo=150):
    """Roda no pty. Se `digitar`, escreve a senha quando o prompt aparecer (ou, se o
    processo sair antes, não escreve). Devolve (código, tudo o que apareceu na tela,
    se o prompt apareceu)."""
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(WARD, [WARD] + args, env or ambiente())
    tela = b""
    viu_prompt = False
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
            if digitar is not None and not viu_prompt and b"Senha mestra" in tela:
                viu_prompt = True
                time.sleep(0.05)
                os.write(fd, digitar.encode() + b"\r")
    _, st = os.waitpid(pid, 0)
    os.close(fd)
    return os.waitstatus_to_exitcode(st), tela.decode("utf-8", "replace"), viu_prompt


# --- 1. V4.2: servidor falso no socket de teste -----------------------------------
pasta = f"/run/user/{UID}/ward-cli-falso-{os.getpid()}"
os.mkdir(pasta, 0o700)
caminho = os.path.join(pasta, "wardd.sock")
registro = bytearray()
srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
srv.bind(caminho)
os.chmod(caminho, 0o600)
srv.listen(8)


def falso():
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
                registro.extend(b)
        except OSError:
            pass
        c.close()


threading.Thread(target=falso, daemon=True).start()
try:
    env = ambiente({"WARD_DEV_SOCKET_DIR": pasta})
    cod, tela, prompt = rodar_pty(["destravar"], digitar=SENHA, env=env, prazo=20)
    conferir(cod != 0, f"V4.2 destravar contra o falso sai ≠ 0 (saiu {cod})")
    conferir("servidor não é o wardd" in tela, "V4.2 mensagem 'servidor não é o wardd'")
    conferir(not prompt, "V4.2 o prompt da senha não apareceu")
    for args in (["lista", "--json"], ["travar"]):
        cod, tela, _ = rodar_pty(args, env=env, prazo=20)
        conferir(cod != 0 and "servidor não é o wardd" in tela, f"V4.2 {args[0]} recusa o falso")
    time.sleep(0.2)
    conferir(len(registro) == 0, f"V4.2 o falso recebeu {len(registro)} bytes")
    conferir(SENHA.encode() not in registro, "V4.2 a senha não está no registro do falso")
finally:
    srv.close()
    os.unlink(caminho)
    os.rmdir(pasta)

# --- 2. Contra o daemon de teste ----------------------------------------------------
sock = os.path.join(PASTA_SOCKET, "wardd.sock")
if not os.path.exists(sock):
    print("FALHA o daemon de teste não está no ar: rode 'make dev-daemon'")
    sys.exit(1)

cod, tela, _ = rodar_pty(["travar"])
conferir(cod == 0 and "cofre travado" in tela, "travar")
cod, tela, _ = rodar_pty(["lista"])
conferir(cod != 0 and "travado" in tela, "V4.22 lista depois de travar responde 'travado'")

cod, tela, prompt = rodar_pty(["destravar"], digitar="falso-errada-ward-99")
conferir(prompt and cod != 0 and "senha mestra incorreta" in tela, "destravar com senha errada")
conferir("falso-errada-ward-99" not in tela, "a senha errada não ecoou")

cod, tela, prompt = rodar_pty(["destravar"], digitar=SENHA)
conferir(prompt, "destravar mostra o prompt da senha")
conferir(cod == 0 and "cofre destravado" in tela, f"destravar com a senha certa (saiu {cod})")
conferir(SENHA not in tela, "a senha não ecoou na tela")

cod, tela, prompt = rodar_pty(["destravar"], digitar=SENHA)
conferir(cod == 0 and "já está destravado" in tela and not prompt, "destravar de novo não pede senha")

# lista --json: stdout puro, sem pty.
import subprocess  # noqa: E402

r = subprocess.run([WARD, "lista", "--json"], env=ambiente(), capture_output=True, timeout=60)
conferir(r.returncode == 0, "V4.1 lista --json sai 0")
itens = json.loads(r.stdout)
nomes = [i["nome"] for i in itens]
conferir("gama-bloqueada" not in nomes, "V4.1 sem a bloqueada")
esperadas = [e["nome"] for e in json.load(open(os.path.join(RAIZ, "dev/fixtures/entradas.json")))["entradas"]
             if e.get("nivel") != "bloqueado"]
conferir(sorted(nomes) == sorted(esperadas), f"V4.1 {len(nomes)} entradas visíveis, todas menos a bloqueada")
campos = {"nome", "tipo", "grupo", "ambiente", "projeto", "endereco"}
conferir(all(set(i) == campos for i in itens), "V4.1 só nome, tipo, grupo, ambiente, projeto e endereço")
r2 = subprocess.run([WARD, "lista"], env=ambiente(), capture_output=True, timeout=60)
conferir(r2.returncode == 0 and b"NOME" in r2.stdout, "V4.1 lista em texto")

variacoes = os.path.join(RAIZ, ".dev/variacoes.txt")
if os.path.exists(variacoes):
    padroes = [p for p in open(variacoes, "rb").read().split(b"\n") if p]
    saida = r.stdout + r.stderr + r2.stdout + r2.stderr
    achados = sum(1 for p in padroes if p in saida)
    conferir(achados == 0, f"S1 nenhum padrão de .dev/variacoes.txt na saída da lista ({achados})")
else:
    conferir(False, "falta .dev/variacoes.txt: rode 'make dev-variacoes'")
    padroes = []

# --- 3. `ward run` (Fase 26) contra o daemon de teste ---------------------------------
# Só comandos locais (sh, env, cat, sha256sum): nenhum cliente ssh/curl aqui.


def run(args, env=None, entrada=b""):
    return subprocess.run([WARD, "run"] + args, env=env or ambiente(), input=entrada,
                          capture_output=True, timeout=150)


def sem_variacao(r, rotulo):
    tudo = r.stdout + r.stderr
    achados = sum(1 for p in padroes if p in tudo)
    conferir(achados == 0, f"S4 {rotulo}: nenhum padrão de .dev/variacoes.txt na saída ({achados})")


r = run(["acme-wp-prod", "--", "/bin/sh", "-c", 'printf "%s|%s" "$WARD_USER" "$WARD_PASS"; exit 5'])
conferir(r.returncode == 5, f"V4.19 o código do comando volta (saiu {r.returncode})")
conferir(r.stdout.decode().endswith("|••••"), "V4.17 a senha impressa sai ••••")
sem_variacao(r, "login")

r = run(["gama-generico", "--", "/bin/sh", "-c",
         'f="$WARD_ARQUIVO_APP_ENV"; stat -c %a "$f"; cat "$f"; env; echo "$f"'])
saida = r.stdout.decode(errors="replace")
conferir(r.returncode == 0 and saida.startswith("600\n"), "V4.10 anexo 0600 entregue")
caminho = saida.strip().split("\n")[-1]
conferir(caminho.startswith(f"/run/user/{UID}/") and "/tmp/" in caminho, "S2 anexo em <rt>tmp/")
conferir(not os.path.exists(caminho), "V4.15 anexo apagado no fim")
conferir("LISTEN_" not in saida and "WARD_DEV_DIR" not in saida, "D60 nada do ambiente do daemon")
sem_variacao(r, "genérico + anexo + env")

r = run(["acme-wp-prod", "--", "/bin/cat"], entrada=b"entrada padrao 26\n")
conferir(r.returncode == 0 and r.stdout == b"entrada padrao 26\n", "D13 a entrada padrão chega ao comando")

r = run(["gama-nota", "--", "/bin/true"])
conferir(r.returncode != 0 and b"tipo sem entrega" in r.stderr, "V4.11 nota segura: tipo sem entrega")

r = run(["gama-bloqueada", "--", "/bin/true"])
conferir(r.returncode != 0 and b"entrada" in r.stderr, "bloqueada = inexistente")

grande = ambiente({f"GRANDE_{i}": "x" * (100 * 1024) for i in range(8)})
r = run(["acme-wp-prod", "--", "/bin/true"], env=grande)
conferir(r.returncode != 0 and b"passam de 750 KiB" in r.stderr and b"xxxx" not in r.stderr,
         "V4.36 (D52) ambiente acima de 750 KiB recusado sem citar valor")

r = run(["acme-wp-prod", "--", "/usr/bin/env"], env=ambiente({"LD_PRELOAD": "/nao/existe.so"}))
conferir(r.returncode == 0 and b"LD_PRELOAD" not in r.stdout, "D13 LD_* não chega ao filho")

# V4.17 (S4): $PGPASSWORD puro, em base64, com urllib.parse.quote, cortado no `:` com
# `sleep 0.2`, e tudo de novo no stderr.
V417 = r'''
p="$PGPASSWORD"
printf '%s\n' "$p"; printf %s "$p" | base64
python3 -I -c 'import os, urllib.parse; print(urllib.parse.quote(os.environ["PGPASSWORD"], safe=""))'
printf '%s' "${p%%:*}:"; sleep 0.2; printf '%s\n' "${p#*:}"
'''
r = run(["acme-pg-dev", "--", "/bin/sh", "-c", V417 + "{\n" + V417 + "} >&2"])
saida = r.stdout + r.stderr
conferir(r.returncode == 0 and saida.count("••••".encode()) >= 8, "V4.17 as oito formas saem ••••")
sem_variacao(r, "V4.17")

# V4.3–V4.8, V4.10: o filho imprime só o sha256 de cada variável (e de cada arquivo
# entregue); o esperado sai da tabela "Entrega esperada", com os valores falsos lidos de
# dev/fixtures/entradas.json (nenhum segredo escrito aqui).
import hashlib  # noqa: E402
import hmac  # noqa: E402
import struct  # noqa: E402
import urllib.parse  # noqa: E402

FIX = {e["nome"]: e for e in json.load(open(os.path.join(RAIZ, "dev/fixtures/entradas.json")))["entradas"]}


def c(n, k):
    return str(FIX[n]["campos"][k])


def s(n, k):
    return FIX[n]["segredos"][k]


def totp(semente_b32, t):
    chave = __import__("base64").b32decode(semente_b32)
    h = hmac.new(chave, struct.pack(">Q", int(t) // 30), hashlib.sha1).digest()
    o = h[-1] & 15
    return "%06d" % ((struct.unpack(">I", h[o:o + 4])[0] & 0x7FFFFFFF) % 1000000)


def h(v):
    return hashlib.sha256(v if isinstance(v, bytes) else v.encode()).hexdigest()


pg = {"PGHOST": c("acme-pg-dev", "host"), "PGPORT": c("acme-pg-dev", "porta"),
      "PGDATABASE": c("acme-pg-dev", "base"), "PGUSER": c("acme-pg-dev", "usuario"),
      "PGPASSWORD": s("acme-pg-dev", "senha")}
pg["DATABASE_URL"] = "postgresql://%s:%s@%s:%s/%s" % (
    pg["PGUSER"], urllib.parse.quote(pg["PGPASSWORD"], safe=""), pg["PGHOST"], pg["PGPORT"], pg["PGDATABASE"])
ESPERADO = {
    "acme-wp-prod": {"WARD_URL": c("acme-wp-prod", "url"), "WARD_USER": c("acme-wp-prod", "usuario"),
                     "WARD_PASS": s("acme-wp-prod", "senha")},
    "acme-pg-dev": pg,
    "acme-ssh-senha-stage": {"WARD_HOST": "127.0.0.1", "WARD_PORT": "2222", "WARD_USER": "deploy",
                             "WARD_PASS": s("acme-ssh-senha-stage", "senha")},
    "acme-ssh-chave-prod": {"WARD_HOST": "127.0.0.1", "WARD_PORT": "2223", "WARD_USER": "deploy",
                            "WARD_PASSPHRASE": s("acme-ssh-chave-prod", "passphrase")},
    "beta-github-api": {"WARD_URL": c("beta-github-api", "url"), "WARD_TOKEN": s("beta-github-api", "token"),
                        "GITHUB_TOKEN": s("beta-github-api", "token")},
    "beta-aws-dev": {"AWS_ACCESS_KEY_ID": c("beta-aws-dev", "access_key_id"),
                     "AWS_SECRET_ACCESS_KEY": s("beta-aws-dev", "secret_access_key"),
                     "AWS_SESSION_TOKEN": s("beta-aws-dev", "session_token"),
                     "AWS_REGION": c("beta-aws-dev", "regiao"), "AWS_DEFAULT_REGION": c("beta-aws-dev", "regiao"),
                     "AWS_ROLE_ARN": c("beta-aws-dev", "role_arn")},
    "beta-smtp-stage": {"SMTP_HOST": c("beta-smtp-stage", "host"), "SMTP_PORT": c("beta-smtp-stage", "porta"),
                        "SMTP_USER": c("beta-smtp-stage", "usuario"), "SMTP_PASS": s("beta-smtp-stage", "senha"),
                        "SMTP_SECURITY": c("beta-smtp-stage", "seguranca")},
    "beta-sftp-dev": {"WARD_PROTOCOLO": "sftp", "WARD_HOST": "127.0.0.1", "WARD_PORT": "2222",
                      "WARD_USER": "sftpuser", "WARD_PASS": s("beta-sftp-dev", "senha")},
    "gama-generico": {"WARD_REGIAO": "r-01",
                      "WARD_CHAVE_LICENCA": [x for x in FIX["gama-generico"]["extras"]
                                             if x["nome"] == "CHAVE_LICENCA"][0]["valor"]},
}
ARQUIVOS = {
    "acme-ssh-chave-prod": {"WARD_ARQUIVO_CHAVE": s("acme-ssh-chave-prod", "chave_privada_arquivo")},
    "gama-generico": {"WARD_ARQUIVO_APP_ENV": FIX["gama-generico"]["anexos"][0]["arquivo"]},
}
# Para cada nome: "<nome> <sha256 do valor>"; para arquivo: "<nome> <modo> <pasta> <sha256>".
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
for nome, vars_ in ESPERADO.items():
    extra = FIX[nome]
    tudo = dict(vars_, WARD_ENTRADA=nome, WARD_GRUPO=extra["grupo"], WARD_AMBIENTE=extra["ambiente"])
    nomes = list(tudo) + ["arq:" + k for k in ARQUIVOS.get(nome, {})]
    if nome == "acme-wp-prod":
        nomes.append("WARD_TOTP")
    antes = time.time()
    r = run([nome, "--", "/bin/sh", "-c", SCRIPT, "sh"] + nomes)
    depois = time.time()
    linhas = dict(l.split(" ", 1) for l in r.stdout.decode().splitlines() if " " in l)
    bate = r.returncode == 0 and all(linhas.get(k) == h(v) for k, v in tudo.items())
    for k, caminho in ARQUIVOS.get(nome, {}).items():
        conteudo = open(os.path.join(RAIZ, caminho), "rb").read()
        modo, pasta, hash_ = (linhas.get(k) or "  ").split(" ")
        bate = bate and modo == "600" and pasta.startswith(f"/run/user/{UID}/") \
            and pasta.endswith("/tmp") and hash_ == h(conteudo)
    if nome == "acme-wp-prod":
        semente = s(nome, "totp_semente")
        bate = bate and linhas.get("WARD_TOTP") in {h(totp(semente, t)) for t in (antes, depois)}
    conferir(bate, f"V4.3–V4.10 entrega de {nome} bate com a tabela ({len(nomes)} itens)")
    sem_variacao(r, f"hashes de {nome}")

# --- 4. `ward ssh` (Fase 27) contra o daemon de teste e os SSH de teste ---------------
# O `ssh`/`sshpass`/`sftp` quem roda é o daemon (D13: `-F /dev/null`, `known_hosts` do
# Ward em `.dev/known_hosts`); daqui só a CLI. Precisa dos serviços de pé
# (`make dev-servicos-subir`); sem eles, a seção é pulada com aviso (não passa).
import glob  # noqa: E402
import signal  # noqa: E402

DEV = os.path.join(RAIZ, ".dev")
RT_TESTE = PASTA_SOCKET
SENHA_SSH = FIX["acme-ssh-senha-stage"]["segredos"]["senha"]


def porta_aberta(p):
    try:
        socket.create_connection(("127.0.0.1", p), timeout=2).close()
        return True
    except OSError:
        return False


def ssh(args, entrada=b"", env=None, prazo=150):
    return subprocess.run([WARD, "ssh"] + args, env=env or ambiente(), input=entrada,
                          capture_output=True, timeout=prazo)


# Terminal falso da Fase 27a: tira o `-e` e roda o resto, como o `x-terminal-emulator
# -e`; repassa os fds herdados (o que o cosmic-term e o xterm fazem).
TERMINAL_FALSO = ["/bin/sh", "-c", 'shift; exec "$@"', "terminal-falso", "-e"]


def ssh_com_canal(conteudo, args, canal="socket", prazo=150):
    """Como o app (Fase 27a): `conteudo` na fila de um socket Unix (ou de um pipe, para
    o ataque), a ponta de quem escreve fechada, e só a outra ponta herdada pelo terminal
    falso → `ward ssh --token-fd <n>`. O conteúdo nunca vai em argv nem no ambiente."""
    if canal == "socket":
        a, b = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
        if conteudo:
            a.sendall(conteudo)
        a.close()
        fd = b.detach()
    else:
        fd, w = os.pipe()
        os.write(w, conteudo)
        os.close(w)
    try:
        return subprocess.run(TERMINAL_FALSO + [WARD, "ssh", "--token-fd", str(fd)] + args,
                              env=ambiente(), pass_fds=(fd,), stdin=subprocess.DEVNULL,
                              capture_output=True, timeout=prazo)
    finally:
        os.close(fd)


class Varredor(threading.Thread):
    """V4.14: lê `/proc/*/cmdline` a cada 50 ms e guarda o que tiver padrão de segredo."""

    def __init__(self):
        super().__init__(daemon=True)
        self.parar = False
        self.achados = 0
        self.vistos = 0

    def run(self):
        while not self.parar:
            for f in glob.glob("/proc/[0-9]*/cmdline"):
                try:
                    c = open(f, "rb").read()
                except OSError:
                    continue
                self.vistos += 1
                if any(p in c for p in padroes):
                    self.achados += 1
            time.sleep(0.05)


def agentes():
    return glob.glob(os.path.join(RT_TESTE, "ssh-*"))


def esperar(cond, prazo):
    fim = time.time() + prazo
    while time.time() < fim:
        if cond():
            return True
        time.sleep(0.05)
    return cond()


def octal(s):
    return "".join("\\%03o" % b for b in s.encode())


def interativo(entrada, passos, prazo=60):
    """`ward ssh <entrada>` num pty: `passos` é uma lista de (esperar, digitar). Devolve
    (código, tela, [(hora, pedaço)])."""
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(WARD, [WARD, "ssh", entrada], ambiente({"TERM": "xterm"}))
    tela = b""
    pedacos = []
    fim = time.time() + prazo
    passos = list(passos)
    inicio = time.time()
    while time.time() < fim:
        if passos and passos[0][0].encode() in tela:
            _, digitar = passos.pop(0)
            if digitar is not None:
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
            pedacos.append((time.time() - inicio, b))
    codigo = None
    if passos:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
    else:
        _, st = os.waitpid(pid, 0)
        codigo = os.waitstatus_to_exitcode(st)
    os.close(fd)
    return codigo, tela, pedacos


servicos = porta_aberta(2222) and porta_aberta(2223)
if not servicos:
    conferir(False, "Fase 27: SSH de teste fora do ar (make dev-servicos-subir); seção ssh pulada")
else:
    varredor = Varredor()
    varredor.start()

    # V4.23 (senha, com o argv do filho em /proc/<pid>/cmdline) e V4.24 (chave).
    vistos = []

    def olhar_filhos():
        fim = time.time() + 20
        while time.time() < fim and not vistos:
            for f in glob.glob("/proc/[0-9]*/cmdline"):
                try:
                    a = open(f, "rb").read().split(b"\0")
                except OSError:
                    continue
                if a and a[0] == b"/usr/bin/sshpass" and b"--" in a:
                    vistos.append(a)
            time.sleep(0.02)

    t = threading.Thread(target=olhar_filhos, daemon=True)
    t.start()
    r = ssh(["acme-ssh-senha-stage", "sleep 1; whoami"])
    t.join()
    conferir(r.returncode == 0 and r.stdout.strip() == b"deploy", f"V4.23 ssh por senha: whoami → deploy ({r.returncode})")
    a = vistos[0] if vistos else []
    o = [a[i + 1] for i in range(len(a) - 1) if a[i] == b"-o"]
    conferir(a[:3] == [b"/usr/bin/sshpass", b"-e", b"/usr/bin/ssh"] and b"PubkeyAuthentication=no" in o
             and b"IdentityAgent=none" in o and a[3:5] == [b"-F", b"/dev/null"],
             "V4.23 o filho é sshpass -e → ssh -F /dev/null com PubkeyAuthentication=no e IdentityAgent=none")
    i = a.index(b"--") if b"--" in a else -1
    conferir(i > 2 and a[i - 2:i] == [b"-l", b"deploy"] and a[i + 1] == b"127.0.0.1"
             and not any(x.startswith(b"User=") or x.endswith(b"@127.0.0.1") for x in a),
             "V4.35 usuário só depois de -l, host depois de --, sem usuário@host nem User=")
    conferir(f"UserKnownHostsFile={DEV}/known_hosts".encode() in o, "D13 known_hosts do Ward")
    sem_variacao(r, "ssh por senha")

    r = ssh(["acme-ssh-chave-prod", "whoami"])
    conferir(r.returncode == 0 and r.stdout.strip() == b"deploy", f"V4.24 ssh por chave: whoami → deploy ({r.returncode})")
    sem_variacao(r, "ssh por chave")
    conferir(not agentes(), "D12 o agente some no fim da sessão")

    r = ssh(["acme-ssh-senha-stage", "exit", "3"])
    conferir(r.returncode == 3, f"o código do comando remoto volta (saiu {r.returncode})")

    # V4.37: sftp com login de verdade (sftpuser, 2222) e comando pela entrada padrão.
    r = ssh(["beta-sftp-dev"], entrada=b"pwd\n")
    conferir(r.returncode == 0 and b"Remote working directory" in r.stdout, f"V4.37 sftp entra no SSH de teste ({r.returncode})")
    sem_variacao(r, "sftp")
    r = ssh(["beta-sftp-dev", "ls"])
    conferir(r.returncode == 125 and b"pedido inv" in r.stderr, "sftp com comando recusado")

    r = ssh(["gama-nota", "id"])
    conferir(r.returncode == 125 and b"tipo sem entrega" in r.stderr, "nota segura: tipo sem entrega")
    r = ssh(["gama-bloqueada", "id"])
    conferir(r.returncode == 125 and b"entrada" in r.stderr, "bloqueada = inexistente")

    # V4.32: token inventado (pelo canal do fd, Fase 27a) numa entrada com aprovação,
    # sem app: vale o nível. A seção 6 repete com o leitor de /proc.
    r = ssh_com_canal(b"AbCdEfGhIjKlMnOpQrStUvWxYz012345", ["acme-pg-aprovacao", "id"])
    # O daemon de teste abre a ponte de teste como app (WARD_DEV_APP), que nega; sem app
    # seria "aprovação indisponível". Os dois são o nível valendo.
    conferir(r.returncode == 125 and (b"pedido negado" in r.stderr or "aprovação indisponível".encode() in r.stderr),
             f"V4.32 token inventado ignorado: vale o nível ({r.stderr[:80]!r})")

    # V4.26: outro processo do usuário acha o socket do agente e tenta listar, adicionar
    # e remover: recusado nas três. O ssh-add roda com HOME falso, sem config.
    p = subprocess.Popen([WARD, "ssh", "acme-ssh-chave-prod", "sleep 8"], env=ambiente(),
                         stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    conferir(esperar(lambda: glob.glob(os.path.join(RT_TESTE, "ssh-*/agent.sock")), 20), "D12 agente da sessão no ar")
    socks = glob.glob(os.path.join(RT_TESTE, "ssh-*/agent.sock"))
    if socks:
        falsa = os.path.join(DEV, "logs", f"chave-qualquer-{os.getpid()}")
        subprocess.run(["/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", falsa],
                       env=ambiente(), check=True, capture_output=True)
        amb = ambiente({"SSH_AUTH_SOCK": socks[0]})
        for rotulo, args in [("ssh-add -L", ["-L"]), ("ssh-add <chave>", [falsa]), ("ssh-add -D", ["-D"])]:
            x = subprocess.run(["/usr/bin/ssh-add"] + args, env=amb, capture_output=True, timeout=20)
            conferir(x.returncode != 0 and b"ssh-ed25519" not in x.stdout, f"V4.26 {rotulo} de outro processo recusado")
        for f in (falsa, falsa + ".pub"):
            os.remove(f)
    p.wait(timeout=60)
    conferir(p.returncode == 0, f"a sessão do agente atacado terminou normal ({p.returncode})")
    conferir(esperar(lambda: not agentes(), 3), "V4.26 socket e pasta do agente somem no fim")

    # V4.26: SIGKILL na CLI derruba a sessão e o agente.
    p = subprocess.Popen([WARD, "ssh", "acme-ssh-chave-prod", "sleep 20"], env=ambiente(),
                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    conferir(esperar(lambda: agentes(), 20), "agente no ar (SIGKILL)")
    p.kill()
    p.wait()
    conferir(esperar(lambda: not agentes(), 3), "V4.26 SIGKILL na CLI: o agente some")

    # V4.26: travar no meio da sessão derruba o agente.
    p = subprocess.Popen([WARD, "ssh", "acme-ssh-chave-prod", "sleep 5"], env=ambiente(),
                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    conferir(esperar(lambda: agentes(), 20), "agente no ar (travar)")
    cod, _, _ = rodar_pty(["travar"])
    conferir(cod == 0 and esperar(lambda: not agentes(), 3), "V4.26 travar o cofre derruba o agente")
    p.wait(timeout=60)
    cod, tela, _ = rodar_pty(["destravar"], digitar=SENHA)
    conferir(cod == 0, "destravar de novo")

    # V4.31 e V4.34 (D34) interativo num pty: prompt, whoami, um prefixo da senha parado
    # (vira •••• em até 1 s, o resto não aparece) e uma saída que diverge (sai normal).
    prefixo, resto = SENHA_SSH[:4], SENHA_SSH[4:]
    diverge = prefixo + "Z"
    cmd = (f"printf 'P:{octal(prefixo)}'; sleep 2; printf '{octal(resto)}\\n'; "
           f"printf 'Q:{octal(diverge)}\\n'; echo FIM$((30+4))\r")
    cod, tela, pedacos = interativo("acme-ssh-senha-stage", [
        ("$ ", "whoami\r"),
        ("deploy\r\n", cmd),
        ("FIM34\r\n", "exit\r"),
    ])
    conferir(cod == 0 and b"deploy\r\n" in tela, f"V4.31 interativo: whoami → deploy e sai 0 ({cod})")
    # Depois do eco da linha digitada (o eco não tem a saída: só octais).
    depois = tela.split(b"FIM$((30+4))", 1)[-1]
    conferir("P:••••".encode() in depois, "V4.34 o prefixo parado vira ••••")
    t_p = next((t for t, b in pedacos if b"P:" in b and b"printf" not in b), None)
    t_m = next((t for t, b in pedacos if "••••".encode() in b), None)
    conferir(t_p is not None and t_m is not None and 0 <= t_m - t_p <= 1.0,
             f"V4.34 a marca chega em até 1 s ({t_p}, {t_m})")
    conferir(prefixo.encode() + b"\r" not in depois and ("P:" + prefixo).encode() not in tela
             and resto.encode() not in tela and SENHA_SSH.encode() not in tela,
             "V4.34 nenhum pedaço da senha em claro")
    conferir(("Q:" + diverge).encode() in tela, "V4.34 a saída que diverge sai normal")
    conferir(sum(1 for x in padroes if x in tela) == 0, "S4 nenhum padrão de .dev/variacoes.txt no pty")

    # V4.28: a chave de host do 2222 mudou (linha falsa no known_hosts do Ward): recusa.
    kh = os.path.join(DEV, "known_hosts")
    original = open(kh, "rb").read() if os.path.exists(kh) else None
    falsa = os.path.join(DEV, "logs", f"host-falso-{os.getpid()}")
    subprocess.run(["/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", falsa],
                   env=ambiente(), check=True, capture_output=True)
    pub = open(falsa + ".pub").read().split()[:2]
    os.remove(falsa)
    os.remove(falsa + ".pub")
    try:
        with open(kh, "w") as f:
            f.write(f"[127.0.0.1]:2222 {pub[0]} {pub[1]}\n")
        r = ssh(["acme-ssh-senha-stage", "whoami"])
        conferir(r.returncode != 0 and b"deploy" not in r.stdout
                 and (b"IDENTIFICATION HAS CHANGED" in r.stderr or b"verification failed" in r.stderr),
                 f"V4.28 chave de host mudada: recusa ({r.returncode})")
    finally:
        if original is None:
            os.remove(kh)
        else:
            with open(kh, "wb") as f:
                f.write(original)

    varredor.parar = True
    varredor.join()
    conferir(varredor.vistos > 0 and varredor.achados == 0,
             f"V4.14 nenhum segredo em /proc/*/cmdline durante o ward ssh ({varredor.vistos} lidos)")

    # V4.35/V4.37: o argv de cada `sshpass`/`ssh`/`sftp` que o daemon cria, lido em
    # /proc/<pid>/cmdline enquanto o processo vive (é o argv do `execve`). O `strace` do
    # contrato não dá: o `wardd` recusa subir sob depurador (TracerPid) e o
    # `ptrace_scope=1` não deixa anexar a quem não é descendente.
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

    def capturar(rodar):
        vistos = {}
        fim = []

        def laco():
            while not fim:
                for f in glob.glob("/proc/[0-9]*/cmdline"):
                    pid = int(f.split("/")[2])
                    if pid in vistos:
                        continue
                    try:
                        a = open(f, "rb").read().split(b"\0")[:-1]
                    except OSError:
                        continue
                    if a and a[0] in PROGS:
                        pai = ppid(pid)
                        vistos[pid] = (a, pai, comm(pai) if pai else "")
                time.sleep(0.005)

        t = threading.Thread(target=laco, daemon=True)
        t.start()
        r = rodar()
        fim.append(1)
        t.join()
        return r, vistos

    def sftp_devagar():
        p = subprocess.Popen([WARD, "ssh", "beta-sftp-dev"], env=ambiente(), stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        time.sleep(2)
        out, _ = p.communicate(b"pwd\n", timeout=60)
        return out

    r1, v1 = capturar(lambda: ssh(["acme-ssh-senha-stage", "sleep 1; whoami"]))
    out2, v2 = capturar(sftp_devagar)
    conferir(r1.stdout.strip() == b"deploy" and b"Remote working directory" in out2, "ssh e sftp com o argv capturado")
    ssh_l = [x for x in v1.values() if x[0][0] == b"/usr/bin/ssh"]
    sp_l = [x for x in v1.values() if x[0][0] == b"/usr/bin/sshpass"]
    conferir(len(ssh_l) == 1 and all(x[0][1:3] == [b"-F", b"/dev/null"] and b"-l" in x[0]
                                     and x[0][x[0].index(b"-l"):x[0].index(b"-l") + 4] == [b"-l", b"deploy", b"--", b"127.0.0.1"]
                                     and not any(y.startswith(b"User=") or b"@" in y for y in x[0]) for x in ssh_l),
             "V4.35 ssh: -F /dev/null, -l deploy -- 127.0.0.1, sem usuário@host nem User=")
    conferir(len(sp_l) == 1 and sp_l[0][2] == "wardd" and sp_l[0][0][:3] == [b"/usr/bin/sshpass", b"-e", b"/usr/bin/ssh"],
             f"V4.35 sem sh -c: o wardd executa o sshpass direto ({[x[2] for x in sp_l]})")
    sftp_l = [x for x in v2.values() if x[0][0] == b"/usr/bin/sftp"]
    conferir(len(sftp_l) == 1 and sftp_l[0][0][1:3] == [b"-F", b"/dev/null"]
             and sftp_l[0][0][-6:] == [b"-o", b"User=sftpuser", b"-P", b"2222", b"--", b"127.0.0.1"]
             and b"-l" not in sftp_l[0][0] and not any(b"@" in y for y in sftp_l[0][0]),
             "V4.37 sftp: -F /dev/null, -o User=sftpuser -P 2222 -- 127.0.0.1, sem -l nem usuário@host")
    sp2 = [x for x in v2.values() if x[0][0] == b"/usr/bin/sshpass"]
    conferir(len(sp2) == 1 and sp2[0][2] == "wardd", "V4.37 sem sh -c: o wardd executa o sshpass do sftp direto")
    tudo = b"\n".join(b"\0".join(x[0]) for x in list(v1.values()) + list(v2.values()))
    conferir(sum(1 for x in padroes if x in tudo) == 0, "S3 nenhum segredo no argv do ssh/sftp")

# --- 5. Salto e túnel (Fase 28) contra os SSH de teste e o Postgres ---------------------
# V4.25 (salto), V4.29 (túnel do banco), V4.30 (nível combinado), V4.35 com salto (sem
# `sh -c` na cadeia: o `ssh` do salto é filho do daemon, nunca do `ssh` principal) e o
# ataque à ponte (um processo de fora da sessão é fechado sem nenhum byte). O cliente
# do Postgres é `dev/testes/pg_select.py` (o host não tem `psql`).
PG_SELECT = os.path.join(RAIZ, "dev/testes/pg_select.py")
if not (servicos and porta_aberta(2224) and porta_aberta(55432)):
    conferir(False, "Fase 28: ssh-salto (2224) ou postgres (55432) fora do ar; seção salto/túnel pulada")
else:
    amb_trace = ambiente({"RUST_LOG": "trace"})
    r, vistos = capturar(lambda: ssh(["acme-ssh-interno", "sleep 1; hostname"], env=amb_trace))
    conferir(r.returncode == 0 and r.stdout.strip() == b"ssh-interno",
             f"V4.25 ward ssh acme-ssh-interno hostname → ssh-interno ({r.returncode} {r.stderr[-200:]!r})")
    sem_variacao(r, "ssh com salto")
    ssh_v = [x for x in vistos.values() if x[0][0] == b"/usr/bin/ssh"]
    principal = [x for x in ssh_v if b"-W" not in x[0]]
    salto = [x for x in ssh_v if b"-W" in x[0]]
    conferir(len(principal) == 1 and len(salto) == 1, f"V4.35 dois ssh: o principal e o do salto ({len(principal)}, {len(salto)})")
    conferir(all(x[2] in ("wardd", "sshpass") for x in ssh_v),
             f"V4.35 sem sh -c nem ProxyJump: todo ssh é filho do wardd ou do sshpass ({[x[2] for x in ssh_v]})")
    conferir(not any(b"ProxyJump" in y or y.startswith(b"ProxyCommand=") and y != b"ProxyCommand=none"
                     for x in ssh_v for y in x[0]), "V4.35 nada de ProxyJump nem ProxyCommand")
    if principal:
        a = principal[0][0]
        o = [a[i + 1] for i in range(len(a) - 1) if a[i] == b"-o"]
        i = a.index(b"--") if b"--" in a else -1
        conferir(a[1:3] == [b"-F", b"/dev/null"] and b"HostKeyAlias=ssh-interno" in o and b"CheckHostIP=no" in o
                 and i > 2 and a[i - 2:i] == [b"-l", b"deploy"] and a[i + 1] == b"127.0.0.1"
                 and not any(y.startswith(b"User=") or y.endswith(b"@127.0.0.1") for y in a),
                 "V4.35 principal: -F /dev/null, HostKeyAlias=ssh-interno, -l deploy -- 127.0.0.1")
    if salto:
        a = salto[0][0]
        i = a.index(b"--") if b"--" in a else -1
        conferir(a[1:3] == [b"-F", b"/dev/null"]
                 and a[i - 6:i + 2] == [b"-W", b"ssh-interno:22", b"-p", b"2224", b"-l", b"salto", b"--", b"127.0.0.1"] and a[a.index(b"-W") + 1] == b"ssh-interno:22"
                 and a[i - 2:i] == [b"-l", b"salto"] and a[i + 1] == b"127.0.0.1"
                 and f"UserKnownHostsFile={DEV}/known_hosts".encode() in a
                 and not any(y.startswith(b"User=") or y.endswith(b"@127.0.0.1") for y in a),
                 "V4.35 salto: -F /dev/null, -W ssh-interno:22, -l salto -- 127.0.0.1, known_hosts do Ward")
    tudo = b"\n".join(b"\0".join(x[0]) for x in vistos.values())
    conferir(sum(1 for x in padroes if x in tudo) == 0, "S3 nenhum segredo no argv do salto")
    conferir(not agentes(), "D12 o agente do salto some no fim")

    # V4.30: livre com salto "com aprovação": vale o mais restritivo.
    r = ssh(["acme-ssh-interno-aprov", "hostname"], env=amb_trace)
    conferir(r.returncode == 125 and b"ssh-interno" not in r.stdout
             and (b"pedido negado" in r.stderr or "aprovação indisponível".encode() in r.stderr),
             f"V4.30 salto com aprovação: vale o nível mais restritivo ({r.stderr[:80]!r})")

    # V4.29: `ward run acme-pg-tunel -- … select 1` pela ponte; PGHOST=127.0.0.1 e a porta
    # local; depois do comando, a porta fecha.
    r = run(["acme-pg-tunel", "--", "/bin/sh", "-c",
             'echo "ponta $PGHOST $PGPORT"; /usr/bin/python3 -I "$1" "select 1"', "sh", PG_SELECT],
            env=amb_trace)
    linhas = r.stdout.decode(errors="replace").split("\n")
    ponta = next((x.split() for x in linhas if x.startswith("ponta ")), [])
    conferir(r.returncode == 0 and "1" in linhas, f"V4.29 select 1 pelo túnel → 1 ({r.returncode} {r.stderr[-200:]!r})")
    conferir(len(ponta) == 3 and ponta[1] == "127.0.0.1" and ponta[2].isdigit() and ponta[2] != "5432",
             f"V4.29 PGHOST=127.0.0.1 com a porta local ({ponta})")
    if len(ponta) == 3 and ponta[2].isdigit():
        conferir(not porta_aberta(int(ponta[2])), "V4.29 o túnel fecha ao fim")
    sem_variacao(r, "run com túnel")
    r = run(["acme-pg-tunel", "--", "/bin/sh", "-c", '/usr/bin/python3 -I "$1" "select current_user"', "sh", PG_SELECT])
    conferir(r.returncode == 0 and r.stdout.strip() == b"app", "V4.29 current_user pelo túnel → app")

    # Ataque: durante o `ward run`, outro processo do usuário (este teste) conecta na
    # ponte: fechado sem nenhum byte; o comando continua usando o túnel.
    p = subprocess.Popen([WARD, "run", "acme-pg-tunel", "--", "/bin/sh", "-c",
                          'echo "$PGPORT"; sleep 3; /usr/bin/python3 -I "$1" "select 2"', "sh", PG_SELECT],
                         env=ambiente(), stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    porta_t = p.stdout.readline().strip()
    recebido = b"?"
    if porta_t.isdigit():
        try:
            s = socket.create_connection(("127.0.0.1", int(porta_t)), timeout=5)
            s.settimeout(5)
            s.sendall(b"\0\0\0\x08\x04\xd2\x16\x2f")
            recebido = s.recv(64)
            s.close()
        except OSError:
            recebido = b""
    out, err = p.communicate(timeout=60)
    conferir(recebido == b"", f"ataque: a ponte do túnel fecha a conexão de fora da sessão sem byte ({recebido!r})")
    conferir(p.returncode == 0 and out.strip() == b"2", f"o comando segue com o túnel depois do ataque ({p.returncode})")

    # V4.33 (S13): o log do daemon (com RUST_LOG=trace) não tem nenhum padrão de
    # .dev/variacoes.txt; o stderr da CLI com RUST_LOG=trace também não (sem_variacao).
    log = os.path.join(DEV, "logs", "wardd.log")
    texto_log = open(log, "rb").read() if os.path.exists(log) else b""
    conferir(b"wardd[debug]" in texto_log, "V4.33 o daemon de teste está com RUST_LOG=trace (linhas [debug] no log)")
    conferir(texto_log and sum(1 for x in padroes if x in texto_log) == 0,
             f"V4.33 nenhum padrão de .dev/variacoes.txt no log do daemon ({len(texto_log)} bytes)")

# --- 6. Fase 27a: o token do "Conectar" só pelo canal do fd -----------------------------
# Não precisa dos SSH de teste: `acme-pg-aprovacao` é `banco` com aprovação, e o daemon
# confere o nível antes do tipo (V4.32). O token é inventado aqui (formato do daemon,
# aleatório a cada rodada, nunca escrito no repositório); o daemon de teste o ignora
# (vale o nível) e registra "token inválido" no log, prova de que ele chegou pelo canal.
import secrets  # noqa: E402
import string  # noqa: E402

LOG_DAEMON = os.path.join(DEV, "logs", "wardd.log")
MSG_TOKEN_INVALIDO = "ssh: token inválido, vencido, já usado ou de outra entrada".encode()


def linhas_token_invalido():
    try:
        return open(LOG_DAEMON, "rb").read().count(MSG_TOKEN_INVALIDO)
    except OSError:
        return -1


class LeitorDeProc(threading.Thread):
    """Lê em laço `/proc/*/cmdline` e `/proc/*/environ` de todo processo e tenta reabrir
    e ler cada `/proc/<pid>/fd/*` dos processos do `ward` e do terminal falso. Conta
    onde o token apareceu."""

    def __init__(self, agulha):
        super().__init__(daemon=True)
        self.agulha = agulha
        self.parar = False
        self.achados = []
        self.varridos = 0
        self.wards = set()
        self.fds_tentados = 0

    def run(self):
        while not self.parar:
            for d in glob.glob("/proc/[0-9]*"):
                for arq in ("cmdline", "environ"):
                    try:
                        c = open(os.path.join(d, arq), "rb").read()
                    except OSError:
                        continue
                    self.varridos += 1
                    if self.agulha in c:
                        self.achados.append(f"{d}/{arq}")
                try:
                    a0 = open(os.path.join(d, "cmdline"), "rb").read().split(b"\0")
                except OSError:
                    continue
                if WARD.encode() in a0 or b"terminal-falso" in a0:
                    self.wards.add(d)
                    for f in glob.glob(os.path.join(d, "fd", "*")):
                        # 0–2 são os pipes deste script (ler o 1 ou o 2 roubaria a
                        # saída da CLI); o canal é ≥ 3.
                        if os.path.basename(f) in ("0", "1", "2"):
                            continue
                        self.fds_tentados += 1
                        try:
                            fd = os.open(f, os.O_RDONLY | os.O_NONBLOCK)
                        except OSError:
                            continue
                        try:
                            if self.agulha in os.read(fd, 4096):
                                self.achados.append(f)
                        except OSError:
                            pass
                        finally:
                            os.close(fd)
            time.sleep(0.005)


alfabeto = string.ascii_letters + string.digits
token = "".join(secrets.choice(alfabeto) for _ in range(32)).encode()
antes = linhas_token_invalido()
leitor = LeitorDeProc(token)
leitor.start()
r = ssh_com_canal(token, ["acme-pg-aprovacao", "id"])
time.sleep(0.2)
leitor.parar = True
leitor.join()
conferir(r.returncode == 125 and (b"pedido negado" in r.stderr or "aprovação indisponível".encode() in r.stderr),
         f"27a ward ssh --token-fd pelo terminal falso: token inventado, vale o nível ({r.returncode}, {r.stderr[:80]!r})")
conferir(linhas_token_invalido() == antes + 1,
         "27a o token chegou ao daemon pelo canal (uma linha 'token inválido' a mais no log)")
conferir(leitor.varridos > 0 and leitor.wards, f"27a o leitor varreu {leitor.varridos} arquivos e viu o ward")
conferir(not leitor.achados, f"27a token em nenhum /proc/*/cmdline, environ ou fd reaberto ({leitor.achados})")
conferir(token not in r.stdout + r.stderr, "27a o token não volta na saída")

# `--token` em argv: recusado (2) sem conectar (nada novo no log do daemon).
antes = linhas_token_invalido()
for args in (["--token", token.decode(), "acme-pg-aprovacao", "id"],
             ["--token=" + token.decode(), "acme-pg-aprovacao", "id"]):
    r = ssh(args)
    conferir(r.returncode == 2 and b"linha de comando" in r.stderr and token not in r.stderr,
             f"27a {args[0][:8]} em argv recusado sem eco ({r.returncode})")
conferir(linhas_token_invalido() == antes, "27a --token em argv não chega ao daemon")

# Canal forjado: lixo, vazio, grande, pipe (reabrível por /proc), fd fechado.
casos = [
    (b"falso-lixo;id\n" * 2, "socket", "fora do formato"),
    (b"", "socket", "veio vazio"),
    (b"A" * 4096, "socket", "passa do tamanho"),
    (token, "pipe", "não é o do app"),
]
for conteudo, canal, msg in casos:
    r = ssh_com_canal(conteudo, ["acme-pg-aprovacao", "id"], canal=canal)
    conferir(r.returncode == 125 and msg.encode() in r.stderr and b"falso" not in r.stderr
             and token not in r.stderr and b"AAAA" not in r.stderr,
             f"27a canal {canal} ({msg}) recusado sem eco ({r.returncode}, {r.stderr[:70]!r})")
r = ssh(["--token-fd", "1000", "acme-pg-aprovacao", "id"])
conferir(r.returncode == 125 and "não está aberto".encode() in r.stderr, f"27a fd fechado recusado ({r.returncode})")
conferir(linhas_token_invalido() == antes, "27a nenhum canal forjado chegou ao daemon")

print(f"\n{len(falhas)} falha(s)")
sys.exit(1 if falhas else 0)
