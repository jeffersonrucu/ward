#!/usr/bin/python3 -I
"""Lê um pacote CRX3 da extensão do Ward e imprime "<id> <versão>" (Fase 51; D18).

Uso: crx.py <arquivo.crx>. Sai 1, com a mensagem no stderr, se o pacote não é um CRX3
bem formado, se o id do cabeçalho assinado não é o sha256 de nenhuma chave RSA do
cabeçalho (é assim que o Chrome tira o id), se o `manifest.json` do zip falta, não é a
versão 3 ou traz versão fora do formato do Chrome.

O que este script **não** faz: conferir as assinaturas (exigiria cripto de RSA; o Chrome
confere ao instalar e recusa pacote com assinatura errada). O que ele garante é que o id
que vai para a política, para o `update.xml` e para o `allowed_origins` do manifesto de
native messaging é o id deste pacote (o instalar.sh compara com EXTENSAO_ID).

Só a biblioteca padrão. O protobuf do cabeçalho (`crx3.proto` do Chromium) é lido à mão,
só os tipos 0 (varint) e 2 (bytes); qualquer outro tipo recusa.
"""

import hashlib
import io
import json
import re
import struct
import sys
import zipfile

MAX_PACOTE = 64 * 1024 * 1024
MAX_CABECALHO = 1024 * 1024
MAX_MANIFESTO = 1024 * 1024
VERSAO = re.compile(r"^(0|[1-9][0-9]{0,8})(\.(0|[1-9][0-9]{0,8})){0,3}$")


class Recusa(Exception):
    pass


def varint(b, i):
    n = 0
    for k in range(10):
        if i >= len(b):
            raise Recusa("varint cortado")
        c = b[i]
        i += 1
        n |= (c & 0x7F) << (7 * k)
        if not c & 0x80:
            return n, i
    raise Recusa("varint longo demais")


def campos(b):
    """(número, valor) de cada campo de uma mensagem protobuf."""
    i = 0
    while i < len(b):
        chave, i = varint(b, i)
        numero, tipo = chave >> 3, chave & 7
        if tipo == 0:
            valor, i = varint(b, i)
        elif tipo == 2:
            n, i = varint(b, i)
            if n > len(b) - i:
                raise Recusa("campo maior que a mensagem")
            valor, i = b[i : i + n], i + n
        else:
            raise Recusa(f"tipo de campo {tipo} não aceito")
        yield numero, tipo, valor


def ler(caminho):
    with open(caminho, "rb") as f:
        dados = f.read(MAX_PACOTE + 1)
    if len(dados) > MAX_PACOTE:
        raise Recusa("pacote grande demais")
    if len(dados) < 12 or dados[:4] != b"Cr24":
        raise Recusa("não é um pacote CRX")
    versao, tam = struct.unpack("<II", dados[4:12])
    if versao != 3:
        raise Recusa(f"CRX versão {versao}; só a 3 é aceita")
    if tam > MAX_CABECALHO or 12 + tam > len(dados):
        raise Recusa("cabeçalho fora do tamanho")
    cabecalho, zip_ = dados[12 : 12 + tam], dados[12 + tam :]

    chaves_rsa, assinado = [], None
    for numero, tipo, valor in campos(cabecalho):
        if tipo != 2:
            continue
        if numero == 2:  # sha256_with_rsa: AsymmetricKeyProof
            for n2, t2, v2 in campos(valor):
                if n2 == 1 and t2 == 2:
                    chaves_rsa.append(bytes(v2))
        elif numero == 10000:  # signed_header_data
            if assinado is not None:
                raise Recusa("dois cabeçalhos assinados")
            assinado = bytes(valor)
    if assinado is None:
        raise Recusa("sem cabeçalho assinado")
    crx_id = None
    for numero, tipo, valor in campos(assinado):
        if numero == 1 and tipo == 2:
            if crx_id is not None:
                raise Recusa("dois ids")
            crx_id = bytes(valor)
    if crx_id is None or len(crx_id) != 16:
        raise Recusa("id do pacote ausente ou fora do tamanho")
    if not any(hashlib.sha256(k).digest()[:16] == crx_id for k in chaves_rsa):
        raise Recusa("o id do pacote não é o de nenhuma chave do cabeçalho")
    ident = "".join(chr(ord("a") + int(c, 16)) for c in crx_id.hex())

    try:
        with zipfile.ZipFile(io.BytesIO(zip_)) as z:
            info = z.getinfo("manifest.json")
            if info.file_size > MAX_MANIFESTO:
                raise Recusa("manifest.json grande demais")
            manifesto = json.loads(z.read(info).decode("utf-8"))
    except (zipfile.BadZipFile, KeyError, UnicodeDecodeError, ValueError) as e:
        raise Recusa(f"zip ou manifest.json ilegível ({type(e).__name__})") from None
    if not isinstance(manifesto, dict) or manifesto.get("manifest_version") != 3:
        raise Recusa("manifest.json não é da versão 3")
    versao_ext = manifesto.get("version")
    if not isinstance(versao_ext, str) or not VERSAO.match(versao_ext):
        raise Recusa("versão da extensão fora do formato")
    return ident, versao_ext


def main():
    if len(sys.argv) != 2:
        print("uso: crx.py <arquivo.crx>", file=sys.stderr)
        return 2
    try:
        ident, versao = ler(sys.argv[1])
    except (Recusa, OSError) as e:
        print(f"crx: {e}", file=sys.stderr)
        return 1
    print(ident, versao)
    return 0


if __name__ == "__main__":
    sys.exit(main())
