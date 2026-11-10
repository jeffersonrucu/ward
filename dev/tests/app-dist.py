#!/usr/bin/python3 -IB
"""Confere os pacotes da interface (Fase 29, V5.2, V5.6, S11). Sai 1 no primeiro problema.

`app/dist/` (embutido no Tauri):
- nenhuma URL remota: o único `http(s)://` aceito é o namespace XML do HTML/SVG
  (nome, não endereço; o navegador não busca);
- nada da ponte de teste nem da vitrine (o ramo `import.meta.env.MODE === 'teste'`
  some no build de produção);
- `index.html` sem script ou estilo inline, sem `data:`/`blob:`, sem source map;
- só arquivos de tipos esperados.

`app/dist-teste/` (Chrome com a ponte): o mesmo, mas a ponte tem de estar lá.

Uso: python3 -I -B dev/tests/app-dist.py   (ou `make app-check`)
"""

import os
import re
import sys

RAIZ = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
NAMESPACES = {
    "http://www.w3.org/1999/xhtml",
    "http://www.w3.org/2000/svg",
    "http://www.w3.org/1999/xlink",
    "http://www.w3.org/1998/Math/MathML",
    "http://www.w3.org/2000/xmlns/",
    "http://www.w3.org/XML/1998/namespace",
}
EXTENSOES = {".html", ".js", ".css", ".svg", ".woff", ".woff2"}
SO_NO_TESTE = ("/ponte/", "token=", "ponte-teste", "Vitrine", "ward-teste")
URL = re.compile(rb"https?://[^\s\"'`)<>]*")

erros = []


def ler_tudo(dist):
    if not os.path.isfile(os.path.join(dist, "index.html")):
        erros.append(f"{os.path.relpath(dist, RAIZ)}: sem index.html (rode o build)")
        return {}
    arquivos = {}
    for raiz, _, nomes in os.walk(dist):
        for n in nomes:
            p = os.path.join(raiz, n)
            rel = os.path.relpath(p, dist)
            if os.path.islink(p):
                erros.append(f"{rel}: link simbólico no pacote")
                continue
            if os.path.splitext(n)[1] not in EXTENSOES:
                erros.append(f"{rel}: tipo de arquivo inesperado no pacote")
            with open(p, "rb") as f:
                arquivos[rel] = f.read()
    return arquivos


def conferir(nome, teste):
    dist = os.path.join(RAIZ, "app", nome)
    arquivos = ler_tudo(dist)
    if not arquivos:
        return
    for rel, dados in arquivos.items():
        for url in set(URL.findall(dados)):
            if url.decode("ascii", "replace") not in NAMESPACES:
                erros.append(f"{nome}/{rel}: URL remota {url[:60].decode('ascii', 'replace')}")
        if rel.endswith((".js", ".css", ".html")):
            for proibido in (b"sourceMappingURL", b"data:text", b"data:application", b"data:image", b"blob:"):
                if proibido in dados:
                    erros.append(f"{nome}/{rel}: contém {proibido.decode()}")
        if not teste:
            for marca in SO_NO_TESTE:
                if marca.encode() in dados or marca in rel:
                    erros.append(f"{nome}/{rel}: código de teste no pacote de produção ({marca})")
    html = arquivos["index.html"].decode("utf-8")
    if re.search(r"<script(?![^>]*\bsrc=)[^>]*>", html) or "<style" in html or re.search(r"\son\w+=", html) or "style=" in html:
        erros.append(f"{nome}/index.html: script, estilo ou evento inline")
    if teste and not any("ponte-teste" in rel for rel in arquivos):
        erros.append(f"{nome}: a ponte de teste não está no pacote de teste")


def main():
    conferir("dist", teste=False)
    conferir("dist-teste", teste=True)
    if erros:
        for e in erros:
            print(f"app-dist: {e}", file=sys.stderr)
        sys.exit(1)
    print("app-dist: sem URL remota, sem inline, sem código de teste no pacote de produção")


if __name__ == "__main__":
    main()
