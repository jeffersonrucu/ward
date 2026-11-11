#!/usr/bin/python3 -I
"""m2:Phase 66c: the path of an integration test binary built by
`cargo test --no-run --message-format=json` (read from standard input).

`make test-playwright` builds `crates/wardd/tests/playwright.rs` on the host and runs the
binary in the browser container (no cargo there). Prints the absolute path of the one
`test` artifact named `<name>` with an executable inside `<root>/target/debug/deps/`;
anything else (none, more than one, outside the folder, a link) exits 1.

Usage: cargo test ... --no-run --message-format=json | test_binary.py <root> <name>
"""

import json
import os
import sys


def main():
    if len(sys.argv) != 3:
        print("uso: test_binary.py <raiz> <nome do teste> < saída json do cargo", file=sys.stderr)
        return 2
    root, name = os.path.realpath(sys.argv[1]), sys.argv[2]
    deps = os.path.join(root, "target", "debug", "deps") + os.sep
    found = set()
    for line in sys.stdin:
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if m.get("reason") != "compiler-artifact":
            continue
        t = m.get("target") or {}
        if t.get("name") == name and "test" in (t.get("kind") or []) and m.get("executable"):
            found.add(m["executable"])
    if len(found) != 1:
        print(f"test_binary: {len(found)} binário(s) de teste '{name}'", file=sys.stderr)
        return 1
    path = found.pop()
    if (not path.startswith(deps) or os.path.islink(path) or os.path.realpath(path) != path
            or not os.access(path, os.X_OK)):
        print("test_binary: binário fora de target/debug/deps", file=sys.stderr)
        return 1
    print(path)
    return 0


if __name__ == "__main__":
    sys.exit(main())
