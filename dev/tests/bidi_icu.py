#!/usr/bin/python3 -IB
"""Phase 39g (D51, V5.29): no code point stays outside the RTL isolation and the bidi escape.

The approval window shows the agent's text in two layers:
- Rust (`crates/ward-core/src/display.rs`, `invisible`) swaps control and formatting
  characters (bidi, zero width...) for a visible escape;
- the interface (`app/src/lib/bidi.js`, `RTL`) isolates each run of RTL letters in
  `<bdi dir="rtl">`.

This test reads both lists from the sources and scans U+0000–U+10FFFF with the system
ICU (the same as WebKitGTK's, through ctypes; nothing is compiled). A code point of strong
RTL bidi class (R, AL), Arabic number (AN), paragraph separator (B) or direction control
(LRE, RLE, LRO, RLO, PDF, LRI, RLI, FSI, PDI) that falls in neither of them is a failure.
Unassigned ones count: ICU gives them the default class of the block (U+FEFD and U+FEFE,
AL, were the finding of the 2nd audit of M5).

First, the self-test: with the old range (U+FE70–U+FEFC) and without the escape of
U+FEFD/U+FEFE, the scan has to find both. Exits 1 if something fails; 2 if ICU is not found.

Usage: python3 -I -B dev/tests/bidi_icu.py   (or `make app-front`)
"""

import ctypes
import glob
import os
import re
import sys

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
BIDI_JS = os.path.join(ROOT, "app", "src", "lib", "bidi.js")
DISPLAY_RS = os.path.join(ROOT, "crates", "ward-core", "src", "display.rs")

# UCharDirection (uchar.h): the classes that reorder the surrounding text.
# 1 R, 2 EN... see the enum; 7 B, 11 LRE, 12 LRO, 13 AL, 14 RLE, 15 RLO, 16 PDF,
# 19 FSI, 20 LRI, 21 RLI, 22 PDI; 5 AN.
NAMES = {
    1: "R",
    5: "AN",
    7: "B",
    11: "LRE",
    12: "LRO",
    13: "AL",
    14: "RLE",
    15: "RLO",
    16: "PDF",
    19: "FSI",
    20: "LRI",
    21: "RLI",
    22: "PDI",
}
U_CONTROL_CHAR = 15  # UCharCategory Cc (Rust's `char::is_control`)


def load_icu():
    for path in sorted(glob.glob("/usr/lib/x86_64-linux-gnu/libicuuc.so.*")) + sorted(
        glob.glob("/usr/lib64/libicuuc.so.*")
    ):
        m = re.search(r"libicuuc\.so\.(\d+)$", path)
        if not m:
            continue
        lib = ctypes.CDLL(path)
        v = m.group(1)
        direction = getattr(lib, f"u_charDirection_{v}")
        char_type = getattr(lib, f"u_charType_{v}")
        for f in (direction, char_type):
            f.argtypes = [ctypes.c_int32]
            f.restype = ctypes.c_int
        return direction, char_type, path
    return None


def js_ranges():
    """The ranges of the `RTL` character class of bidi.js."""
    text = open(BIDI_JS, encoding="utf-8").read()
    m = re.search(r"const RTL = /\[(.*?)\]\+/gu;", text)
    if not m:
        raise SystemExit("bidi_icu: não achei `const RTL = /[...]+/gu;` em bidi.js")
    body = m.group(1)
    items = []
    i = 0
    while i < len(body):
        if body.startswith("\\u{", i):
            end = body.index("}", i)
            items.append(int(body[i + 3 : end], 16))
            i = end + 1
        elif body[i] == "-":
            items.append("-")
            i += 1
        elif body[i] == "\\":
            raise SystemExit(f"bidi_icu: escape não suportado em bidi.js: {body[i:i+6]!r}")
        else:
            items.append(ord(body[i]))
            i += 1
    ranges = []
    j = 0
    while j < len(items):
        if j + 2 < len(items) and items[j + 1] == "-":
            ranges.append((items[j], items[j + 2]))
            j += 3
        else:
            ranges.append((items[j], items[j]))
            j += 1
    return ranges


def rust_ranges():
    """The ranges of the `matches!` of `invisible` in display.rs."""
    text = open(DISPLAY_RS, encoding="utf-8").read()
    m = re.search(r"pub fn invisible\(c: char\) -> bool \{(.*?)\n\}", text, re.S)
    if not m or "c.is_control()" not in m.group(1):
        raise SystemExit("bidi_icu: não achei `invisible` com `c.is_control()` em display.rs")
    body = re.sub(r"//[^\n]*", "", m.group(1))
    ranges = []
    for a, b in re.findall(r"0x([0-9A-Fa-f]+)(?:\s*\.\.=\s*0x([0-9A-Fa-f]+))?", body):
        ranges.append((int(a, 16), int(b or a, 16)))
    return ranges


def inside(c, ranges):
    return any(a <= c <= b for a, b in ranges)


def scan(direction, char_type, js, rust):
    found = []
    for c in range(0x110000):
        d = direction(c)
        if d not in NAMES:
            continue
        if inside(c, js) or inside(c, rust) or char_type(c) == U_CONTROL_CHAR:
            continue
        found.append((c, NAMES[d]))
    return found


def main():
    icu = load_icu()
    if icu is None:
        print("bidi_icu: libicuuc não encontrada", file=sys.stderr)
        return 2
    direction, char_type, path = icu
    js = js_ranges()
    rust = rust_ranges()
    if not js or not rust:
        print("bidi_icu: faixas vazias", file=sys.stderr)
        return 1

    # Self-test: the state before Phase 39g (range up to U+FEFC, no escape of FEFD/FEFE).
    js_old = [(a, 0xFEFC if (a, b) == (0xFE70, 0xFEFE) else b) for a, b in js]
    rust_old = [f for f in rust if f not in ((0xFEFD, 0xFEFE), (0xFEFD, 0xFEFD), (0xFEFE, 0xFEFE))]
    old_found = {c for c, _ in scan(direction, char_type, js_old, rust_old)}
    if not {0xFEFD, 0xFEFE} <= old_found:
        print("FALHA autoteste: a varredura não achou U+FEFD/U+FEFE no estado antigo", file=sys.stderr)
        return 1

    found = scan(direction, char_type, js, rust)
    for c, name in found[:50]:
        print(f"FALHA U+{c:04X} ({name}) sem isolamento nem escape", file=sys.stderr)
    if found:
        print(f"bidi_icu: {len(found)} código(s) descoberto(s)", file=sys.stderr)
        return 1
    print(f"bidi_icu: ok ({os.path.basename(path)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
