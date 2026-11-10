#!/usr/bin/python3 -IB
"""Fase 39g (D51, V5.29): nenhum código fica fora do isolamento RTL e do escape bidi.

A janela de aprovação mostra o texto do agente em duas camadas:
- o Rust (`crates/ward-core/src/exibicao.rs`, `invisivel`) troca por escape visível o
  controle e a formatação (bidi, largura zero...);
- a interface (`app/src/lib/bidi.js`, `RTL`) isola em `<bdi dir="rtl">` cada sequência
  de letras RTL.

Este teste lê as duas listas dos fontes e varre U+0000–U+10FFFF com a ICU do sistema
(a mesma do WebKitGTK, por ctypes; nada é compilado). Um código de classe bidi forte
RTL (R, AL), número árabe (AN), separador de parágrafo (B) ou controle de direção
(LRE, RLE, LRO, RLO, PDF, LRI, RLI, FSI, PDI) que não cai em nenhuma das duas é falha.
Os não atribuídos contam: a ICU dá a eles a classe padrão do bloco (U+FEFD e U+FEFE,
AL, foram o achado da 2ª auditoria do M5).

Antes, o autoteste: com a faixa antiga (U+FE70–U+FEFC) e sem o escape de U+FEFD/U+FEFE,
a varredura tem de achar os dois. Sai 1 se algo falhar; 2 se não achar a ICU.

Uso: python3 -I -B dev/tests/bidi_icu.py   (ou `make app-front`)
"""

import ctypes
import glob
import os
import re
import sys

RAIZ = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
BIDI_JS = os.path.join(RAIZ, "app", "src", "lib", "bidi.js")
EXIBICAO = os.path.join(RAIZ, "crates", "ward-core", "src", "exibicao.rs")

# UCharDirection (uchar.h): as classes que reordenam o texto em volta.
# 1 R, 2 EN... ver a enum; 7 B, 11 LRE, 12 LRO, 13 AL, 14 RLE, 15 RLO, 16 PDF,
# 19 FSI, 20 LRI, 21 RLI, 22 PDI; 5 AN.
NOMES = {
    1: "R",
    5: "AN",
    7: "B",
    11: "LRE",
    12: "LRO",
    13: "AL",
    14: "RLE",
    15: "RLO",
    16: "PDF",
    19: "FSI",
    20: "LRI",
    21: "RLI",
    22: "PDI",
}
U_CONTROL_CHAR = 15  # UCharCategory Cc (o `char::is_control` do Rust)


def carregar_icu():
    for caminho in sorted(glob.glob("/usr/lib/x86_64-linux-gnu/libicuuc.so.*")) + sorted(
        glob.glob("/usr/lib64/libicuuc.so.*")
    ):
        m = re.search(r"libicuuc\.so\.(\d+)$", caminho)
        if not m:
            continue
        lib = ctypes.CDLL(caminho)
        v = m.group(1)
        direcao = getattr(lib, f"u_charDirection_{v}")
        tipo = getattr(lib, f"u_charType_{v}")
        for f in (direcao, tipo):
            f.argtypes = [ctypes.c_int32]
            f.restype = ctypes.c_int
        return direcao, tipo, caminho
    return None


def faixas_js():
    """As faixas da classe de caracteres `RTL` de bidi.js."""
    texto = open(BIDI_JS, encoding="utf-8").read()
    m = re.search(r"const RTL = /\[(.*?)\]\+/gu;", texto)
    if not m:
        raise SystemExit("bidi_icu: não achei `const RTL = /[...]+/gu;` em bidi.js")
    corpo = m.group(1)
    itens = []
    i = 0
    while i < len(corpo):
        if corpo.startswith("\\u{", i):
            fim = corpo.index("}", i)
            itens.append(int(corpo[i + 3 : fim], 16))
            i = fim + 1
        elif corpo[i] == "-":
            itens.append("-")
            i += 1
        elif corpo[i] == "\\":
            raise SystemExit(f"bidi_icu: escape não suportado em bidi.js: {corpo[i:i+6]!r}")
        else:
            itens.append(ord(corpo[i]))
            i += 1
    faixas = []
    j = 0
    while j < len(itens):
        if j + 2 < len(itens) and itens[j + 1] == "-":
            faixas.append((itens[j], itens[j + 2]))
            j += 3
        else:
            faixas.append((itens[j], itens[j]))
            j += 1
    return faixas


def faixas_rust():
    """As faixas do `matches!` de `invisivel` em exibicao.rs."""
    texto = open(EXIBICAO, encoding="utf-8").read()
    m = re.search(r"pub fn invisivel\(c: char\) -> bool \{(.*?)\n\}", texto, re.S)
    if not m or "c.is_control()" not in m.group(1):
        raise SystemExit("bidi_icu: não achei `invisivel` com `c.is_control()` em exibicao.rs")
    corpo = re.sub(r"//[^\n]*", "", m.group(1))
    faixas = []
    for a, b in re.findall(r"0x([0-9A-Fa-f]+)(?:\s*\.\.=\s*0x([0-9A-Fa-f]+))?", corpo):
        faixas.append((int(a, 16), int(b or a, 16)))
    return faixas


def dentro(c, faixas):
    return any(a <= c <= b for a, b in faixas)


def varrer(direcao, tipo, js, rust):
    achados = []
    for c in range(0x110000):
        d = direcao(c)
        if d not in NOMES:
            continue
        if dentro(c, js) or dentro(c, rust) or tipo(c) == U_CONTROL_CHAR:
            continue
        achados.append((c, NOMES[d]))
    return achados


def main():
    icu = carregar_icu()
    if icu is None:
        print("bidi_icu: libicuuc não encontrada", file=sys.stderr)
        return 2
    direcao, tipo, caminho = icu
    js = faixas_js()
    rust = faixas_rust()
    if not js or not rust:
        print("bidi_icu: faixas vazias", file=sys.stderr)
        return 1

    # Autoteste: o estado antes da Fase 39g (faixa até U+FEFC, sem escape de FEFD/FEFE).
    js_velho = [(a, 0xFEFC if (a, b) == (0xFE70, 0xFEFE) else b) for a, b in js]
    rust_velho = [f for f in rust if f not in ((0xFEFD, 0xFEFE), (0xFEFD, 0xFEFD), (0xFEFE, 0xFEFE))]
    velhos = {c for c, _ in varrer(direcao, tipo, js_velho, rust_velho)}
    if not {0xFEFD, 0xFEFE} <= velhos:
        print("FALHA autoteste: a varredura não achou U+FEFD/U+FEFE no estado antigo", file=sys.stderr)
        return 1

    achados = varrer(direcao, tipo, js, rust)
    for c, nome in achados[:50]:
        print(f"FALHA U+{c:04X} ({nome}) sem isolamento nem escape", file=sys.stderr)
    if achados:
        print(f"bidi_icu: {len(achados)} código(s) descoberto(s)", file=sys.stderr)
        return 1
    print(f"bidi_icu: ok ({os.path.basename(caminho)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
