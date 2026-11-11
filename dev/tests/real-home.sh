#!/bin/bash -p
# Attack test (Phase 12a): no test client (ssh, sshpass, ssh-keygen, curl) opens a path
# of the real user's home. ssh finds the default identities through the passwd home
# (getpwuid), not through $HOME, so the fake HOME of the client wrapper is not enough.
#
# The script re-executes itself inside a bwrap with the passwd home covered by a tmpfs
# (only the repository comes back by bind): even if the protection of `common.sh`
# regresses, the real home is never read. Inside it runs each client under strace and
# fails if any home path (outside the repository) shows up in a file call. Since Phase
# 12b it also runs git and docker the way the scripts use them (dev/env.sh and
# common.sh), follows the current directory of each process (chdir/clone) and applies
# mutations to the ssh options array of common.sh: each protection removed
# (IdentityFile=none, -F /dev/null, UserKnownHostsFile, the two together for the
# *-cert.pub files) or swapped (CertificateFile=none) has to be flagged, and so do git
# and docker without the isolation; otherwise the detector or the protection has no
# effect.
#
# Needs the services up, bwrap (bubblewrap) and strace. Without them it fails (closed).
# Usage: dev/tests/real-home.sh  (or `make test-real-home`); `attacks-services.sh` and
# `verify.sh` run it before any other client.

set -euo pipefail

# Fixed PATH before any command (Phase 12b): nothing from the user's PATH runs here.
export PATH=/usr/bin:/bin
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PASSWD_HOME="$(getent passwd "$(id -u)" | cut -d: -f6)"
[ -n "$PASSWD_HOME" ] && [ "$PASSWD_HOME" != / ] || { echo "home do passwd indefinido" >&2; exit 1; }
for p in bwrap strace getent python3; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p (o teste falha fechado)" >&2; exit 1; }
done

# --- Outside the sandbox: re-executes inside it -------------------------------------------
if [ "${WARD_HOME_COVERED:-}" != 1 ]; then
    args=(--die-with-parent --dev-bind / / --tmpfs "$PASSWD_HOME")
    # Only the repository comes back; /tmp too, in case the home tmpfs covered it.
    args+=(--bind "$ROOT" "$ROOT" --bind /tmp /tmp --setenv WARD_HOME_COVERED 1 --chdir "$ROOT")
    exec bwrap "${args[@]}" -- /bin/bash -p "$ROOT/dev/tests/real-home.sh" "$@"
fi

# --- Inside: verifies that the home is really covered --------------------------------------
case "$ROOT/" in
    "$PASSWD_HOME"/*) ;;
    *) echo "aviso: o repositório não está dentro do home do passwd" >&2 ;;
esac
[ "$(stat -f -c %T "$PASSWD_HOME")" = tmpfs ] || { echo "o home não está coberto por tmpfs; abortando" >&2; exit 1; }
# Nothing in the home besides the path to the repository (the tmpfs starts empty).
extra="$(find "$PASSWD_HOME" -mindepth 1 -maxdepth 1 ! -path "$PASSWD_HOME/$(cut -d/ -f1 <<<"${ROOT#"$PASSWD_HOME"/}")" -print -quit)"
[ -z "$extra" ] || { echo "o home coberto tem conteúdo inesperado; abortando" >&2; exit 1; }

# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"
require sshpass ssh ssh-keygen curl git docker

TMP="$(mktemp -d)"
chmod 0700 "$TMP"
trap 'rm -rf "$TMP"' EXIT
# Current directory of the clients, with decoys: a relative path (e.g. `none`) opened
# by ssh would show up as an access to this directory and is flagged too.
mkdir -p "$TMP/cwd"
: >"$TMP/cwd/none"

MUT=() # mutated options of the controls (see mutated_options)
failures=0
ok() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FALHA %s\n' "$1" >&2; failures=$((failures + 1)); }

# trace_run <name> <function> [args...]: runs the function under strace (-f, file
# calls) with the minimal PATH and the current directory in $TMP/cwd. The client's exit
# code does not matter here: only what it tried to open.
trace_run() {
    local name="$1"
    shift
    (
        cd "$TMP/cwd"
        # The functions and variables of common.sh go to the traced bash through declare.
        PATH=/usr/bin:/bin strace -f -qq -e trace=%file,%process,fchdir -o "$TMP/$name.strace" \
            /bin/bash -p -c "$(declare -f); $(declare -p SSH_OPTIONS MUT COMPOSE PG_IMAGE SERVICES_DIR SECRETS_DIR ROOT TMP); \"\$@\"" \
            _ "$@" </dev/null >/dev/null 2>&1 && echo 0 >"$TMP/$name.rc" || echo 1 >"$TMP/$name.rc"
    )
}
# trace_ok: like trace_run, but the client has to succeed (an empty trace of a client
# that did not even connect proves nothing).
trace_ok() {
    trace_run "$@"
    [ "$(cat "$TMP/$1.rc")" = 0 ] || fail "$1: o cliente devia ter entrado e não entrou"
}

# flagged <file.strace>: prints the home paths (outside the repository) or the paths
# relative to the decoy directory that show up in the trace. Never prints content.
flagged() {
    python3 -I -B - "$1" "$PASSWD_HOME" "$ROOT" "$TMP/cwd" <<'PY'
import os, re, sys
trace, home, root, cwd0 = sys.argv[1:5]
# First quoted argument of each call (the path). A relative path is resolved by the
# current directory of that process: it starts at cwd0, follows the chdir() calls that
# succeeded and passes from parent to child on clone/fork (Phase 12b: `git -C` changes
# directory). After an fchdir(), or with a dirfd other than AT_FDCWD, it cannot be
# resolved: flag it.
line_re = re.compile(r'^(\d+)\s+(?:<\.\.\. (\w+) resumed>|(\w+)\()(.*)')
path_re = re.compile(r'"((?:[^"\\]|\\.)*)"')
return_re = re.compile(r'=\s+(-?\d+)(?:\s|$)')
WITH_DIRFD = {
    'openat', 'openat2', 'newfstatat', 'fstatat64', 'faccessat', 'faccessat2', 'statx',
    'readlinkat', 'mkdirat', 'mknodat', 'unlinkat', 'renameat', 'renameat2', 'linkat',
    'symlinkat', 'fchmodat', 'fchmodat2', 'fchownat', 'futimesat', 'utimensat',
    'execveat', 'name_to_handle_at', 'open_tree', 'move_mount', 'fspick',
}
FORKS = {'clone', 'clone3', 'fork', 'vfork'}
def inside(p, base):
    return p == base or p.startswith(base.rstrip('/') + '/')
lines = []
parent_of = {}
for line in open(trace, errors='replace'):
    m = line_re.match(line)
    if not m:
        continue
    pid, resumed, call, rest = m.group(1), m.group(2), m.group(3), m.group(4)
    name = resumed or call
    r = return_re.search(rest.rsplit(')', 1)[-1] if ')' in rest else rest)
    ret = int(r.group(1)) if r else None
    if name in FORKS and ret and ret > 0:
        parent_of[str(ret)] = pid
    lines.append((pid, name, bool(resumed), rest, ret))
cwd_of = {}
first = None
flagged = set()
for pid, call, resumed, rest, ret in lines:
    if pid not in cwd_of:
        if first is None:
            first = pid
            cwd_of[pid] = cwd0
        else:
            cwd_of[pid] = cwd_of.get(parent_of.get(pid), cwd0)
    current = cwd_of[pid]
    if call == 'fchdir' and ret == 0:
        cwd_of[pid] = None
        continue
    if resumed:
        continue  # the path was seen on the <unfinished ...> line
    if call in ('execve', 'execveat'):
        # argv may name repository paths; only the binary counts.
        rest = rest.split(',', 1)[0]
    paths = path_re.findall(rest)[:2]
    if call in ('readlink', 'readlinkat'):
        # The 2nd quoted text is the output buffer (the target read), not an opened path.
        paths = paths[:1]
    if call == 'chdir' and paths and ret == 0:
        c = paths[0]
        if current is None and not c.startswith('/'):
            cwd_of[pid] = None
        else:
            cwd_of[pid] = os.path.normpath(os.path.join(current or '/', c))
        target = cwd_of[pid]
        if target and inside(target, home) and not (inside(target, root) or inside(root, target)):
            flagged.add(target)
        continue
    for c in paths:
        if not c:
            continue
        if not c.startswith('/'):
            if call in WITH_DIRFD and not rest.startswith('AT_FDCWD'):
                flagged.add('(relativo a um dirfd) ' + call)
                continue
            if current is None:
                flagged.add('(relativo depois de fchdir) ' + call)
                continue
            c = os.path.join(current, c)
        p = os.path.normpath(c)
        if p == cwd0:
            continue  # the decoy directory itself (getcwd/stat of "."), not a file in it
        if inside(p, cwd0):
            flagged.add('(relativo ao diretório atual) ' + os.path.relpath(p, cwd0))
            continue
        if not inside(p, home):
            continue
        if inside(p, root) or inside(root, p):
            continue  # the repository, or a directory on the way to it
        flagged.add(p)
for a in sorted(flagged):
    print(a)
PY
}

verify() { # description name
    local out
    out="$(flagged "$TMP/$2.strace")"
    if [ ! -s "$TMP/$2.strace" ] || ! grep -q 'execve("/usr/bin/\(ssh\|curl\|ssh-keygen\|git\|docker\|python3\)"' "$TMP/$2.strace"; then
        fail "$1: o trace não pegou o cliente"
    elif [ -n "$out" ]; then
        fail "$1 abriu caminho fora do permitido:"
        sed 's/^/        /' <<<"$out" >&2
    else
        ok "$1 não abre nada do home real"
    fi
}

wrong_password() { head -c 48 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24; }
wrong_password >"$TMP/wrong-password"
chmod 0600 "$TMP/wrong-password"

auth_methods() {
    client ssh -v "${SSH_OPTIONS[@]}" -o BatchMode=yes -o PreferredAuthentications=none \
        -p 2223 ninguem@127.0.0.1 true
}
channel() { printf 'x' | ssh_password "$SECRETS_DIR/ssh-deploy" 2222 deploy -W postgres:5432; }
sftp_ls() { printf 'ls\n' | ssh_password "$SECRETS_DIR/ssh-sftpuser" 2222 sftpuser -s sftp; }
keygen_y() { ssh-keygen -y -P '' -f "$SERVICES_DIR/keys/acme-deploy_ed25519"; }
# The way generate.py generates the keys (ssh-keygen -t … -f), in a temporary directory.
keygen_new() { ssh-keygen -q -t ed25519 -N '' -C ward-test-home -f "$TMP/new-key"; }
wp() { wp_login wp_admin "$SECRETS_DIR/wp-admin"; }
# git and docker the way the scripts use them (Phase 12b): environment of dev/env.sh and common.sh.
git_ignore() { git -C "$ROOT" check-ignore -q .dev/services/secrets/pg-app; }
compose_ps() { "${COMPOSE[@]}" ps -q postgres; }
psql_one() { psql_app 'select 1'; }
# Phase 28 Python Postgres client (V4.29): password from a file, never in argv.
pg_select_one() {
    client PGHOST=127.0.0.1 PGPORT=55432 PGUSER=app PGDATABASE=app_teste \
        WARD_TEST_PG_PASSWORD_FILE="$SECRETS_DIR/pg-app" python3 -I -B "$ROOT/dev/tests/pg_select.py" 'select 1'
}

trace_ok password ssh_password "$SECRETS_DIR/ssh-deploy" 2222 deploy whoami
verify "ssh_password (2222, deploy)" password
trace_run wrong ssh_password "$TMP/wrong-password" 2224 salto true
verify "ssh_password com senha errada (2224)" wrong
trace_ok key ssh_key 2223 deploy whoami
verify "ssh_key (2223, -i)" key
trace_ok internal ssh_internal hostname
verify "ssh_internal (ProxyCommand pelo salto)" internal
trace_run methods auth_methods
verify "ssh -v sem credencial (2223)" methods
trace_run channel channel
verify "ssh -W (túnel até o postgres)" channel
trace_ok sftp sftp_ls
verify "sftp do sftpuser" sftp
trace_run keygen keygen_y
verify "ssh-keygen -y da chave do deploy" keygen
trace_ok keygen-new keygen_new
verify "ssh-keygen -t ed25519 -f (como no generate.py)" keygen-new
trace_ok wp wp
verify "curl no wp-login" wp
trace_ok git git_ignore
verify "git check-ignore (sem config do usuário)" git
trace_ok compose compose_ps
verify "docker compose ps (DOCKER_CONFIG vazio)" compose
trace_ok psql psql_one
verify "docker run do psql_app" psql
trace_ok pg-select pg_select_one
verify "dev/tests/pg_select.py (cliente Postgres da Fase 28)" pg-select

# Mutations of the ssh options of common.sh (Phase 12b): each protection, removed or
# swapped for the wrong form, MUST be flagged. Safe: here the home is an empty tmpfs.
# mutated_options <mutation>...: MUT = the ssh options with each listed `-x value`
# removed ("-o IdentityFile=none") or swapped ("-o CertificateFile=/dev/null=>CertificateFile=none").
mutated_options() {
    MUT=()
    local i pair m done_ applied=0
    for ((i = 0; i < ${#SSH_OPTIONS[@]}; i += 2)); do
        pair="${SSH_OPTIONS[i]} ${SSH_OPTIONS[i + 1]}"
        done_=0
        for m in "$@"; do
            if [ "$m" = "$pair" ]; then
                done_=1
            elif [[ "$m" == *"=>"* ]] && [ "${m%%=>*}" = "$pair" ]; then
                done_=1
                MUT+=("${SSH_OPTIONS[i]}" "${m#*=>}")
            fi
        done
        if [ "$done_" -eq 1 ]; then applied=$((applied + 1)); else MUT+=("${SSH_OPTIONS[i]}" "${SSH_OPTIONS[i + 1]}"); fi
    done
    [ "$applied" -eq "$#" ] || { echo "mutação não casou com SSH_OPTIONS: $*" >&2; exit 1; }
}
mutated_ssh() {
    client sshpass -f "$SECRETS_DIR/ssh-deploy" ssh "${MUT[@]}" -o PubkeyAuthentication=no \
        -p 2222 deploy@127.0.0.1 true
}
# mutation <name> <description> <pattern that has to show up among the flagged> <mutation>...
mutation() {
    local name="$1" d="$2" pattern="$3"
    shift 3
    mutated_options "$@"
    trace_run "$name" mutated_ssh
    if flagged "$TMP/$name.strace" | grep -q -- "$pattern"; then
        ok "mutação acusada: $d"
    else
        fail "mutação NÃO acusada (o detector ou a proteção não fazem efeito): $d"
    fi
}
mutation m-ident "sem IdentityFile=none (abre as chaves padrão)" "/id_ed25519$" "-o IdentityFile=none"
mutation m-cert-none "CertificateFile=none no lugar de /dev/null (abre ./none)" "relativo ao diretório atual" \
    "-o CertificateFile=/dev/null=>CertificateFile=none"
mutation m-without-both "sem IdentityFile=none e sem CertificateFile=/dev/null (abre os -cert.pub)" "/id_ed25519-cert.pub$" \
    "-o IdentityFile=none" "-o CertificateFile=/dev/null"
mutation m-config "sem -F /dev/null (lê a config do usuário)" "/config$" "-F /dev/null"
mutation m-known "sem UserKnownHostsFile próprio (lê o known_hosts do usuário)" "/known_hosts$" \
    "-o UserKnownHostsFile=$SERVICES_DIR/known_hosts"
# CertificateFile=/dev/null is the second barrier: without IdentityFile=none but with it,
# no *-cert.pub of the home is looked up (only the keys, which the mutation above flags).
if flagged "$TMP/m-ident.strace" | grep -q -- '-cert.pub$'; then
    fail "com CertificateFile=/dev/null o ssh ainda procura *-cert.pub no home"
else
    ok "CertificateFile=/dev/null tira os *-cert.pub do home mesmo sem IdentityFile=none"
fi

# git and docker controls (Phase 12b): without the isolation, both read the home.
control_git() { /usr/bin/env -u GIT_CONFIG_GLOBAL -u GIT_CONFIG_COUNT -u GIT_CONFIG_NOSYSTEM git -C "$ROOT" check-ignore -q .dev/x; }
trace_run control-git control_git
if [ -n "$(flagged "$TMP/control-git.strace")" ]; then ok "controle: git sem o isolamento é acusado"; else fail "controle: git sem o isolamento não foi acusado"; fi
control_docker() { /usr/bin/env -u DOCKER_CONFIG docker compose -f "$ROOT/dev/compose.yml" ps -q; }
trace_run control-docker control_docker
if [ -n "$(flagged "$TMP/control-docker.strace")" ]; then ok "controle: docker sem DOCKER_CONFIG é acusado"; else fail "controle: docker sem DOCKER_CONFIG não foi acusado"; fi

# curl control: without `-q`, it looks for .curlrc in the passwd home and is flagged.
control_curl() { client curl -s -o /dev/null --max-time 5 http://localhost:8083/login; }
trace_run curl control_curl
if [ -n "$(flagged "$TMP/curl.strace")" ]; then
    ok "controle: curl sem -q é acusado"
else
    fail "controle: curl sem -q não foi acusado"
fi

if [ "$failures" -ne 0 ]; then
    printf '\n%d cliente(s) tocaram o home real ou o detector falhou.\n' "$failures" >&2
    exit 1
fi
printf '\nNenhum cliente de teste abre caminho do home real.\n'
