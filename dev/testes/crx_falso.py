#!/usr/bin/python3 -I
"""Pacote CRX3 falso para o `make teste-install` (Fase 51). Nunca é uma extensão de
verdade: a "chave pública" são bytes aleatórios e a "assinatura" também (o `crx.py` da
instalação não confere assinatura; quem confere é o Chrome). Serve para o id e a versão.

Uso: crx_falso.py <saída> [--adulterado] [--versao <v>]. Imprime o id do pacote.
`--adulterado`: o id do cabeçalho assinado não é o sha256 da chave (o `crx.py` recusa).
"""

import hashlib
import io
import json
import os
import struct
import sys
import zipfile


def varint(n):
    b = bytearray()
    while True:
        c = n & 0x7F
        n >>= 7
        if n:
            b.append(c | 0x80)
        else:
            b.append(c)
            return bytes(b)


def campo(numero, valor):
    return varint(numero << 3 | 2) + varint(len(valor)) + valor


def main():
    args = sys.argv[1:]
    if not args:
        print("uso: crx_falso.py <saída> [--adulterado] [--versao <v>]", file=sys.stderr)
        return 2
    saida = args[0]
    adulterado = "--adulterado" in args
    versao = args[args.index("--versao") + 1] if "--versao" in args else "0.1.0"
    chave = os.urandom(270)
    crx_id = hashlib.sha256(chave).digest()[:16]
    if adulterado:
        crx_id = os.urandom(16)
    prova = campo(1, chave) + campo(2, os.urandom(256))
    cabecalho = campo(2, prova) + campo(10000, campo(1, crx_id))
    z = io.BytesIO()
    with zipfile.ZipFile(z, "w") as arq:
        arq.writestr("manifest.json", json.dumps(
            {"manifest_version": 3, "name": "Ward (pacote falso de teste)", "version": versao}))
        arq.writestr("sw.js", "// falso\n")
    with open(saida, "wb") as f:
        f.write(b"Cr24" + struct.pack("<II", 3, len(cabecalho)) + cabecalho + z.getvalue())
    print("".join(chr(ord("a") + int(c, 16)) for c in crx_id.hex()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
