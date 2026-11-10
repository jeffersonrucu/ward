#!/usr/bin/python3 -I -B
"""V3.3 (S1, scrutiny do M3): matriz operação × entrada × campo pelo socket, como agente.

Uso: socket_matriz.py <socket> <raiz do repositório> <variacoes.txt>
Chamado por `dev/testes/socket-ataques.sh` (`make teste-socket-ataques`), com um
`wardd` de teste só dele, com uma cópia do cofre de teste.

- Destrava com a senha de teste (falsa, D15) e pega a lista do agente.
- Para cada operação da tabela do protocolo (lida de
  `crates/ward-core/src/protocolo.rs`, para não ficar velha), cada entrada do cofre de
  teste (todas as de `dev/fixtures/entradas.json`, inclusive a bloqueada, mais uma
  inexistente) e com e sem campos inventados (`"revelar": true`, `"campo": "senha"`,
  no topo e nos dados): nenhuma resposta tem padrão de `.dev/variacoes.txt`, nenhuma é
  de tipo que carrega credencial, e toda operação só de app ou de extensão responde
  `nao_autorizado`.
- Nunca imprime resposta nem padrão: só operação, entrada e contagens.
"""

import base64
import collections
import json
import re
import socket
import struct
import sys

CAMINHO, RAIZ, VARIACOES = sys.argv[1], sys.argv[2], sys.argv[3]
CREDENCIAL = {"segredo", "token", "codigo_totp", "preenchimento"}
DO_AGENTE = {"TODOS", "AGENTE_E_APP", "SO_AGENTE"}
# Nunca pedidas aqui: mudam o estado do próprio teste (o resto é recusado ou inofensivo).
FORA = {"travar", "destravar"}

falhas = []


def falhou(msg):
    print(f"FALHA {msg}", file=sys.stderr)
    falhas.append(msg)


def b64(x):
    return base64.b64encode(x.encode()).decode()


def ler_exato(s, n):
    d = b""
    while len(d) < n:
        c = s.recv(n - len(d))
        if not c:
            return None
        d += c
    return d


def req(obj):
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(60)
    s.connect(CAMINHO)
    try:
        b = json.dumps(obj).encode()
        s.sendall(struct.pack(">I", len(b)) + b)
        n = ler_exato(s, 4)
        if n is None:
            return b""
        return ler_exato(s, struct.unpack(">I", n)[0]) or b""
    finally:
        s.close()


def operacoes():
    """(nome, papéis) de cada operação, da tabela `Operacao` do protocolo."""
    texto = open(f"{RAIZ}/crates/ward-core/src/protocolo.rs", encoding="utf-8").read()
    bloco = texto.split("Operacao, ", 1)[1].split("tabela!", 1)[0]
    ops = re.findall(r'^\s*\w+ = "([a-z_]+)", ([A-Z_]+),', bloco, re.M)
    if len(ops) < 25:
        print(f"socket_matriz: só {len(ops)} operações lidas da tabela", file=sys.stderr)
        sys.exit(1)
    return ops


variacoes = []
with open(VARIACOES, "rb") as f:
    for linha in f:
        linha = linha.rstrip(b"\n")
        if len(linha) >= 6:
            variacoes.append(linha)
if len(variacoes) < 50:
    print("socket_matriz: lista de variações curta demais; rode 'make dev-variacoes'", file=sys.stderr)
    sys.exit(1)

fixtures = json.load(open(f"{RAIZ}/dev/fixtures/entradas.json", encoding="utf-8"))
# Senha mestra e PIN de teste (D15, falsos), das fixtures.
SENHA_TESTE, PIN_TESTE = fixtures["senha_mestra"], fixtures["pin"]
nomes = sorted({e["nome"] for e in fixtures["entradas"]} | {"inexistente-ward"})

r = json.loads(req({"v": 1, "op": "destravar", "dados": {"senha": b64(SENHA_TESTE)}}))
if r.get("tipo") != "ok":
    print(f"socket_matriz: destravar respondeu {r.get('tipo')}", file=sys.stderr)
    sys.exit(1)
lista = json.loads(req({"v": 1, "op": "lista", "dados": {}}))
vistos = {i["nome"] for i in lista["dados"]["itens"]}
if not vistos or not vistos <= set(nomes):
    falhou("lista do agente fora das fixtures")
if "gama-bloqueada" in vistos:
    falhou("a bloqueada apareceu na lista do agente")
print(f"info  lista do agente: {len(vistos)} entradas; matriz com {len(nomes)}")

contagem = collections.Counter()
total = 0
for op, papeis in operacoes():
    if op in FORA:
        continue
    do_agente = papeis in DO_AGENTE
    for nome in nomes:
        for extra in ({}, {"revelar": True, "campo": "senha"}):
            dados = {
                "entrada": nome,
                "campo": "senha",
                "alvo": {"campo": "senha"},
                "pedido": 1,
                "nivel": "livre",
                "pin": b64(PIN_TESTE),
            }
            dados.update(extra)
            quadros = [
                {"v": 1, "op": op, "dados": dados},
                {"v": 1, "op": op, "revelar": True, "campo": "senha", "dados": dados},
                {"v": 1, "op": op},
            ]
            for q in quadros:
                total += 1
                bruto = req(q)
                for v in variacoes:
                    if v in bruto:
                        falhou(f"segredo na resposta de {op} ({nome})")
                        break
                try:
                    resp = json.loads(bruto)
                    tipo = resp.get("tipo")
                    cod = resp.get("dados", {}).get("codigo") if tipo == "erro" else tipo
                except (ValueError, AttributeError):
                    tipo, cod = None, "fechou" if not bruto else "invalida"
                contagem[(op, cod)] += 1
                if tipo in CREDENCIAL:
                    falhou(f"{op} ({nome}): resposta de credencial ({tipo}) para o agente")
                if not do_agente and cod != "nao_autorizado":
                    falhou(f"{op} ({nome}): só de {papeis}, mas respondeu {cod}")



def sessao(obj):
    """Todos os quadros de uma sessão de `executar` até `fim`, erro ou fechamento."""
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(60)
    s.connect(CAMINHO)
    quadros = []
    try:
        b = json.dumps(obj).encode()
        s.sendall(struct.pack(">I", len(b)) + b)
        while True:
            n = ler_exato(s, 4)
            if n is None:
                break
            corpo = ler_exato(s, struct.unpack(">I", n)[0]) or b""
            quadros.append(corpo)
            tipo = json.loads(corpo).get("tipo")
            if tipo in ("fim", "erro"):
                break
    finally:
        s.close()
    return quadros


# `executar` válido (`env`, que imprime o ambiente com a credencial) com cada entrada:
# a saída chega mascarada, sem padrão da lista; com aprovação, sem app no daemon de
# teste, é "aprovação indisponível"; a bloqueada responde como a inexistente.
respostas_exec = {}
for nome in nomes:
    pedido = {
        "v": 1,
        "op": "executar",
        "dados": {
            "entrada": nome,
            "argv": [b64("env")],
            "cwd": b64("/"),
            "tipo_entrada": "nula",
            "ambiente": [],
        },
    }
    quadros = sessao(pedido)
    total += 1
    junto = b"".join(quadros)
    for v in variacoes:
        if v in junto:
            falhou(f"segredo na saída de executar ({nome})")
            break
    tipos = []
    for q in quadros:
        r = json.loads(q)
        tipos.append(r.get("dados", {}).get("codigo") if r.get("tipo") == "erro" else r.get("tipo"))
        if r.get("tipo") in CREDENCIAL:
            falhou(f"executar ({nome}): resposta de credencial ({r.get('tipo')})")
    respostas_exec[nome] = junto
    contagem[("executar(env)", tipos[-1] if tipos else "fechou")] += 1
if respostas_exec.get("gama-bloqueada") != respostas_exec.get("inexistente-ward"):
    falhou("executar: a bloqueada responde diferente da inexistente")

print(f"info  {total} pedidos")
for (op, cod), n in sorted(contagem.items(), key=lambda x: (x[0][0], str(x[0][1]))):
    print(f"info  {op:24} {cod} × {n}")
if falhas:
    print(f"socket_matriz: {len(falhas)} falha(s)", file=sys.stderr)
    sys.exit(1)
print(f"socket_matriz: V3.3 ok ({total} pedidos, nenhum segredo, app/extensão não autorizado)")
