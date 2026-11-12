#!/usr/bin/python3 -IB
"""V5.27 (S13, Phase 39): no pattern of `.dev/variations.txt` in the logs of the app, the
bridge, the test daemon and in the webview console recorded by QA.

Usage (from the root): python3 -I -B dev/tests/log-scan.py [file or folder ...]
(`make test-app-logs`). Without arguments, scans the whole `.dev/logs/`.

- Only files inside the repository's `.dev/` (resolved, without following a link out) and
  only regular files: a link or a path outside `.dev/` is refused (exits 2).
- The pattern list is never printed, nor the matched excerpt: the output says only the
  file and how many patterns matched. A pattern shorter than 6 bytes is refused (the list
  from `dev/variations.py` has none; a short one would give a false positive).
- Exits 0 clean, 1 with a finding, 2 with a usage error (no list, nothing to scan, path
  refused).
- `--self-test` checks the scan with a planted fake log (the attack has to be found) and a
  clean one, in `.dev/log-scan-self-test/`.
"""

from __future__ import annotations

import os
import stat
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEV = ROOT / ".dev"
PATTERN_LIST = DEV / "variations.txt"
MIN_PATTERN = 6
MAX_FILE = 256 * 1024 * 1024


def fail(msg: str, code: int = 2) -> None:
    print(f"varredura-logs: {msg}", file=sys.stderr)
    sys.exit(code)


def patterns(pattern_list: Path) -> list[bytes]:
    try:
        st = os.lstat(pattern_list)
    except FileNotFoundError:
        fail("falta .dev/variations.txt: rode 'make dev-variations'")
    if not stat.S_ISREG(st.st_mode):
        fail("a lista de padrões não é arquivo comum")
    if st.st_mode & 0o077:
        fail("a lista de padrões está aberta a grupo ou outros")
    v = [p for p in pattern_list.read_bytes().split(b"\n") if p]
    if not v:
        fail("a lista de padrões está vazia")
    if any(len(p) < MIN_PATTERN for p in v):
        fail("a lista tem padrão curto demais")
    return v


def inside_dev(p: Path) -> bool:
    try:
        p.resolve(strict=True).relative_to(DEV.resolve(strict=True))
        return True
    except (ValueError, FileNotFoundError):
        return False


def files(targets: list[Path]) -> list[Path]:
    found: list[Path] = []
    for a in targets:
        if not a.is_absolute():
            a = ROOT / a
        if os.path.islink(a) or not inside_dev(a):
            fail(f"recusado (fora de .dev/ ou link): {a.name}")
        if a.is_dir():
            for dirpath, dirnames, filenames in os.walk(a, followlinks=False):
                dirnames[:] = [d for d in dirnames if not os.path.islink(os.path.join(dirpath, d))]
                for n in sorted(filenames):
                    f = Path(dirpath) / n
                    if stat.S_ISREG(os.lstat(f).st_mode):
                        found.append(f)
        elif stat.S_ISREG(os.lstat(a).st_mode):
            found.append(a)
    return found


def scan(pattern_list: list[bytes], targets: list[Path]) -> int:
    fs = files(targets)
    if not fs:
        fail("nada para varrer")
    dirty_count = 0
    for f in fs:
        if os.path.getsize(f) > MAX_FILE:
            fail(f"arquivo grande demais: {f.relative_to(ROOT)}")
        body = f.read_bytes()
        n = sum(1 for p in pattern_list if p in body)
        rel = f.relative_to(ROOT)
        if n:
            dirty_count += 1
            print(f"SEGREDO  {rel}: {n} padrão(ões) de .dev/variations.txt")
        else:
            print(f"limpo    {rel} ({len(body)} bytes)")
    print(f"{len(fs)} arquivo(s), {dirty_count} com segredo")
    return 1 if dirty_count else 0


def self_test(pattern_list: list[bytes]) -> int:
    folder = DEV / "log-scan-self-test"
    folder.mkdir(mode=0o700, exist_ok=True)
    clean = folder / "clean.log"
    dirty = folder / "dirty.log"
    clean.write_bytes(b"ward-app: sem bandeja (teste)\n")
    # The attack: a log that leaked a value of the list in the middle of another line.
    dirty.write_bytes(b"[console] valor=" + pattern_list[len(pattern_list) // 2] + b" fim\n")
    errors = 0
    if scan(pattern_list, [clean]) != 0:
        print("FALHOU: log limpo acusado")
        errors += 1
    if scan(pattern_list, [dirty]) != 1:
        print("FALHOU: log com segredo não acusado")
        errors += 1
    dirty.unlink()
    clean.unlink()
    folder.rmdir()
    print("autoteste ok" if not errors else "autoteste FALHOU")
    return 1 if errors else 0


def main() -> int:
    args = sys.argv[1:]
    pattern_list = patterns(PATTERN_LIST)
    if args == ["--self-test"]:
        return self_test(pattern_list)
    targets = [Path(a) for a in args] or [DEV / "logs"]
    return scan(pattern_list, targets)


if __name__ == "__main__":
    sys.exit(main())
