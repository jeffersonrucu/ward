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


def ambiente(extra=None):
    e = {"PATH": "/usr/bin:/bin", "HOME": "/nao-existe", "LANG": "C.UTF-8"}
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
sock = f"/run/user/{UID}/ward-teste/wardd.sock"
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

print(f"\n{len(falhas)} falha(s)")
sys.exit(1 if falhas else 0)
