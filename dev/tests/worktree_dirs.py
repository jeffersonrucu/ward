#!/usr/bin/python3 -I
"""Git directory outside the repository root, for the suites that cover the home with a
tmpfs (`real-home.sh`, `home-decoys.sh`; m2: Phase 62a).

In a linked git worktree, `<root>/.git` is a file `gitdir: <common>/worktrees/<name>`
and the objects, refs and config live in `<common>` (the main checkout's `.git`), out
of the root. Under the bwrap only the root comes back, so git fails; and under strace
the reads of `<common>` look like reads of the real home.

Usage: worktree_dirs.py <root>. Prints `<common>` (resolved, absolute) when the root
is a linked worktree whose layout checks out, and nothing when `<root>/.git` is a
directory (a normal clone: `.git` is already inside the root). Anything else exits 1
with a reason and the suite stops (fails closed). The checks, none of them taken from
the environment:
- `<root>/.git` is a regular file (not a link) with exactly `gitdir: <absolute path>`;
- that path is not a link, is `<common>/worktrees/<name>`, and its `gitdir` file
  points back to `<root>/.git` (the worktree is really this one);
- `<common>` comes from its `commondir` file, is named `.git`, is a real directory
  (no link on the way) with `HEAD`, `objects/` and `refs/`;
- `<common>` is neither `/`, the passwd home, an ancestor of the home, nor a folder
  directly in the home: so the allowance never covers the home itself, only one git
  directory.

`--self-test` plants each refused layout in a copy under `.dev/` and requires exit 1;
the right layout has to pass.
"""
import os
import pwd
import shutil
import sys
import tempfile


def fail(msg):
    print("worktree_dirs: " + msg, file=sys.stderr)
    sys.exit(1)


class Refused(Exception):
    pass


def read_one_line(path):
    if os.path.islink(path) or not os.path.isfile(path):
        raise Refused(f"{path} não é arquivo comum")
    with open(path, "rb") as f:
        data = f.read(4097)
    if len(data) > 4096:
        raise Refused(f"{path} grande demais")
    text = data.decode("utf-8", "strict")
    if text.endswith("\n"):
        text = text[:-1]
    if not text or "\n" in text or "\r" in text or "\0" in text:
        raise Refused(f"{path} não tem uma linha só")
    return text


def inside(p, base):
    return p == base or p.startswith(base.rstrip("/") + "/")


def no_link_on_the_way(p):
    return os.path.realpath(p) == p


def common_dir(root, home):
    root = os.path.realpath(root)
    dot_git = os.path.join(root, ".git")
    if os.path.islink(dot_git):
        raise Refused(".git é link")
    if os.path.isdir(dot_git):
        return None
    line = read_one_line(dot_git)
    if not line.startswith("gitdir: "):
        raise Refused(".git sem 'gitdir: '")
    gitdir = line[len("gitdir: "):]
    if not os.path.isabs(gitdir) or os.path.normpath(gitdir) != gitdir:
        raise Refused("gitdir não é caminho absoluto normalizado")
    if not os.path.isdir(gitdir) or not no_link_on_the_way(gitdir):
        raise Refused("gitdir não é pasta real")
    if read_one_line(os.path.join(gitdir, "gitdir")) != dot_git:
        raise Refused("o gitdir do worktree não aponta de volta para este .git")
    rel = read_one_line(os.path.join(gitdir, "commondir"))
    common = os.path.normpath(os.path.join(gitdir, rel))
    if not os.path.isdir(common) or not no_link_on_the_way(common):
        raise Refused("commondir não é pasta real")
    if os.path.basename(common) != ".git":
        raise Refused("commondir não se chama .git")
    if os.path.dirname(gitdir) != os.path.join(common, "worktrees"):
        raise Refused("gitdir fora de <commondir>/worktrees/")
    if not os.path.isfile(os.path.join(common, "HEAD")) or \
            not os.path.isdir(os.path.join(common, "objects")) or \
            not os.path.isdir(os.path.join(common, "refs")):
        raise Refused("commondir não tem HEAD, objects/ e refs/")
    home = os.path.realpath(home)
    if common == "/" or inside(home, common) or os.path.dirname(common) == home:
        raise Refused("commondir cobriria o home")
    if inside(common, root):
        return None  # already inside the root: nothing to add
    return common


def passwd_home():
    home = pwd.getpwuid(os.getuid()).pw_dir
    if not home or home == "/":
        fail("home do passwd indefinido")
    return home


def self_test(root):
    base = os.path.join(os.path.realpath(root), ".dev")
    os.makedirs(base, mode=0o700, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="git-common-test.", dir=base)
    failures = 0
    try:
        home = os.path.join(tmp, "home")

        def layout(name):
            top = os.path.join(home, "proj", name)
            common = os.path.join(top, ".git")
            for d in ("objects", "refs", "worktrees/w"):
                os.makedirs(os.path.join(common, d))
            with open(os.path.join(common, "HEAD"), "w") as f:
                f.write("ref: refs/heads/master\n")
            wt = os.path.join(top, "wt")
            os.makedirs(wt)
            gitdir = os.path.join(common, "worktrees", "w")
            with open(os.path.join(wt, ".git"), "w") as f:
                f.write("gitdir: " + gitdir + "\n")
            with open(os.path.join(gitdir, "gitdir"), "w") as f:
                f.write(os.path.join(wt, ".git") + "\n")
            with open(os.path.join(gitdir, "commondir"), "w") as f:
                f.write("../..\n")
            return wt, common, gitdir

        def check(label, wt, want_refused, want=None):
            nonlocal failures
            try:
                got = common_dir(wt, home)
                ok = not want_refused and got == want
            except Refused:
                ok = want_refused
            print(("ok    " if ok else "FALHA ") + "worktree_dirs: " + label)
            failures += 0 if ok else 1

        wt, common, _ = layout("good")
        check("worktree certo passa", wt, False, common)
        clone = os.path.join(home, "proj", "clone")
        os.makedirs(os.path.join(clone, ".git"))
        check("clone comum não acrescenta nada", clone, False, None)

        # Attacks: each one points the allowance at something other than a git dir.
        wt, _, gitdir = layout("home")
        with open(os.path.join(gitdir, "commondir"), "w") as f:
            f.write(home + "\n")
        check("commondir = home é recusado", wt, True)
        wt, _, gitdir = layout("root")
        with open(os.path.join(gitdir, "commondir"), "w") as f:
            f.write("/\n")
        check("commondir = / é recusado", wt, True)
        wt, _, gitdir = layout("dothome")
        fake = os.path.join(home, ".git")
        for d in ("objects", "refs", "worktrees"):
            os.makedirs(os.path.join(fake, d), exist_ok=True)
        open(os.path.join(fake, "HEAD"), "w").close()
        moved = os.path.join(fake, "worktrees", "w")
        shutil.move(gitdir, moved)
        with open(os.path.join(wt, ".git"), "w") as f:
            f.write("gitdir: " + moved + "\n")
        check("commondir direto no home (~/.git) é recusado", wt, True)
        wt, _, gitdir = layout("noback")
        with open(os.path.join(gitdir, "gitdir"), "w") as f:
            f.write("/elsewhere/.git\n")
        check("gitdir que não aponta de volta é recusado", wt, True)
        wt, common, gitdir = layout("notgit")
        os.makedirs(os.path.join(home, ".ss" "h", "worktrees", "w"))
        for d in ("objects", "refs"):
            os.makedirs(os.path.join(home, ".ss" "h", d))
        open(os.path.join(home, ".ss" "h", "HEAD"), "w").close()
        other = os.path.join(home, ".ss" "h", "worktrees", "w")
        for n in ("gitdir", "commondir"):
            shutil.copy(os.path.join(gitdir, n), os.path.join(other, n))
        with open(os.path.join(wt, ".git"), "w") as f:
            f.write("gitdir: " + other + "\n")
        check("commondir que não se chama .git é recusado", wt, True)
        wt, common, gitdir = layout("link")
        real = os.path.join(tmp, "real-gitdir")
        shutil.move(gitdir, real)
        os.symlink(real, gitdir)
        check("gitdir por link é recusado", wt, True)
        wt, _, _ = layout("dotlink")
        os.remove(os.path.join(wt, ".git"))
        os.symlink(os.path.join(home, "proj", "good", ".git"), os.path.join(wt, ".git"))
        check(".git por link é recusado", wt, True)
        wt, _, _ = layout("relative")
        with open(os.path.join(wt, ".git"), "w") as f:
            f.write("gitdir: ../.git/worktrees/w\n")
        check("gitdir relativo é recusado", wt, True)
        wt, _, _ = layout("twolines")
        with open(os.path.join(wt, ".git"), "a") as f:
            f.write("gitdir: /\n")
        check(".git com duas linhas é recusado", wt, True)
        wt, common, _ = layout("nohead")
        os.remove(os.path.join(common, "HEAD"))
        check("commondir sem HEAD é recusado", wt, True)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return failures


def main():
    args = sys.argv[1:]
    if len(args) == 2 and args[0] == "--self-test":
        sys.exit(1 if self_test(args[1]) else 0)
    if len(args) != 1:
        fail("uso: worktree_dirs.py <raiz> | --self-test <raiz>")
    try:
        common = common_dir(args[0], passwd_home())
    except (Refused, OSError, UnicodeDecodeError) as e:
        fail(f"layout do git recusado: {e}")
    if common:
        print(common)


if __name__ == "__main__":
    main()
