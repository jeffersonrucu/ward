#!/usr/bin/python3 -IB
"""Lista de segredos do cofre de teste e das variações de D11, D36, D37, D54, D58 e D61
(Fases 19, 19d, 19e e 22a).

Uso (da raiz):
    python3 -I -B dev/variations.py [--output .dev/<nome>.txt]      # make dev-variations

Os validadores usam a lista com `grep -F -f .dev/variations.txt` ("contém segredo" do
contrato). Uma linha por padrão, sem linha vazia, nenhum padrão com menos de 6 bytes;
forma com quebra de linha ou nulo não cabe numa linha e fica de fora.

Para cada valor secreto das fixtures (`segredos`, extras com `segredo: true`, anexos, a
chave privada de `.dev/services/keys/`, a senha mestra e o PIN), a tabela única de
D54, a mesma que a máscara implementa (sem uma chamar a outra):
1. segredo inteiro: o literal; URL-encoded (`quote` sem nada seguro, com `/` seguro,
   `quote_plus`, hex maiúsculo e minúsculo, todo byte codificado); base64 padrão e
   URL-safe, com e sem `=`; o miolo do base64 do valor dentro de um base64 maior (3
   alinhamentos); e as formas de texto;
2. Basic: base64 de `usuário:segredo`, só para segredo de campo de entrada com usuário;
3. por linha (segredo com `\\n` e anexo; D36, D54): cada linha com 6+ caracteres, sem o
   `\\r` do fim e aparada, URL-encoded e com as formas de texto dela, sem base64 nem
   miolos (o base64 de linha solta fica em RA3); os campos de 6+ de uma linha de texto
   com tab; numa linha que não é texto, os campos de tab de 6+ bytes só pelo literal e
   URL-encoded (D61);
4. valor de linha (`CHAVE=valor`/`export CHAVE=valor`, `"chave": "valor"` com a chave de
   qualquer caractere menos `"` e também colado, `chave: valor`, `chave : valor`,
   `chave:\\tvalor`, `- chave: valor`, e o campo de tab), com e sem aspas, 6+: tratado
   como segredo inteiro (1). Mais (D61): o comentário no fim (`v # x`, `"v" # x`) do
   YAML e do dotenv; a chave YAML com espaço ou `/` (`private key: v`); o `''` das aspas
   simples do YAML; e a string decodificada de cada par `"chave": "string"` em qualquer
   posição de uma linha de texto (também no segredo de uma linha só que começa por `{`
   ou `[`).
Anexo inteiro: o conteúdo, sem a quebra de linha final, URL-encoded e base64.

Formas de texto (D37, D54), cada uma só quando difere do literal e só para texto
(UTF-8 válido): escape JSON padrão, com e sem `\\/`; JSON do Go (`&`, `<`, `>`, U+2028 e
U+2029 como `\\u0026`, `\\u003c`, `\\u003e`, `\\u2028`, `\\u2029`); `ensure_ascii` do
Python (`\\uXXXX` minúsculo, com par substituto acima de U+FFFF); `json_encode` do PHP
(`\\/` e `\\uXXXX` juntos, só com `/` e caractere fora do ASCII); Go com `ensure_ascii`
(D58: `\\uXXXX` de `&<>` e de todo caractere fora do ASCII, só com os dois); aspas duplas do shell
(`\\` antes de `$`, `` ` ``, `"` e `\\`); aspas simples do shell (`'` vira `'\\''`, só
com `'`); e `printf %q` do bash 5.2 (só ASCII visível e espaço). O hex em maiúsculas,
o `$'…'` do bash, o `expand`, o `repr()`, o `iconv`, o elemento de array JSON e o flow
do YAML ficam de fora (RA3). Mais o `DATABASE_URL` das entradas
`banco` (D10).

Independente do código da máscara (Fases 17, 19c e 19e) de propósito: é o oráculo de
quem confere; não importa nem chama o `ward-core`. Testes: `dev/tests/variations_test.py`
e o teste cruzado oráculo → máscara de `dev/tests/dev-vault.sh` (os dois rodados por
`make test-dev-vault`). Só a biblioteca padrão. Nada vai para stdout/stderr além do
caminho e da contagem; a lista é gravada 0600, de forma atômica, só dentro de `.dev/`.
Nunca lê fora do repositório.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import re
import stat
import sys
from pathlib import Path, PurePosixPath
from urllib.parse import quote, quote_plus

os.environ["PATH"] = "/usr/bin:/bin"

RAIZ = Path(__file__).resolve().parents[1]
FIXTURES = RAIZ / "dev" / "fixtures" / "entries.json"
DEV = RAIZ / ".dev"
SAIDA_PADRAO = ".dev/variations.txt"
BASE_CHAVES = ".dev/services/keys"
BASE_ANEXOS = "dev/fixtures/attachments"
MIN_PADRAO = 6
MAX_ARQUIVO = 16 * 1024 * 1024
ESQUEMAS = {
    "postgres": "postgresql",
    "mysql": "mysql",
    "mongo": "mongodb",
    "redis": "redis",
    "sqlserver": "sqlserver",
}


class Recusa(Exception):
    """Erro com texto fixo (nunca com valor de segredo)."""


def falhar(msg: str) -> None:
    print(f"dev-variations: {msg}", file=sys.stderr)
    sys.exit(1)


# --- Caminhos ------------------------------------------------------------------------


def lexical(pedido: str) -> PurePosixPath:
    """Caminho absoluto só pelo texto; relativo é relativo à raiz; `..` é recusado."""
    p = PurePosixPath(pedido)
    if ".." in p.parts:
        raise Recusa("não pode ter '..'")
    if not p.is_absolute():
        p = PurePosixPath(RAIZ) / p
    return PurePosixPath(*[c for c in p.parts if c != "."])


def sem_link(ate: PurePosixPath, desde: PurePosixPath) -> None:
    """Todo diretório entre `desde` (exclusive) e `ate` (exclusive) é diretório real."""
    atual = Path(desde)
    for parte in ate.relative_to(desde).parts[:-1]:
        atual = atual / parte
        try:
            m = os.lstat(atual)
        except FileNotFoundError:
            raise Recusa("diretório do caminho não existe") from None
        if not stat.S_ISDIR(m.st_mode):
            raise Recusa("um diretório do caminho é link simbólico")


def validar_saida(pedido: str) -> Path:
    """Saída só dentro de `<raiz>/.dev/`, nome `*.txt`, sem link simbólico (S15)."""
    abs_ = lexical(pedido)
    dev = PurePosixPath(DEV)
    # 1) Só texto: nada fora do repositório é consultado no disco.
    try:
        rel = abs_.relative_to(dev)
    except ValueError:
        raise Recusa("tem de ficar dentro de .dev/ do repositório") from None
    if not rel.parts or rel.name.startswith(".") or not rel.name.endswith(".txt") or rel.name == ".txt":
        raise Recusa("o nome tem de terminar em .txt e não começar com '.'")
    # 2) Disco.
    try:
        m = os.lstat(DEV)
    except FileNotFoundError:
        os.mkdir(DEV, 0o700)
        m = os.lstat(DEV)
    if not stat.S_ISDIR(m.st_mode) or Path(os.path.realpath(DEV)) != DEV:
        raise Recusa(".dev não é um diretório da raiz (link simbólico?)")
    sem_link(abs_, dev)
    try:
        m = os.lstat(abs_)
        if not stat.S_ISREG(m.st_mode):
            raise Recusa("o destino existe e não é arquivo comum")
    except FileNotFoundError:
        pass
    return Path(abs_)


def ler_do_repositorio(caminho: str, base: str) -> bytes:
    """Arquivo citado nas fixtures: dentro de `base`, sem link, arquivo comum."""
    if PurePosixPath(caminho).is_absolute():
        raise Recusa(f"{caminho}: tem de ser relativo à raiz")
    abs_ = lexical(caminho)
    try:
        abs_.relative_to(PurePosixPath(RAIZ) / base)
    except ValueError:
        raise Recusa(f"{caminho}: fora da pasta permitida") from None
    try:
        sem_link(abs_, PurePosixPath(RAIZ))
        fd = os.open(abs_, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    except (Recusa, FileNotFoundError):
        dica = "; rode make dev-services antes" if base == BASE_CHAVES else ""
        raise Recusa(f"{caminho}: não existe{dica}") from None
    except OSError:
        raise Recusa(f"{caminho}: não é arquivo comum (link simbólico?)") from None
    try:
        m = os.fstat(fd)
        if not stat.S_ISREG(m.st_mode) or m.st_size > MAX_ARQUIVO:
            raise Recusa(f"{caminho}: não é arquivo comum ou é grande demais")
        with os.fdopen(fd, "rb", closefd=False) as f:
            return f.read(MAX_ARQUIVO + 1)
    finally:
        os.close(fd)


def gravar(caminho: Path, conteudo: bytes) -> None:
    """Grava 0600 de forma atômica (temp O_EXCL|O_NOFOLLOW + fsync + rename)."""
    temp = caminho.with_name(f".{caminho.name}.tmp")
    try:
        if stat.S_ISREG(os.lstat(temp).st_mode):
            os.unlink(temp)
    except FileNotFoundError:
        pass
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    try:
        os.fchmod(fd, 0o600)
        os.write(fd, conteudo)
        os.fsync(fd)
    finally:
        os.close(fd)
    os.replace(temp, caminho)
    dfd = os.open(caminho.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(dfd)
    finally:
        os.close(dfd)


# --- Variações -----------------------------------------------------------------------
#
# A tabela única de D54. Escrita aqui a partir do texto do PRD (D11, D34, D36, D37,
# D54), não do `mascara.rs`:
#   1. segredo inteiro: literal, base64 (padrão e URL-safe, com e sem `=`), miolos de
#      base64 (3 alinhamentos), percent e as formas de texto;
#   2. Basic: base64 de `usuário:segredo` (só segredo de campo de entrada com usuário);
#   3. por linha (segredo de várias linhas e anexo): a linha de 6+ (sem o `\r` do fim e
#      aparada), percent e as formas de texto dela, sem base64 nem miolos; os campos de
#      6+ de uma linha com tab;
#   4. valor de linha (`CHAVE=valor`, `"chave": "valor"`, `chave: valor`, campo de tab),
#      com e sem aspas, 6+: tratado como segredo inteiro (1).
# Anexo inteiro: o conteúdo, sem a quebra de linha final, base64 e percent.
# Formas de texto e campos de tab só para texto (UTF-8 válido).

BARRA = chr(92)
# Espaço em branco ASCII das pontas (o mesmo conjunto nas duas implementações:
# espaço, tab, LF, FF, CR; o VT fica).
BRANCO = b" \t\n\x0c\r"


def b64(v: bytes) -> list[bytes]:
    p = base64.b64encode(v)
    u = base64.urlsafe_b64encode(v)
    return [p, p.rstrip(b"="), u, u.rstrip(b"=")]


def miolos_b64(v: bytes) -> list[bytes]:
    """Trecho do base64 que só depende de `v` quando `v` está dentro de um base64 maior,
    com 0, 1 ou 2 bytes antes (e qualquer coisa depois)."""
    if len(v) < 6:
        return []
    saida = []
    for k in (0, 1, 2):
        total = k + len(v)
        inicio = (8 * k + 5) // 6  # primeiro caractere sem bit do prefixo
        fim = (8 * total) // 6  # último caractere completo só com bits de `v`
        for codificar in (base64.b64encode, base64.urlsafe_b64encode):
            m = codificar(b"\0" * k + v)[inicio:fim]
            if len(m) >= 8:
                saida.append(m)
    return saida


def minusculo_hex(s: str) -> str:
    return re.sub(r"%[0-9A-F]{2}", lambda m: m.group(0).lower(), s)


def url(v: bytes) -> list[bytes]:
    formas = [quote(v, safe=""), quote(v, safe="/"), quote_plus(v, safe="")]
    formas += [minusculo_hex(f) for f in formas]
    tudo = "".join(f"%{b:02X}" for b in v)
    formas += [tudo, tudo.lower()]
    return [f.encode("ascii") for f in formas]


def texto(v: bytes) -> str | None:
    try:
        return v.decode("utf-8")
    except UnicodeDecodeError:
        return None


def caracteres(v: bytes) -> int:
    """Caracteres de UTF-8 válido; bytes se não for texto (o mínimo de D34/D36)."""
    t = texto(v)
    return len(v) if t is None else len(t)


def json_escape(v: bytes) -> list[bytes]:
    """Escape JSON padrão, com e sem `\\/`."""
    t = texto(v)
    if t is None:
        return []
    j = json.dumps(t, ensure_ascii=False)[1:-1]
    return [x.encode("utf-8") for x in {j, j.replace("/", BARRA + "/")}]


# D37. Cada forma é escrita aqui a partir da descrição do PRD, não do `mascara.rs`.
GO_HTML = {
    "&": BARRA + "u0026",
    "<": BARRA + "u003c",
    ">": BARRA + "u003e",
    chr(0x2028): BARRA + "u2028",
    chr(0x2029): BARRA + "u2029",
}


def json_go(t: str) -> str:
    """`json.Marshal` do Go: o escape JSON comum mais `&<>` e U+2028/U+2029 em `\\u`."""
    j = json.dumps(t, ensure_ascii=False)[1:-1]
    return "".join(GO_HTML.get(c, c) for c in j)


def json_ascii(t: str) -> str:
    """`json.dumps` do Python (`ensure_ascii`): fora do ASCII vira `\\uXXXX` minúsculo,
    com par substituto acima de U+FFFF."""
    return json.dumps(t, ensure_ascii=True)[1:-1]


def json_php(t: str) -> str | None:
    """`json_encode` do PHP (D54): `/` como `\\/` e todo caractere fora do ASCII como
    `\\uXXXX` minúsculo. Só quando o valor tem os dois."""
    if "/" not in t or all(ord(c) < 0x80 for c in t):
        return None
    partes = []
    for c in t:
        if ord(c) >= 0x80:
            partes.append(json.dumps(c, ensure_ascii=True)[1:-1])
        elif c == "/":
            partes.append(BARRA + "/")
        else:
            partes.append(json.dumps(c, ensure_ascii=False)[1:-1])
    return "".join(partes)


def json_go_ascii(t: str) -> str | None:
    """JSON "html-safe" com `ensure_ascii` (D58): o `json.dumps` padrão do Python com `&`,
    `<`, `>` também em `\\uXXXX`. Só quando o valor tem `&`, `<`, `>`, U+2028 ou U+2029
    e caractere fora do ASCII."""
    if not any(c in GO_HTML for c in t) or all(ord(c) < 0x80 for c in t):
        return None
    j = json.dumps(t, ensure_ascii=True)[1:-1]
    return "".join(GO_HTML.get(c, c) for c in j)


def aspas_duplas(t: str) -> str:
    """Valor dentro de aspas duplas do shell (`declare -p`, `export -p`)."""
    return re.sub(r'([$`"\\])', r"\\\1", t)


def aspas_simples(t: str) -> str | None:
    """Valor dentro de aspas simples do shell (`${X@Q}`, `set`, `set -x`, D54): cada `'`
    vira `'\\''`. Só quando o valor tem `'`."""
    if "'" not in t:
        return None
    return t.replace("'", "'" + BARRA + "''")


# `printf %q` do bash 5.2 (D54): barra antes destes sempre; `#` só no começo; `~` no
# começo ou depois de `=` ou `:`. Só para ASCII visível e espaço (o resto vai para
# `$'…'`, RA3). Conferido contra o bash real em `dev/tests/variations_test.py`.
PRINTF_Q_SEMPRE = set(" !\"$&'()*,;<>?[]^`{|}" + BARRA)


def printf_q(t: str) -> str | None:
    if not t or any(not (0x20 <= ord(c) <= 0x7E) for c in t):
        return None
    saida = []
    for i, c in enumerate(t):
        if c in PRINTF_Q_SEMPRE or (c == "#" and i == 0) or (c == "~" and (i == 0 or t[i - 1] in "=:")):
            saida.append(BARRA)
        saida.append(c)
    return "".join(saida)


def escapadas(v: bytes) -> set[bytes]:
    """As formas de texto de D37 e D54 que diferem do valor literal (vazio se nenhuma
    muda nada): escape JSON padrão (com e sem `\\/`), JSON do Go, `ensure_ascii`, PHP,
    aspas duplas, aspas simples e `printf %q`. Só para texto."""
    t = texto(v)
    if t is None:
        return set()
    formas = {json_go(t), json_ascii(t), aspas_duplas(t)}
    for extra in (json_php(t), json_go_ascii(t), aspas_simples(t), printf_q(t)):
        if extra is not None:
            formas.add(extra)
    return ({f.encode("utf-8") for f in formas} | set(json_escape(v))) - {v}


def inteiro(x: bytes) -> set[bytes]:
    """Linha 1 da tabela: o segredo inteiro e as variações dele."""
    if not x:
        return set()
    return {x} | set(b64(x)) | set(miolos_b64(x)) | set(url(x)) | escapadas(x)


def basic(usuario: str, x: bytes) -> set[bytes]:
    """Linha 2: base64 de `usuário:segredo`."""
    return set(b64(usuario.encode("utf-8") + b":" + x)) if x else set()


def aparar(v: bytes) -> bytes:
    return v.strip(BRANCO)


CHAVE_IGUAL = re.compile(rb"^(?:export\s+)?[A-Za-z_.-][A-Za-z0-9_.-]*\s*=(.*)$")
CHAVE_JSON = re.compile(rb'^(?:- [ \t\n\x0c\r]*)?"[^"]+"[ \t\n\x0c\r]*:(.*)$')
CHAVE_YAML = re.compile(rb"^(?:- [ \t\n\x0c\r]*)?[A-Za-z0-9_.-]+[ \t\n\x0c\r]*:(?:[ \t](.*))?$")
# D61: chave YAML com espaço ou `/`; a chave vai até o primeiro `:` seguido de espaço,
# tab ou fim, sem `#` nem aspas, e não começa por `-`, `{` nem `[`.
CHAVE_YAML_COMPOSTA = re.compile(rb"""^(?:- [ \t\n\x0c\r]*)?[^#"'{\[\- \t\n\x0c\r][^#"']*?:(?:[ \t](.*))?$""", re.S)
# D61: comentário no fim. Sem aspas: até antes do primeiro espaço/tab seguido de `#`.
# Entre aspas: o miolo até a primeira aspa igual seguida de espaços e `#`.
COMENTARIO = re.compile(rb"[ \t]#")
ENTRE_ASPAS_COMENTARIO = re.compile(rb"""^(["'])(.*?)\1[ \t]+#""", re.S)


def valores_da_linha(aparada: bytes) -> list[bytes]:
    """O valor de `CHAVE=valor`, `"chave": "valor"` (também colado `"k":"v"` e com a
    chave de qualquer caractere menos `"`) e `chave: valor` (também `chave : valor` e
    `chave:\\tvalor`) (D11, D36, D54), sem a vírgula do fim no JSON/YAML, com e sem
    aspas, cada um com 6+ caracteres (D34)."""
    aparada = aparar(aparada)
    brutos = []
    for regex, virgula, yaml in (
        (CHAVE_IGUAL, False, False),
        (CHAVE_JSON, True, True),
        (CHAVE_YAML, True, True),
        (CHAVE_YAML_COMPOSTA, True, True),
    ):
        m = regex.match(aparada)
        if not m or m.group(1) is None:
            continue
        valor = aparar(m.group(1))
        if virgula and valor.endswith(b","):
            valor = aparar(valor[:-1])
        brutos.append((valor, yaml))
    saida = []
    for valor, yaml in brutos:
        formas = [valor]
        if len(valor) >= 3 and valor[:1] == valor[-1:] and valor[:1] in (b'"', b"'"):
            formas.append(valor[1:-1])
        formas += comentario_e_aspas(valor, yaml)
        saida += [x for x in formas if caracteres(x) >= MIN_PADRAO]
    return saida


def comentario_e_aspas(valor: bytes, yaml: bool) -> list[bytes]:
    """Os candidatos a mais de D61 para um valor de linha: o valor antes do comentário do
    fim (com ou sem aspas) e, no YAML, `''` trocado por `'` no valor entre aspas
    simples."""
    if not valor:
        return []
    if valor[:1] not in (b'"', b"'"):
        m = COMENTARIO.search(valor)
        return [aparar(valor[: m.start()])] if m else []
    saida = []
    m = ENTRE_ASPAS_COMENTARIO.match(valor)
    miolo = m.group(2) if m else None
    if miolo is not None:
        saida.append(miolo)
    if yaml and valor[:1] == b"'":
        if miolo is None and len(valor) >= 3 and valor[-1:] == b"'":
            miolo = valor[1:-1]
        if miolo is not None and b"''" in miolo:
            saida.append(miolo.replace(b"''", b"'"))
    return saida


STRING_JSON = re.compile(rb'"(?:[^"\\]|\\.)*"', re.S)


def valores_json(linha: bytes) -> list[bytes]:
    """D61: cada par `"chave"` + espaços + `:` + espaços + string JSON, em qualquer posição
    da linha de texto, dá a string decodificada (escape inválido ou substituto sozinho
    descarta o par), com 6+ caracteres. A busca anda de string em string: depois de um
    par, segue do fim do valor; depois de uma string que não é chave, do fim dela."""
    if texto(linha) is None:
        return []
    saida = []
    i = 0
    while True:
        i = linha.find(b'"', i)
        if i < 0:
            break
        chave = STRING_JSON.match(linha, i)
        if not chave:
            break
        j = chave.end()
        while linha[j : j + 1] in (b" ", b"\t"):
            j += 1
        if linha[j : j + 1] != b":":
            i = chave.end()
            continue
        j += 1
        while linha[j : j + 1] in (b" ", b"\t"):
            j += 1
        if linha[j : j + 1] != b'"':
            i = chave.end()
            continue
        valor = STRING_JSON.match(linha, j)
        if not valor:
            break
        try:
            d = json.loads(valor.group(0).decode("utf-8"), strict=False).encode("utf-8")
        except (ValueError, UnicodeEncodeError):
            d = None
        if d is not None and caracteres(d) >= MIN_PADRAO:
            saida.append(d)
        i = valor.end()
    return saida


def campos_de_tab_binarios(linha: bytes) -> list[bytes]:
    """D61: os campos de uma linha com tab que não é texto, aparados e crus, com 6+
    bytes. Só literal (e URL-encoded): sem formas de texto nem base64."""
    if b"\t" not in linha or texto(linha) is not None:
        return []
    saida = []
    for campo in linha.split(b"\t"):
        saida += [x for x in (aparar(campo), campo) if len(x) >= MIN_PADRAO]
    return saida


def campos_de_tab(linha: bytes) -> list[bytes]:
    """Os campos de uma linha de texto com tab (`cut -f`, TSV; D54), aparados, com e sem
    aspas, 6+."""
    if b"\t" not in linha or texto(linha) is None:
        return []
    saida = []
    for campo in linha.split(b"\t"):
        campo = aparar(campo)
        formas = [campo]
        if len(campo) >= 3 and campo[:1] == campo[-1:] and campo[:1] in (b'"', b"'"):
            formas.append(campo[1:-1])
        saida += [x for x in formas if caracteres(x) >= MIN_PADRAO]
    return saida


def linhas_de_anexo(v: bytes) -> list[bytes]:
    """As linhas de 6+ caracteres de um segredo de várias linhas (sem o `\\r` do fim e
    aparadas; D11, D36) e os valores de linha delas (`valores_da_linha`, campos de tab).
    A linha entra como linha (formas de texto, sem base64); o valor, como inteiro."""
    linhas, valores, crus = por_linha(v)
    return sorted(linhas | valores | crus)


def por_linha(v: bytes) -> tuple[set[bytes], set[bytes], set[bytes]]:
    """Linha 3 (as linhas), linha 4 (os valores) da tabela e os campos de tab de linha
    que não é texto (só literal, D61)."""
    linhas: set[bytes] = set()
    valores: set[bytes] = set()
    crus: set[bytes] = set()
    for linha in v.split(b"\n"):
        if linha.endswith(b"\r"):
            linha = linha[:-1]
        if caracteres(linha) < MIN_PADRAO:
            continue
        linhas.add(linha)
        aparada = aparar(linha)
        if caracteres(aparada) >= MIN_PADRAO:
            linhas.add(aparada)
        valores.update(campos_de_tab(linha))
        valores.update(valores_da_linha(linha))
        valores.update(valores_json(linha))
        crus.update(campos_de_tab_binarios(linha))
    return linhas, valores, crus


def de_linha(v: bytes) -> set[bytes]:
    """As variações da passada por linha (linhas 3 e 4)."""
    linhas, valores, crus = por_linha(v)
    pad: set[bytes] = set()
    for linha in linhas:
        pad.add(linha)
        pad |= set(url(linha)) | escapadas(linha)
    for campo in crus:
        pad.add(campo)
        pad |= set(url(campo))
    for valor in valores:
        pad |= inteiro(valor)
    return pad


def variacoes(v: bytes, usuario: str | None, anexo: bool = False) -> set[bytes]:
    """As variações de um valor secreto que cabem numa linha da lista. `usuario`
    (segredo de campo de entrada com usuário) acrescenta o Basic; `anexo` troca a linha
    1 pelo anexo inteiro."""
    pad: set[bytes] = set()
    if anexo:
        sem_quebra = v[:-1] if v.endswith(b"\n") else v
        sem_quebra = sem_quebra[:-1] if sem_quebra.endswith(b"\r") else sem_quebra
        for x in {v, sem_quebra} - {b""}:
            pad.add(x)
            pad |= set(url(x))
        pad |= set(b64(v))
        pad |= de_linha(v)
    else:
        pad |= inteiro(v)
        if usuario:
            pad |= basic(usuario, v)
        if b"\n" in v:
            pad |= de_linha(v)
        elif aparar(v)[:1] in (b"{", b"["):
            # D61: JSON de uma linha só (`{"token": "…"}` num extra).
            for valor in valores_json(v):
                pad |= inteiro(valor)
    return {p for p in pad if listavel(p)}


def database_url(e: dict) -> list[bytes]:
    c, s = e.get("campos", {}), e.get("segredos", {})
    esquema = ESQUEMAS.get(c.get("motor", ""))
    senha = s.get("senha")
    if not esquema or not senha:
        return []
    usuario = quote(c.get("usuario", ""), safe="")
    cifrada = quote(senha, safe="")
    host, porta, base = c.get("host", ""), c.get("porta", ""), c.get("base", "")
    alvo = f"{esquema}://{usuario}:{cifrada}@{host}:{porta}/{base}"
    # Sem host/porta (o túnel troca pelos locais): o pedaço com a senha.
    return [alvo.encode(), f"{esquema}://{usuario}:{cifrada}@".encode()]


def listavel(p: bytes) -> bool:
    """Padrão que cabe numa linha do `grep -F -f` (sem quebra de linha nem nulo)."""
    return b"\n" not in p and b"\r" not in p and b"\0" not in p


# --- Montagem ------------------------------------------------------------------------


def da_entrada(e: dict) -> set[bytes]:
    """As variações dos segredos de uma entrada das fixtures, sem o `DATABASE_URL` (que
    vem da entrega, com host e porta de quem roda) e só as que cabem numa linha."""
    pad: set[bytes] = set()
    usuario = e.get("campos", {}).get("usuario") or None
    for campo, valor in e.get("segredos", {}).items():
        if campo.endswith("_arquivo"):
            bruto = ler_do_repositorio(valor, BASE_CHAVES)
        else:
            bruto = valor.encode("utf-8")
        pad |= variacoes(bruto, usuario)
    for x in e.get("extras", []):
        if x.get("segredo") is True:
            pad |= variacoes(x["valor"].encode("utf-8"), None)
    for a in e.get("anexos", []):
        pad |= variacoes(ler_do_repositorio(a["arquivo"], BASE_ANEXOS), None, anexo=True)
    return {p for p in pad if listavel(p)}


def coletar(dados: dict) -> set[bytes]:
    pad: set[bytes] = set()
    for chave in ("senha_mestra", "pin"):
        pad |= variacoes(dados[chave].encode("utf-8"), None)
    for e in dados["entradas"]:
        pad |= da_entrada(e)
        pad.update(database_url(e))
    return {p for p in pad if listavel(p)}


def conferir_padroes(pad: set[bytes]) -> list[bytes]:
    lista = sorted(pad)
    for p in lista:
        # Linha vazia ou curta faria `grep -F -f` casar com quase tudo; quebra de linha
        # partiria o padrão em dois.
        if len(p) < MIN_PADRAO or b"\n" in p or b"\r" in p or b"\0" in p:
            raise Recusa("padrão curto ou com quebra de linha; confira as fixtures")
    return lista


def main() -> None:
    ap = argparse.ArgumentParser(description="Lista de segredos e variações do cofre de teste.")
    ap.add_argument("--output", default=SAIDA_PADRAO)
    args = ap.parse_args()
    try:
        saida = validar_saida(args.output or SAIDA_PADRAO)
    except Recusa as e:
        falhar(f"caminho de saída recusado: {e}")
    try:
        if os.lstat(FIXTURES).st_size > 1024 * 1024 or not FIXTURES.is_file() or FIXTURES.is_symlink():
            raise Recusa("fixtures ausentes ou grandes demais")
        dados = json.loads(FIXTURES.read_text(encoding="utf-8"))
        if dados.get("versao") != 1:
            raise Recusa("versão das fixtures desconhecida")
        lista = conferir_padroes(coletar(dados))
        gravar(saida, b"\n".join(lista) + b"\n")
    except Recusa as e:
        falhar(str(e))
    except (KeyError, TypeError, AttributeError, ValueError):
        falhar("fixtures com forma inesperada")
    rel = saida.relative_to(RAIZ)
    print(f"dev-variations: gravado {rel} (0600) com {len(lista)} padrões; uso: grep -F -f {rel}")


if __name__ == "__main__":
    main()
