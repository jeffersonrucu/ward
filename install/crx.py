#!/usr/bin/python3 -I
"""Reads a CRX3 package of the Ward extension and prints "<id> <version>" (Phase 51; D18).

Usage: crx.py <file.crx>. Exits 1, with the message on stderr, if the package is not a
well-formed CRX3, if the id of the signed header is not the sha256 of any RSA key of the
header (that is how Chrome derives the id), if the zip's `manifest.json` is missing, is
not version 3 or carries a version outside Chrome's format.

What this script does **not** do: verify the signatures (that would need RSA crypto;
Chrome verifies them on install and refuses a package with a wrong signature). What it
guarantees is that the id that goes into the policy, the `update.xml` and the
`allowed_origins` of the native messaging manifest is this package's id (install.sh
compares it with EXTENSION_ID).

Standard library only. The header protobuf (Chromium's `crx3.proto`) is parsed by hand,
only wire types 0 (varint) and 2 (bytes); any other type refuses.
"""

import hashlib
import io
import json
import re
import struct
import sys
import zipfile

MAX_PACKAGE = 64 * 1024 * 1024
MAX_HEADER = 1024 * 1024
MAX_MANIFEST = 1024 * 1024
VERSION_RE = re.compile(r"^(0|[1-9][0-9]{0,8})(\.(0|[1-9][0-9]{0,8})){0,3}$")


class Refusal(Exception):
    pass


def varint(b, i):
    n = 0
    for k in range(10):
        if i >= len(b):
            raise Refusal("varint cortado")
        c = b[i]
        i += 1
        n |= (c & 0x7F) << (7 * k)
        if not c & 0x80:
            return n, i
    raise Refusal("varint longo demais")


def fields(b):
    """(number, kind, value) of each field of a protobuf message."""
    i = 0
    while i < len(b):
        key, i = varint(b, i)
        number, kind = key >> 3, key & 7
        if kind == 0:
            value, i = varint(b, i)
        elif kind == 2:
            n, i = varint(b, i)
            if n > len(b) - i:
                raise Refusal("campo maior que a mensagem")
            value, i = b[i : i + n], i + n
        else:
            raise Refusal(f"tipo de campo {kind} não aceito")
        yield number, kind, value


def read_package(path):
    with open(path, "rb") as f:
        data = f.read(MAX_PACKAGE + 1)
    if len(data) > MAX_PACKAGE:
        raise Refusal("pacote grande demais")
    if len(data) < 12 or data[:4] != b"Cr24":
        raise Refusal("não é um pacote CRX")
    version, size = struct.unpack("<II", data[4:12])
    if version != 3:
        raise Refusal(f"CRX versão {version}; só a 3 é aceita")
    if size > MAX_HEADER or 12 + size > len(data):
        raise Refusal("cabeçalho fora do tamanho")
    header, zip_ = data[12 : 12 + size], data[12 + size :]

    rsa_keys, signed = [], None
    for number, kind, value in fields(header):
        if kind != 2:
            continue
        if number == 2:  # sha256_with_rsa: AsymmetricKeyProof
            for n2, t2, v2 in fields(value):
                if n2 == 1 and t2 == 2:
                    rsa_keys.append(bytes(v2))
        elif number == 10000:  # signed_header_data
            if signed is not None:
                raise Refusal("dois cabeçalhos assinados")
            signed = bytes(value)
    if signed is None:
        raise Refusal("sem cabeçalho assinado")
    crx_id = None
    for number, kind, value in fields(signed):
        if number == 1 and kind == 2:
            if crx_id is not None:
                raise Refusal("dois ids")
            crx_id = bytes(value)
    if crx_id is None or len(crx_id) != 16:
        raise Refusal("id do pacote ausente ou fora do tamanho")
    if not any(hashlib.sha256(k).digest()[:16] == crx_id for k in rsa_keys):
        raise Refusal("o id do pacote não é o de nenhuma chave do cabeçalho")
    ident = "".join(chr(ord("a") + int(c, 16)) for c in crx_id.hex())

    try:
        with zipfile.ZipFile(io.BytesIO(zip_)) as z:
            info = z.getinfo("manifest.json")
            if info.file_size > MAX_MANIFEST:
                raise Refusal("manifest.json grande demais")
            manifest = json.loads(z.read(info).decode("utf-8"))
    except (zipfile.BadZipFile, KeyError, UnicodeDecodeError, ValueError) as e:
        raise Refusal(f"zip ou manifest.json ilegível ({type(e).__name__})") from None
    if not isinstance(manifest, dict) or manifest.get("manifest_version") != 3:
        raise Refusal("manifest.json não é da versão 3")
    ext_version = manifest.get("version")
    if not isinstance(ext_version, str) or not VERSION_RE.match(ext_version):
        raise Refusal("versão da extensão fora do formato")
    return ident, ext_version


def main():
    if len(sys.argv) != 2:
        print("uso: crx.py <arquivo.crx>", file=sys.stderr)
        return 2
    try:
        ident, version = read_package(sys.argv[1])
    except (Refusal, OSError) as e:
        print(f"crx: {e}", file=sys.stderr)
        return 1
    print(ident, version)
    return 0


if __name__ == "__main__":
    sys.exit(main())
