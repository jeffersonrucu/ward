#!/usr/bin/python3 -I
"""Fase 65 (m2:V4.22): compara as permissões das capabilities do app com os comandos que
o front-end chama (`invoke('<nome>'`) e com as da 0.1.4 pelo glossário (contagem e janela),
e a CSP com a da 0.1.4. Só lê arquivos.

Uso: f65_caps.py <raiz> <src/ da 0.1.4 (worktree da tag) ou ->"""
import json
import os
import re
import sys

root = sys.argv[1]
caps = {}
for name in sorted(os.listdir(f"{root}/app/src-tauri/capabilities")):
    with open(f"{root}/app/src-tauri/capabilities/{name}") as f:
        c = json.load(f)
    caps[c["identifier"]] = sorted(p.removeprefix("allow-").replace("-", "_") for p in c["permissions"])

called = set()
for d, _, files in os.walk(f"{root}/app/src"):
    if "/lib/test" in d:
        continue
    for fn in files:
        if fn.endswith((".js", ".svelte")):
            src = open(os.path.join(d, fn), encoding="utf-8").read()
            called |= set(re.findall(r"""invoke\(\s*['"]([a-z_]+)['"]""", src))
            called |= set(re.findall(r"""(?:call|command)\(\s*['"]([a-z_]+)['"]""", src))

allowed = set(caps.get("main", [])) | set(caps.get("approval", []))
print("janelas:", {k: len(v) for k, v in caps.items()})
print("liberados e não chamados pelo front-end:", sorted(allowed - called - {"listen_notices"}))
print("chamados e não liberados:", sorted(called - allowed))
with open(f"{root}/app/src-tauri/tauri.conf.json") as f:
    conf = json.load(f)
print("csp:", json.dumps(conf["app"]["security"]["csp"], sort_keys=True))
