# Shared functions of dev/services/verify.sh and dev/tests/attacks-services.sh.
# Usage: `. dev/services/common.sh` (with ROOT set).
#
# Isolated test clients (R19): `env -i` with a fake HOME in .dev/services/, ssh with
# `-F /dev/null`, its own known_hosts, no agent and none of the default identities (only
# the key asked for with -i). Password and passphrase always through `sshpass -f <file>`:
# never in argv nor in the environment.

# Fixed PATH, git without the user's config (dev/env.sh).
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"

SERVICES_DIR="$ROOT/.dev/services"
SECRETS_DIR="$SERVICES_DIR/secrets"

# Docker with nothing from the user (Phase 12b). docker reads `~/.docker/config.json`
# (credential helpers, which it executes, and `cliPluginsExtraDirs`), loads plugins from
# `~/.docker/cli-plugins` (a `docker-compose` planted there would run in place of
# compose) and follows DOCKER_HOST/DOCKER_CONTEXT to another daemon. Here:
# - DOCKER_CONFIG points to `.dev/docker/`, recreated empty (only `{}`) by every script:
#   nothing planted inside it (plugin, context, buildx builder) survives;
# - inherited DOCKER_*, COMPOSE_*, BUILDX_* and BUILDKIT_* leave the environment (compose
#   uses the default socket and the project of `dev/compose.yml`).
while IFS= read -r _var; do unset "$_var"; done \
    < <(compgen -e | grep -E '^(DOCKER_|COMPOSE_|BUILDX_|BUILDKIT_)' || true)
unset _var
DOCKER_EMPTY="$ROOT/.dev/docker"
rm -rf -- "$DOCKER_EMPTY"
mkdir -p -m 0700 -- "$ROOT/.dev"
mkdir -m 0700 -- "$DOCKER_EMPTY"
printf '{}\n' >"$DOCKER_EMPTY/config.json"
chmod 0600 "$DOCKER_EMPTY/config.json"
export DOCKER_CONFIG="$DOCKER_EMPTY"
COMPOSE=(docker compose -f "$ROOT/dev/compose.yml")
PG_IMAGE="postgres:16-alpine@sha256:0cefaabd882e42bcb2a2f3dc56d3ce4770560ba62fd7f197f1a2e008fd5731ab"

require() {
    local p
    for p in "$@"; do
        command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
    done
    [ -f "$SERVICES_DIR/known_hosts" ] || { echo "rode antes: make dev-services" >&2; exit 1; }
}

# Minimal environment of the clients: nothing from the user (no agent, no real HOME).
# curl also looks for .curlrc in the passwd home when it does not find one in $HOME:
# every `client curl` starts with `-q` (it has to be the first argument), which reads none.
client() {
    /usr/bin/env -i PATH=/usr/bin:/bin HOME="$SERVICES_DIR/client-home" LANG=C.UTF-8 "$@"
}

# Options of every test ssh client. `build_ssh_options <known_hosts>` swaps the
# known_hosts (the swapped host key test uses an old copy).
#
# ssh finds the default identities and certificates through the passwd home (getpwuid),
# not through $HOME: without `IdentityFile=none`, every client without `-i` (and the
# jump of the ProxyCommand) opened the real user's default keys. `IdentityFile=none`
# removes the default ones (`-i` still applies). `CertificateFile=/dev/null` pins the
# certificate list to an absolute path: OpenSSH 9.6 does not treat `CertificateFile=none`
# as "none" and opened a file `none` relative to the current directory.
# `dev/tests/real-home.sh` proves under strace, with the home covered by tmpfs, that no
# home path is opened.
build_ssh_options() {
    SSH_OPTIONS=(
        -F /dev/null
        -o IdentityFile=none
        -o CertificateFile=/dev/null
        -o UserKnownHostsFile="$1"
        -o GlobalKnownHostsFile=/dev/null
        -o StrictHostKeyChecking=yes
        -o IdentityAgent=none
        -o IdentitiesOnly=yes
        -o ConnectTimeout=5
        -o ServerAliveInterval=5
        -o ServerAliveCountMax=2
        -o LogLevel=ERROR
    )
}
build_ssh_options "$SERVICES_DIR/known_hosts"

# ssh_password <password-file> <port> <user> [command...]
ssh_password() {
    local file="$1" port="$2" user="$3"
    shift 3
    client sshpass -f "$file" ssh "${SSH_OPTIONS[@]}" \
        -o PubkeyAuthentication=no -o PreferredAuthentications=password,keyboard-interactive \
        -o NumberOfPasswordPrompts=1 -p "$port" "$user@127.0.0.1" "$@"
}

# ssh_key <port> <user> [command...] (deploy key, passphrase through sshpass -f)
ssh_key() {
    local port="$1" user="$2"
    shift 2
    client sshpass -f "$SECRETS_DIR/chave-passphrase" -P passphrase ssh "${SSH_OPTIONS[@]}" \
        -o PasswordAuthentication=no -o KbdInteractiveAuthentication=no \
        -i "$SERVICES_DIR/keys/acme-deploy_ed25519" -p "$port" "$user@127.0.0.1" "$@"
}

# ssh_internal [command...]: deploy on the internal host, by ProxyCommand through the jump (2224).
ssh_internal() {
    local jump
    printf -v jump 'sshpass -f %q ssh' "$SECRETS_DIR/ssh-salto"
    local o
    for o in "${SSH_OPTIONS[@]}"; do printf -v jump '%s %q' "$jump" "$o"; done
    jump+=" -o PubkeyAuthentication=no -o NumberOfPasswordPrompts=1 -p 2224 -W %h:%p salto@127.0.0.1"
    client sshpass -f "$SECRETS_DIR/chave-passphrase" -P passphrase ssh "${SSH_OPTIONS[@]}" \
        -o PasswordAuthentication=no -o KbdInteractiveAuthentication=no \
        -o ProxyCommand="$jump" \
        -i "$SERVICES_DIR/keys/acme-deploy_ed25519" deploy@ssh-interno "$@"
}

# psql_app <sql>: psql of the postgres image on the host network, password through the
# environment (`-e PGPASSWORD` without a value: docker reads it from the environment, it
# does not go in argv).
psql_app() {
    PGPASSWORD="$(cat "$SECRETS_DIR/pg-app")" docker run --rm --network host -e PGPASSWORD \
        "$PG_IMAGE" psql -h 127.0.0.1 -p 55432 -U app -d app_teste -tAc "$1"
}

# wp_login <user> <password-file>: prints the response headers of wp-login.php.
wp_login() {
    local user="$1" file="$2"
    client curl -q -sS -o /dev/null -D - --max-time 15 \
        -b 'wordpress_test_cookie=WP%20Cookie%20check' \
        --data-urlencode "log=$user" --data-urlencode "pwd@$file" \
        --data-urlencode 'testcookie=1' \
        http://localhost:8081/wp-login.php
}

# Waits for wpcli to finish (installs WordPress and creates the users).
wait_wordpress() {
    local i
    for i in $(seq 1 90); do
        case "$("${COMPOSE[@]}" ps -a --format '{{.State}} {{.ExitCode}}' wpcli 2>/dev/null)" in
            "exited 0") return 0 ;;
            exited*) echo "wpcli saiu com erro; veja: docker compose -f dev/compose.yml logs wpcli" >&2; return 1 ;;
        esac
        sleep 2
    done
    echo "wpcli não terminou a tempo" >&2
    return 1
}
