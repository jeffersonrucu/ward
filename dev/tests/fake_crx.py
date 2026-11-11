#!/usr/bin/python3 -I
"""Fake CRX3 package for `make test-install` (Phase 51). Never a real extension: the
"public key" is random bytes and so is the "signature" (the installer's `crx.py` does not
check the signature; Chrome does). It serves for the id and the version.

Usage: see the usage line in `main()`; prints the package id. The tamper option makes the
id of the signed header not the sha256 of the key (`crx.py` refuses it); the version
option sets the manifest version. The option names are passed by `dev/tests/install.sh`.
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


def field(number, value):
    return varint(number << 3 | 2) + varint(len(value)) + value


def main():
    args = sys.argv[1:]
    if not args:
        print("uso: fake_crx.py <saída> [--adulterado] [--versao <v>]", file=sys.stderr)
        return 2
    output = args[0]
    tampered = "--adulterado" in args
    version = args[args.index("--versao") + 1] if "--versao" in args else "0.1.0"
    key = os.urandom(270)
    crx_id = hashlib.sha256(key).digest()[:16]
    if tampered:
        crx_id = os.urandom(16)
    proof = field(1, key) + field(2, os.urandom(256))
    header = field(2, proof) + field(10000, field(1, crx_id))
    z = io.BytesIO()
    with zipfile.ZipFile(z, "w") as zf:
        zf.writestr("manifest.json", json.dumps(
            {"manifest_version": 3, "name": "Ward (pacote falso de teste)", "version": version}))
        zf.writestr("sw.js", "// falso\n")
    with open(output, "wb") as f:
        f.write(b"Cr24" + struct.pack("<II", 3, len(header)) + header + z.getvalue())
    print("".join(chr(ord("a") + int(c, 16)) for c in crx_id.hex()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
