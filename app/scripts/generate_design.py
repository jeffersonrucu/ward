#!/usr/bin/python3 -IB
"""Generates the CSS tokens and the interface icons from the FigCraft export (Phase 29).

Input (versioned in `app/design/`, never edited by hand):
- `figcraft-export.jsonl`: output of `FigCraft.AppImage cli run` with the steps
  `variable.export {"format":"css"}`, `style.list {}` and `variable.list {}`, run on a
  COPY of the design file named in the scope document (the design file is not changed);
- `icons/*.svg`: `export` of each icon component (frame `178`) as SVG with
  `includeBoundingBox` (the 20×20 box of the component).

Output:
- `src/lib/style/tokens.css`: every variable (sizes in px) and the text and effect
  styles as custom properties and classes bound to those variables;
- `src/lib/components/icons.js`: the drawing of each icon (transform + path).

The FigCraft names stay in pt-BR (m2:PRD rule 2: the design file does not change). The
CSS names are the English ones of `TOKEN_NAMES` (docs/glossario.md, section 14; m2:V3.6:
same values, only the name): a variable, style or section with no English name stops
the script instead of leaking a Portuguese name.

Usage (from the repository root): python3 -I -B app/scripts/generate_design.py
"""

import json
import os
import re
import sys
import xml.etree.ElementTree as ET

APP = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
EXPORT = os.path.join(APP, "design", "figcraft-export.jsonl")
ICONS = os.path.join(APP, "design", "icons")
TOKENS_OUT = os.path.join(APP, "src", "lib", "style", "tokens.css")
ICONS_OUT = os.path.join(APP, "src", "lib", "components", "icons.js")

# FigCraft collections whose numbers are screen sizes (they become px). In the
# typography collection only the weights stay unitless.
PX_COLLECTIONS = {"Espaçamento", "Raio", "Largura", "Elevação"}
TYPOGRAPHY = "Tipografia"
TYPE_PX_PREFIXES = ("--tamanho-", "--altura-linha-", "--espacamento-letra-")
WEIGHTS = {"Regular": "var(--weight-regular)", "Medium": "var(--weight-medium)",
           "Semi Bold": "var(--weight-semibold)", "Bold": "var(--weight-bold)"}
FALLBACK_FAMILIES = {"--familia-texto": "system-ui, sans-serif", "--familia-mono": "ui-monospace, monospace"}

# English names of the FigCraft collections and modes, for the section comments and
# the icon tone attribute (`data-icon-tone`).
COLLECTION_NAMES = {
    "Primitivas": "Primitives", "Cores": "Colors", "Tipografia": "Typography",
    "Espaçamento": "Spacing", "Raio": "Radius", "Largura": "Width", "Elevação": "Elevation",
    "Tom do ícone": "Icon tone",
}
MODE_NAMES = {
    "Valor": "Value", "Escuro": "Dark", "padrão": "default", "sobre-cor": "on-color",
    "forte": "strong", "desabilitado": "disabled", "marca": "brand", "perigo": "danger",
    "aviso": "warning", "sucesso": "success",
}

# FigCraft name (pt-BR) -> CSS name (English): custom properties and text style classes.
TOKEN_NAMES = {
    "--primitiva-neutro-0": "--primitive-neutral-0",
    "--primitiva-neutro-50": "--primitive-neutral-50",
    "--primitiva-neutro-400": "--primitive-neutral-400",
    "--primitiva-neutro-600": "--primitive-neutral-600",
    "--primitiva-neutro-700": "--primitive-neutral-700",
    "--primitiva-neutro-800": "--primitive-neutral-800",
    "--primitiva-neutro-850": "--primitive-neutral-850",
    "--primitiva-neutro-900": "--primitive-neutral-900",
    "--primitiva-neutro-950": "--primitive-neutral-950",
    "--primitiva-azul-300": "--primitive-blue-300",
    "--primitiva-azul-400": "--primitive-blue-400",
    "--primitiva-azul-500": "--primitive-blue-500",
    "--primitiva-azul-900": "--primitive-blue-900",
    "--primitiva-vermelho-400": "--primitive-red-400",
    "--primitiva-vermelho-500": "--primitive-red-500",
    "--primitiva-vermelho-600": "--primitive-red-600",
    "--primitiva-vermelho-900": "--primitive-red-900",
    "--primitiva-ambar-400": "--primitive-amber-400",
    "--primitiva-ambar-500": "--primitive-amber-500",
    "--primitiva-ambar-900": "--primitive-amber-900",
    "--primitiva-verde-400": "--primitive-green-400",
    "--primitiva-verde-500": "--primitive-green-500",
    "--primitiva-verde-900": "--primitive-green-900",
    "--primitiva-ciano-400": "--primitive-cyan-400",
    "--primitiva-ciano-500": "--primitive-cyan-500",
    "--primitiva-ciano-900": "--primitive-cyan-900",
    "--primitiva-sobreposicao-escuro": "--primitive-overlay-dark",
    "--primitiva-sombra-preto-24": "--primitive-shadow-black-24",
    "--primitiva-azul-500-40": "--primitive-blue-500-40",
    "--primitiva-vermelho-500-16": "--primitive-red-500-16",
    "--primitiva-vermelho-500-24": "--primitive-red-500-24",
    "--primitiva-azul-600": "--primitive-blue-600",
    "--primitiva-ciano-400-16": "--primitive-cyan-400-16",
    "--primitiva-ambar-400-16": "--primitive-amber-400-16",
    "--primitiva-vermelho-400-16": "--primitive-red-400-16",
    "--primitiva-neutro-400-12": "--primitive-neutral-400-12",
    "--primitiva-violeta-400": "--primitive-violet-400",
    "--primitiva-rosa-400": "--primitive-pink-400",
    "--primitiva-laranja-400": "--primitive-orange-400",
    "--primitiva-verde-400-16": "--primitive-green-400-16",
    "--primitiva-azul-400-32": "--primitive-blue-400-32",
    "--primitiva-ambar-400-32": "--primitive-amber-400-32",
    "--primitiva-vermelho-400-32": "--primitive-red-400-32",
    "--primitiva-verde-400-32": "--primitive-green-400-32",
    "--primitiva-vermelho-300": "--primitive-red-300",
    "--neutro-900-0": "--neutral-900-0",
    "--cor-fundo-padrao": "--color-bg-default",
    "--cor-fundo-sutil": "--color-bg-subtle",
    "--cor-fundo-sobreposicao": "--color-bg-overlay",
    "--cor-superficie-padrao": "--color-surface-default",
    "--cor-superficie-elevada": "--color-surface-raised",
    "--cor-superficie-campo": "--color-surface-field",
    "--cor-superficie-hover": "--color-surface-hover",
    "--cor-superficie-selecionada": "--color-surface-selected",
    "--cor-superficie-desabilitada": "--color-surface-disabled",
    "--cor-texto-primario": "--color-text-primary",
    "--cor-texto-secundario": "--color-text-secondary",
    "--cor-texto-desabilitado": "--color-text-disabled",
    "--cor-texto-inverso": "--color-text-inverse",
    "--cor-texto-marca": "--color-text-brand",
    "--cor-borda-padrao": "--color-border-default",
    "--cor-borda-forte": "--color-border-strong",
    "--cor-borda-foco": "--color-border-focus",
    "--cor-primaria-padrao": "--color-primary-default",
    "--cor-primaria-hover": "--color-primary-hover",
    "--cor-primaria-sutil": "--color-primary-subtle",
    "--cor-primaria-sobre": "--color-primary-on",
    "--cor-perigo-padrao": "--color-danger-default",
    "--cor-perigo-hover": "--color-danger-hover",
    "--cor-perigo-sutil": "--color-danger-subtle",
    "--cor-perigo-texto": "--color-danger-text",
    "--cor-perigo-sobre": "--color-danger-on",
    "--cor-aviso-padrao": "--color-warning-default",
    "--cor-aviso-sutil": "--color-warning-subtle",
    "--cor-aviso-texto": "--color-warning-text",
    "--cor-sucesso-padrao": "--color-success-default",
    "--cor-sucesso-sutil": "--color-success-subtle",
    "--cor-sucesso-texto": "--color-success-text",
    "--cor-ambiente-dev-fundo": "--color-environment-dev-bg",
    "--cor-ambiente-dev-texto": "--color-environment-dev-text",
    "--cor-ambiente-dev-borda": "--color-environment-dev-border",
    "--cor-ambiente-stage-fundo": "--color-environment-stage-bg",
    "--cor-ambiente-stage-texto": "--color-environment-stage-text",
    "--cor-ambiente-stage-borda": "--color-environment-stage-border",
    "--cor-ambiente-prod-fundo": "--color-environment-prod-bg",
    "--cor-ambiente-prod-texto": "--color-environment-prod-text",
    "--cor-ambiente-prod-borda": "--color-environment-prod-border",
    "--cor-ambiente-prod-destaque": "--color-environment-prod-highlight",
    "--cor-ambiente-prod-sobre": "--color-environment-prod-on",
    "--cor-icone-padrao": "--color-icon-default",
    "--cor-icone-forte": "--color-icon-strong",
    "--cor-icone-desabilitado": "--color-icon-disabled",
    "--cor-icone-marca": "--color-icon-brand",
    "--cor-icone-sobre-cor": "--color-icon-on-color",
    "--cor-borda-sutil": "--color-border-subtle",
    "--cor-sombra-suave": "--color-shadow-soft",
    "--cor-primaria-brilho": "--color-primary-glow",
    "--cor-perigo-translucido": "--color-danger-translucent",
    "--cor-perigo-translucido-hover": "--color-danger-translucent-hover",
    "--cor-perigo-borda": "--color-danger-border",
    "--cor-marca-gradiente-inicio": "--color-brand-gradient-start",
    "--cor-marca-gradiente-fim": "--color-brand-gradient-end",
    "--cor-marca-simbolo": "--color-brand-symbol",
    "--cor-marca-sobre-cor": "--color-brand-on-color",
    "--cor-ambiente-dev-tint": "--color-environment-dev-tint",
    "--cor-ambiente-stage-tint": "--color-environment-stage-tint",
    "--cor-ambiente-prod-tint": "--color-environment-prod-tint",
    "--cor-selo-neutro-fundo": "--color-badge-neutral-bg",
    "--cor-grupo-1-cor": "--color-group-1-color",
    "--cor-grupo-2-cor": "--color-group-2-color",
    "--cor-grupo-3-cor": "--color-group-3-color",
    "--cor-grupo-4-cor": "--color-group-4-color",
    "--cor-grupo-5-cor": "--color-group-5-color",
    "--cor-impressao-papel": "--color-print-paper",
    "--cor-impressao-texto": "--color-print-text",
    "--cor-impressao-texto-secundario": "--color-print-text-secondary",
    "--cor-impressao-linha": "--color-print-line",
    "--cor-marca-sobre-cor-inativo": "--color-brand-on-color-inactive",
    "--cor-marca-inativo": "--color-brand-inactive",
    "--cor-sucesso-tint": "--color-success-tint",
    "--cor-info-borda-suave": "--color-info-border-soft",
    "--cor-aviso-borda-suave": "--color-warning-border-soft",
    "--cor-perigo-borda-suave": "--color-danger-border-soft",
    "--cor-sucesso-borda-suave": "--color-success-border-soft",
    "--cor-info-sutil": "--color-info-subtle",
    "--cor-marca-grafite": "--color-brand-graphite",
    "--cor-fundo-sutil-transparente": "--color-bg-subtle-transparent",
    "--familia-texto": "--font-family-text",
    "--familia-mono": "--font-family-mono",
    "--peso-regular": "--weight-regular",
    "--peso-medio": "--weight-medium",
    "--peso-semibold": "--weight-semibold",
    "--peso-negrito": "--weight-bold",
    "--tamanho-xs": "--size-xs",
    "--altura-linha-xs": "--line-height-xs",
    "--tamanho-sm": "--size-sm",
    "--altura-linha-sm": "--line-height-sm",
    "--tamanho-base": "--size-base",
    "--altura-linha-base": "--line-height-base",
    "--tamanho-md": "--size-md",
    "--altura-linha-md": "--line-height-md",
    "--tamanho-lg": "--size-lg",
    "--altura-linha-lg": "--line-height-lg",
    "--tamanho-xl": "--size-xl",
    "--altura-linha-xl": "--line-height-xl",
    "--tamanho-2xl": "--size-2xl",
    "--altura-linha-2xl": "--line-height-2xl",
    "--espacamento-letra-normal": "--letter-spacing-normal",
    "--espacamento-letra-titulo": "--letter-spacing-title",
    "--tamanho-3xl": "--size-3xl",
    "--altura-linha-3xl": "--line-height-3xl",
    "--tamanho-2xs": "--size-2xs",
    "--espaco-0": "--space-0",
    "--espaco-2": "--space-2",
    "--espaco-4": "--space-4",
    "--espaco-6": "--space-6",
    "--espaco-8": "--space-8",
    "--espaco-12": "--space-12",
    "--espaco-16": "--space-16",
    "--espaco-20": "--space-20",
    "--espaco-24": "--space-24",
    "--espaco-32": "--space-32",
    "--espaco-40": "--space-40",
    "--espaco-48": "--space-48",
    "--espaco-64": "--space-64",
    "--controle-sm": "--control-sm",
    "--controle-md": "--control-md",
    "--controle-lg": "--control-lg",
    "--icone-sm": "--icon-sm",
    "--icone-md": "--icon-md",
    "--icone-lg": "--icon-lg",
    "--linha-fina": "--line-thin",
    "--linha-grossa": "--line-thick",
    "--linha-icone": "--line-icon",
    "--espaco-80": "--space-80",
    "--espaco-96": "--space-96",
    "--controle-padrao": "--control-default",
    "--icone-xs": "--icon-xs",
    "--raio-nenhum": "--radius-none",
    "--raio-sm": "--radius-sm",
    "--raio-md": "--radius-md",
    "--raio-lg": "--radius-lg",
    "--raio-xl": "--radius-xl",
    "--raio-2xl": "--radius-2xl",
    "--raio-total": "--radius-full",
    "--raio-3xl": "--radius-3xl",
    "--raio-controle": "--radius-control",
    "--raio-item": "--radius-item",
    "--raio-cartao": "--radius-card",
    "--raio-painel": "--radius-panel",
    "--raio-janela": "--radius-window",
    "--raio-pilula": "--radius-pill",
    "--largura-janela": "--width-window",
    "--largura-barra-lateral": "--width-sidebar",
    "--largura-conteudo": "--width-content",
    "--largura-formulario": "--width-form",
    "--largura-campo": "--width-field",
    "--largura-select-filtro": "--width-filter-select",
    "--largura-dialogo": "--width-dialog",
    "--largura-dialogo-largo": "--width-wide-dialog",
    "--largura-toast": "--width-toast",
    "--largura-popup": "--width-popup",
    "--largura-coluna-tipo": "--width-column-type",
    "--largura-coluna-ambiente": "--width-column-environment",
    "--largura-coluna-nivel": "--width-column-level",
    "--largura-coluna-acesso": "--width-column-access",
    "--largura-coluna-lista": "--width-column-list",
    "--largura-busca": "--width-search",
    "--largura-interruptor": "--width-switch",
    "--pagina-a4-largura": "--page-a4-width",
    "--pagina-a4-altura": "--page-a4-height",
    "--largura-menu": "--width-menu",
    "--largura-select-compacto": "--width-compact-select",
    "--elevacao-nenhum": "--elevation-none",
    "--elevacao-flutuante-contato-y": "--elevation-floating-contact-y",
    "--elevacao-flutuante-contato-desfoque": "--elevation-floating-contact-blur",
    "--elevacao-flutuante-ambiente-y": "--elevation-floating-environment-y",
    "--elevacao-flutuante-ambiente-desfoque": "--elevation-floating-environment-blur",
    "--elevacao-modal-contato-y": "--elevation-modal-contact-y",
    "--elevacao-modal-contato-desfoque": "--elevation-modal-contact-blur",
    "--elevacao-modal-ambiente-y": "--elevation-modal-environment-y",
    "--elevacao-modal-ambiente-desfoque": "--elevation-modal-environment-blur",
    "--elevacao-brilho-y": "--elevation-glow-y",
    "--elevacao-brilho-desfoque": "--elevation-glow-blur",
    "--tom-icone": "--icon-tone",
    "--sombra-brilho-primario": "--shadow-glow-primary",
    "--sombra-flutuante": "--shadow-floating",
    "--sombra-modal": "--shadow-modal",
    ".visualmente-oculto": ".visually-hidden",
    ".mono-corpo": ".mono-body",
    ".mono-pequeno": ".mono-small",
    ".mono-pin": ".mono-pin",
    ".texto-corpo": ".text-body",
    ".texto-corpo-forte": ".text-body-strong",
    ".texto-etiqueta": ".text-tag",
    ".texto-legenda": ".text-caption",
    ".texto-pequeno": ".text-small",
    ".texto-rotulo": ".text-label",
    ".texto-selo": ".text-badge",
    ".texto-selo-grande": ".text-badge-large",
    ".titulo-destaque": ".title-highlight",
    ".titulo-modal": ".title-modal",
    ".titulo-pagina": ".title-page",
    ".titulo-secao": ".title-section",
}

HEADER = (
    "/* GENERATED by app/scripts/generate_design.py from app/design/figcraft-export.jsonl\n"
    " * (variables and styles of Ward.figcraft). Do not edit by hand: run the script again.\n"
    " * Dark theme only (D1): no light mode and no reading of the system theme. */\n"
)


def english(name):
    """The English CSS name of a FigCraft name (a custom property or a text class)."""
    if name not in TOKEN_NAMES:
        sys.exit(f"no English name for {name} (add it to TOKEN_NAMES and docs/glossario.md)")
    return TOKEN_NAMES[name]


def rename_vars(text):
    """Every `--name` in a piece of CSS, renamed to English."""
    return re.sub(r"--[\w-]+", lambda m: english(m.group()), text)


def read_export():
    steps = {}
    with open(EXPORT, encoding="utf-8") as f:
        for line in f:
            item = json.loads(line)
            if not item.get("ok"):
                sys.exit(f"step {item.get('command')} failed in the export")
            steps[item["command"]] = item["result"]
    for name in ("variable.export", "style.list", "variable.list"):
        if name not in steps:
            sys.exit(f"export without the step {name}")
    return steps


def number(v):
    t = f"{v:.3f}".rstrip("0").rstrip(".")
    return "0" if t in ("-0", "") else t


def section_comment(collection, mode):
    if collection not in COLLECTION_NAMES or mode not in MODE_NAMES:
        sys.exit(f"no English name for the section {collection} · {mode}")
    return f"/* {COLLECTION_NAMES[collection]} · {MODE_NAMES[mode]} */"


def tokens(steps):
    css = steps["variable.export"]["text"]
    out = []
    collection = None
    for line in css.splitlines():
        m = re.match(r"/\* (.+?) · (.+?) \*/$", line)
        if m:
            collection = m.group(1)
            out.append(section_comment(collection, m.group(2)))
            continue
        # Icon tone: the export uses the pt-BR attribute and mode of FigCraft; both become
        # ASCII English (`[data-icon-tone="on-color"]`).
        m = re.match(r'\[data-tom-do-ícone="(.+?)"\] \{$', line)
        if m:
            if m.group(1) not in MODE_NAMES:
                sys.exit(f"no English name for the icon tone {m.group(1)}")
            out.append(f'[data-icon-tone="{MODE_NAMES[m.group(1)]}"] {{')
            continue
        m = re.match(r"(\s*)(--[\w-]+): (.+);$", line)
        if not m:
            out.append(line)
            continue
        indent, name, value = m.groups()
        if name.startswith("--estilo-"):
            continue  # FigCraft style name ("Semi Bold"); the CSS uses the weights
        if re.fullmatch(r"-?\d+(\.\d+)?", value):
            in_px = collection in PX_COLLECTIONS or (collection == TYPOGRAPHY and name.startswith(TYPE_PX_PREFIXES))
            if in_px:
                value = f"{number(float(value))}px"
        if name in FALLBACK_FAMILIES:
            value = f"{value}, {FALLBACK_FAMILIES[name]}"
        out.append(f"{indent}{english(name)}: {rename_vars(value)};")
    return "\n".join(out)


def styles(steps):
    collections = {c["id"]: c["name"] for c in steps["variable.list"]["collections"]}
    prefix = {"Cores": "cor-", "Primitivas": "primitiva-"}

    def css_name(v):
        web = (v.get("codeSyntax") or {}).get("WEB")
        if web:
            return rename_vars(web)
        base = prefix.get(collections[v["collection"]], "") + v["name"].replace("/", "-")
        return f"var({english('--' + base)})"

    # Named only when a style uses it: the font style variables (FigCraft style names) have
    # no CSS name.
    by_id = {v["id"]: v for v in steps["variable.list"]["variables"]}

    def var(bound, field, fallback):
        vid = bound.get(field)
        if vid is None:
            sys.exit(f"style with {field} and no bound variable (fallback {fallback})")
        return css_name(by_id[vid])

    props, classes = [], []
    for s in sorted(steps["style.list"], key=lambda s: s["name"]):
        slug = re.sub(r"[^a-z0-9]+", "-", s["name"].lower()
                      .translate(str.maketrans("áâãéêíóôõúç", "aaaeeiooouc"))).strip("-")
        b = s["bound"]
        if s["type"] == "text":
            st = s["style"]
            family = var(b, "fontFamily", st["font_family"])
            weight = WEIGHTS[st["font_style"]]
            classes.append((
                english("." + slug),
                f" {{\n"
                f"  font-family: {family};\n"
                f"  font-weight: {weight};\n"
                f"  font-size: {var(b, 'fontSize', st['font_size'])};\n"
                f"  line-height: {var(b, 'lineHeight', st['line_height'])};\n"
                f"  letter-spacing: {var(b, 'letterSpacing', st['letter_spacing'])};\n"
                f"}}",
            ))
        elif s["type"] == "effect":
            layers = []
            for i, e in enumerate(s["effects"]):
                if e["type"] != "dropShadow" or not e["visible"]:
                    sys.exit(f"unsupported effect in {s['name']}")
                p = f"effects/{i}/"
                layers.append(
                    f"{var(b, p + 'offsetX', 0)} {var(b, p + 'offsetY', 0)} "
                    f"{var(b, p + 'radius', 0)} {var(b, p + 'spread', 0)} {var(b, p + 'color', 0)}"
                )
            props.append(f"  {english('--' + slug)}: {', '.join(layers)};")
    # Same order as before the rename (by the FigCraft name): the cascade does not change.
    return "/* Effect styles */\n:root {\n" + "\n".join(props) + "\n}\n" \
        + "/* Text styles (classes named after the style) */\n" \
        + "\n".join(name + body for name, body in classes)


def icons():
    ns = {"s": "http://www.w3.org/2000/svg"}
    out = {}
    for name in sorted(os.listdir(ICONS)):
        if not name.endswith(".svg"):
            continue
        root = ET.parse(os.path.join(ICONS, name)).getroot()
        if root.get("viewBox") != "0 0 20 20":
            sys.exit(f"{name}: viewBox other than 0 0 20 20")
        paths = []
        for p in root.iterfind(".//s:g/s:path", ns):
            if p.get("fill") != "none" or p.get("stroke-width") != "1.5":
                sys.exit(f"{name}: path outside the pattern (1.5 stroke, no fill)")
            paths.append({"t": p.get("transform", ""), "d": p.get("d")})
        out[name[:-4]] = paths
    return (
        "// GENERATED by app/scripts/generate_design.py from app/design/icons/ (SVG export\n"
        "// of the icon components of Ward.figcraft, frame 178). Do not edit by hand.\n"
        "// Each icon: 20×20 box, line/icon stroke (1.5), round caps and joins.\n"
        f"export const ICONS = Object.freeze({json.dumps(out, ensure_ascii=False, indent=2)});\n"
    )


def main():
    steps = read_export()
    with open(TOKENS_OUT, "w", encoding="utf-8") as f:
        f.write(HEADER + tokens(steps) + "\n" + styles(steps) + "\n")
    with open(ICONS_OUT, "w", encoding="utf-8") as f:
        f.write(icons())
    print("tokens.css and icons.js generated")


if __name__ == "__main__":
    main()
