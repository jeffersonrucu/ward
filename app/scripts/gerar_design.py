#!/usr/bin/python3 -IB
"""Gera os tokens CSS e os ícones da interface a partir do que saiu do FigCraft (Fase 29).

Entrada (versionada em `app/design/`, nunca editada à mão):
- `figcraft-export.jsonl`: saída de `FigCraft.AppImage cli run` com os passos
  `variable.export {"format":"css"}`, `style.list {}` e `variable.list {}`, rodados
  numa CÓPIA de `~/documentos/Ward.figcraft` (o arquivo de design não é alterado);
- `icones/*.svg`: `export` de cada `Ícone/*` (frame `178`) em SVG com
  `includeBoundingBox` (caixa 20×20 do componente).

Saída:
- `src/lib/estilo/tokens.css`: todas as variables (medidas em px) e os estilos de texto e
  de efeito como propriedades e classes ligadas a essas variables;
- `src/lib/componentes/icones.js`: o desenho de cada ícone (transform + caminho).

Uso (da raiz do repositório): python3 -I -B app/scripts/gerar_design.py
"""

import json
import os
import re
import sys
import xml.etree.ElementTree as ET

APP = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
EXPORT = os.path.join(APP, "design", "figcraft-export.jsonl")
ICONES = os.path.join(APP, "design", "icones")
SAIDA_TOKENS = os.path.join(APP, "src", "lib", "estilo", "tokens.css")
SAIDA_ICONES = os.path.join(APP, "src", "lib", "componentes", "icones.js")

# Coleções cujos números são medidas de tela (viram px). Em `Tipografia` só os pesos ficam
# sem unidade.
COLECOES_EM_PX = {"Espaçamento", "Raio", "Largura", "Elevação"}
PREFIXOS_TIPO_EM_PX = ("--tamanho-", "--altura-linha-", "--espacamento-letra-")
PESOS = {"Regular": "var(--peso-regular)", "Medium": "var(--peso-medio)",
         "Semi Bold": "var(--peso-semibold)", "Bold": "var(--peso-negrito)"}
FAMILIAS_RESERVA = {"--familia-texto": "system-ui, sans-serif", "--familia-mono": "ui-monospace, monospace"}

CABECALHO = (
    "/* GERADO por app/scripts/gerar_design.py a partir de app/design/figcraft-export.jsonl\n"
    " * (variables e estilos do Ward.figcraft). Não edite à mão: rode o script de novo.\n"
    " * Só tema escuro (D1): não há modo claro nem leitura do tema do sistema. */\n"
)


def ler_export():
    passos = {}
    with open(EXPORT, encoding="utf-8") as f:
        for linha in f:
            item = json.loads(linha)
            if not item.get("ok"):
                sys.exit(f"passo {item.get('command')} falhou no export")
            passos[item["command"]] = item["result"]
    for nome in ("variable.export", "style.list", "variable.list"):
        if nome not in passos:
            sys.exit(f"export sem o passo {nome}")
    return passos


def numero(v):
    t = f"{v:.3f}".rstrip("0").rstrip(".")
    return "0" if t in ("-0", "") else t


def tokens(passos):
    css = passos["variable.export"]["text"]
    saida = []
    colecao = None
    for linha in css.splitlines():
        m = re.match(r"/\* (.+?) · (.+?) \*/", linha)
        if m:
            colecao = m.group(1)
            saida.append(linha)
            continue
        # Tom do ícone: o atributo vira ASCII (o export usa `data-tom-do-ícone`).
        linha = linha.replace("[data-tom-do-ícone=", "[data-tom-icone=")
        m = re.match(r"(\s*)(--[\w-]+): (.+);$", linha)
        if not m:
            saida.append(linha)
            continue
        recuo, nome, valor = m.groups()
        if nome.startswith("--estilo-"):
            continue  # nome de estilo do FigCraft ("Semi Bold"); o CSS usa os pesos
        if re.fullmatch(r"-?\d+(\.\d+)?", valor):
            em_px = colecao in COLECOES_EM_PX or (colecao == "Tipografia" and nome.startswith(PREFIXOS_TIPO_EM_PX))
            if em_px:
                valor = f"{numero(float(valor))}px"
        if nome in FAMILIAS_RESERVA:
            valor = f"{valor}, {FAMILIAS_RESERVA[nome]}"
        saida.append(f"{recuo}{nome}: {valor};")
    return "\n".join(saida)


def estilos(passos):
    colecoes = {c["id"]: c["name"] for c in passos["variable.list"]["collections"]}
    prefixo = {"Cores": "cor-", "Primitivas": "primitiva-"}

    def nome_css(v):
        web = (v.get("codeSyntax") or {}).get("WEB")
        if web:
            return web
        base = prefixo.get(colecoes[v["collection"]], "") + v["name"].replace("/", "-")
        return f"var(--{base})"

    por_id = {v["id"]: nome_css(v) for v in passos["variable.list"]["variables"]}

    def var(bound, campo, reserva):
        vid = bound.get(campo)
        if vid is None:
            sys.exit(f"estilo com {campo} sem variable ligada (reserva {reserva})")
        return por_id[vid]

    props, classes = [], []
    for s in sorted(passos["style.list"], key=lambda s: s["name"]):
        slug = re.sub(r"[^a-z0-9]+", "-", s["name"].lower()
                      .translate(str.maketrans("áâãéêíóôõúç", "aaaeeiooouc"))).strip("-")
        b = s["bound"]
        if s["type"] == "text":
            st = s["style"]
            familia = var(b, "fontFamily", st["font_family"])
            peso = PESOS[st["font_style"]]
            classes.append(
                f".{slug} {{\n"
                f"  font-family: {familia};\n"
                f"  font-weight: {peso};\n"
                f"  font-size: {var(b, 'fontSize', st['font_size'])};\n"
                f"  line-height: {var(b, 'lineHeight', st['line_height'])};\n"
                f"  letter-spacing: {var(b, 'letterSpacing', st['letter_spacing'])};\n"
                f"}}"
            )
        elif s["type"] == "effect":
            camadas = []
            for i, e in enumerate(s["effects"]):
                if e["type"] != "dropShadow" or not e["visible"]:
                    sys.exit(f"efeito não suportado em {s['name']}")
                p = f"effects/{i}/"
                camadas.append(
                    f"{var(b, p + 'offsetX', 0)} {var(b, p + 'offsetY', 0)} "
                    f"{var(b, p + 'radius', 0)} {var(b, p + 'spread', 0)} {var(b, p + 'color', 0)}"
                )
            props.append(f"  --{slug}: {', '.join(camadas)};")
    return "/* Estilos de efeito */\n:root {\n" + "\n".join(props) + "\n}\n" \
        + "/* Estilos de texto (classes com o nome do estilo) */\n" + "\n".join(classes)


def icones():
    ns = {"s": "http://www.w3.org/2000/svg"}
    saida = {}
    for arq in sorted(os.listdir(ICONES)):
        if not arq.endswith(".svg"):
            continue
        raiz = ET.parse(os.path.join(ICONES, arq)).getroot()
        if raiz.get("viewBox") != "0 0 20 20":
            sys.exit(f"{arq}: viewBox diferente de 0 0 20 20")
        caminhos = []
        for p in raiz.iterfind(".//s:g/s:path", ns):
            if p.get("fill") != "none" or p.get("stroke-width") != "1.5":
                sys.exit(f"{arq}: caminho fora do padrão (traço 1,5 sem preenchimento)")
            caminhos.append({"t": p.get("transform", ""), "d": p.get("d")})
        saida[arq[:-4]] = caminhos
    return (
        "// GERADO por app/scripts/gerar_design.py a partir de app/design/icones/ (export\n"
        "// SVG dos componentes Ícone/* do Ward.figcraft, frame 178). Não edite à mão.\n"
        "// Cada ícone: caixa 20×20, traço linha/icone (1,5), pontas e cantos arredondados.\n"
        f"export const ICONES = Object.freeze({json.dumps(saida, ensure_ascii=False, indent=2)});\n"
    )


def main():
    passos = ler_export()
    with open(SAIDA_TOKENS, "w", encoding="utf-8") as f:
        f.write(CABECALHO + tokens(passos) + "\n" + estilos(passos) + "\n")
    with open(SAIDA_ICONES, "w", encoding="utf-8") as f:
        f.write(icones())
    print("tokens.css e icones.js gerados")


if __name__ == "__main__":
    main()
