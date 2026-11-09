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

print(f"\n{len(falhas)} falha(s)")
sys.exit(1 if falhas else 0)
