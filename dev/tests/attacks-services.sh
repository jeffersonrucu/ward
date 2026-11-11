#!/bin/bash -p
# Attack tests of the test services (Phase 12). Every attack has to FAIL.
# Needs `make dev-services` and the compose up (`docker compose -f dev/compose.yml
# up -d --build`). Usage: dev/tests/attacks-services.sh  (or `make test-attacks-services`)
#
# Never prints a secret: the searches use `grep -q`/`-c` and the messages only say where.
# At the end phish's posts.log is empty and the password ssh service is back to a valid
# host key (the key swap of the `host_key` attack is permanent, as in V4.28).

set -euo pipefail

# Fixed PATH before any command (Phase 12b): nothing from the user's PATH runs here.
export PATH=/usr/bin:/bin
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"
require docker sshpass ssh ssh-keygen curl python3

TMP="$(mktemp -d)"
chmod 0700 "$TMP"
trap 'rm -rf "$TMP"' EXIT

failures=0
ok() { printf 'ok    %s\n' "$1"; }
failed() { printf 'FALHA %s\n' "$1" >&2; failures=$((failures + 1)); }
expect_refused() { # description command...: the command HAS to fail
    local d="$1"
    shift
    if "$@" >/dev/null 2>&1; then failed "$d (foi aceito)"; else ok "$d"; fi
}

wait_wordpress

# Before any other client: proves, under strace and with the home covered by tmpfs, that
# no test client opens a path of the real user's home (Phase 12a).
"$ROOT/dev/tests/real-home.sh" || { echo "clientes de teste tocam o home real; abortando" >&2; exit 1; }

# Random wrong password, in a file (never in argv).
head -c 48 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24 >"$TMP/wrong-password"
chmod 0600 "$TMP/wrong-password"

# --- Phase 12b: docker and compose without anything from the user's environment ---------
# A DOCKER_CONFIG with a planted `docker-compose` plugin, DOCKER_HOST/DOCKER_CONTEXT
# pointing elsewhere, COMPOSE_FILE/COMPOSE_PROJECT_NAME swapped, BASH_ENV and a PATH with
# a fake `docker` in front. Through dev/services/compose.sh and through `make`, compose
# has to find the real services, without running anything planted.
docker_without_user_env() {
    local bad="$TMP/docker-bad" marks="$TMP/docker-marks" fakes="$TMP/docker-fakes"
    mkdir -p "$bad/cli-plugins" "$fakes"
    cat >"$bad/cli-plugins/docker-compose" <<EOF
#!/bin/sh
echo "plugin \$*" >>"$marks"
if [ "\$1" = docker-cli-plugin-metadata ]; then
    echo '{"SchemaVersion":"0.1.0","Vendor":"teste","Version":"v9.9.9","ShortDescription":"falso"}'
fi
exit 0
EOF
    chmod +x "$bad/cli-plugins/docker-compose"
    printf '{"credsStore": "bad", "cliPluginsExtraDirs": ["%s/cli-plugins"]}\n' "$bad" >"$bad/config.json"
    printf '#!/bin/sh\necho docker-falso >>"%s"\nexit 0\n' "$marks" >"$fakes/docker"
    chmod +x "$fakes/docker"
    printf 'echo bash_env >>"%s"\n' "$marks" >"$TMP/bash_env"
    local hostile=(DOCKER_CONFIG="$bad" DOCKER_HOST=unix:///does/not/exist.sock DOCKER_CONTEXT=bad
        COMPOSE_FILE=/does/not/exist.yml COMPOSE_PROJECT_NAME=other-project BASH_ENV="$TMP/bash_env"
        PATH="$fakes:$PATH")
    # Control: the planted plugin runs when the user's DOCKER_CONFIG counts.
    rm -f "$marks"
    DOCKER_CONFIG="$bad" /usr/bin/docker compose version >/dev/null 2>&1 || true
    if grep -q '^plugin compose' "$marks" 2>/dev/null; then
        ok "controle: plugin docker-compose plantado no DOCKER_CONFIG roda sem o isolamento"
    else
        failed "controle: plugin plantado não rodou nem sem isolamento (teste sem efeito)"
    fi
    local id
    rm -f "$marks"
    id="$(/usr/bin/env "${hostile[@]}" "$ROOT/dev/services/compose.sh" ps -q postgres 2>/dev/null || true)"
    if [ -n "$id" ] && [ ! -e "$marks" ]; then
        ok "compose.sh com DOCKER_CONFIG/DOCKER_HOST/COMPOSE_*/PATH/BASH_ENV hostis acha o postgres e não roda nada plantado"
    else
        failed "compose.sh usou o ambiente do usuário (id: ${id:-vazio}, marcas: $(tr '\n' ' ' <"$marks" 2>/dev/null))"
    fi
    rm -f "$marks"
    id="$(cd "$ROOT" && /usr/bin/env "${hostile[@]}" /usr/bin/make -s dev-services-status 2>/dev/null | grep -c '^postgres running' || true)"
    if [ "$id" -ge 1 ] && [ ! -e "$marks" ]; then
        ok "make dev-services-status com o mesmo ambiente hostil lista os serviços de verdade"
    else
        failed "make dev-services-status usou o ambiente do usuário (marcas: $(tr '\n' ' ' <"$marks" 2>/dev/null))"
    fi
    # Plugin planted in .dev/docker/ itself: the dir is recreated empty on every use.
    mkdir -p "$ROOT/.dev/docker/cli-plugins"
    cp "$bad/cli-plugins/docker-compose" "$ROOT/.dev/docker/cli-plugins/docker-compose"
    rm -f "$marks"
    id="$("$ROOT/dev/services/compose.sh" ps -q postgres 2>/dev/null || true)"
    if [ -n "$id" ] && [ ! -e "$marks" ] && [ ! -e "$ROOT/.dev/docker/cli-plugins" ]; then
        ok "plugin plantado em .dev/docker/cli-plugins é apagado antes de usar o docker"
    else
        failed "plugin plantado em .dev/docker/ rodou ou ficou"
    fi
    # This script's DOCKER_CONFIG is still the empty one (compose.sh recreated it the same).
    [ "$(cat "$DOCKER_CONFIG/config.json")" = '{}' ] || failed ".dev/docker/config.json não é {}"
}

# --- Ports only on 127.0.0.1 ---------------------------------------------------------------
ports_loopback_only() {
    local c lines
    lines="$("${COMPOSE[@]}" ps -q | xargs -r -n1 docker port)"
    if [ -n "$lines" ] && ! grep -vqE -- '-> 127\.0\.0\.1:[0-9]+$' <<<"$lines"; then
        ok "portas publicadas só em 127.0.0.1 ($(wc -l <<<"$lines") portas)"
    else
        failed "porta publicada fora de 127.0.0.1"
    fi
    # Through the machine's network IP, no port answers.
    local ip
    ip="$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p')"
    if [ -z "$ip" ]; then
        ok "sem IP de rede além do loopback (nada a testar)"
        return
    fi
    local p open_ports=0
    for p in 55432 2222 2223 2224 8081 8082 8083 8084; do
        if timeout 3 bash -c "</dev/tcp/$ip/$p" 2>/dev/null; then
            open_ports=$((open_ports + 1))
            failed "porta $p responde no IP de rede $ip"
        fi
    done
    if [ "$open_ports" -eq 0 ]; then ok "nenhuma porta responde pelo IP de rede $ip"; fi
}

# --- The internal ssh host is not reachable from the host, not even by the container IP --
internal_unreachable() {
    local id ips ip found=0
    id="$("${COMPOSE[@]}" ps -q ssh-interno)"
    ips="$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' "$id")"
    for ip in $ips; do
        if timeout 3 bash -c "</dev/tcp/$ip/22" 2>/dev/null; then
            found=1
            failed "ssh-interno responde do host em $ip:22"
        fi
    done
    if [ "$found" -eq 0 ]; then ok "ssh-interno não responde do host (IPs: ${ips:-nenhum})"; fi
}

# --- Authentication methods ---------------------------------------------------------------
auth_methods() { # port: methods the server offers to a client without a credential
    { client ssh -v "${SSH_OPTIONS[@]}" -o BatchMode=yes \
        -o PreferredAuthentications=none -p "$1" ninguem@127.0.0.1 true 2>&1 |
        sed -n 's/.*Authentications that can continue: //p' | head -n1 | tr -d '\r'; } || true
}
authentication() {
    local m
    m="$(auth_methods 2223)"
    [ "$m" = "publickey" ] && ok "ssh-chave 2223 só oferece publickey" || failed "ssh-chave 2223 oferece '$m'"
    m="$(auth_methods 2222)"
    [ "$m" = "password" ] && ok "ssh-senha 2222 só oferece password" || failed "ssh-senha 2222 oferece '$m'"
    expect_refused "ssh-chave 2223 recusa a senha do deploy" ssh_password "$SECRETS_DIR/ssh-deploy" 2223 deploy true
    expect_refused "ssh-senha 2222 recusa senha errada" ssh_password "$TMP/wrong-password" 2222 deploy true
    expect_refused "ssh-salto 2224 recusa senha errada" ssh_password "$TMP/wrong-password" 2224 salto true
    expect_refused "root recusado em 2222 (com a senha do deploy)" ssh_password "$SECRETS_DIR/ssh-deploy" 2222 root true
    expect_refused "root recusado em 2224 (com a senha do salto)" ssh_password "$SECRETS_DIR/ssh-salto" 2224 root true
    expect_refused "deploy não entra em 2222 com a senha do salto" ssh_password "$SECRETS_DIR/ssh-salto" 2222 deploy true
}

# --- sftpuser only sftp, no tunnel --------------------------------------------------------
sftp_restricted() {
    local output
    output="$(ssh_password "$SECRETS_DIR/ssh-sftpuser" 2222 sftpuser id 2>&1 || true)"
    if grep -q 'uid=' <<<"$output"; then failed "sftpuser executou comando"; else ok "sftpuser não executa comando (só sftp)"; fi
    local cfg
    cfg="$("${COMPOSE[@]}" exec -T ssh-senha sshd -T -C user=sftpuser,host=x,addr=10.0.0.1)"
    if grep -qx 'allowtcpforwarding no' <<<"$cfg" && grep -qx 'forcecommand internal-sftp' <<<"$cfg" \
        && grep -qx 'chrootdirectory /srv/sftp' <<<"$cfg"; then
        ok "sftpuser: forcecommand internal-sftp, chroot e sem encaminhamento"
    else
        failed "configuração do sftpuser"
    fi
    expect_refused "sftpuser não abre túnel até o postgres" \
        ssh_password "$SECRETS_DIR/ssh-sftpuser" 2222 sftpuser -W postgres:5432 </dev/null
}

# --- Forwarding only to the intended destination -------------------------------------------
# `-W destination` opens a direct-tcpip channel; the channel only opens if sshd allows it.
# Positive proof: the destination answers through the channel (sshd sends the "SSH-"
# banner; postgres answers "N"/"S" to the SSLRequest). A refused channel returns nothing.
channel_opens() { # file port user destination
    local output
    output="$( { printf '\x00\x00\x00\x08\x04\xd2\x16\x2f'; sleep 2; } |
        ssh_password "$1" "$2" "$3" -W "$4" 2>/dev/null | head -c 4 | tr -dc 'A-Za-z-' || true)"
    [ -n "$output" ]
}
tunnels() {
    if channel_opens "$SECRETS_DIR/ssh-deploy" 2222 deploy postgres:5432; then ok "deploy@2222 abre túnel até postgres:5432 (V4.29)"; else failed "túnel deploy@2222 até postgres"; fi
    if channel_opens "$SECRETS_DIR/ssh-deploy" 2222 deploy ssh-chave:22; then failed "deploy@2222 abriu túnel até ssh-chave:22"; else ok "deploy@2222 não abre túnel fora de postgres:5432"; fi
    if channel_opens "$SECRETS_DIR/ssh-salto" 2224 salto postgres:5432; then failed "salto abriu túnel até postgres:5432"; else ok "salto não abre túnel fora de ssh-interno:22"; fi
    if channel_opens "$SECRETS_DIR/ssh-salto" 2224 salto ssh-interno:22; then ok "salto abre túnel até ssh-interno:22"; else failed "salto até ssh-interno:22"; fi
}

# --- Host key: unknown and swapped are refused ----------------------------------------------
host_key() {
    : >"$TMP/known_hosts_empty"
    build_ssh_options "$TMP/known_hosts_empty"
    expect_refused "host desconhecido recusado (StrictHostKeyChecking=yes, sem TOFU)" \
        ssh_password "$SECRETS_DIR/ssh-deploy" 2222 deploy true
    cp "$SERVICES_DIR/known_hosts" "$TMP/known_hosts_old"
    python3 -I -B "$ROOT/dev/services/generate.py" --rotate-host-key ssh-senha >/dev/null
    "${COMPOSE[@]}" restart ssh-senha >/dev/null 2>&1
    local i
    # Waits for the sshd banner (docker-proxy accepts the connection before sshd is up).
    for i in $(seq 1 30); do
        timeout 2 bash -c 'exec 3<>/dev/tcp/127.0.0.1/2222; head -c 4 <&3' 2>/dev/null | grep -q '^SSH-' && break
        sleep 1
    done
    build_ssh_options "$TMP/known_hosts_old"
    local output
    output="$( (ssh_password "$SECRETS_DIR/ssh-deploy" 2222 deploy true && echo ACEITO) 2>&1 || true)"
    if grep -q 'HOST IDENTIFICATION HAS CHANGED' <<<"$output" && ! grep -q ACEITO <<<"$output"; then
        ok "chave de host trocada recusada com aviso (V4.28)"
    else
        failed "chave de host trocada não foi recusada"
    fi
    build_ssh_options "$SERVICES_DIR/known_hosts"
    if [ "$(ssh_password "$SECRETS_DIR/ssh-deploy" 2222 deploy whoami 2>/dev/null)" = deploy ]; then
        ok "com o known_hosts novo o ssh-senha volta a entrar"
    else
        failed "ssh-senha depois da troca de chave"
    fi
}

# --- WordPress ---------------------------------------------------------------------------
wordpress() {
    local headers
    headers="$(wp_login wp_admin "$TMP/wrong-password" 2>/dev/null || true)"
    if grep -qi 'set-cookie: wordpress_logged_in' <<<"$headers"; then failed "wordpress aceitou senha errada"; else ok "wordpress recusa senha errada"; fi
    headers="$(wp_login wp_admin "$SECRETS_DIR/wp-editor" 2>/dev/null || true)"
    if grep -qi 'set-cookie: wordpress_logged_in' <<<"$headers"; then failed "wp_admin entrou com a senha do wp_editor"; else ok "wp_admin não entra com a senha do wp_editor"; fi
}

# --- totp (8083) ---------------------------------------------------------------------------
# Code computed here by the standard library (RFC 6238); seed read from the file.
totp_code() { # user [offset-in-periods]
    python3 -I -B - "$SECRETS_DIR/totp-contas.json" "$1" "${2:-0}" <<'PY'
import base64, hashlib, hmac, json, struct, sys, time
accounts = {c["usuario"]: c for c in json.load(open(sys.argv[1]))}
s = accounts[sys.argv[2]]["totp_semente"]
k = base64.b32decode(s + "=" * (-len(s) % 8))
n = int(time.time()) // 30 + int(sys.argv[3])
m = hmac.new(k, struct.pack(">Q", n), hashlib.sha1).digest()
o = m[-1] & 15
print(f"{(struct.unpack('>I', m[o:o+4])[0] & 0x7FFFFFFF) % 1000000:06d}")
PY
}
post_totp() { # user password-file code -> HTTP status
    client curl -q -s -o /dev/null -w '%{http_code}' --max-time 10 \
        --data-urlencode "usuario=$1" --data-urlencode "senha@$2" --data-urlencode "codigo=$3" \
        http://localhost:8083/login
}
totp() {
    local good bad_code
    good="$(totp_code wp_admin)"
    [ "$(post_totp wp_admin "$SECRETS_DIR/wp-admin" "$good")" = 200 ] && ok "totp aceita senha e código certos" || failed "totp recusou o login certo"
    bad_code="$(printf '%06d' $(( (10#$good + 500000) % 1000000 )))"
    [ "$(post_totp wp_admin "$SECRETS_DIR/wp-admin" "$bad_code")" = 401 ] && ok "totp recusa código errado" || failed "totp aceitou código errado"
    [ "$(post_totp wp_admin "$SECRETS_DIR/wp-admin" "")" = 401 ] && ok "totp recusa código vazio" || failed "totp aceitou sem código"
    [ "$(post_totp wp_admin "$SECRETS_DIR/wp-admin" "$(totp_code wp_admin 10)")" = 401 ] && ok "totp recusa código de 5 min à frente" || failed "totp aceitou código fora da janela"
    [ "$(post_totp wp_admin "$TMP/wrong-password" "$good")" = 401 ] && ok "totp recusa senha errada com código certo" || failed "totp aceitou senha errada"
    [ "$(post_totp wp_admin "$SECRETS_DIR/wp-editor" "$good")" = 401 ] && ok "totp recusa a senha de outra conta" || failed "totp aceitou senha de outra conta"
    local st
    st="$(client curl -q -s -o /dev/null -w '%{http_code}' --max-time 10 -H 'Content-Type: application/x-www-form-urlencoded' \
        --data-binary @<(head -c 20000 /dev/zero | tr '\0' a) http://localhost:8083/login)"
    [ "$st" = 413 ] && ok "totp recusa corpo de 20 KB (413)" || failed "totp aceitou corpo grande ($st)"
}

# --- phish (8082) and redir (8084) ----------------------------------------------------------
phish() {
    local mark
    mark="marca-$(head -c 12 /dev/urandom | base64 | tr -dc 'a-z0-9' | head -c 12)"
    client curl -q -s -o /dev/null --max-time 10 --data-urlencode "log=$mark" --data-urlencode 'pwd=x' \
        http://localhost:8082/wp-login.php
    if "${COMPOSE[@]}" exec -T phish grep -q "$mark" /registro/posts.log; then
        ok "phish grava o POST em /registro/posts.log"
    else
        failed "phish não gravou o POST"
    fi
    local mode
    mode="$("${COMPOSE[@]}" exec -T phish stat -c '%a' /registro/posts.log)"
    [ "$mode" = 600 ] && ok "posts.log é 0600" || failed "posts.log com modo $mode"
    client curl -q -s -o /dev/null --max-time 10 -H 'Content-Type: application/x-www-form-urlencoded' \
        --data-binary @<(head -c 20000 /dev/zero | tr '\0' b) http://localhost:8082/wp-login.php || true
    if "${COMPOSE[@]}" exec -T phish grep -q bbbbbbbbbbbbbbbbbbbb /registro/posts.log; then failed "phish gravou corpo de 20 KB"; else ok "phish não grava corpo acima do limite"; fi
    # Leaves the log empty for the next suites (V6.3, V6.15 count on it being empty).
    "${COMPOSE[@]}" exec -T phish sh -c ': >/registro/posts.log'
    local d
    d="$(client curl -q -s -o /dev/null -w '%{http_code} %{redirect_url}' --max-time 10 -X POST http://localhost:8084/qualquer)"
    [ "$d" = "302 http://localhost:8082/wp-login.php" ] && ok "redir manda até POST para o phish" || failed "redir ($d)"
}

# --- No secret in docker inspect, in the containers' argv nor in the logs ------------------
# Secret list: the D15 ones from the fixtures and the random internal passwords.
no_exposed_secret() {
    python3 -I -B - "$ROOT/dev/fixtures/entries.json" >"$TMP/secrets.txt" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
vals = {d["senha_mestra"], d["pin"]}
for e in d["entradas"]:
    for k, v in e.get("segredos", {}).items():
        if not k.endswith("_arquivo"):
            vals.add(v)
    for x in e.get("extras", []):
        if x.get("segredo"):
            vals.add(x["valor"])
print("\n".join(sorted(vals)))
PY
    cat "$SECRETS_DIR/mariadb-root" >>"$TMP/secrets.txt"; echo >>"$TMP/secrets.txt"
    cat "$SECRETS_DIR/mariadb-wp" >>"$TMP/secrets.txt"; echo >>"$TMP/secrets.txt"
    sed '/^$/d' -i "$TMP/secrets.txt"
    [ "$(wc -l <"$TMP/secrets.txt")" -ge 20 ] || { failed "lista de segredos curta demais"; return; }

    local ids id name found=0
    ids="$("${COMPOSE[@]}" ps -aq)"
    for id in $ids; do
        name="$(docker inspect -f '{{.Name}}' "$id")"
        if docker inspect "$id" | grep -qF -f "$TMP/secrets.txt"; then
            found=1; failed "segredo no docker inspect de $name"
        fi
        if [ "$(docker inspect -f '{{.State.Running}}' "$id")" = true ] &&
            docker exec "$id" sh -c 'for f in /proc/[0-9]*/cmdline; do tr "\0" " " <"$f"; echo; done' 2>/dev/null |
            grep -qF -f "$TMP/secrets.txt"; then
            found=1; failed "segredo no argv de um processo de $name"
        fi
        if docker logs "$id" 2>&1 | grep -qF -f "$TMP/secrets.txt"; then
            found=1; failed "segredo no log de $name"
        fi
    done
    if "${COMPOSE[@]}" config 2>/dev/null | grep -qF -f "$TMP/secrets.txt"; then
        found=1; failed "segredo no docker compose config"
    fi
    if [ "$found" -eq 0 ]; then ok "nenhum segredo no inspect, argv ou logs dos $(wc -w <<<"$ids") contêineres nem no compose config"; fi
}

# --- Files in .dev/services -----------------------------------------------------------------
permissions() {
    local d wrong=0
    for d in "$SERVICES_DIR" "$SECRETS_DIR" "$SERVICES_DIR/keys" "$SERVICES_DIR/ssh-host" "$SERVICES_DIR"/ssh-host/*; do
        [ "$(stat -c '%a' "$d")" = 700 ] || { wrong=1; failed "$d não é 0700"; }
    done
    local f
    for f in "$SERVICES_DIR/keys/acme-deploy_ed25519" "$SERVICES_DIR"/ssh-host/*/ssh_host_ed25519_key \
        "$SECRETS_DIR/ssh-deploy" "$SECRETS_DIR/ssh-sftpuser" "$SECRETS_DIR/ssh-salto" "$SECRETS_DIR/chave-passphrase" "$SERVICES_DIR/known_hosts"; do
        [ "$(stat -c '%a' "$f")" = 600 ] || { wrong=1; failed "$f não é 0600"; }
    done
    if find "$SERVICES_DIR" -perm /022 -print -quit | grep -q .; then wrong=1; failed "arquivo gravável por grupo/outros em .dev/services"; fi
    if [ "$wrong" -eq 0 ]; then ok "diretórios 0700, chaves e senhas de SSH 0600, nada gravável por outros"; fi
    expect_refused "chave do deploy é cifrada (abrir sem passphrase falha)" \
        ssh-keygen -y -P '' -f "$SERVICES_DIR/keys/acme-deploy_ed25519"
    if git -C "$ROOT" check-ignore -q .dev/services/secrets/pg-app; then ok ".dev/services/ fica fora do git"; else failed ".dev/services/ não é ignorado"; fi
}

# --- Without `make dev-services`, compose does not start with an empty dir in its place ----
no_files_no_start() {
    mkdir -p "$TMP/copy/dev"
    cp -r "$ROOT/dev/compose.yml" "$ROOT/dev/services" "$TMP/copy/dev/"
    local project="ward-attack-$$"
    if docker compose -p "$project" -f "$TMP/copy/dev/compose.yml" create postgres >/dev/null 2>&1; then
        failed "compose criou o postgres sem o arquivo de senha"
    else
        ok "sem .dev/services/ o compose recusa criar o contêiner"
    fi
    docker compose -p "$project" -f "$TMP/copy/dev/compose.yml" down -v >/dev/null 2>&1 || true
    [ ! -e "$TMP/copy/.dev" ] && ok "e não criou .dev/ vazio no lugar" || failed "compose criou .dev/ vazio"
}

docker_without_user_env
ports_loopback_only
internal_unreachable
authentication
sftp_restricted
tunnels
wordpress
totp
phish
no_exposed_secret
permissions
no_files_no_start
host_key

if [ "$failures" -ne 0 ]; then
    printf '\n%d ataque(s) passaram: corrigir antes de seguir.\n' "$failures" >&2
    exit 1
fi
printf '\nTodos os ataques foram barrados.\n'
