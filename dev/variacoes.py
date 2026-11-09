#!/usr/bin/python3 -IB
"""Lista de segredos do cofre de teste e das variações de D11, D36 e D37 (Fases 19, 19d).

Uso (da raiz):
    python3 -I -B dev/variacoes.py [--saida .dev/<nome>.txt]      # make dev-variacoes

Os validadores usam a lista com `grep -F -f .dev/variacoes.txt` ("contém segredo" do
contrato). Uma linha por padrão, sem linha vazia, nenhum padrão com menos de 6 bytes.

O que entra, para cada valor secreto das fixtures (`segredos`, extras com
`segredo: true`, anexos, a chave privada de `.dev/servicos/chaves/`, a senha mestra e o
PIN):
- o valor literal;
- segredo de várias linhas (chave privada, extra secreto com `\\n`, anexos; D36): cada
  linha aparada com 6+ caracteres e o valor, com e sem aspas e com 6+ caracteres, das
  linhas `CHAVE=valor`/`export CHAVE=valor`, `"chave": "valor"` (JSON, com ou sem a
  vírgula do fim) e `chave: valor` (YAML, também `- chave: valor`); cada linha e cada
  valor recebem as mesmas codificações abaixo;
- URL-encoded: `quote` sem nada seguro, com `/` seguro, `quote_plus` (formulário), com
  hexadecimal maiúsculo e minúsculo, e todo byte codificado;
- base64 padrão e URL-safe, com e sem `=`, e o miolo do base64 do valor dentro de um
  base64 maior (os 3 alinhamentos);
- Basic auth: base64 de `usuário:segredo` para as entradas com usuário;
- escape JSON (e o `\\/` do PHP);
- as formas escapadas de D37, cada uma só quando difere da literal: JSON do Go (`&`,
  `<`, `>`, U+2028 e U+2029 como `\\u0026`, `\\u003c`, `\\u003e`, `\\u2028`,
  `\\u2029`), JSON com `ensure_ascii` do Python (`\\uXXXX` minúsculo, com par
  substituto acima de U+FFFF) e aspas duplas do shell (`\\` antes de `$`, `` ` ``,
  `"` e `\\`). O hex em maiúsculas fica de fora (RA3);
- `DATABASE_URL` das entradas `banco` (D10).

Independente do código da máscara (Fases 17 e 19c) de propósito: é o oráculo de quem
confere; não importa nem chama o `ward-core`. Testes: `dev/testes/variacoes_teste.py`
(rodado por `make teste-dev-cofre`). Só a biblioteca padrão. Nada vai para
stdout/stderr além do caminho e da contagem; a lista é gravada 0600, de forma atômica,
só dentro de `.dev/`. Nunca lê fora do repositório.
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
FIXTURES = RAIZ / "dev" / "fixtures" / "entradas.json"
DEV = RAIZ / ".dev"
SAIDA_PADRAO = ".dev/variacoes.txt"
BASE_CHAVES = ".dev/servicos/chaves"
BASE_ANEXOS = "dev/fixtures/anexos"
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
    print(f"dev-variacoes: {msg}", file=sys.stderr)
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
        dica = "; rode make dev-servicos antes" if base == BASE_CHAVES else ""
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
    t = texto(v)
    if t is None:
        return []
    j = json.dumps(t, ensure_ascii=False)[1:-1]
    return [x.encode("utf-8") for x in {j, j.replace("/", "\\/")}]


# D37. Cada forma é escrita aqui a partir da descrição do PRD, não do `mascara.rs`.
GO_HTML = {"&": "\\u0026", "<": "\\u003c", ">": "\\u003e", "\u2028": "\\u2028", "\u2029": "\\u2029"}


def json_go(t: str) -> str:
    """`json.Marshal` do Go: o escape JSON comum mais `&<>` e U+2028/U+2029 em `\\u`."""
    j = json.dumps(t, ensure_ascii=False)[1:-1]
    return "".join(GO_HTML.get(c, c) for c in j)


def json_ascii(t: str) -> str:
    """`json.dumps` do Python (`ensure_ascii`): fora do ASCII vira `\\uXXXX` minúsculo,
    com par substituto acima de U+FFFF."""
    return json.dumps(t, ensure_ascii=True)[1:-1]


def aspas_duplas(t: str) -> str:
    """Valor dentro de aspas duplas do shell (`declare -p`, `export -p`)."""
    return re.sub(r'([$`"\\])', r"\\\1", t)


def escapadas(v: bytes) -> set[bytes]:
    """As formas de D37 que diferem do valor literal (vazio se nenhuma muda nada)."""
    t = texto(v)
    if t is None:
        return set()
    formas = {json_go(t), json_ascii(t)}
    if "\n" not in t and "\r" not in t:
        formas.add(aspas_duplas(t))
    return {f.encode("utf-8") for f in formas} - {v}


CHAVE_IGUAL = re.compile(rb"^(?:export\s+)?[A-Za-z_.-][A-Za-z0-9_.-]*\s*=(.*)$")
CHAVE_JSON = re.compile(rb'^(?:-\s+)?"[^"]+"\s*:(.*)$')
CHAVE_YAML = re.compile(rb"^(?:-\s+)?[A-Za-z0-9_.-]+:(?:[ \t](.*))?$")


def valores_da_linha(aparada: bytes) -> list[bytes]:
    """O valor de `CHAVE=valor`, `"chave": "valor"` e `chave: valor` (D11, D36), sem a
    vírgula do fim no JSON/YAML, com e sem aspas, cada um com 6+ caracteres (D34)."""
    brutos = []
    for regex, virgula in ((CHAVE_IGUAL, False), (CHAVE_JSON, True), (CHAVE_YAML, True)):
        m = regex.match(aparada)
        if not m or m.group(1) is None:
            continue
        valor = m.group(1).strip()
        if virgula and valor.endswith(b","):
            valor = valor[:-1].rstrip()
        brutos.append(valor)
    saida = []
    for valor in brutos:
        formas = [valor]
        if len(valor) >= 3 and valor[:1] == valor[-1:] and valor[:1] in (b'"', b"'"):
            formas.append(valor[1:-1])
        saida += [x for x in formas if caracteres(x) >= MIN_PADRAO]
    return saida


def linhas_de_anexo(v: bytes) -> list[bytes]:
    """Passada por linha de um segredo de várias linhas (D11, D36): cada linha aparada
    com 6+ caracteres e os valores de `valores_da_linha`."""
    saida = []
    for linha in v.split(b"\n"):
        aparada = linha.rstrip(b"\r").strip()
        if caracteres(aparada) >= MIN_PADRAO:
            saida.append(aparada)
            saida += valores_da_linha(aparada)
    return saida


def todas(x: bytes) -> set[bytes]:
    """Codificações de um valor (o literal fica de fora: pode ter quebra de linha)."""
    return set(b64(x)) | set(miolos_b64(x)) | set(json_escape(x)) | escapadas(x) | set(url(x))


def variacoes(v: bytes, usuario: str | None) -> set[bytes]:
    pad: set[bytes] = set()
    inteiro = {v, v.rstrip(b"\r\n")} - {b""}
    for x in inteiro:
        if b"\n" not in x and b"\r" not in x:
            pad.add(x)
            if usuario:
                pad.update(b64(usuario.encode("utf-8") + b":" + x))
        pad |= todas(x)
    # Segredo de várias linhas (chave, anexo): cada linha e cada valor `CHAVE=valor`.
    if b"\n" in v or b"\r" in v:
        for linha in linhas_de_anexo(v):
            pad.add(linha)
            pad |= todas(linha)
    return pad


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


# --- Montagem ------------------------------------------------------------------------


def coletar(dados: dict) -> set[bytes]:
    pad: set[bytes] = set()
    for chave in ("senha_mestra", "pin"):
        pad |= variacoes(dados[chave].encode("utf-8"), None)
    for e in dados["entradas"]:
        usuario = e.get("campos", {}).get("usuario") or None
        for campo, valor in e.get("segredos", {}).items():
            if campo.endswith("_arquivo"):
                bruto = ler_do_repositorio(valor, BASE_CHAVES)
            else:
                bruto = valor.encode("utf-8")
            pad |= variacoes(bruto, usuario)
        for x in e.get("extras", []):
            if x.get("segredo") is True:
                pad |= variacoes(x["valor"].encode("utf-8"), usuario)
        for a in e.get("anexos", []):
            pad |= variacoes(ler_do_repositorio(a["arquivo"], BASE_ANEXOS), None)
        pad.update(database_url(e))
    return pad


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
    ap.add_argument("--saida", default=SAIDA_PADRAO)
    args = ap.parse_args()
    try:
        saida = validar_saida(args.saida or SAIDA_PADRAO)
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
    print(f"dev-variacoes: gravado {rel} (0600) com {len(lista)} padrões; uso: grep -F -f {rel}")


if __name__ == "__main__":
    main()
