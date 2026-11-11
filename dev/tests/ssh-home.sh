#!/bin/bash -p
# `make test-ssh-home` (Phase 28b, scrutiny of M4): V4.27 (R19, D13, S1) end to end.
# The `ward ssh` and tunnel suite opens nothing from the passwd home.
#
# ssh finds its folder in the home through the passwd home (getpwuid), not through
# `$HOME`. So the test runs in a bwrap with the passwd home covered by tmpfs (the real
# home is never seen; only the repository comes back by bind) and `/tmp` in tmpfs too.
# The covered home holds the decoys, in the ssh folder: `config` with `Host *`, a
# `ProxyCommand` that touches a sentinel file in `/tmp`, `LocalCommand` and the fake
# agent's `IdentityAgent`; a `known_hosts` with a swapped host key; a sentinel
# `id_ed25519`; and FIFOs on the other names ssh looks for (`id_rsa`, `id_ecdsa`,
# `*-cert.pub`, `known_hosts2`, `authorized_keys`, `.pgpass`...).
#
# Three independent detectors (`wardd` refuses to start under `strace`, so the tracing
# is only on the programs it executes):
# - `/usr/bin/ssh`, `/usr/bin/sshpass` and `/usr/bin/sftp` are swapped, only inside the
#   bwrap, for a wrapper that runs the original under `strace -D -f` (the program's pid
#   does not change: the D12 agent and the bridge check the pid and the process group);
#   every file call of each `ssh`/`sshpass`/`sftp` is checked against the home;
# - inotify (ctypes, no new dependency) on every folder of the covered home: any event
#   during the suite fails it;
# - writers on the FIFOs record every open, and a fake agent in ssh's agent variable
#   (set in the environment of the daemon and the CLI) counts the connections.
# Positive controls at the end prove that each detector flags an `ssh` without the
# isolation.
#
# The test `wardd` belongs to this script only: a copy of the test vault in its own
# folder in `.dev/ssh-home.XXXXXX` and its own socket (`WARD_DEV_SOCKET_DIR`); never the
# daemon of `make dev-daemon`. Ward's `known_hosts` stays in that folder (starts empty).
#
# Needs bwrap, strace, `make dev-vault`, `make dev-variations` and the services up
# (2222, 2223, 2224, 55432). Usage: dev/tests/ssh-home.sh <wardd> <ward> (test-mode).

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
[ -n "$PASSWD_HOME" ] && [ "$PASSWD_HOME" != / ] || { echo "home do passwd indefinido" >&2; exit 1; }

WARDD="${1:?uso: ssh-home.sh <wardd> <ward>}"
WARD="${2:?uso: ssh-home.sh <wardd> <ward>}"
case "$WARDD" in /*) ;; *) WARDD="$ROOT/$WARDD" ;; esac
case "$WARD" in /*) ;; *) WARD="$ROOT/$WARD" ;; esac

# --- Inside the bwrap: the Python driver does the rest ---------------------------------
if [ "${WARD_SSH_HOME_DENTRO:-}" = 1 ]; then
    [ "$(stat -f -c %T "$PASSWD_HOME")" = tmpfs ] || { echo "o home não está coberto por tmpfs; abortando" >&2; exit 1; }
    [ "$(stat -f -c %T /tmp)" = tmpfs ] || { echo "o /tmp não está em tmpfs; abortando" >&2; exit 1; }
    extra="$(find "$PASSWD_HOME" -mindepth 1 -maxdepth 1 ! -path "$PASSWD_HOME/$(cut -d/ -f1 <<<"${ROOT#"$PASSWD_HOME"/}")" -print -quit)"
    [ -z "$extra" ] || { echo "o home coberto tem conteúdo inesperado; abortando" >&2; exit 1; }
    exec /usr/bin/python3 -I -B "$ROOT/dev/tests/ssh_home.py" "$WARDD" "$WARD" "$ROOT" "$PASSWD_HOME" "$WARD_SSH_HOME_TRAB"
fi

# --- Outside: prepares and enters the bwrap --------------------------------------------
for p in bwrap strace python3 ssh ssh-keygen sshpass sftp; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p (o teste falha fechado)" >&2; exit 1; }
done
[ -x "$WARDD" ] && [ -x "$WARD" ] || { echo "falta $WARDD ou $WARD" >&2; exit 1; }
VAULT="$ROOT/.dev/test-vault.ward"
PATTERN_LIST="$ROOT/.dev/variations.txt"
[ -f "$VAULT" ] && [ ! -L "$VAULT" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$PATTERN_LIST" ] && [ ! -L "$PATTERN_LIST" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }
case "$ROOT/" in
    "$PASSWD_HOME"/*) ;;
    *) echo "aviso: o repositório não está dentro do home do passwd" >&2 ;;
esac

umask 077
WORK_DIR="$(mktemp -d "$ROOT/.dev/ssh-home.XXXXXX")"
trap 'rm -rf -- "$WORK_DIR"' EXIT
mkdir -m 0700 "$WORK_DIR/tracos"

# Wrapper: the original program under `strace -D -f` (only file and process calls; the
# `execve` comes out with argv cut at 32 bytes and without the environment, and ssh's
# argv has no secret, V4.14). Already traced (the `ssh` child of `sshpass`), it runs the
# original directly. Writes the starting current directory next to the trace (relative
# paths). Read only by bash's `read`: no extra program in the trace.
cat >"$WORK_DIR/wrapper" <<EOF
#!/bin/bash -p
n="\${0##*/}"
real="/tmp/ward-real/\$n"
t=0
while read -r k v; do [ "\$k" = TracerPid: ] && t="\$v"; done </proc/self/status
if [ "\$t" = 0 ]; then
    printf '%s\n' "\$PWD" >"$WORK_DIR/tracos/\$n.\$\$.cwd"
    exec /usr/bin/strace -D -f -qq -e trace=%file,%process,fchdir -o "$WORK_DIR/tracos/\$n.\$\$" -- "\$real" "\$@"
fi
exec "\$real" "\$@"
EOF
chmod 0755 "$WORK_DIR/wrapper"

# Root of the bwrap: its new tmpfs (the user's, 0755), with each folder of `/` back by
# bind (links as links). So the test socket folder (`/ward-ssh-home/sock`) has only the
# user's folders above it, as the test `wardd` requires (D7): inside the user namespace
# root's folders show up as `nobody`'s and would be refused.
args=(--die-with-parent)
for d in /*; do
    n="${d#/}"
    case "$n" in tmp | lost+found | root) continue ;; esac
    if [ -L "$d" ]; then
        args+=(--symlink "$(readlink -- "$d")" "$d")
    elif [ -d "$d" ]; then
        args+=(--dev-bind "$d" "$d")
    fi
done
args+=(--tmpfs "$PASSWD_HOME" --bind "$ROOT" "$ROOT" --tmpfs /tmp --dir /tmp/ward-real
    --perms 0755 --dir /ward-ssh-home)
for p in ssh sshpass sftp; do
    args+=(--ro-bind "/usr/bin/$p" "/tmp/ward-real/$p" --ro-bind "$WORK_DIR/wrapper" "/usr/bin/$p")
done
args+=(--setenv WARD_SSH_HOME_DENTRO 1 --setenv WARD_SSH_HOME_TRAB "$WORK_DIR" --chdir "$ROOT")
rc=0
/usr/bin/env -u WARD_HOME_COBERTO bwrap "${args[@]}" -- /bin/bash -p "$ROOT/dev/tests/ssh-home.sh" "$WARDD" "$WARD" || rc=$?
exit "$rc"
