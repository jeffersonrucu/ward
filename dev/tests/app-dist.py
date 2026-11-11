#!/usr/bin/python3 -IB
"""Checks the interface bundles (Phase 29, V5.2, V5.6, S11). Exits 1 on the first problem.

`app/dist/` (embedded in Tauri):
- no remote URL: the only `http(s)://` accepted is the XML namespace of HTML/SVG
  (a name, not an address; the browser does not fetch it);
- nothing from the test bridge nor the showcase (the branch on the test build mode of
  `import.meta.env.MODE` disappears in the production build);
- `index.html` without inline script or style, without `data:`/`blob:`, without source map;
- only files of the expected types.

The test bundle next to it (Chrome with the bridge): the same, but the bridge has to be there.

Usage: python3 -I -B dev/tests/app-dist.py   (or `make app-check`)
"""

import os
import re
import sys

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
NAMESPACES = {
    "http://www.w3.org/1999/xhtml",
    "http://www.w3.org/2000/svg",
    "http://www.w3.org/1999/xlink",
    "http://www.w3.org/1998/Math/MathML",
    "http://www.w3.org/2000/xmlns/",
    "http://www.w3.org/XML/1998/namespace",
}
EXTENSIONS = {".html", ".js", ".css", ".svg", ".woff", ".woff2"}
# m2:Phase 56: the bridge path and the test socket dir in the new form and the 0.1.x one.
TEST_ONLY = ("/test-bridge/", "/ponte/", "token=", "ponte-teste", "test-bridge", "Vitrine", "Showcase",
             "ward-test", "ward-teste")
URL = re.compile(rb"https?://[^\s\"'`)<>]*")

errors = []


def read_all(dist):
    if not os.path.isfile(os.path.join(dist, "index.html")):
        errors.append(f"{os.path.relpath(dist, ROOT)}: sem index.html (rode o build)")
        return {}
    files = {}
    for dirpath, _, names in os.walk(dist):
        for n in names:
            p = os.path.join(dirpath, n)
            rel = os.path.relpath(p, dist)
            if os.path.islink(p):
                errors.append(f"{rel}: link simbólico no pacote")
                continue
            if os.path.splitext(n)[1] not in EXTENSIONS:
                errors.append(f"{rel}: tipo de arquivo inesperado no pacote")
            with open(p, "rb") as f:
                files[rel] = f.read()
    return files


def check(name, test):
    dist = os.path.join(ROOT, "app", name)
    files = read_all(dist)
    if not files:
        return
    for rel, data in files.items():
        for url in set(URL.findall(data)):
            if url.decode("ascii", "replace") not in NAMESPACES:
                errors.append(f"{name}/{rel}: URL remota {url[:60].decode('ascii', 'replace')}")
        if rel.endswith((".js", ".css", ".html")):
            for forbidden in (b"sourceMappingURL", b"data:text", b"data:application", b"data:image", b"blob:"):
                if forbidden in data:
                    errors.append(f"{name}/{rel}: contém {forbidden.decode()}")
        if not test:
            for mark in TEST_ONLY:
                if mark.encode() in data or mark in rel:
                    errors.append(f"{name}/{rel}: código de teste no pacote de produção ({mark})")
    html = files["index.html"].decode("utf-8")
    if re.search(r"<script(?![^>]*\bsrc=)[^>]*>", html) or "<style" in html or re.search(r"\son\w+=", html) or "style=" in html:
        errors.append(f"{name}/index.html: script, estilo ou evento inline")
    if test and not any("test-bridge" in rel for rel in files):
        errors.append(f"{name}: a ponte de teste não está no pacote de teste")


def main():
    check("dist", test=False)
    check("dist-teste", test=True)
    if errors:
        for e in errors:
            print(f"app-dist: {e}", file=sys.stderr)
        sys.exit(1)
    print("app-dist: sem URL remota, sem inline, sem código de teste no pacote de produção")


if __name__ == "__main__":
    main()
