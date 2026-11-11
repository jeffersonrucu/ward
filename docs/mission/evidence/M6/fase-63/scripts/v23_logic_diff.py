"""Phase 63 (m2:V2.3): compares the logic of two Rust files modulo names and comments.

Strips comments, tokenizes, and renames identifiers consistently (alpha-renaming by
first appearance, Rust keywords and primitive types kept). String literals become a
placeholder and are compared apart. Prints whether the token streams match and, if
not, the first differing windows.

Usage: python3 -I v23_logic_diff.py <old.rs> <new.rs>
"""

import difflib
import re
import sys

KEYWORDS = set(
    """as async await break const continue crate dyn else enum extern false fn for if impl in
    let loop match mod move mut pub ref return self Self static struct super trait true type
    unsafe use where while u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize f32 f64 bool
    char str String Vec Option Some None Result Ok Err Box""".split()
)
TOKEN = re.compile(
    r"""(?P<s>b?r\#*"(?:.|\n)*?"\#*|b?"(?:\\.|[^"\\])*"|b?'(?:\\.|[^'\\])')"""
    r"""|(?P<id>[A-Za-z_][A-Za-z0-9_]*)"""
    r"""|(?P<n>0x[0-9a-fA-F_]+|\d[\d_]*(?:\.\d+)?(?:[a-z]\w*)?)"""
    r"""|(?P<p>::|->|=>|==|!=|<=|>=|&&|\|\||[^\s\w])"""
)


def strip_comments(src):
    out, i, n = [], 0, len(src)
    while i < n:
        if src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
        elif src.startswith("/*", i):
            depth, i = 1, i + 2
            while i < n and depth:
                if src.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                elif src.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
        elif src[i] == '"' or (src[i] == "r" and src.startswith('r#"', i)):
            m = TOKEN.match(src, i)
            out.append(m.group(0))
            i = m.end()
        else:
            out.append(src[i])
            i += 1
    return "".join(out)


def tokens(path):
    src = strip_comments(open(path, encoding="utf-8").read())
    # Attributes (#[...], #![...]) are kept: cfg/derive changes are logic.
    toks = []
    for m in TOKEN.finditer(src):
        if m.group("s"):
            toks.append(("s", m.group("s")))
        elif m.group("id"):
            w = m.group("id")
            toks.append(("k", w) if w in KEYWORDS else ("id", w))
        else:
            toks.append(("p", m.group(0)))
    return toks


def shape(toks):
    return [{"s": "STR", "id": "ID"}.get(k, v) for k, v in toks]


def main():
    a = tokens(sys.argv[1])
    b = tokens(sys.argv[2])
    sa, sb = shape(a), shape(b)
    sm = difflib.SequenceMatcher(None, sa, sb, autojunk=False)
    ops = [op for op in sm.get_opcodes() if op[0] != "equal"]
    if not ops:
        print(f"lógica igual (estrutura idêntica; só nomes, literais e comentários): {len(a)} tokens")
    else:
        print(f"DIFERENTE na estrutura: {len(a)} x {len(b)} tokens, {len(ops)} trecho(s)")
        for tag, i1, i2, j1, j2 in ops[:20]:
            print(f"  {tag}: antes  {' '.join(v for _, v in a[max(0, i1 - 8):i2 + 4])[:240]}")
            print(f"        depois {' '.join(v for _, v in b[max(0, j1 - 8):j2 + 4])[:240]}")
    old2new, new2old, strings = {}, {}, []
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag != "equal":
            continue
        for (ka, va), (kb, vb) in zip(a[i1:i2], b[j1:j2]):
            if ka == "id":
                old2new.setdefault(va, set()).add(vb)
                new2old.setdefault(vb, set()).add(va)
            elif ka == "s" and va != vb:
                strings.append((va, vb))
    split = {k: v for k, v in old2new.items() if len(v) > 1}
    merged = {k: v for k, v in new2old.items() if len(v) > 1}
    print(f"nomes: {len(old2new)} antigos, {len(split)} viraram mais de um nome, {len(merged)} nomes novos juntam mais de um antigo")
    for k, v in list(split.items())[:15]:
        print(f"  antigo {k} -> {sorted(v)}")
    for k, v in list(merged.items())[:15]:
        print(f"  novo {k} <- {sorted(v)}")
    print(f"literais diferentes: {len(strings)}")
    for x, y in strings[:40]:
        print(f"  {x[:100]}  ->  {y[:100]}")


if __name__ == "__main__":
    main()
