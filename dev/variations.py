#!/usr/bin/python3 -IB
"""List of the test vault's secrets and of the variations of D11, D36, D37, D54, D58 and
D61 (Phases 19, 19d, 19e and 22a).

Usage (from the root):
    python3 -I -B dev/variations.py [--output .dev/<name>.txt]      # make dev-variations

The validators use the list with `grep -F -f .dev/variations.txt` (the contract's
"contains a secret"). One pattern per line, no empty line, no pattern shorter than 6
bytes; a form with a newline or a NUL does not fit on a line and is left out.

For each secret value of the fixtures (the secrets map, extras marked secret,
attachments, the private key in `.dev/services/keys/`, the master password and the
PIN), the single table of D54, the same one the mask implements (neither calls the
other):
1. whole secret: the literal; URL-encoded (`quote` with nothing safe, with `/` safe,
   `quote_plus`, upper and lower case hex, every byte encoded); standard and URL-safe
   base64, with and without `=`; the core of the value's base64 inside a larger base64
   (3 alignments); and the text forms;
2. Basic: base64 of `username:secret`, only for a secret of an entry field with a
   username;
3. per line (secret with `\\n` and attachment; D36, D54): each line with 6+ characters,
   without the trailing `\\r` and trimmed, URL-encoded and with its text forms, without
   base64 or cores (the base64 of a loose line stays in RA3); the 6+ fields of a text
   line with a tab; on a line that is not text, the 6+ byte tab fields only as the
   literal and URL-encoded (D61);
4. line value (`KEY=value`/`export KEY=value`, `"key": "value"` with a key of any
   character but `"` and also without spaces, `key: value`, `key : value`,
   `key:\\tvalue`, `- key: value`, and the tab field), with and without quotes, 6+:
   treated as a whole secret (1). Plus (D61): the trailing comment (`v # x`, `"v" # x`)
   of YAML and dotenv; the YAML key with a space or `/` (`private key: v`); the `''` of
   YAML single quotes; and the decoded string of each `"key": "string"` pair anywhere
   on a text line (also in a single-line secret that starts with `{` or `[`).
Whole attachment: the content, without the final newline, URL-encoded and base64.

Text forms (D37, D54), each only when it differs from the literal and only for text
(valid UTF-8): standard JSON escape, with and without `\\/`; Go JSON (`&`, `<`, `>`,
U+2028 and U+2029 as `\\u0026`, `\\u003c`, `\\u003e`, `\\u2028`, `\\u2029`); Python's
`ensure_ascii` (lower case `\\uXXXX`, with a surrogate pair above U+FFFF); PHP's
`json_encode` (`\\/` and `\\uXXXX` together, only with `/` and a non-ASCII character);
Go with `ensure_ascii` (D58: `\\uXXXX` for `&<>` and for every non-ASCII character, only
with both); shell double quotes (`\\` before `$`, `` ` ``, `"` and `\\`); shell single
quotes (`'` becomes `'\\''`, only with `'`); and bash 5.2 `printf %q` (only visible ASCII
and space). Upper case hex, bash `$'…'`, `expand`, `repr()`, `iconv`, the JSON array
element and YAML flow are left out (RA3). Plus the `DATABASE_URL` of database entries
(D10).

Independent of the mask code (Phases 17, 19c and 19e) on purpose: it is the oracle of
whoever verifies; it never imports or calls the core crate. Tests:
`dev/tests/variations_test.py` and the oracle → mask cross test of
`dev/tests/dev-vault.sh` (both run by `make test-dev-vault`). Standard library only.
Nothing goes to stdout/stderr besides the path and the count; the list is written 0600,
atomically, only inside `.dev/`. Never reads outside the repository.
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

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "dev" / "fixtures" / "entries.json"
DEV = ROOT / ".dev"
DEFAULT_OUTPUT = ".dev/variations.txt"
KEYS_BASE = ".dev/services/keys"
ATTACHMENTS_BASE = "dev/fixtures/attachments"
MIN_PATTERN = 6
MAX_FILE = 16 * 1024 * 1024
SCHEMES = {
    "postgres": "postgresql",
    "mysql": "mysql",
    "mongo": "mongodb",
    "redis": "redis",
    "sqlserver": "sqlserver",
}


class Refusal(Exception):
    """Error with fixed text (never with a secret value)."""


def fail(msg: str) -> None:
    print(f"dev-variations: {msg}", file=sys.stderr)
    sys.exit(1)


# --- Paths ---------------------------------------------------------------------------


def lexical(requested: str) -> PurePosixPath:
    """Absolute path from the text alone; relative means relative to the root; `..` is
    refused."""
    p = PurePosixPath(requested)
    if ".." in p.parts:
        raise Refusal("não pode ter '..'")
    if not p.is_absolute():
        p = PurePosixPath(ROOT) / p
    return PurePosixPath(*[c for c in p.parts if c != "."])


def no_links(stop: PurePosixPath, start: PurePosixPath) -> None:
    """Every directory between `start` (exclusive) and `stop` (exclusive) is a real
    directory."""
    current = Path(start)
    for part in stop.relative_to(start).parts[:-1]:
        current = current / part
        try:
            m = os.lstat(current)
        except FileNotFoundError:
            raise Refusal("diretório do caminho não existe") from None
        if not stat.S_ISDIR(m.st_mode):
            raise Refusal("um diretório do caminho é link simbólico")


def validate_output(requested: str) -> Path:
    """Output only inside `<root>/.dev/`, named `*.txt`, without a symbolic link (S15)."""
    abs_ = lexical(requested)
    dev = PurePosixPath(DEV)
    # 1) Text only: nothing outside the repository is looked up on disk.
    try:
        rel = abs_.relative_to(dev)
    except ValueError:
        raise Refusal("tem de ficar dentro de .dev/ do repositório") from None
    if not rel.parts or rel.name.startswith(".") or not rel.name.endswith(".txt") or rel.name == ".txt":
        raise Refusal("o nome tem de terminar em .txt e não começar com '.'")
    # 2) Disk.
    try:
        m = os.lstat(DEV)
    except FileNotFoundError:
        os.mkdir(DEV, 0o700)
        m = os.lstat(DEV)
    if not stat.S_ISDIR(m.st_mode) or Path(os.path.realpath(DEV)) != DEV:
        raise Refusal(".dev não é um diretório da raiz (link simbólico?)")
    no_links(abs_, dev)
    try:
        m = os.lstat(abs_)
        if not stat.S_ISREG(m.st_mode):
            raise Refusal("o destino existe e não é arquivo comum")
    except FileNotFoundError:
        pass
    return Path(abs_)


def read_from_repo(path: str, base_dir: str) -> bytes:
    """File named in the fixtures: inside `base_dir`, no link, regular file."""
    if PurePosixPath(path).is_absolute():
        raise Refusal(f"{path}: tem de ser relativo à raiz")
    abs_ = lexical(path)
    try:
        abs_.relative_to(PurePosixPath(ROOT) / base_dir)
    except ValueError:
        raise Refusal(f"{path}: fora da pasta permitida") from None
    try:
        no_links(abs_, PurePosixPath(ROOT))
        fd = os.open(abs_, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    except (Refusal, FileNotFoundError):
        hint = "; rode make dev-services antes" if base_dir == KEYS_BASE else ""
        raise Refusal(f"{path}: não existe{hint}") from None
    except OSError:
        raise Refusal(f"{path}: não é arquivo comum (link simbólico?)") from None
    try:
        m = os.fstat(fd)
        if not stat.S_ISREG(m.st_mode) or m.st_size > MAX_FILE:
            raise Refusal(f"{path}: não é arquivo comum ou é grande demais")
        with os.fdopen(fd, "rb", closefd=False) as f:
            return f.read(MAX_FILE + 1)
    finally:
        os.close(fd)


def write_file(path: Path, content: bytes) -> None:
    """Writes 0600 atomically (temp O_EXCL|O_NOFOLLOW + fsync + rename)."""
    temp = path.with_name(f".{path.name}.tmp")
    try:
        if stat.S_ISREG(os.lstat(temp).st_mode):
            os.unlink(temp)
    except FileNotFoundError:
        pass
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    try:
        os.fchmod(fd, 0o600)
        os.write(fd, content)
        os.fsync(fd)
    finally:
        os.close(fd)
    os.replace(temp, path)
    dfd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(dfd)
    finally:
        os.close(dfd)


# --- Variations ----------------------------------------------------------------------
#
# The single table of D54. Written here from the text of the PRD (D11, D34, D36, D37,
# D54), not from the mask code:
#   1. whole secret: literal, base64 (standard and URL-safe, with and without `=`),
#      base64 cores (3 alignments), percent and the text forms;
#   2. Basic: base64 of `username:secret` (only a secret of an entry field with a
#      username);
#   3. per line (multi-line secret and attachment): the 6+ line (without the trailing
#      `\r` and trimmed), percent and its text forms, without base64 or cores; the 6+
#      fields of a line with a tab;
#   4. line value (`KEY=value`, `"key": "value"`, `key: value`, tab field), with and
#      without quotes, 6+: treated as a whole secret (1).
# Whole attachment: the content, without the final newline, base64 and percent.
# Text forms and tab fields only for text (valid UTF-8).

BACKSLASH = chr(92)
# ASCII whitespace at the ends (the same set in both implementations: space, tab, LF,
# FF, CR; VT stays).
WHITESPACE = b" \t\n\x0c\r"


def b64(v: bytes) -> list[bytes]:
    p = base64.b64encode(v)
    u = base64.urlsafe_b64encode(v)
    return [p, p.rstrip(b"="), u, u.rstrip(b"=")]


def b64_cores(v: bytes) -> list[bytes]:
    """The part of the base64 that depends only on `v` when `v` is inside a larger
    base64, with 0, 1 or 2 bytes before it (and anything after)."""
    if len(v) < 6:
        return []
    out = []
    for k in (0, 1, 2):
        total = k + len(v)
        begin = (8 * k + 5) // 6  # first character with no bit of the prefix
        end = (8 * total) // 6  # last full character with only bits of `v`
        for encode in (base64.b64encode, base64.urlsafe_b64encode):
            m = encode(b"\0" * k + v)[begin:end]
            if len(m) >= 8:
                out.append(m)
    return out


def lowercase_hex(s: str) -> str:
    return re.sub(r"%[0-9A-F]{2}", lambda m: m.group(0).lower(), s)


def url(v: bytes) -> list[bytes]:
    forms = [quote(v, safe=""), quote(v, safe="/"), quote_plus(v, safe="")]
    forms += [lowercase_hex(f) for f in forms]
    every_byte = "".join(f"%{b:02X}" for b in v)
    forms += [every_byte, every_byte.lower()]
    return [f.encode("ascii") for f in forms]


def as_text(v: bytes) -> str | None:
    try:
        return v.decode("utf-8")
    except UnicodeDecodeError:
        return None


def char_count(v: bytes) -> int:
    """Characters of valid UTF-8; bytes if it is not text (the minimum of D34/D36)."""
    t = as_text(v)
    return len(v) if t is None else len(t)


def json_escape(v: bytes) -> list[bytes]:
    """Standard JSON escape, with and without `\\/`."""
    t = as_text(v)
    if t is None:
        return []
    j = json.dumps(t, ensure_ascii=False)[1:-1]
    return [x.encode("utf-8") for x in {j, j.replace("/", BACKSLASH + "/")}]


# D37. Each form is written here from the PRD description, not from the mask code.
GO_HTML = {
    "&": BACKSLASH + "u0026",
    "<": BACKSLASH + "u003c",
    ">": BACKSLASH + "u003e",
    chr(0x2028): BACKSLASH + "u2028",
    chr(0x2029): BACKSLASH + "u2029",
}


def json_go(t: str) -> str:
    """Go's `json.Marshal`: the usual JSON escape plus `&<>` and U+2028/U+2029 as `\\u`."""
    j = json.dumps(t, ensure_ascii=False)[1:-1]
    return "".join(GO_HTML.get(c, c) for c in j)


def json_ascii(t: str) -> str:
    """Python's `json.dumps` (`ensure_ascii`): non-ASCII becomes lower case `\\uXXXX`,
    with a surrogate pair above U+FFFF."""
    return json.dumps(t, ensure_ascii=True)[1:-1]


def json_php(t: str) -> str | None:
    """PHP's `json_encode` (D54): `/` as `\\/` and every non-ASCII character as lower
    case `\\uXXXX`. Only when the value has both."""
    if "/" not in t or all(ord(c) < 0x80 for c in t):
        return None
    parts = []
    for c in t:
        if ord(c) >= 0x80:
            parts.append(json.dumps(c, ensure_ascii=True)[1:-1])
        elif c == "/":
            parts.append(BACKSLASH + "/")
        else:
            parts.append(json.dumps(c, ensure_ascii=False)[1:-1])
    return "".join(parts)


def json_go_ascii(t: str) -> str | None:
    """"html-safe" JSON with `ensure_ascii` (D58): Python's default `json.dumps` with `&`,
    `<`, `>` also as `\\uXXXX`. Only when the value has `&`, `<`, `>`, U+2028 or U+2029
    and a non-ASCII character."""
    if not any(c in GO_HTML for c in t) or all(ord(c) < 0x80 for c in t):
        return None
    j = json.dumps(t, ensure_ascii=True)[1:-1]
    return "".join(GO_HTML.get(c, c) for c in j)


def double_quoted(t: str) -> str:
    """Value inside shell double quotes (`declare -p`, `export -p`)."""
    return re.sub(r'([$`"\\])', r"\\\1", t)


def single_quoted(t: str) -> str | None:
    """Value inside shell single quotes (`${X@Q}`, `set`, `set -x`, D54): each `'`
    becomes `'\\''`. Only when the value has `'`."""
    if "'" not in t:
        return None
    return t.replace("'", "'" + BACKSLASH + "''")


# Bash 5.2 `printf %q` (D54): a backslash before these always; `#` only at the start;
# `~` at the start or after `=` or `:`. Only for visible ASCII and space (the rest goes
# to `$'…'`, RA3). Verified against the real bash in `dev/tests/variations_test.py`.
PRINTF_Q_ALWAYS = set(" !\"$&'()*,;<>?[]^`{|}" + BACKSLASH)


def printf_q(t: str) -> str | None:
    if not t or any(not (0x20 <= ord(c) <= 0x7E) for c in t):
        return None
    out = []
    for i, c in enumerate(t):
        if c in PRINTF_Q_ALWAYS or (c == "#" and i == 0) or (c == "~" and (i == 0 or t[i - 1] in "=:")):
            out.append(BACKSLASH)
        out.append(c)
    return "".join(out)


def escaped_forms(v: bytes) -> set[bytes]:
    """The text forms of D37 and D54 that differ from the literal value (empty if none
    changes anything): standard JSON escape (with and without `\\/`), Go JSON,
    `ensure_ascii`, PHP, double quotes, single quotes and `printf %q`. Only for text."""
    t = as_text(v)
    if t is None:
        return set()
    forms = {json_go(t), json_ascii(t), double_quoted(t)}
    for extra in (json_php(t), json_go_ascii(t), single_quoted(t), printf_q(t)):
        if extra is not None:
            forms.add(extra)
    return ({f.encode("utf-8") for f in forms} | set(json_escape(v))) - {v}


def whole(x: bytes) -> set[bytes]:
    """Row 1 of the table: the whole secret and its variations."""
    if not x:
        return set()
    return {x} | set(b64(x)) | set(b64_cores(x)) | set(url(x)) | escaped_forms(x)


def basic(username: str, x: bytes) -> set[bytes]:
    """Row 2: base64 of `username:secret`."""
    return set(b64(username.encode("utf-8") + b":" + x)) if x else set()


def trim(v: bytes) -> bytes:
    return v.strip(WHITESPACE)


KEY_EQUALS = re.compile(rb"^(?:export\s+)?[A-Za-z_.-][A-Za-z0-9_.-]*\s*=(.*)$")
KEY_JSON = re.compile(rb'^(?:- [ \t\n\x0c\r]*)?"[^"]+"[ \t\n\x0c\r]*:(.*)$')
KEY_YAML = re.compile(rb"^(?:- [ \t\n\x0c\r]*)?[A-Za-z0-9_.-]+[ \t\n\x0c\r]*:(?:[ \t](.*))?$")
# D61: YAML key with a space or `/`; the key runs up to the first `:` followed by a
# space, a tab or the end, without `#` or quotes, and does not start with `-`, `{` or
# `[`.
KEY_YAML_COMPOSITE = re.compile(rb"""^(?:- [ \t\n\x0c\r]*)?[^#"'{\[\- \t\n\x0c\r][^#"']*?:(?:[ \t](.*))?$""", re.S)
# D61: trailing comment. Without quotes: up to before the first space/tab followed by
# `#`. Within quotes: the inside up to the first matching quote followed by spaces and
# `#`.
COMMENT = re.compile(rb"[ \t]#")
QUOTED_COMMENT = re.compile(rb"""^(["'])(.*?)\1[ \t]+#""", re.S)


def line_values(trimmed: bytes) -> list[bytes]:
    """The value of `KEY=value`, `"key": "value"` (also without spaces `"k":"v"` and with
    a key of any character but `"`) and `key: value` (also `key : value` and
    `key:\\tvalue`) (D11, D36, D54), without the trailing comma in JSON/YAML, with and
    without quotes, each with 6+ characters (D34)."""
    trimmed = trim(trimmed)
    raws = []
    for regex, comma, yaml in (
        (KEY_EQUALS, False, False),
        (KEY_JSON, True, True),
        (KEY_YAML, True, True),
        (KEY_YAML_COMPOSITE, True, True),
    ):
        m = regex.match(trimmed)
        if not m or m.group(1) is None:
            continue
        value = trim(m.group(1))
        if comma and value.endswith(b","):
            value = trim(value[:-1])
        raws.append((value, yaml))
    out = []
    for value, yaml in raws:
        forms = [value]
        if len(value) >= 3 and value[:1] == value[-1:] and value[:1] in (b'"', b"'"):
            forms.append(value[1:-1])
        forms += comment_and_quotes(value, yaml)
        out += [x for x in forms if char_count(x) >= MIN_PATTERN]
    return out


def comment_and_quotes(value: bytes, yaml: bool) -> list[bytes]:
    """The extra candidates of D61 for a line value: the value before the trailing
    comment (with or without quotes) and, in YAML, `''` replaced by `'` in a single
    quoted value."""
    if not value:
        return []
    if value[:1] not in (b'"', b"'"):
        m = COMMENT.search(value)
        return [trim(value[: m.start()])] if m else []
    out = []
    m = QUOTED_COMMENT.match(value)
    core = m.group(2) if m else None
    if core is not None:
        out.append(core)
    if yaml and value[:1] == b"'":
        if core is None and len(value) >= 3 and value[-1:] == b"'":
            core = value[1:-1]
        if core is not None and b"''" in core:
            out.append(core.replace(b"''", b"'"))
    return out


STRING_JSON = re.compile(rb'"(?:[^"\\]|\\.)*"', re.S)


def json_values(line: bytes) -> list[bytes]:
    """D61: each `"key"` + spaces + `:` + spaces + JSON string pair, anywhere on the text
    line, gives the decoded string (an invalid escape or a lone surrogate discards the
    pair), with 6+ characters. The search walks from string to string: after a pair, it
    goes on from the end of the value; after a string that is not a key, from its end."""
    if as_text(line) is None:
        return []
    out = []
    i = 0
    while True:
        i = line.find(b'"', i)
        if i < 0:
            break
        key = STRING_JSON.match(line, i)
        if not key:
            break
        j = key.end()
        while line[j : j + 1] in (b" ", b"\t"):
            j += 1
        if line[j : j + 1] != b":":
            i = key.end()
            continue
        j += 1
        while line[j : j + 1] in (b" ", b"\t"):
            j += 1
        if line[j : j + 1] != b'"':
            i = key.end()
            continue
        value = STRING_JSON.match(line, j)
        if not value:
            break
        try:
            d = json.loads(value.group(0).decode("utf-8"), strict=False).encode("utf-8")
        except (ValueError, UnicodeEncodeError):
            d = None
        if d is not None and char_count(d) >= MIN_PATTERN:
            out.append(d)
        i = value.end()
    return out


def binary_tab_fields(line: bytes) -> list[bytes]:
    """D61: the fields of a line with a tab that is not text, trimmed and raw, with 6+
    bytes. Literal only (and URL-encoded): no text forms and no base64."""
    if b"\t" not in line or as_text(line) is not None:
        return []
    out = []
    for field in line.split(b"\t"):
        out += [x for x in (trim(field), field) if len(x) >= MIN_PATTERN]
    return out


def tab_fields(line: bytes) -> list[bytes]:
    """The fields of a text line with a tab (`cut -f`, TSV; D54), trimmed, with and
    without quotes, 6+."""
    if b"\t" not in line or as_text(line) is None:
        return []
    out = []
    for field in line.split(b"\t"):
        field = trim(field)
        forms = [field]
        if len(field) >= 3 and field[:1] == field[-1:] and field[:1] in (b'"', b"'"):
            forms.append(field[1:-1])
        out += [x for x in forms if char_count(x) >= MIN_PATTERN]
    return out


def attachment_lines(v: bytes) -> list[bytes]:
    """The 6+ character lines of a multi-line secret (without the trailing `\\r` and
    trimmed; D11, D36) and their line values (`line_values`, tab fields). The line goes
    in as a line (text forms, no base64); the value, as a whole secret."""
    lines, values, raw_fields = per_line(v)
    return sorted(lines | values | raw_fields)


def per_line(v: bytes) -> tuple[set[bytes], set[bytes], set[bytes]]:
    """Row 3 (the lines), row 4 (the values) of the table and the tab fields of a line
    that is not text (literal only, D61)."""
    lines: set[bytes] = set()
    values: set[bytes] = set()
    raw_fields: set[bytes] = set()
    for line in v.split(b"\n"):
        if line.endswith(b"\r"):
            line = line[:-1]
        if char_count(line) < MIN_PATTERN:
            continue
        lines.add(line)
        trimmed = trim(line)
        if char_count(trimmed) >= MIN_PATTERN:
            lines.add(trimmed)
        values.update(tab_fields(line))
        values.update(line_values(line))
        values.update(json_values(line))
        raw_fields.update(binary_tab_fields(line))
    return lines, values, raw_fields


def line_patterns(v: bytes) -> set[bytes]:
    """The variations of the per-line pass (rows 3 and 4)."""
    lines, values, raw_fields = per_line(v)
    pad: set[bytes] = set()
    for line in lines:
        pad.add(line)
        pad |= set(url(line)) | escaped_forms(line)
    for field in raw_fields:
        pad.add(field)
        pad |= set(url(field))
    for value in values:
        pad |= whole(value)
    return pad


def variations(v: bytes, username: str | None, attachment: bool = False) -> set[bytes]:
    """The variations of a secret value that fit on a line of the list. `username` (a
    secret of an entry field with a username) adds the Basic; `attachment` replaces row
    1 by the whole attachment."""
    pad: set[bytes] = set()
    if attachment:
        no_newline = v[:-1] if v.endswith(b"\n") else v
        no_newline = no_newline[:-1] if no_newline.endswith(b"\r") else no_newline
        for x in {v, no_newline} - {b""}:
            pad.add(x)
            pad |= set(url(x))
        pad |= set(b64(v))
        pad |= line_patterns(v)
    else:
        pad |= whole(v)
        if username:
            pad |= basic(username, v)
        if b"\n" in v:
            pad |= line_patterns(v)
        elif trim(v)[:1] in (b"{", b"["):
            # D61: single-line JSON (`{"token": "…"}` in an extra).
            for value in json_values(v):
                pad |= whole(value)
    return {p for p in pad if listable(p)}


def database_url(e: dict) -> list[bytes]:
    c, s = e.get("campos", {}), e.get("segredos", {})
    scheme = SCHEMES.get(c.get("motor", ""))
    password = s.get("senha")
    if not scheme or not password:
        return []
    username = quote(c.get("usuario", ""), safe="")
    encoded = quote(password, safe="")
    host, port, db_name = c.get("host", ""), c.get("porta", ""), c.get("base", "")
    target = f"{scheme}://{username}:{encoded}@{host}:{port}/{db_name}"
    # Without host/port (the tunnel swaps them for the local ones): the piece with the
    # password.
    return [target.encode(), f"{scheme}://{username}:{encoded}@".encode()]


def listable(p: bytes) -> bool:
    """A pattern that fits on a line of `grep -F -f` (no newline and no NUL)."""
    return b"\n" not in p and b"\r" not in p and b"\0" not in p


# --- Assembly ------------------------------------------------------------------------


def entry_patterns(e: dict) -> set[bytes]:
    """The variations of the secrets of a fixtures entry, without the `DATABASE_URL`
    (which comes from the delivery, with the host and port of whoever runs it) and only
    the ones that fit on a line."""
    pad: set[bytes] = set()
    username = e.get("campos", {}).get("usuario") or None
    for field, value in e.get("segredos", {}).items():
        if field.endswith("_arquivo"):
            raw = read_from_repo(value, KEYS_BASE)
        else:
            raw = value.encode("utf-8")
        pad |= variations(raw, username)
    for x in e.get("extras", []):
        if x.get("segredo") is True:
            pad |= variations(x["valor"].encode("utf-8"), None)
    for a in e.get("anexos", []):
        pad |= variations(read_from_repo(a["arquivo"], ATTACHMENTS_BASE), None, attachment=True)
    return {p for p in pad if listable(p)}


def collect(data: dict) -> set[bytes]:
    pad: set[bytes] = set()
    for key in ("senha_mestra", "pin"):
        pad |= variations(data[key].encode("utf-8"), None)
    for e in data["entradas"]:
        pad |= entry_patterns(e)
        pad.update(database_url(e))
    return {p for p in pad if listable(p)}


def verify_patterns(pad: set[bytes]) -> list[bytes]:
    ordered = sorted(pad)
    for p in ordered:
        # An empty or short line would make `grep -F -f` match almost anything; a
        # newline would split the pattern in two.
        if len(p) < MIN_PATTERN or b"\n" in p or b"\r" in p or b"\0" in p:
            raise Refusal("padrão curto ou com quebra de linha; confira as fixtures")
    return ordered


def main() -> None:
    ap = argparse.ArgumentParser(description="Lista de segredos e variações do cofre de teste.")
    ap.add_argument("--output", default=DEFAULT_OUTPUT)
    args = ap.parse_args()
    try:
        out = validate_output(args.output or DEFAULT_OUTPUT)
    except Refusal as e:
        fail(f"caminho de saída recusado: {e}")
    try:
        if os.lstat(FIXTURES).st_size > 1024 * 1024 or not FIXTURES.is_file() or FIXTURES.is_symlink():
            raise Refusal("fixtures ausentes ou grandes demais")
        data = json.loads(FIXTURES.read_text(encoding="utf-8"))
        if data.get("versao") != 1:
            raise Refusal("versão das fixtures desconhecida")
        ordered = verify_patterns(collect(data))
        write_file(out, b"\n".join(ordered) + b"\n")
    except Refusal as e:
        fail(str(e))
    except (KeyError, TypeError, AttributeError, ValueError):
        fail("fixtures com forma inesperada")
    rel = out.relative_to(ROOT)
    print(f"dev-variations: gravado {rel} (0600) com {len(ordered)} padrões; uso: grep -F -f {rel}")


if __name__ == "__main__":
    main()
