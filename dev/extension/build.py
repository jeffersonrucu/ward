#!/usr/bin/python3 -IB
"""Builds the Chrome extension in `extension/dist/` (Phase 41) and checks the package.

Sources (nothing copied by hand):
- `extension/`: manifest, popup (HTML, CSS, JS) and the functions injected into the tab;
- `app/src/lib/style/tokens.css`: the single tokens file, generated from the
  Ward.figcraft variables (Phase 29); the extension uses the same one;
- `app/src/lib/components/icons.js` and `ward-symbol.svg`: icons and the Ward symbol,
  also generated from FigCraft;
- `app/node_modules/@fontsource/*`: Inter and JetBrains Mono (installed by
  `make app-front`, with `npm ci --ignore-scripts`);
- the unlocked extension icon PNGs in `docs/design/` (Phase 9).

Checks (exits 1 on the first one that fails):
- the id derived from the public key of the `key` field is the extension id constant of
  `ward-nm`;
- manifest without `externally_connectable`, `web_accessible_resources`,
  `optional_host_permissions`, `optional_permissions` and without the `storage`
  permission; only `nativeMessaging`, `activeTab` and `scripting`;
- Phase 43: `host_permissions` exactly the origins D9 accepts (`https://*/*`,
  `http://localhost/*` and `http://127.0.0.1/*`): `ward login` injects the probe and the
  fill into the active tab without the user's click (without `activeTab`);
- Phase 42: `background` only the service worker `service.js` (module) and
  `content_scripts` only `submit.js`, in the main frame (`all_frames: false`), isolated
  world, on http/https, without CSS;
- message listener (`runtime.onMessage`) only in `service.js`; `submit.js` without
  `sendNativeMessage`, `executeScript` or a reply (`sendResponse`); no `onConnect`; no
  `prefers-color-scheme` (D1: dark theme only);
- CSP of the extension pages without `unsafe-*`, without a remote origin and with
  `default-src 'none'`;
- no file with a remote URL (only the XML namespaces), `eval`, `new Function`,
  `innerHTML`, `localStorage`/`sessionStorage`/`indexedDB` or `chrome.storage`;
- HTML without inline script or style;
- only expected file types, no symbolic link.

Usage (from the root): python3 -I -B dev/extension/build.py
"""

import base64
import hashlib
import json
import os
import re
import shutil
import sys
import xml.etree.ElementTree as ET

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
SOURCE = os.path.join(ROOT, "extension")
DIST = os.path.join(SOURCE, "dist")
APP = os.path.join(ROOT, "app")
FONTSOURCE = os.path.join(APP, "node_modules", "@fontsource")

FILES = ["manifest.json", "popup.html", "popup.css", "popup.js", "page.js", "submit.js", "service.js",
         "login.js"]
FONTS = [
    ("Inter", 400, "inter", "inter-latin-400-normal.woff2"),
    ("Inter", 500, "inter", "inter-latin-500-normal.woff2"),
    ("Inter", 600, "inter", "inter-latin-600-normal.woff2"),
    ("JetBrains Mono", 400, "jetbrains-mono", "jetbrains-mono-latin-400-normal.woff2"),
]
ICON_SIZES = (16, 32, 48, 128)
PERMISSIONS = {"nativeMessaging", "activeTab", "scripting"}
FORBIDDEN_KEYS = {
    "externally_connectable",
    "web_accessible_resources",
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
CONTENT_PATTERNS = {"http://*/*", "https://*/*"}
# Phase 43 (D9): `https` on any host; `http` only on localhost and 127.0.0.1.
HOST_PERMISSIONS = ["https://*/*", "http://localhost/*", "http://127.0.0.1/*"]
EXTENSIONS = {".json", ".html", ".css", ".js", ".woff2", ".png"}
URL = re.compile(rb"https?://[^\s\"'`)<>]*")
FORBIDDEN = [
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
# Phase 42: only the service worker listens to messages (and checks the `sender`); the
# content script only reports the submit, without talking to the host, injecting
# anything or waiting for a reply.
FORBIDDEN_PER_FILE = {
    "submit.js": [rb"sendNativeMessage", rb"executeScript", rb"runtime\.onMessage", rb"sendResponse",
                  rb"\.then\s*\(\s*\(?\s*r"],
    "popup.js": [rb"runtime\.onMessage"],
    "page.js": [rb"runtime\.onMessage", rb"sendMessage"],
    # Phase 43: `ward login` only talks to `ward-nm` over its own port; it neither listens
    # to nor sends messages to the page, content script or popup.
    "login.js": [rb"runtime\.onMessage", rb"sendMessage", rb"sendNativeMessage", rb"console\."],
}
BACKGROUND = {"service_worker": "service.js", "type": "module"}
CONTENT_SCRIPTS = [{
    "matches": ["http://*/*", "https://*/*"],
    "js": ["submit.js"],
    "run_at": "document_start",
    "all_frames": False,
    "match_about_blank": False,
    "world": "ISOLATED",
}]


def fail(msg):
    print(f"extension-build: {msg}", file=sys.stderr)
    sys.exit(1)


def public_key():
    """The public key (base64 of the DER) from `extension/public-key.txt`. It stays out
    of the repository manifest (gitleaks reads `"key": <base64>` as a secret) and goes
    only into `dist/manifest.json`; the private key never leaves `.dev/extension/`."""
    with open(os.path.join(SOURCE, "public-key.txt"), encoding="ascii") as f:
        lines = f.read().split("\n")
    if lines[0] != "-----BEGIN PUBLIC KEY-----" or "-----END PUBLIC KEY-----" not in lines:
        fail("public-key.txt não é uma chave pública PEM")
    body = "".join(lines[1:lines.index("-----END PUBLIC KEY-----")])
    if "PRIVATE" in body or not re.fullmatch(r"[A-Za-z0-9+/=]+", body):
        fail("public-key.txt inválida")
    return body


def key_id(key_b64):
    der = base64.b64decode(key_b64, validate=True)
    return "".join(chr(ord("a") + int(c, 16)) for c in hashlib.sha256(der).hexdigest()[:32])


def host_id():
    with open(os.path.join(ROOT, "crates", "ward-nm", "src", "lib.rs"), encoding="utf-8") as f:
        m = re.search(r'pub const ID_EXTENSAO: &str = "([a-p]{32})";', f.read())
    if not m:
        fail("ID_EXTENSAO não achado em crates/ward-nm/src/lib.rs")
    return m.group(1)


def check_manifest(m):
    if m.get("manifest_version") != 3:
        fail("manifesto não é MV3")
    forbidden = FORBIDDEN_KEYS & set(m)
    if forbidden:
        fail(f"manifesto com {sorted(forbidden)}")
    perm = set(m.get("permissions", []))
    if perm != PERMISSIONS:
        fail(f"permissões {sorted(perm)} (só {sorted(PERMISSIONS)})")
    if m.get("host_permissions") != HOST_PERMISSIONS:
        fail(f"host_permissions só {HOST_PERMISSIONS} (D9)")
    if m.get("background") != BACKGROUND:
        fail("background só o service worker service.js (módulo)")
    if m.get("content_scripts") != CONTENT_SCRIPTS:
        fail("content_scripts só o submit.js, quadro principal, mundo isolado, http/https")
    csp = m.get("content_security_policy", {})
    if set(csp) != {"extension_pages"}:
        fail("CSP só de extension_pages")
    page = csp["extension_pages"]
    if "unsafe-" in page or "http" in page or "*" in page or "default-src 'none'" not in page:
        fail("CSP das páginas da extensão não é estrita")
    if m.get("incognito") != "not_allowed":
        fail("a extensão não pode rodar em janela anônima")
    if "key" in m:
        fail("a `key` vem de extension/public-key.txt, não do manifesto do repositório")
    m["key"] = public_key()
    eid = key_id(m["key"])
    if eid != host_id():
        fail(f"id da chave do manifesto ({eid}) ≠ ID_EXTENSAO do ward-nm")
    return eid


def fonts_css():
    rules = []
    os.makedirs(os.path.join(DIST, "fonts"))
    for family, weight, package, file_name in FONTS:
        source = os.path.join(FONTSOURCE, package, "files", file_name)
        if not os.path.isfile(source):
            fail(f"fonte {file_name} não achada: rode 'make app-front'")
        shutil.copyfile(source, os.path.join(DIST, "fonts", file_name))
        rules.append(
            "@font-face {\n"
            f"  font-family: '{family}';\n"
            "  font-style: normal;\n"
            "  font-display: block;\n"
            f"  font-weight: {weight};\n"
            f"  src: url(fonts/{file_name}) format('woff2');\n"
            "}\n"
        )
    with open(os.path.join(DIST, "fonts.css"), "w", encoding="utf-8") as f:
        f.write("/* GENERATED by dev/extension/build.py (fonts from the app's @fontsource). */\n")
        f.write("".join(rules))


def brand_js():
    svg = ET.parse(os.path.join(APP, "src", "lib", "components", "ward-symbol.svg")).getroot()
    paths = [p for p in svg.iter("{http://www.w3.org/2000/svg}path") if p.get("fill")]
    if len(paths) != 1:
        fail("ward-symbol.svg deveria ter um caminho preenchido")
    p = paths[0]
    data = {"box": svg.get("viewBox"), "t": p.get("transform", ""), "d": p.get("d")}
    with open(os.path.join(DIST, "brand.js"), "w", encoding="utf-8") as f:
        f.write("// GENERATED by dev/extension/build.py from app/src/lib/components/ward-symbol.svg.\n")
        f.write(f"export const SYMBOL = Object.freeze({json.dumps(data, ensure_ascii=False)});\n")


def check_package():
    for dirpath, _, names in os.walk(DIST):
        for n in names:
            p = os.path.join(dirpath, n)
            rel = os.path.relpath(p, DIST)
            if os.path.islink(p):
                fail(f"{rel}: link simbólico no pacote")
            ext = os.path.splitext(n)[1]
            if ext not in EXTENSIONS:
                fail(f"{rel}: tipo de arquivo inesperado")
            if ext in {".woff2", ".png"}:
                continue
            with open(p, "rb") as f:
                data = f.read()
            # In the manifest, only the `content_scripts` and `host_permissions` patterns
            # (checked above).
            accepted = NAMESPACES | (CONTENT_PATTERNS | set(HOST_PERMISSIONS) if rel == "manifest.json" else set())
            for u in URL.findall(data):
                if u.decode() not in accepted:
                    fail(f"{rel}: URL remota {u.decode()!r}")
            if rel in {"icons.js", "brand.js", "tokens.css", "fonts.css", "manifest.json"}:
                continue
            for pattern in FORBIDDEN + FORBIDDEN_PER_FILE.get(rel, []):
                if re.search(pattern, data):
                    fail(f"{rel}: contém {pattern.decode()!r}")
            if ext == ".html":
                text = data.decode()
                for s in re.findall(r"<script\b[^>]*>(.*?)</script>", text, re.S):
                    if s.strip():
                        fail(f"{rel}: script inline")
                if re.search(r"<style\b|\sstyle\s*=|\son[a-z]+\s*=", text):
                    fail(f"{rel}: estilo ou tratador inline")


def main():
    with open(os.path.join(SOURCE, "manifest.json"), encoding="utf-8") as f:
        manifest = json.load(f)
    eid = check_manifest(manifest)
    if os.path.lexists(DIST):
        shutil.rmtree(DIST)
    os.makedirs(os.path.join(DIST, "icons"))
    for n in FILES:
        if n == "manifest.json":
            with open(os.path.join(DIST, n), "w", encoding="utf-8") as f:
                json.dump(manifest, f, ensure_ascii=False, indent=2)
                f.write("\n")
            continue
        shutil.copyfile(os.path.join(SOURCE, n), os.path.join(DIST, n))
    shutil.copyfile(os.path.join(APP, "src", "lib", "style", "tokens.css"), os.path.join(DIST, "tokens.css"))
    shutil.copyfile(os.path.join(APP, "src", "lib", "components", "icons.js"), os.path.join(DIST, "icons.js"))
    for t in ICON_SIZES:
        shutil.copyfile(
            os.path.join(ROOT, "docs", "design", "icones", f"extensao-destravado-{t}.png"),
            os.path.join(DIST, "icons", f"extension-{t}.png"),
        )
    fonts_css()
    brand_js()
    check_package()
    print(f"extensão montada em extension/dist/ (id {eid})")


if __name__ == "__main__":
    main()
