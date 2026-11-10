#!/usr/bin/python3 -IB
"""Syntax check of the scripts for `make compile` (m2:V1.5; mission 2, Phase 2).

Nothing here runs the checked code:
- shell (`*.sh` and files whose shebang is bash or sh): `bash -n`;
- Python (`*.py` and python shebangs): the built-in `compile()`, the same parser as
  `py_compile`, but without writing any `__pycache__` into the repository;
- JavaScript outside `app/src` (the extension, `dev/`, `app/vite.config.js`):
  `node --check`. The Svelte app is checked by the Vite build of `make compile`.

Files come from `git ls-files` (tracked plus untracked and not ignored), with the
user's git configuration and hooks turned off. Links are skipped, and so are the
pt-BR documentation folders. Only the standard library.

Usage: python3 -I -B dev/syntax_check.py --node <path of the node binary>
Exit: 0 clean, 1 syntax errors, 2 usage error.
"""

import argparse
import os
import re
import stat
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))

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
SKIP_PREFIXES = ("docs/", ".claude/", "app/src/", "app/node_modules/")
SHEBANG = re.compile(rb"#![^\n]*\b(python3?|bash|sh|dash)\b")


def list_files():
    out = subprocess.run(
        ["/usr/bin/git", "-C", ROOT, "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        env=GIT_ENV, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if out.returncode != 0:
        print("git ls-files falhou: " + out.stderr.decode(errors="replace").strip(), file=sys.stderr)
        sys.exit(2)
    return sorted({p for p in out.stdout.decode("utf-8", "surrogateescape").split("\0") if p})


def read_regular(path):
    """Bytes of a regular file of the repository, never through a link; None otherwise."""
    full = os.path.join(ROOT, path)
    try:
        fd = os.open(full, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
    except OSError:
        return None
    try:
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            return None
        chunks = []
        while True:
            data = os.read(fd, 1 << 20)
            if not data:
                break
            chunks.append(data)
        return b"".join(chunks)
    finally:
        os.close(fd)


def classify(path, data):
    ext = os.path.splitext(path)[1]
    if ext in (".sh", ".bash"):
        return "shell"
    if ext == ".py":
        return "python"
    if ext in (".js", ".mjs", ".cjs"):
        return "js"
    m = SHEBANG.match(data[:200])
    if m:
        return "python" if m.group(1).startswith(b"python") else "shell"
    return None


def main(argv):
    parser = argparse.ArgumentParser(description="Syntax check of shell, Python and JS (make compile)")
    parser.add_argument("--node", required=True, help="absolute path of the node binary")
    args = parser.parse_args(argv)
    node = args.node
    if not os.path.isabs(node) or not os.access(node, os.X_OK):
        print(f"node não encontrado: {node}; use NODE_DIR=/caminho/do/node/bin", file=sys.stderr)
        return 2
    env = {"PATH": os.path.dirname(node) + ":/usr/bin:/bin", "LC_ALL": "C.UTF-8"}
    errors, counts = [], {"shell": 0, "python": 0, "js": 0}
    for path in list_files():
        if path.startswith(SKIP_PREFIXES) or os.path.islink(os.path.join(ROOT, path)):
            continue
        data = read_regular(path)
        if data is None:
            continue
        kind = classify(path, data)
        if kind is None:
            continue
        counts[kind] += 1
        if kind == "python":
            try:
                compile(data, path, "exec", dont_inherit=True)
            except (SyntaxError, ValueError) as exc:
                errors.append(f"{path}:{getattr(exc, 'lineno', '?')}: {type(exc).__name__}: {exc.msg if hasattr(exc, 'msg') else exc}")
            continue
        cmd = ["/bin/bash", "-n", "--", path] if kind == "shell" else [node, "--check", path]
        res = subprocess.run(cmd, cwd=ROOT, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                             stderr=subprocess.STDOUT, check=False)
        if res.returncode != 0:
            errors.append(f"{path}: " + res.stdout.decode(errors="replace").strip())
    for e in errors:
        print(e, file=sys.stderr)
    print(f"sintaxe: {counts['shell']} shell, {counts['python']} Python, {counts['js']} JS; {len(errors)} erro(s)")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
