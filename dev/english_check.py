#!/usr/bin/python3 -IB
"""Portuguese leftovers checker (m2:DT17; mission 2, Phase 2).

Reads the files of the repository (tracked plus untracked and not ignored) outside
the documentation (`docs/`, `README.md`, `CLAUDE.md`, `skill/**/*.md`, `.claude/`),
the lockfiles and the binaries. Each language is split into comments, identifiers
and literals by a small lexer of its own. Comments, identifiers and the names of
files and folders are checked; literals never are (messages shown to the user stay
in pt-BR, m2:PRD rule 5).

A comment or identifier is flagged when it has:
  (a) a letter outside ASCII;
  (b) a word of `dev/english-check/pt-words.txt`: a `[stems]` prefix, a `[words]`
      word, or (identifiers only) a `[function]` word as a segment. In a comment, a
      sentence is flagged when it has two `[function]` words, or one `[function]`
      word plus one `[ambiguous]` word, so a lone "a" or "de" in English text is
      never flagged.

Exceptions (one per line, `#` starts a comment, fields split by whitespace):
  dev/english-check/allow.txt   <path-glob> <word-or-identifier> <reason...>
  dev/english-check/frozen.txt  <path-glob> <word-or-identifier> <phase> <reason...>
`allow.txt` is permanent; `frozen.txt` holds wire names frozen until the phase that
changes them (m2:DT3). The glob follows `fnmatch` (`*` also crosses `/`). An
exception that matches no finding fails the run (reported as unused); with `--paths`
only the exceptions whose glob matches a checked file are held to that.

Usage (always with `python3 -I`; only the standard library):
  dev/english_check.py                      whole repository
  dev/english_check.py --paths P [P ...]    only these files or folders (phases)
  dev/english_check.py --literals [--with-location] [--paths ...]
                                            print the literals, one JSON string per
                                            line, sorted and unique (baseline, DT18)
  dev/english_check.py --self-test          plant violations in a copy in .dev/
Exit: 0 clean, 1 findings or unused exceptions, 2 usage or internal error.

Security (m2:V1.4, S14): never follows a symbolic link (links are only checked by
name; files are opened with O_NOFOLLOW and must be regular files whose folder
resolves inside the repository), never runs anything but `git ls-files` (with the
user's git configuration and hooks turned off), and never writes outside `.dev/`.
"""

import argparse
import bisect
import fnmatch
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import unicodedata

ROOT = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
LIST_DIR = "dev/english-check"
PT_WORDS = LIST_DIR + "/pt-words.txt"
ALLOW = LIST_DIR + "/allow.txt"
FROZEN = LIST_DIR + "/frozen.txt"
MAX_FILE = 4 * 1024 * 1024

# Neither content nor name is checked (pt-BR documentation, m2:PRD rule 1).
DOC_PREFIXES = ("docs/", ".claude/")
DOC_FILES = ("README.md", "CLAUDE.md")
# Content is not checked, the name is.
DATA_PREFIXES = (LIST_DIR + "/", "dev/fixtures/legacy/")
LOCKFILES = ("Cargo.lock", "package-lock.json")
BINARY_EXT = {
    ".png", ".jpg", ".jpeg", ".gif", ".ico", ".icns", ".webp", ".woff", ".woff2",
    ".ttf", ".otf", ".pdf", ".gz", ".zip", ".tar", ".crx", ".deb", ".ward", ".pem",
    ".jsonl", ".figcraft",
}

# --------------------------------------------------------------------------------
# Word lists


class Words:
    def __init__(self, text):
        self.stems, self.words, self.function, self.ambiguous = [], set(), set(), set()
        section = None
        for raw in text.splitlines():
            line = raw.split("#", 1)[0].strip()
            if not line:
                continue
            if line.startswith("[") and line.endswith("]"):
                section = line[1:-1]
                continue
            word = line.lower()
            if section == "stems":
                self.stems.append(word)
            elif section == "words":
                self.words.add(word)
            elif section == "function":
                self.function.add(word)
            elif section == "ambiguous":
                self.ambiguous.add(word)
        self.stems.sort()
        self._stem_re = re.compile("|".join(re.escape(s) for s in self.stems)) if self.stems else None

    def domain(self, word):
        """True when the word is a Portuguese domain word (stem or exact word)."""
        if word in self.words:
            return True
        return bool(self._stem_re and self._stem_re.match(word))

    def ident_hit(self, word):
        if self.domain(word) or word in self.function:
            return True
        # Svelte event props: "on" + verb (onfechar).
        if word.startswith("on") and len(word) > 4:
            rest = word[2:]
            return self.domain(rest) or rest in self.function
        return False


def fold(text):
    """Lowercase without accents (NFKD minus the combining marks)."""
    decomposed = unicodedata.normalize("NFKD", text)
    return "".join(c for c in decomposed if not unicodedata.combining(c)).lower()


_CHUNK = re.compile(r"[^\W_]+")
_CAMEL = re.compile(r"[A-Z]+(?=[A-Z][a-z])|[A-Z]?[a-z]+|[A-Z]+|[0-9]+|[^\W\d_]+")


def split_words(text):
    """Yields (word, offset) for snake, kebab, camel and SCREAMING case, lowercased
    and without accents."""
    for chunk in _CHUNK.finditer(text):
        base = chunk.start()
        folded = unicodedata.normalize("NFKD", chunk.group())
        if len(folded) != len(chunk.group()):
            # Accents change the length: keep the chunk offset for every piece.
            plain = "".join(c for c in folded if not unicodedata.combining(c))
            for piece in _CAMEL.finditer(plain):
                if not piece.group().isdigit():
                    yield piece.group().lower(), base
            continue
        for piece in _CAMEL.finditer(chunk.group()):
            if not piece.group().isdigit():
                yield piece.group().lower(), base + piece.start()


def non_ascii_letters(text):
    return [i for i, c in enumerate(text) if ord(c) > 127 and c.isalpha()]


# --------------------------------------------------------------------------------
# Lexers. Each one calls emit(kind, offset, text) with kind in
# "comment", "ident", "literal"; everything else is skipped.

def is_ident_start(c):
    return c == "_" or c.isalpha()


def is_ident_char(c):
    return c == "_" or c.isalnum()


def scan_ident(src, i, end, extra=""):
    j = i
    while j < end and (is_ident_char(src[j]) or src[j] in extra):
        j += 1
    return j


def scan_quoted(src, i, end, quote, escapes=True, multiline=True):
    """i points after the opening quote; returns the index of the closing quote
    (or end)."""
    j = i
    while j < end:
        c = src[j]
        if escapes and c == "\\":
            j += 2
            continue
        if c == quote:
            return j
        if c == "\n" and not multiline:
            return j
        j += 1
    return end


def skip_line(src, i, end):
    k = src.find("\n", i, end)
    return end if k < 0 else k


# ---- Rust and C


RUST_STR = re.compile(r'(br|cr|b|c|r)(#*)"')


def lex_rust(src, i, end, emit, c_lang=False):
    while i < end:
        c = src[i]
        nxt = src[i + 1] if i + 1 < end else ""
        if c == "/" and nxt == "/":
            k = skip_line(src, i, end)
            emit("comment", i + 2, src[i + 2:k])
            i = k
        elif c == "/" and nxt == "*":
            depth, j = 1, i + 2
            while j < end and depth:
                if src.startswith("/*", j) and not c_lang:
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            emit("comment", i + 2, src[i + 2:max(i + 2, j - 2)])
            i = j
        elif c == '"':
            k = scan_quoted(src, i + 1, end, '"')
            emit("literal", i + 1, src[i + 1:k])
            i = k + 1
        elif c == "'":
            if nxt == "\\":
                k = src.find("'", i + 3, end)
                k = end if k < 0 else k
                emit("literal", i + 1, src[i + 1:k])
                i = k + 1
            elif i + 2 < end and src[i + 2] == "'":
                emit("literal", i + 1, src[i + 1])
                i += 3
            elif c_lang:
                k = scan_quoted(src, i + 1, end, "'", multiline=False)
                emit("literal", i + 1, src[i + 1:k])
                i = k + 1
            else:
                j = scan_ident(src, i + 1, end)
                if j > i + 1:
                    emit("ident", i + 1, src[i + 1:j])
                i = max(j, i + 1)
        elif is_ident_start(c):
            m = None if c_lang else RUST_STR.match(src, i, end)
            if m:
                hashes = m.group(2)
                start = m.end()
                if "r" in m.group(1):
                    k = src.find('"' + hashes, start, end)
                    k = end if k < 0 else k
                    emit("literal", start, src[start:k])
                    i = k + 1 + len(hashes)
                else:
                    k = scan_quoted(src, start, end, '"')
                    emit("literal", start, src[start:k])
                    i = k + 1
                continue
            if not c_lang and c == "b" and nxt == "'":
                k = scan_quoted(src, i + 2, end, "'", multiline=False)
                emit("literal", i + 2, src[i + 2:k])
                i = k + 1
                continue
            if not c_lang and c == "r" and nxt == "#" and i + 2 < end and is_ident_start(src[i + 2]):
                i += 2
                continue
            j = scan_ident(src, i, end)
            emit("ident", i, src[i:j])
            i = j
        elif c.isdigit():
            i = scan_ident(src, i, end, ".")
        else:
            i += 1
    return end


def lex_c(src, i, end, emit):
    return lex_rust(src, i, end, emit, c_lang=True)


# ---- JavaScript (also inside Svelte)

JS_REGEX_KW = {
    "return", "typeof", "instanceof", "in", "of", "new", "delete", "void", "throw",
    "case", "do", "else", "yield", "await",
}


def lex_js(src, i, end, emit, stop=None):
    """Returns the index of the unmatched `stop` brace, or end."""
    depth = 0
    last = None  # None, "value", "kw" or the punctuation character
    while i < end:
        c = src[i]
        nxt = src[i + 1] if i + 1 < end else ""
        if c in " \t\r\n":
            i += 1
        elif c == "/" and nxt == "/":
            k = skip_line(src, i, end)
            emit("comment", i + 2, src[i + 2:k])
            i = k
        elif c == "/" and nxt == "*":
            k = src.find("*/", i + 2, end)
            k = end if k < 0 else k
            emit("comment", i + 2, src[i + 2:k])
            i = k + 2
        elif c in "'\"":
            k = scan_quoted(src, i + 1, end, c, multiline=False)
            emit("literal", i + 1, src[i + 1:k])
            i, last = k + 1, "value"
        elif c == "`":
            j = i + 1
            piece = j
            while j < end and src[j] != "`":
                if src[j] == "\\":
                    j += 2
                elif src.startswith("${", j):
                    emit("literal", piece, src[piece:j])
                    j = lex_js(src, j + 2, end, emit, stop="}") + 1
                    piece = j
                else:
                    j += 1
            emit("literal", piece, src[piece:min(j, end)])
            i, last = j + 1, "value"
        elif c == "/" and (last is None or last == "kw" or (last not in ("value",) and last in "(,=:[!&|?{};+-*%<>~^")):
            j, in_class = i + 1, False
            while j < end and src[j] != "\n":
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == "[":
                    in_class = True
                elif src[j] == "]":
                    in_class = False
                elif src[j] == "/" and not in_class:
                    break
                j += 1
            i = scan_ident(src, j + 1, end)
            last = "value"
        elif c == "#" and is_ident_start(nxt):
            j = scan_ident(src, i + 1, end, "$")
            emit("ident", i + 1, src[i + 1:j])
            i, last = j, "value"
        elif is_ident_start(c) or c == "$":
            j = scan_ident(src, i, end, "$")
            word = src[i:j]
            emit("ident", i, word)
            i, last = j, ("kw" if word in JS_REGEX_KW else "value")
        elif c.isdigit():
            i, last = scan_ident(src, i, end, "."), "value"
        elif c == "{":
            depth += 1
            i, last = i + 1, "{"
        elif c == "}":
            if depth == 0 and stop == "}":
                return i
            depth -= 1
            i, last = i + 1, "}"
        elif c in ")]":
            i, last = i + 1, "value"
        else:
            i, last = i + 1, c
    return end


# ---- CSS

HEX_COLOR = re.compile(r"#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?![\w-])")


def lex_css(src, i, end, emit):
    while i < end:
        c = src[i]
        nxt = src[i + 1] if i + 1 < end else ""
        if c == "/" and nxt == "*":
            k = src.find("*/", i + 2, end)
            k = end if k < 0 else k
            emit("comment", i + 2, src[i + 2:k])
            i = k + 2
        elif c in "'\"":
            k = scan_quoted(src, i + 1, end, c, multiline=False)
            emit("literal", i + 1, src[i + 1:k])
            i = k + 1
        elif src.startswith("url(", i):
            k = src.find(")", i, end)
            i = end if k < 0 else k + 1
        elif c.isdigit() or (c in ".-" and nxt.isdigit()):
            j = i + 1
            while j < end and (src[j].isalnum() or src[j] in ".%"):
                j += 1
            i = j
        elif c == "#":
            m = HEX_COLOR.match(src, i, end)
            if m:
                i = m.end()
            else:
                j = scan_ident(src, i + 1, end, "-")
                if j > i + 1:
                    emit("ident", i + 1, src[i + 1:j])
                i = max(j, i + 1)
        elif is_ident_start(c) or (c == "-" and (is_ident_start(nxt) or nxt == "-")):
            j = scan_ident(src, i, end, "-")
            emit("ident", i, src[i:j])
            i = j
        else:
            i += 1
    return end


# ---- Markup: Svelte, HTML, XML, SVG

TAG_NAME = re.compile(r"[A-Za-z][\w:.\-]*")


def _emit_text(emit, src, i, j):
    text = src[i:j]
    if text.strip():
        lead = len(text) - len(text.lstrip())
        emit("literal", i + lead, " ".join(text.split()))


def _attr_static(emit, src, name, i, j):
    if i >= j:
        return
    if name in ("class", "id"):
        for m in re.finditer(r"[^\s]+", src[i:j]):
            emit("ident", i + m.start(), m.group())
    elif name == "style":
        lex_css(src, i, j, emit)
    else:
        _emit_text(emit, src, i, j)


def _lex_tag(src, i, end, emit, svelte):
    """i points after '<' (and after '/' of a closing tag). Returns (index after '>',
    tag name)."""
    m = TAG_NAME.match(src, i, end)
    name = ""
    if m:
        name = m.group().lower()
        emit("ident", i, m.group())
        i = m.end()
    while i < end:
        c = src[i]
        if c in " \t\r\n/":
            i += 1
        elif c == ">":
            return i + 1, name
        elif svelte and c == "{":
            i = lex_js(src, i + 1, end, emit, stop="}") + 1
        else:
            j = i
            while j < end and src[j] not in " \t\r\n=>/" and not (svelte and src[j] == "{"):
                j += 1
            attr = src[i:j]
            if attr:
                emit("ident", i, attr)
            i = j
            while i < end and src[i] in " \t\r\n":
                i += 1
            if i < end and src[i] == "=":
                i += 1
                while i < end and src[i] in " \t\r\n":
                    i += 1
                if i < end and src[i] in "'\"":
                    quote = src[i]
                    i += 1
                    piece = i
                    while i < end and src[i] != quote:
                        if svelte and src[i] == "{":
                            _attr_static(emit, src, attr.lower(), piece, i)
                            i = lex_js(src, i + 1, end, emit, stop="}") + 1
                            piece = i
                        else:
                            i += 1
                    _attr_static(emit, src, attr.lower(), piece, i)
                    i += 1
                elif svelte and i < end and src[i] == "{":
                    i = lex_js(src, i + 1, end, emit, stop="}") + 1
                else:
                    j = i
                    while j < end and src[j] not in " \t\r\n>":
                        j += 1
                    _attr_static(emit, src, attr.lower(), i, j)
                    i = j
            elif j == i and i < end and src[i] not in " \t\r\n>/{":
                i += 1
    return end, name


def lex_markup(src, i, end, emit, svelte=False):
    while i < end:
        if src.startswith("<!--", i):
            k = src.find("-->", i + 4, end)
            k = end if k < 0 else k
            emit("comment", i + 4, src[i + 4:k])
            i = k + 3
        elif src[i] == "<" and i + 1 < end and src[i + 1] in "!?":
            k = src.find(">", i, end)
            i = end if k < 0 else k + 1
        elif src[i] == "<" and i + 1 < end and (src[i + 1].isalpha() or src[i + 1] == "/"):
            closing = src[i + 1] == "/"
            i, name = _lex_tag(src, i + 2 if closing else i + 1, end, emit, svelte)
            if not closing and name in ("script", "style"):
                k = src.lower().find("</" + name, i, end)
                k = end if k < 0 else k
                if name == "script":
                    lex_js(src, i, k, emit)
                else:
                    lex_css(src, i, k, emit)
                i = k
        elif svelte and src[i] == "{":
            if src.startswith("{/", i):
                k = src.find("}", i, end)
                k = end if k < 0 else k
                for m in re.finditer(r"[A-Za-z_]\w*", src[i + 2:k]):
                    emit("ident", i + 2 + m.start(), m.group())
                i = k + 1
            else:
                i = lex_js(src, i + 1, end, emit, stop="}") + 1
        else:
            j = i + 1
            while j < end and src[j] != "<" and not (svelte and src[j] == "{"):
                j += 1
            _emit_text(emit, src, i, j)
            i = j
    return end


def lex_svelte(src, i, end, emit):
    return lex_markup(src, i, end, emit, svelte=True)


# ---- Python

PY_STR = re.compile(r"""([rRbBuUfF]{0,2})('''|\"\"\"|'|")""")
PY_FEXPR = re.compile(r"(?<!\{)\{([^{}]+)\}")


def lex_python(src, i, end, emit):
    last_sig = None
    while i < end:
        c = src[i]
        if c in " \t\r\n\\":
            i += 1
        elif c == "#":
            k = skip_line(src, i, end)
            emit("comment", i + 1, src[i + 1:k])
            i = k
        elif c in "'\"" or (is_ident_start(c) and PY_STR.match(src, i, end) and PY_STR.match(src, i, end).group(1)):
            m = PY_STR.match(src, i, end)
            prefix, quote = m.group(1), m.group(2)
            start = m.end()
            if len(quote) == 3:
                k = start
                while k < end and not src.startswith(quote, k):
                    k += 2 if src[k] == "\\" else 1
                body = src[start:min(k, end)]
                docstring = last_sig in (None, ":")
                emit("comment" if docstring else "literal", start, body)
                i = k + 3
            else:
                k = scan_quoted(src, start, end, quote, multiline=False)
                body = src[start:k]
                emit("literal", start, body)
                i = k + 1
            if "f" in prefix.lower():
                for fm in PY_FEXPR.finditer(body):
                    expr = fm.group(1).split("!", 1)[0].split(":", 1)[0]
                    for im in re.finditer(r"[^\W\d]\w*", expr):
                        emit("ident", start + fm.start(1) + im.start(), im.group())
            last_sig = '"'
        elif is_ident_start(c):
            j = scan_ident(src, i, end)
            emit("ident", i, src[i:j])
            i, last_sig = j, "a"
        elif c.isdigit():
            i, last_sig = scan_ident(src, i, end, "."), "0"
        else:
            i, last_sig = i + 1, c
    return end


# ---- Shell (also Dockerfile and Makefile recipes)

SH_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
SH_ASSIGN = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)(\[[^\]\n]*\])?\+?=")
SH_DECL = {"local", "export", "readonly", "declare", "typeset", "unset", "read", "ENV", "ARG"}
SH_CMD_KW = {"then", "do", "else", "elif", "if", "while", "until", "!", "{", "time"}
SH_HEREDOC = re.compile(r"<<(-?)[ \t]*(?:'([^'\n]*)'|\"([^\"\n]*)\"|\\?([A-Za-z_][\w.-]*))")
SH_VAR_IN_TEXT = re.compile(r"\$\{?([A-Za-z_][A-Za-z0-9_]*)")


def _match_brace(src, i, end):
    """i points after '${'; returns the index of the matching '}'."""
    depth = 1
    while i < end:
        c = src[i]
        if c == "\\":
            i += 2
            continue
        if c == "'":
            i = scan_quoted(src, i + 1, end, "'", escapes=False) + 1
            continue
        if c == '"':
            i = scan_quoted(src, i + 1, end, '"') + 1
            continue
        if src.startswith("${", i):
            depth += 1
            i += 2
            continue
        if c == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return end


def _sh_dollar(src, i, end, emit):
    """i points at '$'; returns the index after the expansion."""
    nxt = src[i + 1] if i + 1 < end else ""
    if src.startswith("$((", i):
        depth, j = 0, i + 1
        while j < end:
            if src[j] == "(":
                depth += 1
            elif src[j] == ")":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        for m in SH_NAME.finditer(src, i + 3, j):
            emit("ident", m.start(), m.group())
        return j + 1
    if nxt == "(":
        return lex_shell(src, i + 2, end, emit, stop=")") + 1
    if nxt == "{":
        k = _match_brace(src, i + 2, end)
        m = re.compile(r"[#!]?([A-Za-z_][A-Za-z0-9_]*)").match(src, i + 2, k)
        rest = i + 2
        if m:
            emit("ident", m.start(1), m.group(1))
            rest = m.end()
        if rest < k:
            lex_shell(src, rest, k, emit, comments=False)
        return k + 1
    if nxt == "'":
        k = scan_quoted(src, i + 2, end, "'")
        emit("literal", i + 2, src[i + 2:k])
        return k + 1
    if nxt == '"':
        return _sh_dquote(src, i + 1, end, emit)
    m = SH_NAME.match(src, i + 1, end)
    if m:
        emit("ident", m.start(), m.group())
        return m.end()
    return i + 2


def _sh_dquote(src, i, end, emit):
    """i points at '"'; returns the index after the closing quote."""
    j = i + 1
    while j < end and src[j] != '"':
        c = src[j]
        if c == "\\":
            j += 2
        elif c == "$":
            j = _sh_dollar(src, j, end, emit)
        elif c == "`":
            k = scan_quoted(src, j + 1, end, "`")
            lex_shell(src, j + 1, k, emit)
            j = k + 1
        else:
            j += 1
    emit("literal", i + 1, src[i + 1:min(j, end)])
    return j + 1


def lex_shell(src, i, end, emit, stop=None, comments=True):
    """Returns the index of the unmatched ')' when stop is ')', or end."""
    word_start, cmd_pos, decl, next_name = True, True, False, False
    depth = 0
    heredocs = []
    while i < end:
        c = src[i]
        if c == "\n":
            i += 1
            for strip, delim, quoted in heredocs:
                body = i
                while i < end:
                    k = skip_line(src, i, end)
                    line = src[i:k]
                    if (line.lstrip("\t") if strip else line) == delim:
                        emit("literal", body, src[body:i])
                        if not quoted:
                            for m in SH_VAR_IN_TEXT.finditer(src, body, i):
                                emit("ident", m.start(1), m.group(1))
                        i = k + 1
                        break
                    i = k + 1
                else:
                    emit("literal", body, src[body:end])
            heredocs = []
            word_start, cmd_pos, decl, next_name = True, True, False, False
            continue
        if c in " \t\r":
            i += 1
            word_start = True
            continue
        if c == "\\":
            i += 2
            continue
        if c == "#" and word_start and comments:
            k = skip_line(src, i, end)
            emit("comment", i + 1, src[i + 1:k])
            i = k
            continue
        if src.startswith("<<", i) and not src.startswith("<<<", i):
            m = SH_HEREDOC.match(src, i, end)
            if m:
                quoted = m.group(4) is None or src[m.start(4) - 1] == "\\"
                delim = m.group(2) if m.group(2) is not None else m.group(3) if m.group(3) is not None else m.group(4)
                heredocs.append((m.group(1) == "-", delim, quoted))
                i = m.end()
                word_start = True
                continue
        if c in ";&|()<>":
            if c == "(":
                depth += 1
            elif c == ")":
                if depth == 0 and stop == ")":
                    return i
                depth -= 1
            i += 1
            word_start = True
            if c in ";&|(":
                cmd_pos, decl, next_name = True, False, False
            continue
        # A word.
        start = i
        m = SH_ASSIGN.match(src, i, end)
        if m and (cmd_pos or decl):
            emit("ident", i, m.group(1))
            i = m.end()
        bare = i == start
        while i < end:
            c = src[i]
            if c in " \t\r\n;&|()<>":
                break
            if c == "\\":
                i += 2
                bare = False
            elif c == "'":
                k = scan_quoted(src, i + 1, end, "'", escapes=False)
                emit("literal", i + 1, src[i + 1:k])
                i, bare = k + 1, False
            elif c == '"':
                i, bare = _sh_dquote(src, i, end, emit), False
            elif c == "$":
                i, bare = _sh_dollar(src, i, end, emit), False
            elif c == "`":
                k = scan_quoted(src, i + 1, end, "`")
                lex_shell(src, i + 1, k, emit)
                i, bare = k + 1, False
            else:
                i += 1
        word = src[start:i]
        word_start = False
        if m and (cmd_pos or decl):
            continue
        if bare and SH_NAME.fullmatch(word):
            rest = src[i:i + 4].lstrip(" \t")
            if (decl or next_name) or (cmd_pos and rest.startswith("()")):
                emit("ident", start, word)
                next_name = False
                if not decl:
                    cmd_pos = False
                continue
            if cmd_pos and word in SH_DECL:
                decl, cmd_pos = True, False
                continue
            if cmd_pos and word in ("for", "function", "select"):
                next_name, cmd_pos = True, False
                continue
            if cmd_pos and word in SH_CMD_KW:
                continue
        if word.startswith("-") and decl:
            continue
        cmd_pos = False
    return end


def lex_dockerfile(src, i, end, emit):
    return lex_shell(src, i, end, emit)


# ---- Makefile


MAKE_ASSIGN = re.compile(r"[ \t]*(?:(override|export|private)[ \t]+)*([A-Za-z_.][\w.\-]*)[ \t]*(:::=|::=|:=|\?=|\+=|!=|=)")
MAKE_DIRECTIVE = re.compile(r"[ \t]*(ifeq|ifneq|ifdef|ifndef|else|endif|include|-include|sinclude|export|unexport|override|vpath|undefine)\b")


def _make_refs(src, i, end, emit):
    """Emits the names of the $(...) and ${...} references in [i, end) and returns
    their spans."""
    spans = []
    j = i
    while j < end:
        if src[j] == "$" and j + 1 < end:
            if src[j + 1] == "$":
                j += 2
                continue
            if src[j + 1] in "({":
                close = ")" if src[j + 1] == "(" else "}"
                opening = src[j + 1]
                depth, k = 1, j + 2
                while k < end and depth:
                    if src[k] == opening:
                        depth += 1
                    elif src[k] == close:
                        depth -= 1
                    k += 1
                m = re.compile(r"[A-Za-z_][\w.\-]*").match(src, j + 2, k)
                if m:
                    emit("ident", m.start(), m.group())
                    _make_refs(src, m.end(), k - 1, emit)
                spans.append((j, k))
                j = k
                continue
        j += 1
    return spans


def _make_comment(src, i, end):
    """Index of the first unescaped '#' outside references, or -1."""
    depth = 0
    j = i
    while j < end:
        c = src[j]
        if c == "\\":
            j += 2
            continue
        if c == "$" and j + 1 < end and src[j + 1] in "({":
            depth += 1
            j += 2
            continue
        if c in ")}" and depth:
            depth -= 1
        elif c == "#" and not depth:
            return j
        j += 1
    return -1


def _make_value(src, i, end, emit):
    k = _make_comment(src, i, end)
    stop = end if k < 0 else k
    _make_refs(src, i, stop, emit)
    _emit_text(emit, src, i, stop)
    if k >= 0:
        emit("comment", k + 1, src[k + 1:end])


def _make_recipe(src, i, end, emit):
    chars = list(src[i:end])
    for s, e in _make_refs(src, i, end, emit):
        for p in range(s, e):
            if chars[p - i] != "\n":
                chars[p - i] = " "
    text = "".join(chars).replace("$$", " $")
    shadow = src[:i] + text + src[end:]
    lex_shell(shadow, i, end, emit)


def lex_make(src, i, end, emit):
    in_rule = False
    in_define = False
    while i < end:
        # Logical line: physical lines joined by a trailing backslash.
        j = i
        while True:
            k = skip_line(src, j, end)
            back = 0
            while k - back - 1 >= j and src[k - back - 1] == "\\":
                back += 1
            if back % 2 == 1 and k < end:
                j = k + 1
                continue
            break
        s, e = i, k
        i = k + 1
        line = src[s:e]
        stripped = line.strip()
        if in_define:
            if stripped == "endef":
                in_define = False
            else:
                _emit_text(emit, src, s, e)
            continue
        if line.startswith("\t") and in_rule:
            _make_recipe(src, s, e, emit)
            continue
        if not stripped:
            continue
        if stripped.startswith("#"):
            off = s + line.index("#")
            emit("comment", off + 1, src[off + 1:e])
            continue
        m = re.match(r"[ \t]*define[ \t]+([A-Za-z_][\w.\-]*)", line)
        if m:
            emit("ident", s + m.start(1), m.group(1))
            in_define = True
            in_rule = False
            continue
        m = MAKE_ASSIGN.match(line)
        if m:
            emit("ident", s + m.start(2), m.group(2))
            _make_value(src, s + m.end(), e, emit)
            in_rule = False
            continue
        m = MAKE_DIRECTIVE.match(line)
        if m:
            word = m.group(1)
            rest = s + m.end()
            if word in ("export", "unexport", "override", "undefine", "ifdef", "ifndef"):
                k = _make_comment(src, rest, e)
                stop = e if k < 0 else k
                _make_refs(src, rest, stop, emit)
                for nm in re.finditer(r"[A-Za-z_][\w.\-]*", src[rest:stop]):
                    emit("ident", rest + nm.start(), nm.group())
                if k >= 0:
                    emit("comment", k + 1, src[k + 1:e])
            else:
                _make_value(src, rest, e, emit)
            continue
        colon = -1
        depth = 0
        for p in range(s, e):
            ch = src[p]
            if ch == "$" and p + 1 < e and src[p + 1] in "({":
                depth += 1
            elif ch in ")}" and depth:
                depth -= 1
            elif ch == ":" and not depth and src[p + 1:p + 2] != "=":
                colon = p
                break
        if colon < 0:
            _make_value(src, s, e, emit)
            continue
        in_rule = True
        _make_refs(src, s, colon, emit)
        for tm in re.finditer(r"[^\s]+", src[s:colon]):
            if "$" not in tm.group():
                emit("ident", s + tm.start(), tm.group())
        after = colon + 1
        while after < e and src[after] == ":":
            after += 1
        m = MAKE_ASSIGN.match(src, after, e)
        if m:
            emit("ident", m.start(2), m.group(2))
            _make_value(src, m.end(), e, emit)
            continue
        semi = src.find(";", after, e)
        k = _make_comment(src, after, e if semi < 0 else semi)
        stop = e if semi < 0 else semi
        stop = stop if k < 0 else k
        _make_refs(src, after, stop, emit)
        for pm in re.finditer(r"[^\s|]+", src[after:stop]):
            word = pm.group()
            if "$" in word or "/" in word or "." in word.lstrip("."):
                continue
            emit("ident", after + pm.start(), word)
        if k >= 0:
            emit("comment", k + 1, src[k + 1:e])
        elif semi >= 0:
            _make_recipe(src, semi + 1, e, emit)
    return end


# ---- TOML, YAML, JSON, INI-like

TOML_STR = re.compile(r"'''|\"\"\"|'|\"")


def lex_toml(src, i, end, emit):
    while i < end:
        c = src[i]
        if c == "#":
            k = skip_line(src, i, end)
            emit("comment", i + 1, src[i + 1:k])
            i = k
        elif c in "'\"":
            q = TOML_STR.match(src, i, end).group()
            if len(q) == 3:
                k = src.find(q, i + 3, end)
                k = end if k < 0 else k
            else:
                k = scan_quoted(src, i + 1, end, q, escapes=(q == '"'), multiline=False)
            emit("literal", i + len(q), src[i + len(q):k])
            i = k + len(q)
        elif c.isalpha() or c == "_":
            j = scan_ident(src, i, end, "-")
            emit("ident", i, src[i:j])
            i = j
        elif c.isdigit():
            i = scan_ident(src, i, end, ".-:+")
        else:
            i += 1
    return end


YAML_KEY = re.compile(r"([ \t]*)((?:-[ \t]+)*)([^\s#'\"\-{\[][^:#\n]*?|\"[^\"\n]*\"|'[^'\n]*')[ \t]*:(?=[ \t]|$)")


YAML_ANCHOR = re.compile(r"(?:^|(?<=[\s\[,]))[&*]([\w\-]+)")


def _yaml_value(src, i, end, emit):
    """Scalar value until a ' #' comment. Returns True when it opens a block scalar."""
    while i < end and src[i] in " \t":
        i += 1
    if i >= end:
        return False
    c = src[i]
    if c == "#":
        emit("comment", i + 1, src[i + 1:end])
        return False
    if c in "|>":
        k = src.find("#", i, end)
        if k >= 0:
            emit("comment", k + 1, src[k + 1:end])
        return True
    if c in "'\"":
        k = scan_quoted(src, i + 1, end, c, escapes=(c == '"'))
        emit("literal", i + 1, src[i + 1:k])
        k = src.find("#", k + 1, end)
        if k >= 0:
            emit("comment", k + 1, src[k + 1:end])
        return False
    m = re.compile(r"[ \t]#").search(src, i, end)
    stop = m.start() if m else end
    for am in YAML_ANCHOR.finditer(src, i, stop):
        emit("ident", am.start(1), am.group(1))
    _emit_text(emit, src, i, stop)
    if m:
        emit("comment", m.start() + 2, src[m.start() + 2:end])
    return False


def lex_yaml(src, i, end, emit):
    block_indent = None
    while i < end:
        k = skip_line(src, i, end)
        line = src[i:k]
        indent = len(line) - len(line.lstrip(" "))
        if block_indent is not None:
            if not line.strip() or indent > block_indent:
                _emit_text(emit, src, i, k)
                i = k + 1
                continue
            block_indent = None
        stripped = line.strip()
        if stripped.startswith("#"):
            off = i + line.index("#")
            emit("comment", off + 1, src[off + 1:k])
        elif stripped:
            m = YAML_KEY.match(src, i, k)
            if m:
                key = m.group(3)
                if key[0] in "'\"":
                    emit("literal", m.start(3) + 1, key[1:-1])
                else:
                    emit("ident", m.start(3), key)
                if _yaml_value(src, m.end(), k, emit):
                    block_indent = indent
            else:
                m = re.compile(r"[ \t]*(?:-[ \t]*)*").match(src, i, k)
                if _yaml_value(src, m.end(), k, emit):
                    block_indent = indent
        i = k + 1
    return end


def lex_json(src, i, end, emit):
    while i < end:
        c = src[i]
        if c == '"':
            k = scan_quoted(src, i + 1, end, '"')
            j = k + 1
            while j < end and src[j] in " \t\r\n":
                j += 1
            kind = "ident" if j < end and src[j] == ":" else "literal"
            emit(kind, i + 1, src[i + 1:k])
            i = k + 1
        else:
            i += 1
    return end


INI_KEY = re.compile(r"[ \t]*([A-Za-z_][\w.\-@]*(?:\[[^\]\n]*\])?)[ \t]*=")


def lex_ini(src, i, end, emit):
    while i < end:
        k = skip_line(src, i, end)
        line = src[i:k]
        stripped = line.strip()
        if stripped.startswith(("#", ";")):
            off = i + len(line) - len(line.lstrip())
            emit("comment", off + 1, src[off + 1:k])
        elif stripped.startswith("[") and stripped.endswith("]"):
            off = i + line.index("[")
            emit("ident", off + 1, stripped[1:-1])
        else:
            m = INI_KEY.match(line)
            if m:
                emit("ident", i + m.start(1), m.group(1))
                _emit_text(emit, src, i + m.end(), k)
        i = k + 1
    return end


def lex_none(src, i, end, emit):
    return end


SHEBANG = re.compile(r"#![^\n]*\b(python3?|bash|sh|dash)\b")


def language(path, head):
    name = os.path.basename(path)
    ext = os.path.splitext(name)[1].lower()
    if name in LOCKFILES or ext in BINARY_EXT:
        return None
    if name == "Makefile" or ext == ".mk":
        return lex_make
    if name == "Dockerfile" or name.startswith("Dockerfile."):
        return lex_dockerfile
    table = {
        ".rs": lex_rust, ".c": lex_c, ".h": lex_c, ".js": lex_js, ".mjs": lex_js,
        ".cjs": lex_js, ".ts": lex_js, ".svelte": lex_svelte, ".css": lex_css,
        ".html": lex_markup, ".xml": lex_markup, ".svg": lex_markup, ".py": lex_python,
        ".sh": lex_shell, ".bash": lex_shell, ".toml": lex_toml, ".yml": lex_yaml,
        ".yaml": lex_yaml, ".json": lex_json, ".service": lex_ini, ".socket": lex_ini,
        ".timer": lex_ini, ".target": lex_ini, ".path": lex_ini, ".mount": lex_ini,
        ".desktop": lex_ini, ".conf": lex_ini, ".env": lex_ini, ".npmrc": lex_ini,
        ".gitignore": lex_ini, ".txt": lex_ini, ".cfg": lex_ini, ".ini": lex_ini,
        ".md": lex_none,
    }
    if ext in table:
        return table[ext]
    if name.startswith(".") and name in table:
        return table[name]
    m = SHEBANG.match(head)
    if m:
        return lex_python if m.group(1).startswith("python") else lex_shell
    return lex_ini


# --------------------------------------------------------------------------------
# Findings


class Finding:
    __slots__ = ("path", "line", "kind", "token", "words", "reason")

    def __init__(self, path, line, kind, token, words, reason):
        self.path, self.line, self.kind = path, line, kind
        self.token, self.words, self.reason = token, words, reason

    def key(self):
        return (self.path, self.line, self.kind, self.reason)

    def __str__(self):
        where = f"{self.path}:{self.line}" if self.line else self.path
        token = " ".join(self.token.split())
        if len(token) > 60:
            token = token[:57] + "..."
        return f"{where}: {self.kind} '{token}': {self.reason}"


def check_ident(words, path, line, token, out, kind="identifier"):
    if non_ascii_letters(token):
        out.append(Finding(path, line, kind, token, [w for w, _ in split_words(token)] + [fold(token)],
                           "letra fora do ASCII"))
    for word, _ in split_words(token):
        if words.ident_hit(word):
            out.append(Finding(path, line, kind, token, [word, fold(token)], f'palavra pt-BR "{word}"'))


SENTENCE_END = re.compile(r"[.!?;](?=\s|$)")


def check_comment_group(words, path, items, out):
    """items: list of (line_of, text, offset) of consecutive comment lines."""
    sentence_fn, sentence_ambiguous = [], []

    def close():
        if len(sentence_fn) >= 2 or (sentence_fn and sentence_ambiguous):
            line = sentence_fn[0][1]
            fn_words = [w for w, _ in sentence_fn]
            out.append(Finding(path, line, "comment", " ".join(fn_words), fn_words,
                               "frase pt-BR (" + ", ".join(fn_words) + ")"))
        sentence_fn.clear()
        sentence_ambiguous.clear()

    for line_of, text, base in items:
        for p in non_ascii_letters(text):
            word_m = next((wm for wm in re.finditer(r"\w+", text) if wm.start() <= p < wm.end()), None)
            word = word_m.group() if word_m else text[p]
            out.append(Finding(path, line_of(base + p), "comment", word, [fold(word), word.lower()],
                               "letra fora do ASCII"))
        ends = [m.start() for m in SENTENCE_END.finditer(text)]
        e_idx = 0
        for word, off in split_words(text):
            while e_idx < len(ends) and ends[e_idx] < off:
                close()
                e_idx += 1
            if words.domain(word):
                out.append(Finding(path, line_of(base + off), "comment", word, [word],
                                   f'palavra pt-BR "{word}"'))
            elif word in words.function:
                sentence_fn.append((word, line_of(base + off)))
            elif word in words.ambiguous:
                sentence_ambiguous.append(word)
    close()


class Source:
    def __init__(self, text):
        self.text = text
        self.starts = [0] + [m.end() for m in re.finditer("\n", text)]

    def line_of(self, offset):
        return bisect.bisect_right(self.starts, offset)


def tokens_of(text, lexer):
    tokens = []

    def emit(kind, offset, value):
        if value:
            tokens.append((kind, offset, value))

    lexer(text, 0, len(text), emit)
    tokens.sort(key=lambda t: t[1])
    return tokens


def check_content(words, path, text, lexer, out):
    src = Source(text)
    tokens = tokens_of(text, lexer)
    group, group_end = [], -1
    for kind, offset, value in tokens:
        if kind == "comment":
            first = src.line_of(offset)
            if group and first > group_end + 1:
                check_comment_group(words, path, group, out)
                group = []
            group.append((src.line_of, value, offset))
            group_end = src.line_of(offset + len(value))
            continue
        if group:
            check_comment_group(words, path, group, out)
            group = []
        if kind == "ident":
            check_ident(words, path, src.line_of(offset), value, out)
    if group:
        check_comment_group(words, path, group, out)


def check_name(words, path, component, out):
    check_ident(words, path, 0, component, out, kind="name")


# --------------------------------------------------------------------------------
# Files


class UsageError(Exception):
    """Bad arguments or a repository the checker refuses to read (exit 2)."""


GIT_ENV = {
    "PATH": "/usr/bin:/bin",
    "GIT_CONFIG_GLOBAL": "/dev/null",
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_CONFIG_COUNT": "4",
    "GIT_CONFIG_KEY_0": "core.excludesFile", "GIT_CONFIG_VALUE_0": "/dev/null",
    "GIT_CONFIG_KEY_1": "core.attributesFile", "GIT_CONFIG_VALUE_1": "/dev/null",
    "GIT_CONFIG_KEY_2": "core.fsmonitor", "GIT_CONFIG_VALUE_2": "false",
    "GIT_CONFIG_KEY_3": "core.hooksPath", "GIT_CONFIG_VALUE_3": "/dev/null",
    "LC_ALL": "C",
}

SKIP_DIRS = {".git", "target", "node_modules", "dist", "dist-teste", ".dev"}


def list_files(root):
    if os.path.lexists(os.path.join(root, ".git")):
        out = subprocess.run(
            ["/usr/bin/git", "-C", root, "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
            env=GIT_ENV, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            check=False,
        )
        if out.returncode != 0:
            raise UsageError("git ls-files falhou: " + out.stderr.decode(errors="replace").strip())
        files = sorted({p for p in out.stdout.decode("utf-8", "surrogateescape").split("\0") if p})
    else:
        files = []
        for dirpath, dirnames, filenames in os.walk(root):
            rel = os.path.relpath(dirpath, root)
            rel = "" if rel == "." else rel + "/"
            keep = []
            for d in dirnames:
                if d in SKIP_DIRS:
                    continue
                if os.path.islink(os.path.join(dirpath, d)):
                    files.append(rel + d)
                else:
                    keep.append(d)
            dirnames[:] = sorted(keep)
            files.extend(rel + f for f in filenames)
        files.sort()
    return [f for f in files if os.path.lexists(os.path.join(root, f))]


def is_doc(path):
    if path.startswith(DOC_PREFIXES) or path in DOC_FILES or path + "/" in DOC_PREFIXES:
        return True
    return path.startswith("skill/") and path.endswith(".md")


def read_text(root, path):
    """Regular file inside the repository, never through a link. Returns None when
    the content must not be read."""
    full = os.path.join(root, path)
    real_root = os.path.realpath(root)
    parent = os.path.realpath(os.path.dirname(full))
    if parent != real_root and not parent.startswith(real_root + os.sep):
        return None
    try:
        fd = os.open(full, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
    except OSError:
        return None
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode) or st.st_size > MAX_FILE:
            return None
        chunks = []
        while True:
            data = os.read(fd, 1 << 20)
            if not data:
                break
            chunks.append(data)
    finally:
        os.close(fd)
    data = b"".join(chunks)
    if b"\0" in data[:8192]:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def read_list(root, rel):
    text = read_text(root, rel)
    if text is None:
        if os.path.lexists(os.path.join(root, rel)):
            raise UsageError(f"{rel}: não é arquivo comum do repositório")
        return ""
    return text


class Rule:
    __slots__ = ("source", "line", "glob", "word", "used")

    def __init__(self, source, line, glob, word):
        self.source, self.line, self.glob, self.word = source, line, glob, word
        self.used = False

    def __str__(self):
        return f"{self.source}:{self.line}: {self.glob} {self.word}"


def load_exceptions(root):
    out, errors = [], []
    for rel, min_fields in ((ALLOW, 3), (FROZEN, 4)):
        for n, raw in enumerate(read_list(root, rel).splitlines(), 1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            fields = line.split()
            if len(fields) < min_fields or (rel == FROZEN and not re.fullmatch(r"\d+", fields[2])):
                errors.append(f"{rel}:{n}: linha mal formada (esperado: caminho palavra"
                              + (" fase" if rel == FROZEN else "") + " motivo)")
                continue
            out.append(Rule(rel, n, fields[0], fold(fields[1])))
    return out, errors


def scope(files, paths):
    """Files under the given paths, and the name components to check."""
    if not paths:
        names = set()
        for f in files:
            parts = f.split("/")
            for k in range(1, len(parts) + 1):
                names.add("/".join(parts[:k]))
        return files, sorted(names)
    chosen, names = [], set()
    for p in paths:
        prefix = p.rstrip("/")
        hits = [f for f in files if f == prefix or f.startswith(prefix + "/")]
        if not hits:
            raise UsageError(f"--paths: nada versionado em {p}")
        chosen.extend(hits)
        base = prefix.count("/")
        for f in hits:
            parts = f.split("/")
            for k in range(base + 1, len(parts) + 1):
                names.add("/".join(parts[:k]))
    return sorted(set(chosen)), sorted(names)


def normalize_paths(root, paths):
    out = []
    for p in paths or []:
        if p.startswith("-"):
            raise UsageError(f"--paths: caminho inválido {p!r}")
        full = os.path.normpath(os.path.join(root, p))
        if full != root and not full.startswith(root + os.sep):
            raise UsageError(f"--paths: fora do repositório: {p}")
        if full == root:
            return []
        out.append(os.path.relpath(full, root))
    return out


def run_check(root, paths, words=None):
    """Returns (findings, problems). problems: unused or malformed exceptions."""
    root = os.path.realpath(root)
    if words is None:
        words = Words(read_list(root, PT_WORDS))
    files, names = scope(list_files(root), normalize_paths(root, paths))
    findings = []
    for name in names:
        if is_doc(name):
            continue
        check_name(words, name, os.path.basename(name), findings)
    for path in files:
        if is_doc(path) or path.startswith(DATA_PREFIXES):
            continue
        full = os.path.join(root, path)
        if os.path.islink(full):
            continue
        text = read_text(root, path)
        if text is None:
            continue
        lexer = language(path, text[:200])
        if lexer is None:
            continue
        check_content(words, path, text, lexer, findings)
    seen, unique = set(), []
    for f in findings:
        if f.key() not in seen:
            seen.add(f.key())
            unique.append(f)
    exceptions, problems = load_exceptions(root)
    left = []
    for f in unique:
        matched = False
        token = fold(f.token)
        for ex in exceptions:
            if fnmatch.fnmatchcase(f.path, ex.glob) and (ex.word in f.words or ex.word == token):
                ex.used = True
                matched = True
        if not matched:
            left.append(f)
    for ex in exceptions:
        if ex.used:
            continue
        in_scope = not paths or any(fnmatch.fnmatchcase(f, ex.glob) for f in files) or any(
            fnmatch.fnmatchcase(n, ex.glob) for n in names)
        if in_scope:
            problems.append(f"exceção sem uso: {ex}")
    left.sort(key=lambda f: (f.path, f.line))
    return left, problems


def run_literals(root, paths, with_location):
    root = os.path.realpath(root)
    files, _ = scope(list_files(root), normalize_paths(root, paths))
    out = set()
    for path in files:
        if is_doc(path) or path.startswith(DATA_PREFIXES) or os.path.islink(os.path.join(root, path)):
            continue
        text = read_text(root, path)
        if text is None:
            continue
        lexer = language(path, text[:200])
        if lexer is None:
            continue
        src = Source(text)
        for kind, offset, value in tokens_of(text, lexer):
            if kind != "literal" or not value.strip():
                continue
            literal = json.dumps(value, ensure_ascii=False)
            out.add(f"{path}:{src.line_of(offset)}: {literal}" if with_location else literal)
    return sorted(out)


# --------------------------------------------------------------------------------
# Self-test (m2:V1.2, m2:V1.3, m2:V1.4)

PLANTS = [
    # (language, new file, content, [(line, kind)], existing file glob, appended lines)
    ("rust", "crates/selftest_plant.rs",
     "fn check() {}\nfn cofre_aberto() {}\n// isso fica para depois\n// keeps the senha here\n",
     [(2, "identifier"), (3, "comment"), (4, "comment")],
     "crates/*.rs", "fn cofre_aberto() {}\n// isso fica para depois\n"),
    ("js", "extension/selftest_plant.js",
     "const ok = 1;\nconst cofreAberto = 2;\n// isso fica para depois\n",
     [(2, "identifier"), (3, "comment")],
     "extension/*.js", "const cofreAberto = 2;\n// isso fica para depois\n"),
    ("svelte", "app/src/selftest_plant.svelte",
     "<script>\n  let ok = 1;\n  let cofreAberto = 2;\n  // isso fica para depois\n</script>\n"
     "<div class=\"campo-senha\"></div>\n<!-- isso fica para depois -->\n",
     [(3, "identifier"), (4, "comment"), (6, "identifier"), (7, "comment")],
     "app/src/*.svelte", "<div class=\"campo-senha\"></div>\n<!-- isso fica para depois -->\n"),
    ("css", "app/src/selftest_plant.css",
     ".ok { color: red; }\n.cofre-aberto { color: red; }\n/* isso fica para depois */\n",
     [(2, "identifier"), (3, "comment")],
     "app/src/*.css", ".cofre-aberto { color: red; }\n/* isso fica para depois */\n"),
    ("shell", "dev/selftest_plant.sh",
     "#!/bin/bash\nok=1\ncofre_aberto=2\n# isso fica para depois\n",
     [(3, "identifier"), (4, "comment")],
     "dev/*.sh", "cofre_aberto=2\n# isso fica para depois\n"),
    ("python", "dev/selftest_plant.py",
     "ok = 1\ncofre_aberto = 2\n# isso fica para depois\n",
     [(2, "identifier"), (3, "comment")],
     "dev/*.py", "cofre_aberto = 2\n# isso fica para depois\n"),
    ("make", "dev/selftest_plant.mk",
     "ok:\n\ttrue\ncofre-aberto:\n\ttrue\n# isso fica para depois\n",
     [(3, "identifier"), (5, "comment")],
     "Makefile", "cofre-aberto:\n\ttrue\n# isso fica para depois\n"),
    ("systemd", "dev/selftest-plant/selftest.service",
     "[Unit]\nDescription=ok\nChaveCofre=1\n# isso fica para depois\n",
     [(3, "identifier"), (4, "comment")],
     "*.service", "ChaveCofre=1\n# isso fica para depois\n"),
    ("toml", "crates/selftest_plant.toml",
     "[package]\nname = \"ok\"\ncofre_aberto = true\n# isso fica para depois\n",
     [(3, "identifier"), (4, "comment")],
     "*Cargo.toml", "cofre_aberto = true\n# isso fica para depois\n"),
    ("yaml", "dev/selftest_plant.yml",
     "ok: 1\ncofre_aberto: 2\n# isso fica para depois\n",
     [(2, "identifier"), (3, "comment")],
     "*.yml", "cofre_aberto: 2\n# isso fica para depois\n"),
    ("dockerfile", "dev/selftest-plant/Dockerfile",
     "FROM scratch\nARG COFRE_ABERTO=1\n# isso fica para depois\n",
     [(2, "identifier"), (3, "comment")],
     "*Dockerfile", "ARG COFRE_ABERTO=1\n# isso fica para depois\n"),
    ("c", "dev/selftest_plant.c",
     "int ok;\nint cofre_aberto;\n/* isso fica para depois */\n",
     [(2, "identifier"), (3, "comment")],
     "*.c", "int cofre_aberto;\n/* isso fica para depois */\n"),
    ("non-ascii", "dev/selftest_accent.py",
     "ok = 1\n# a naive façade\n",
     [(2, "comment")], None, None),
]

CLEAN = {
    "dev/selftest_clean.rs":
        'fn message() -> &\'static str {\n    "senha incorreta"\n}\n'
        "// Set a to the de-duplicated list.\n// Keep a de-dup flag here.\n",
    "dev/selftest_clean.py":
        'def message():\n    return "senha incorreta"  # a de-dup step\n',
    "dev/selftest_clean.js":
        "const message = 'senha incorreta'; // a de-dup step\n"
        "const other = `senha ${message} incorreta`;\n",
    "dev/selftest_clean.sh":
        '#!/bin/bash\necho "senha incorreta" # a de-dup step\n'
        "cat <<'EOF'\nsenha do cofre\nEOF\n",
    "dev/selftest_clean.svelte":
        "<p>Senha incorreta</p>\n<button title=\"senha do cofre\">ok</button>\n",
    "dev/selftest_clean.yml":
        "message: \"senha incorreta\"\n# a de-dup step\n",
}


def _copy_repo(root, dest):
    for path in list_files(root):
        if is_doc(path):
            continue
        src = os.path.join(root, path)
        dst = os.path.join(dest, path)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        if os.path.islink(src):
            os.symlink(os.readlink(src), dst)
        elif os.path.isfile(src):
            shutil.copyfile(src, dst, follow_symlinks=False)


def _write(dest, rel, text, mode="w"):
    full = os.path.join(dest, rel)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    with open(full, mode, encoding="utf-8") as fh:
        fh.write(text)


def self_test():
    dev = os.path.join(ROOT, ".dev")
    if os.path.islink(dev):
        raise UsageError(".dev é link simbólico; recusado")
    os.makedirs(dev, mode=0o700, exist_ok=True)
    work = tempfile.mkdtemp(prefix="english-check-selftest.", dir=dev)
    failures, cases = [], 0
    words = Words(read_list(ROOT, PT_WORDS))

    def expect(name, cond, detail=""):
        nonlocal cases
        cases += 1
        if not cond:
            failures.append(f"{name}: {detail}")

    try:
        dest = os.path.join(work, "repo")
        outside = os.path.join(work, "outside")
        os.makedirs(dest)
        os.makedirs(outside)
        _copy_repo(ROOT, dest)
        # Exceptions of the copy start empty: each case writes its own.
        _write(dest, ALLOW, "")
        _write(dest, FROZEN, "")
        copied = list_files(dest)

        for lang, rel, content, expected, glob, appended in PLANTS:
            _write(dest, rel, content)
            found, problems = run_check(dest, [rel], words)
            got = {(f.line, f.kind) for f in found}
            expect(f"{lang}: falha", bool(found), "nenhum achado")
            for line, kind in expected:
                expect(f"{lang}: {rel}:{line} ({kind})", (line, kind) in got,
                       f"achados: {sorted(got)}")
            if glob is None:
                continue
            targets = [f for f in copied if fnmatch.fnmatchcase(f, glob) and language(f, "") is not None
                       and not os.path.islink(os.path.join(dest, f))]
            expect(f"{lang}: arquivo existente para {glob}", bool(targets))
            if not targets:
                continue
            target = targets[0]
            with open(os.path.join(dest, target), encoding="utf-8") as fh:
                old = fh.read()
            prefix = "" if old.endswith("\n") else "\n"
            prefix += "\n"
            first = old.count("\n") + prefix.count("\n") + 1
            _write(dest, target, prefix + appended, mode="a")
            found, _ = run_check(dest, [target], words)
            got = {(f.line, f.kind) for f in found}
            for k, text in enumerate(appended.splitlines()):
                if not re.search(r"(?i)cofre|senha|isso", text):
                    continue
                line = first + k
                expect(f"{lang}: {target}:{line} (plantado)", any(g[0] == line for g in got),
                       "linha plantada sem achado")

        # Names of a folder and of a file.
        _write(dest, "dev/cofre_plant/ok.py", "ok = 1\n")
        found, _ = run_check(dest, ["dev/cofre_plant"], words)
        expect("pasta pt-BR", any(f.kind == "name" and f.path == "dev/cofre_plant" for f in found),
               str([str(f) for f in found]))
        _write(dest, "dev/plant/senha_lista.txt", "ok\n")
        found, _ = run_check(dest, ["dev/plant"], words)
        expect("arquivo pt-BR", any(f.kind == "name" and f.path == "dev/plant/senha_lista.txt" for f in found),
               str([str(f) for f in found]))

        # Literals and English comments with a lone "a" or "de" are not flagged.
        for rel, content in CLEAN.items():
            _write(dest, rel, content)
            found, problems = run_check(dest, [rel], words)
            expect(f"limpo: {rel}", not found and not problems, str([str(f) for f in found] + problems))

        # Links are never followed (V1.4).
        _write(outside, "plant.rs", "fn cofre_fora() {}\n// isso fica para depois\n")
        os.symlink(os.path.join(outside, "plant.rs"), os.path.join(dest, "dev/link_out.rs"))
        os.symlink(outside, os.path.join(dest, "dev/link_dir"))
        found, problems = run_check(dest, ["dev/link_out.rs", "dev/link_dir"], words)
        expect("link para fora não seguido", not found, str([str(f) for f in found]))

        # Exceptions: a used one silences, an unused one fails (V1.3).
        _write(dest, "dev/selftest_allowed.rs", "fn cofre_x() {}\n")
        _write(dest, ALLOW, "dev/selftest_allowed.rs cofre self-test\n")
        found, problems = run_check(dest, ["dev/selftest_allowed.rs"], words)
        expect("allow usada", not found and not problems, str([str(f) for f in found] + problems))
        _write(dest, ALLOW, "dev/selftest_clean.rs senha self-test\n")
        found, problems = run_check(dest, ["dev/selftest_clean.rs"], words)
        expect("allow sem uso", any("exceção sem uso" in p for p in problems), str(problems))
        _write(dest, ALLOW, "")
        _write(dest, FROZEN, "dev/selftest_clean.rs destravar 54 self-test\n")
        found, problems = run_check(dest, ["dev/selftest_clean.rs"], words)
        expect("frozen sem uso", any("exceção sem uso" in p for p in problems), str(problems))
        _write(dest, FROZEN, "nao/existe.rs cofre 54 self-test\n")
        found, problems = run_check(dest, [], words)
        expect("frozen sem uso (tudo)", any("exceção sem uso" in p and "nao/existe.rs" in p for p in problems),
               str(problems))
        _write(dest, FROZEN, "dev/selftest_allowed.rs cofre self-test\n")
        found, problems = run_check(dest, ["dev/selftest_allowed.rs"], words)
        expect("frozen sem fase", any("mal formada" in p for p in problems), str(problems))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    for f in failures:
        print(f"autoteste FALHOU: {f}", file=sys.stderr)
    print(f"autoteste: {cases - len(failures)}/{cases} casos ok")
    return 1 if failures else 0


# --------------------------------------------------------------------------------


def main(argv):
    parser = argparse.ArgumentParser(description="Portuguese leftovers checker (m2:DT17)")
    parser.add_argument("--paths", nargs="+", metavar="PATH")
    parser.add_argument("--literals", action="store_true")
    parser.add_argument("--with-location", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    if args.self_test:
        return self_test()
    if args.literals:
        for line in run_literals(ROOT, args.paths, args.with_location):
            print(line)
        return 0
    if args.with_location:
        parser.error("--with-location só com --literals")
    findings, problems = run_check(ROOT, args.paths)
    for f in findings:
        print(f)
    for p in problems:
        print(p)
    files = len({f.path for f in findings})
    print(f"english-check: {len(findings)} achado(s) em {files} arquivo(s), {len(problems)} problema(s) nas exceções",
          file=sys.stderr)
    return 1 if findings or problems else 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except UsageError as exc:
        print(f"english-check: {exc}", file=sys.stderr)
        sys.exit(2)
    except Exception as exc:  # noqa: BLE001 - report and exit 2, never a traceback with data
        print(f"english-check: erro interno: {type(exc).__name__}: {exc}", file=sys.stderr)
        sys.exit(2)
