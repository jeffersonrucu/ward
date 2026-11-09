#!/usr/bin/python3 -IB
"""Monta a extensão do Chrome em `extensao/dist/` (Fase 41) e confere o pacote.

Fontes (nada copiado à mão):
- `extensao/`: manifesto, popup (HTML, CSS, JS) e as funções injetadas na aba;
- `app/src/lib/estilo/tokens.css`: o arquivo único de tokens, gerado das variables do
  Ward.figcraft (Fase 29); a extensão usa o mesmo;
- `app/src/lib/componentes/icones.js` e `ward-simbolo.svg`: ícones e o símbolo do Ward,
  também gerados do FigCraft;
- `app/node_modules/@fontsource/*`: Inter e JetBrains Mono (instaladas por
  `make app-front`, com `npm ci --ignore-scripts`);
- `docs/design/icones/extensao-destravado-*.png`: ícone da extensão (Fase 9).

Conferências (sai 1 na primeira que falhar):
- o id que sai da chave pública do campo `key` é o `ID_EXTENSAO` do `ward-nm`;
- manifesto sem `externally_connectable`, `web_accessible_resources`,
  `host_permissions`, `optional_permissions` e sem a permissão `storage`; só
  `nativeMessaging`, `activeTab` e `scripting`;
- Fase 42: `background` só o service worker `servico.js` (módulo) e `content_scripts`
  só o `envio.js`, no quadro principal (`all_frames: false`), mundo isolado, em
  http/https, sem CSS;
- ouvinte de mensagem (`runtime.onMessage`) só no `servico.js`; `envio.js` sem
  `sendNativeMessage`, `executeScript` nem resposta (`sendResponse`); nenhum
  `onConnect`; nenhum `prefers-color-scheme` (D1: só tema escuro);
- CSP das páginas da extensão sem `unsafe-*`, sem origem remota e com `default-src 'none'`;
- nenhum arquivo com URL remota (só os namespaces XML), `eval`, `new Function`,
  `innerHTML`, `localStorage`/`sessionStorage`/`indexedDB` ou `chrome.storage`;
- HTML sem script nem estilo inline;
- só tipos de arquivo esperados, sem link simbólico.

Uso (da raiz): python3 -I -B dev/extensao/montar.py
"""

import base64
import hashlib
import json
import os
import re
import shutil
import sys
import xml.etree.ElementTree as ET

RAIZ = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
FONTE = os.path.join(RAIZ, "extensao")
DIST = os.path.join(FONTE, "dist")
APP = os.path.join(RAIZ, "app")
FONTSOURCE = os.path.join(APP, "node_modules", "@fontsource")

ARQUIVOS = ["manifest.json", "popup.html", "popup.css", "popup.js", "pagina.js", "envio.js", "servico.js"]
FONTES = [
    ("Inter", 400, "inter", "inter-latin-400-normal.woff2"),
    ("Inter", 500, "inter", "inter-latin-500-normal.woff2"),
    ("Inter", 600, "inter", "inter-latin-600-normal.woff2"),
    ("JetBrains Mono", 400, "jetbrains-mono", "jetbrains-mono-latin-400-normal.woff2"),
]
TAMANHOS_ICONE = (16, 32, 48, 128)
PERMISSOES = {"nativeMessaging", "activeTab", "scripting"}
CHAVES_PROIBIDAS = {
    "externally_connectable",
    "web_accessible_resources",
    "host_permissions",
    "optional_host_permissions",
    "optional_permissions",
    "sandbox",
    "chrome_url_overrides",
    "devtools_page",
    "options_page",
    "options_ui",
    "update_url",
}
NAMESPACES = {"http://www.w3.org/2000/svg"}
PADROES_CONTENT = {"http://*/*", "https://*/*"}
EXTENSOES = {".json", ".html", ".css", ".js", ".woff2", ".png"}
URL = re.compile(rb"https?://[^\s\"'`)<>]*")
PROIBIDOS = [
    rb"\beval\s*\(",
    rb"new\s+Function\b",
    rb"\binnerHTML\b",
    rb"\bouterHTML\b",
    rb"insertAdjacentHTML",
    rb"document\.write",
    rb"localStorage",
    rb"sessionStorage",
    rb"indexedDB",
    rb"chrome\.storage",
    rb"onMessageExternal",
    rb"onConnectExternal",
    rb"addEventListener\(\s*['\"]message['\"]",
    rb"onConnect\b",
    rb"prefers-color-scheme",
    rb"color-scheme:\s*light",
]
# Fase 42: só o service worker escuta mensagens (e confere o `sender`); o content script
# só avisa o envio, sem falar com o host, injetar nada nem esperar resposta.
PROIBIDOS_POR_ARQUIVO = {
    "envio.js": [rb"sendNativeMessage", rb"executeScript", rb"runtime\.onMessage", rb"sendResponse",
                 rb"\.then\s*\(\s*\(?\s*r"],
    "popup.js": [rb"runtime\.onMessage"],
    "pagina.js": [rb"runtime\.onMessage", rb"sendMessage"],
}
BACKGROUND = {"service_worker": "servico.js", "type": "module"}
CONTENT_SCRIPTS = [{
    "matches": ["http://*/*", "https://*/*"],
    "js": ["envio.js"],
    "run_at": "document_start",
    "all_frames": False,
    "match_about_blank": False,
    "world": "ISOLATED",
}]


def falha(msg):
    print(f"montar-extensao: {msg}", file=sys.stderr)
    sys.exit(1)


def chave_publica():
    """A chave pública (base64 do DER) de `extensao/chave-publica.txt`. Ela fica fora do
    manifesto do repositório (o gitleaks lê `"key": <base64>` como segredo) e entra só no
    `dist/manifest.json`; a privada nunca sai de `.dev/extensao/`."""
    with open(os.path.join(FONTE, "chave-publica.txt"), encoding="ascii") as f:
        linhas = f.read().split("\n")
    if linhas[0] != "-----BEGIN PUBLIC KEY-----" or "-----END PUBLIC KEY-----" not in linhas:
        falha("chave-publica.txt não é uma chave pública PEM")
    corpo = "".join(linhas[1:linhas.index("-----END PUBLIC KEY-----")])
    if "PRIVATE" in corpo or not re.fullmatch(r"[A-Za-z0-9+/=]+", corpo):
        falha("chave-publica.txt inválida")
    return corpo


def id_da_chave(chave_b64):
    der = base64.b64decode(chave_b64, validate=True)
    return "".join(chr(ord("a") + int(c, 16)) for c in hashlib.sha256(der).hexdigest()[:32])


def id_do_host():
    with open(os.path.join(RAIZ, "crates", "ward-nm", "src", "lib.rs"), encoding="utf-8") as f:
        m = re.search(r'pub const ID_EXTENSAO: &str = "([a-p]{32})";', f.read())
    if not m:
        falha("ID_EXTENSAO não achado em crates/ward-nm/src/lib.rs")
    return m.group(1)


def conferir_manifesto(m):
    if m.get("manifest_version") != 3:
        falha("manifesto não é MV3")
    proibidas = CHAVES_PROIBIDAS & set(m)
    if proibidas:
        falha(f"manifesto com {sorted(proibidas)}")
    perm = set(m.get("permissions", []))
    if perm != PERMISSOES:
        falha(f"permissões {sorted(perm)} (só {sorted(PERMISSOES)})")
    if m.get("background") != BACKGROUND:
        falha("background só o service worker servico.js (módulo)")
    if m.get("content_scripts") != CONTENT_SCRIPTS:
        falha("content_scripts só o envio.js, quadro principal, mundo isolado, http/https")
    csp = m.get("content_security_policy", {})
    if set(csp) != {"extension_pages"}:
        falha("CSP só de extension_pages")
    pagina = csp["extension_pages"]
    if "unsafe-" in pagina or "http" in pagina or "*" in pagina or "default-src 'none'" not in pagina:
        falha("CSP das páginas da extensão não é estrita")
    if m.get("incognito") != "not_allowed":
        falha("a extensão não pode rodar em janela anônima")
    if "key" in m:
        falha("a `key` vem de extensao/chave-publica.txt, não do manifesto do repositório")
    m["key"] = chave_publica()
    eid = id_da_chave(m["key"])
    if eid != id_do_host():
        falha(f"id da chave do manifesto ({eid}) ≠ ID_EXTENSAO do ward-nm")
    return eid


def fontes_css():
    regras = []
    os.makedirs(os.path.join(DIST, "fontes"))
    for familia, peso, pacote, arquivo in FONTES:
        origem = os.path.join(FONTSOURCE, pacote, "files", arquivo)
        if not os.path.isfile(origem):
            falha(f"fonte {arquivo} não achada: rode 'make app-front'")
        shutil.copyfile(origem, os.path.join(DIST, "fontes", arquivo))
        regras.append(
            "@font-face {\n"
            f"  font-family: '{familia}';\n"
            "  font-style: normal;\n"
            "  font-display: block;\n"
            f"  font-weight: {peso};\n"
            f"  src: url(fontes/{arquivo}) format('woff2');\n"
            "}\n"
        )
    with open(os.path.join(DIST, "fontes.css"), "w", encoding="utf-8") as f:
        f.write("/* GERADO por dev/extensao/montar.py (fontes do @fontsource do app). */\n")
        f.write("".join(regras))


def marca_js():
    svg = ET.parse(os.path.join(APP, "src", "lib", "componentes", "ward-simbolo.svg")).getroot()
    caminhos = [p for p in svg.iter("{http://www.w3.org/2000/svg}path") if p.get("fill")]
    if len(caminhos) != 1:
        falha("ward-simbolo.svg deveria ter um caminho preenchido")
    p = caminhos[0]
    dados = {"caixa": svg.get("viewBox"), "t": p.get("transform", ""), "d": p.get("d")}
    with open(os.path.join(DIST, "marca.js"), "w", encoding="utf-8") as f:
        f.write("// GERADO por dev/extensao/montar.py a partir de app/src/lib/componentes/ward-simbolo.svg.\n")
        f.write(f"export const SIMBOLO = Object.freeze({json.dumps(dados, ensure_ascii=False)});\n")


def conferir_pacote():
    for raiz, _, nomes in os.walk(DIST):
        for n in nomes:
            p = os.path.join(raiz, n)
            rel = os.path.relpath(p, DIST)
            if os.path.islink(p):
                falha(f"{rel}: link simbólico no pacote")
            ext = os.path.splitext(n)[1]
            if ext not in EXTENSOES:
                falha(f"{rel}: tipo de arquivo inesperado")
            if ext in {".woff2", ".png"}:
                continue
            with open(p, "rb") as f:
                dados = f.read()
            # No manifesto, só os padrões do `content_scripts` (conferidos acima).
            aceitas = NAMESPACES | (PADROES_CONTENT if rel == "manifest.json" else set())
            for u in URL.findall(dados):
                if u.decode() not in aceitas:
                    falha(f"{rel}: URL remota {u.decode()!r}")
            if rel in {"icones.js", "marca.js", "tokens.css", "fontes.css", "manifest.json"}:
                continue
            for padrao in PROIBIDOS + PROIBIDOS_POR_ARQUIVO.get(rel, []):
                if re.search(padrao, dados):
                    falha(f"{rel}: contém {padrao.decode()!r}")
            if ext == ".html":
                texto = dados.decode()
                for s in re.findall(r"<script\b[^>]*>(.*?)</script>", texto, re.S):
                    if s.strip():
                        falha(f"{rel}: script inline")
                if re.search(r"<style\b|\sstyle\s*=|\son[a-z]+\s*=", texto):
                    falha(f"{rel}: estilo ou tratador inline")


def main():
    with open(os.path.join(FONTE, "manifest.json"), encoding="utf-8") as f:
        manifesto = json.load(f)
    eid = conferir_manifesto(manifesto)
    if os.path.lexists(DIST):
        shutil.rmtree(DIST)
    os.makedirs(os.path.join(DIST, "icones"))
    for n in ARQUIVOS:
        if n == "manifest.json":
            with open(os.path.join(DIST, n), "w", encoding="utf-8") as f:
                json.dump(manifesto, f, ensure_ascii=False, indent=2)
                f.write("\n")
            continue
        shutil.copyfile(os.path.join(FONTE, n), os.path.join(DIST, n))
    shutil.copyfile(os.path.join(APP, "src", "lib", "estilo", "tokens.css"), os.path.join(DIST, "tokens.css"))
    shutil.copyfile(os.path.join(APP, "src", "lib", "componentes", "icones.js"), os.path.join(DIST, "icones.js"))
    for t in TAMANHOS_ICONE:
        shutil.copyfile(
            os.path.join(RAIZ, "docs", "design", "icones", f"extensao-destravado-{t}.png"),
            os.path.join(DIST, "icones", f"extensao-{t}.png"),
        )
    fontes_css()
    marca_js()
    conferir_pacote()
    print(f"extensão montada em extensao/dist/ (id {eid})")


if __name__ == "__main__":
    main()
