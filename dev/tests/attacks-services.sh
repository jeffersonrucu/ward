#!/bin/bash -p
# Testes de ataque dos serviços de teste (Fase 12). Cada ataque tem de FALHAR.
# Precisa de `make dev-services` e do compose de pé (`docker compose -f dev/compose.yml
# up -d --build`). Uso: dev/tests/attacks-services.sh  (ou `make test-attacks-services`)
#
# Nunca imprime segredo: as buscas usam `grep -q`/`-c` e as mensagens só dizem onde.
# Ao fim o posts.log do phish fica vazio e o ssh-senha volta a uma chave de host válida
# (a troca de chave do ataque `chave_host_trocada` é permanente, como em V4.28).

set -euo pipefail

# PATH fixo antes de qualquer comando (Fase 12b): nada do PATH do usuário roda aqui.
export PATH=/usr/bin:/bin
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/services/common.sh
. "$RAIZ/dev/services/common.sh"
exigir docker sshpass ssh ssh-keygen curl python3

TMP="$(mktemp -d)"
chmod 0700 "$TMP"
trap 'rm -rf "$TMP"' EXIT

falhas=0
ok() { printf 'ok    %s\n' "$1"; }
falhou() { printf 'FALHA %s\n' "$1" >&2; falhas=$((falhas + 1)); }
recusado() { # descrição comando...: o comando TEM de falhar
    local d="$1"
    shift
    if "$@" >/dev/null 2>&1; then falhou "$d (foi aceito)"; else ok "$d"; fi
}

esperar_wordpress

# Antes de qualquer outro cliente: prova, sob strace e com o home coberto por tmpfs, que
# nenhum cliente de teste abre caminho do home do usuário real (Fase 12a).
"$RAIZ/dev/tests/real-home.sh" || { echo "clientes de teste tocam o home real; abortando" >&2; exit 1; }

# Senha errada aleatória, em arquivo (nunca em argv).
head -c 48 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24 >"$TMP/senha-errada"
chmod 0600 "$TMP/senha-errada"

# --- Fase 12b: docker e compose sem nada do ambiente do usuário ---------------------------
# Um DOCKER_CONFIG com plugin `docker-compose` plantado, DOCKER_HOST/DOCKER_CONTEXT para
# outro lugar, COMPOSE_FILE/COMPOSE_PROJECT_NAME trocados, BASH_ENV e um PATH com `docker`
# falso na frente. Pelo dev/services/compose.sh e pelo `make`, o compose tem de achar os
# serviços de verdade, sem rodar nada plantado.
docker_sem_ambiente_do_usuario() {
    local mau="$TMP/docker-mau" marcas="$TMP/marcas-docker" falsos="$TMP/falsos-docker"
    mkdir -p "$mau/cli-plugins" "$falsos"
    cat >"$mau/cli-plugins/docker-compose" <<EOF
#!/bin/sh
echo "plugin \$*" >>"$marcas"
if [ "\$1" = docker-cli-plugin-metadata ]; then
    echo '{"SchemaVersion":"0.1.0","Vendor":"teste","Version":"v9.9.9","ShortDescription":"falso"}'
fi
exit 0
EOF
    chmod +x "$mau/cli-plugins/docker-compose"
    printf '{"credsStore": "mau", "cliPluginsExtraDirs": ["%s/cli-plugins"]}\n' "$mau" >"$mau/config.json"
    printf '#!/bin/sh\necho docker-falso >>"%s"\nexit 0\n' "$marcas" >"$falsos/docker"
    chmod +x "$falsos/docker"
    printf 'echo bash_env >>"%s"\n' "$marcas" >"$TMP/bash_env"
    local hostil=(DOCKER_CONFIG="$mau" DOCKER_HOST=unix:///nao/existe.sock DOCKER_CONTEXT=mau
        COMPOSE_FILE=/nao/existe.yml COMPOSE_PROJECT_NAME=outro-projeto BASH_ENV="$TMP/bash_env"
        PATH="$falsos:$PATH")
    # Controle: o plugin plantado roda quando o DOCKER_CONFIG do usuário vale.
    rm -f "$marcas"
    DOCKER_CONFIG="$mau" /usr/bin/docker compose version >/dev/null 2>&1 || true
    if grep -q '^plugin compose' "$marcas" 2>/dev/null; then
        ok "controle: plugin docker-compose plantado no DOCKER_CONFIG roda sem o isolamento"
    else
        falhou "controle: plugin plantado não rodou nem sem isolamento (teste sem efeito)"
    fi
    local id
    rm -f "$marcas"
    id="$(/usr/bin/env "${hostil[@]}" "$RAIZ/dev/services/compose.sh" ps -q postgres 2>/dev/null || true)"
    if [ -n "$id" ] && [ ! -e "$marcas" ]; then
        ok "compose.sh com DOCKER_CONFIG/DOCKER_HOST/COMPOSE_*/PATH/BASH_ENV hostis acha o postgres e não roda nada plantado"
    else
        falhou "compose.sh usou o ambiente do usuário (id: ${id:-vazio}, marcas: $(tr '\n' ' ' <"$marcas" 2>/dev/null))"
    fi
    rm -f "$marcas"
    id="$(cd "$RAIZ" && /usr/bin/env "${hostil[@]}" /usr/bin/make -s dev-services-status 2>/dev/null | grep -c '^postgres running' || true)"
    if [ "$id" -ge 1 ] && [ ! -e "$marcas" ]; then
        ok "make dev-services-status com o mesmo ambiente hostil lista os serviços de verdade"
    else
        falhou "make dev-services-status usou o ambiente do usuário (marcas: $(tr '\n' ' ' <"$marcas" 2>/dev/null))"
    fi
    # Plugin plantado no próprio .dev/docker/: o diretório é recriado vazio a cada uso.
    mkdir -p "$RAIZ/.dev/docker/cli-plugins"
    cp "$mau/cli-plugins/docker-compose" "$RAIZ/.dev/docker/cli-plugins/docker-compose"
    rm -f "$marcas"
    id="$("$RAIZ/dev/services/compose.sh" ps -q postgres 2>/dev/null || true)"
    if [ -n "$id" ] && [ ! -e "$marcas" ] && [ ! -e "$RAIZ/.dev/docker/cli-plugins" ]; then
        ok "plugin plantado em .dev/docker/cli-plugins é apagado antes de usar o docker"
    else
        falhou "plugin plantado em .dev/docker/ rodou ou ficou"
    fi
    # O DOCKER_CONFIG deste script continua o vazio (o compose.sh o recriou igual).
    [ "$(cat "$DOCKER_CONFIG/config.json")" = '{}' ] || falhou ".dev/docker/config.json não é {}"
}

# --- Portas só em 127.0.0.1 ------------------------------------------------------------
portas_so_loopback() {
    local c linhas
    linhas="$("${COMPOSE[@]}" ps -q | xargs -r -n1 docker port)"
    if [ -n "$linhas" ] && ! grep -vqE -- '-> 127\.0\.0\.1:[0-9]+$' <<<"$linhas"; then
        ok "portas publicadas só em 127.0.0.1 ($(wc -l <<<"$linhas") portas)"
    else
        falhou "porta publicada fora de 127.0.0.1"
    fi
    # Pelo IP da máquina na rede, nenhuma porta responde.
    local ip
    ip="$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p')"
    if [ -z "$ip" ]; then
        ok "sem IP de rede além do loopback (nada a testar)"
        return
    fi
    local p abertas=0
    for p in 55432 2222 2223 2224 8081 8082 8083 8084; do
        if timeout 3 bash -c "</dev/tcp/$ip/$p" 2>/dev/null; then
            abertas=$((abertas + 1))
            falhou "porta $p responde no IP de rede $ip"
        fi
    done
    if [ "$abertas" -eq 0 ]; then ok "nenhuma porta responde pelo IP de rede $ip"; fi
}

# --- ssh-interno não é alcançável do host, nem pelo IP do contêiner ----------------------
interno_inalcancavel() {
    local id ips ip achou=0
    id="$("${COMPOSE[@]}" ps -q ssh-interno)"
    ips="$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' "$id")"
    for ip in $ips; do
        if timeout 3 bash -c "</dev/tcp/$ip/22" 2>/dev/null; then
            achou=1
            falhou "ssh-interno responde do host em $ip:22"
        fi
    done
    if [ "$achou" -eq 0 ]; then ok "ssh-interno não responde do host (IPs: ${ips:-nenhum})"; fi
}

# --- Métodos de autenticação ------------------------------------------------------------
metodos() { # porta: métodos que o servidor oferece a um cliente sem credencial
    { cliente ssh -v "${OPCOES_SSH[@]}" -o BatchMode=yes \
        -o PreferredAuthentications=none -p "$1" ninguem@127.0.0.1 true 2>&1 |
        sed -n 's/.*Authentications that can continue: //p' | head -n1 | tr -d '\r'; } || true
}
autenticacao() {
    local m
    m="$(metodos 2223)"
    [ "$m" = "publickey" ] && ok "ssh-chave 2223 só oferece publickey" || falhou "ssh-chave 2223 oferece '$m'"
    m="$(metodos 2222)"
    [ "$m" = "password" ] && ok "ssh-senha 2222 só oferece password" || falhou "ssh-senha 2222 oferece '$m'"
    recusado "ssh-chave 2223 recusa a senha do deploy" ssh_senha "$SEG/ssh-deploy" 2223 deploy true
    recusado "ssh-senha 2222 recusa senha errada" ssh_senha "$TMP/senha-errada" 2222 deploy true
    recusado "ssh-salto 2224 recusa senha errada" ssh_senha "$TMP/senha-errada" 2224 salto true
    recusado "root recusado em 2222 (com a senha do deploy)" ssh_senha "$SEG/ssh-deploy" 2222 root true
    recusado "root recusado em 2224 (com a senha do salto)" ssh_senha "$SEG/ssh-salto" 2224 root true
    recusado "deploy não entra em 2222 com a senha do salto" ssh_senha "$SEG/ssh-salto" 2222 deploy true
}

# --- sftpuser só sftp, sem túnel ---------------------------------------------------------
sftp_restrito() {
    local saida
    saida="$(ssh_senha "$SEG/ssh-sftpuser" 2222 sftpuser id 2>&1 || true)"
    if grep -q 'uid=' <<<"$saida"; then falhou "sftpuser executou comando"; else ok "sftpuser não executa comando (só sftp)"; fi
    local cfg
    cfg="$("${COMPOSE[@]}" exec -T ssh-senha sshd -T -C user=sftpuser,host=x,addr=10.0.0.1)"
    if grep -qx 'allowtcpforwarding no' <<<"$cfg" && grep -qx 'forcecommand internal-sftp' <<<"$cfg" \
        && grep -qx 'chrootdirectory /srv/sftp' <<<"$cfg"; then
        ok "sftpuser: forcecommand internal-sftp, chroot e sem encaminhamento"
    else
        falhou "configuração do sftpuser"
    fi
    recusado "sftpuser não abre túnel até o postgres" \
        ssh_senha "$SEG/ssh-sftpuser" 2222 sftpuser -W postgres:5432 </dev/null
}

# --- Encaminhamento só para o destino previsto ------------------------------------------
# `-W destino` abre um canal direct-tcpip; o canal só abre se o sshd permitir.
# Prova positiva: o destino responde pelo canal (o sshd manda o banner "SSH-"; o
# postgres responde "N"/"S" ao SSLRequest). Canal recusado não devolve nada.
canal_abre() { # arquivo porta usuario destino
    local saida
    saida="$( { printf '\x00\x00\x00\x08\x04\xd2\x16\x2f'; sleep 2; } |
        ssh_senha "$1" "$2" "$3" -W "$4" 2>/dev/null | head -c 4 | tr -dc 'A-Za-z-' || true)"
    [ -n "$saida" ]
}
tuneis() {
    if canal_abre "$SEG/ssh-deploy" 2222 deploy postgres:5432; then ok "deploy@2222 abre túnel até postgres:5432 (V4.29)"; else falhou "túnel deploy@2222 até postgres"; fi
    if canal_abre "$SEG/ssh-deploy" 2222 deploy ssh-chave:22; then falhou "deploy@2222 abriu túnel até ssh-chave:22"; else ok "deploy@2222 não abre túnel fora de postgres:5432"; fi
    if canal_abre "$SEG/ssh-salto" 2224 salto postgres:5432; then falhou "salto abriu túnel até postgres:5432"; else ok "salto não abre túnel fora de ssh-interno:22"; fi
    if canal_abre "$SEG/ssh-salto" 2224 salto ssh-interno:22; then ok "salto abre túnel até ssh-interno:22"; else falhou "salto até ssh-interno:22"; fi
}

# --- Chave de host: desconhecida e trocada são recusadas ---------------------------------
chave_host() {
    : >"$TMP/known_hosts_vazio"
    montar_opcoes_ssh "$TMP/known_hosts_vazio"
    recusado "host desconhecido recusado (StrictHostKeyChecking=yes, sem TOFU)" \
        ssh_senha "$SEG/ssh-deploy" 2222 deploy true
    cp "$DS/known_hosts" "$TMP/known_hosts_antigo"
    python3 -I -B "$RAIZ/dev/services/generate.py" --rotate-host-key ssh-senha >/dev/null
    "${COMPOSE[@]}" restart ssh-senha >/dev/null 2>&1
    local i
    # Espera o banner do sshd (o docker-proxy aceita a conexão antes do sshd subir).
    for i in $(seq 1 30); do
        timeout 2 bash -c 'exec 3<>/dev/tcp/127.0.0.1/2222; head -c 4 <&3' 2>/dev/null | grep -q '^SSH-' && break
        sleep 1
    done
    montar_opcoes_ssh "$TMP/known_hosts_antigo"
    local saida
    saida="$( (ssh_senha "$SEG/ssh-deploy" 2222 deploy true && echo ACEITO) 2>&1 || true)"
    if grep -q 'HOST IDENTIFICATION HAS CHANGED' <<<"$saida" && ! grep -q ACEITO <<<"$saida"; then
        ok "chave de host trocada recusada com aviso (V4.28)"
    else
        falhou "chave de host trocada não foi recusada"
    fi
    montar_opcoes_ssh "$DS/known_hosts"
    if [ "$(ssh_senha "$SEG/ssh-deploy" 2222 deploy whoami 2>/dev/null)" = deploy ]; then
        ok "com o known_hosts novo o ssh-senha volta a entrar"
    else
        falhou "ssh-senha depois da troca de chave"
    fi
}

# --- WordPress ---------------------------------------------------------------------------
wordpress() {
    local cab
    cab="$(wp_login wp_admin "$TMP/senha-errada" 2>/dev/null || true)"
    if grep -qi 'set-cookie: wordpress_logged_in' <<<"$cab"; then falhou "wordpress aceitou senha errada"; else ok "wordpress recusa senha errada"; fi
    cab="$(wp_login wp_admin "$SEG/wp-editor" 2>/dev/null || true)"
    if grep -qi 'set-cookie: wordpress_logged_in' <<<"$cab"; then falhou "wp_admin entrou com a senha do wp_editor"; else ok "wp_admin não entra com a senha do wp_editor"; fi
}

# --- totp (8083) ---------------------------------------------------------------------------
# Código calculado aqui pela biblioteca padrão (RFC 6238); semente lida do arquivo.
codigo_totp() { # usuario [deslocamento-em-periodos]
    python3 -I -B - "$SEG/totp-contas.json" "$1" "${2:-0}" <<'PY'
import base64, hashlib, hmac, json, struct, sys, time
contas = {c["usuario"]: c for c in json.load(open(sys.argv[1]))}
s = contas[sys.argv[2]]["totp_semente"]
k = base64.b32decode(s + "=" * (-len(s) % 8))
n = int(time.time()) // 30 + int(sys.argv[3])
m = hmac.new(k, struct.pack(">Q", n), hashlib.sha1).digest()
o = m[-1] & 15
print(f"{(struct.unpack('>I', m[o:o+4])[0] & 0x7FFFFFFF) % 1000000:06d}")
PY
}
post_totp() { # usuario arquivo-senha codigo -> status HTTP
    cliente curl -q -s -o /dev/null -w '%{http_code}' --max-time 10 \
        --data-urlencode "usuario=$1" --data-urlencode "senha@$2" --data-urlencode "codigo=$3" \
        http://localhost:8083/login
}
totp() {
    local bom ruim
    bom="$(codigo_totp wp_admin)"
    [ "$(post_totp wp_admin "$SEG/wp-admin" "$bom")" = 200 ] && ok "totp aceita senha e código certos" || falhou "totp recusou o login certo"
    ruim="$(printf '%06d' $(( (10#$bom + 500000) % 1000000 )))"
    [ "$(post_totp wp_admin "$SEG/wp-admin" "$ruim")" = 401 ] && ok "totp recusa código errado" || falhou "totp aceitou código errado"
    [ "$(post_totp wp_admin "$SEG/wp-admin" "")" = 401 ] && ok "totp recusa código vazio" || falhou "totp aceitou sem código"
    [ "$(post_totp wp_admin "$SEG/wp-admin" "$(codigo_totp wp_admin 10)")" = 401 ] && ok "totp recusa código de 5 min à frente" || falhou "totp aceitou código fora da janela"
    [ "$(post_totp wp_admin "$TMP/senha-errada" "$bom")" = 401 ] && ok "totp recusa senha errada com código certo" || falhou "totp aceitou senha errada"
    [ "$(post_totp wp_admin "$SEG/wp-editor" "$bom")" = 401 ] && ok "totp recusa a senha de outra conta" || falhou "totp aceitou senha de outra conta"
    local st
    st="$(cliente curl -q -s -o /dev/null -w '%{http_code}' --max-time 10 -H 'Content-Type: application/x-www-form-urlencoded' \
        --data-binary @<(head -c 20000 /dev/zero | tr '\0' a) http://localhost:8083/login)"
    [ "$st" = 413 ] && ok "totp recusa corpo de 20 KB (413)" || falhou "totp aceitou corpo grande ($st)"
}

# --- phish (8082) e redir (8084) ----------------------------------------------------------
phish() {
    local marca
    marca="marca-$(head -c 12 /dev/urandom | base64 | tr -dc 'a-z0-9' | head -c 12)"
    cliente curl -q -s -o /dev/null --max-time 10 --data-urlencode "log=$marca" --data-urlencode 'pwd=x' \
        http://localhost:8082/wp-login.php
    if "${COMPOSE[@]}" exec -T phish grep -q "$marca" /registro/posts.log; then
        ok "phish grava o POST em /registro/posts.log"
    else
        falhou "phish não gravou o POST"
    fi
    local modo
    modo="$("${COMPOSE[@]}" exec -T phish stat -c '%a' /registro/posts.log)"
    [ "$modo" = 600 ] && ok "posts.log é 0600" || falhou "posts.log com modo $modo"
    cliente curl -q -s -o /dev/null --max-time 10 -H 'Content-Type: application/x-www-form-urlencoded' \
        --data-binary @<(head -c 20000 /dev/zero | tr '\0' b) http://localhost:8082/wp-login.php || true
    if "${COMPOSE[@]}" exec -T phish grep -q bbbbbbbbbbbbbbbbbbbb /registro/posts.log; then falhou "phish gravou corpo de 20 KB"; else ok "phish não grava corpo acima do limite"; fi
    # Deixa o log vazio para as suítes seguintes (V6.3, V6.15 contam com ele vazio).
    "${COMPOSE[@]}" exec -T phish sh -c ': >/registro/posts.log'
    local d
    d="$(cliente curl -q -s -o /dev/null -w '%{http_code} %{redirect_url}' --max-time 10 -X POST http://localhost:8084/qualquer)"
    [ "$d" = "302 http://localhost:8082/wp-login.php" ] && ok "redir manda até POST para o phish" || falhou "redir ($d)"
}

# --- Nenhum segredo no docker inspect, no argv dos contêineres nem nos logs ---------------
# Lista de segredos: os de D15 das fixtures e as senhas internas aleatórias.
sem_segredo_exposto() {
    python3 -I -B - "$RAIZ/dev/fixtures/entries.json" >"$TMP/segredos.txt" <<'PY'
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
    cat "$SEG/mariadb-root" >>"$TMP/segredos.txt"; echo >>"$TMP/segredos.txt"
    cat "$SEG/mariadb-wp" >>"$TMP/segredos.txt"; echo >>"$TMP/segredos.txt"
    sed '/^$/d' -i "$TMP/segredos.txt"
    [ "$(wc -l <"$TMP/segredos.txt")" -ge 20 ] || { falhou "lista de segredos curta demais"; return; }

    local ids id nome achou=0
    ids="$("${COMPOSE[@]}" ps -aq)"
    for id in $ids; do
        nome="$(docker inspect -f '{{.Name}}' "$id")"
        if docker inspect "$id" | grep -qF -f "$TMP/segredos.txt"; then
            achou=1; falhou "segredo no docker inspect de $nome"
        fi
        if [ "$(docker inspect -f '{{.State.Running}}' "$id")" = true ] &&
            docker exec "$id" sh -c 'for f in /proc/[0-9]*/cmdline; do tr "\0" " " <"$f"; echo; done' 2>/dev/null |
            grep -qF -f "$TMP/segredos.txt"; then
            achou=1; falhou "segredo no argv de um processo de $nome"
        fi
        if docker logs "$id" 2>&1 | grep -qF -f "$TMP/segredos.txt"; then
            achou=1; falhou "segredo no log de $nome"
        fi
    done
    if "${COMPOSE[@]}" config 2>/dev/null | grep -qF -f "$TMP/segredos.txt"; then
        achou=1; falhou "segredo no docker compose config"
    fi
    if [ "$achou" -eq 0 ]; then ok "nenhum segredo no inspect, argv ou logs dos $(wc -w <<<"$ids") contêineres nem no compose config"; fi
}

# --- Arquivos em .dev/services -----------------------------------------------------------
permissoes() {
    local d errado=0
    for d in "$DS" "$SEG" "$DS/keys" "$DS/ssh-host" "$DS"/ssh-host/*; do
        [ "$(stat -c '%a' "$d")" = 700 ] || { errado=1; falhou "$d não é 0700"; }
    done
    local f
    for f in "$DS/keys/acme-deploy_ed25519" "$DS"/ssh-host/*/ssh_host_ed25519_key \
        "$SEG/ssh-deploy" "$SEG/ssh-sftpuser" "$SEG/ssh-salto" "$SEG/chave-passphrase" "$DS/known_hosts"; do
        [ "$(stat -c '%a' "$f")" = 600 ] || { errado=1; falhou "$f não é 0600"; }
    done
    if find "$DS" -perm /022 -print -quit | grep -q .; then errado=1; falhou "arquivo gravável por grupo/outros em .dev/services"; fi
    if [ "$errado" -eq 0 ]; then ok "diretórios 0700, chaves e senhas de SSH 0600, nada gravável por outros"; fi
    recusado "chave do deploy é cifrada (abrir sem passphrase falha)" \
        ssh-keygen -y -P '' -f "$DS/keys/acme-deploy_ed25519"
    if git -C "$RAIZ" check-ignore -q .dev/services/secrets/pg-app; then ok ".dev/services/ fica fora do git"; else falhou ".dev/services/ não é ignorado"; fi
}

# --- Sem `make dev-services`, o compose não sobe com diretório vazio no lugar ------------
sem_arquivos_nao_sobe() {
    mkdir -p "$TMP/copia/dev"
    cp -r "$RAIZ/dev/compose.yml" "$RAIZ/dev/services" "$TMP/copia/dev/"
    local projeto="ward-ataque-$$"
    if docker compose -p "$projeto" -f "$TMP/copia/dev/compose.yml" create postgres >/dev/null 2>&1; then
        falhou "compose criou o postgres sem o arquivo de senha"
    else
        ok "sem .dev/services/ o compose recusa criar o contêiner"
    fi
    docker compose -p "$projeto" -f "$TMP/copia/dev/compose.yml" down -v >/dev/null 2>&1 || true
    [ ! -e "$TMP/copia/.dev" ] && ok "e não criou .dev/ vazio no lugar" || falhou "compose criou .dev/ vazio"
}

docker_sem_ambiente_do_usuario
portas_so_loopback
interno_inalcancavel
autenticacao
sftp_restrito
tuneis
wordpress
totp
phish
sem_segredo_exposto
permissoes
sem_arquivos_nao_sobe
chave_host

if [ "$falhas" -ne 0 ]; then
    printf '\n%d ataque(s) passaram: corrigir antes de seguir.\n' "$falhas" >&2
    exit 1
fi
printf '\nTodos os ataques foram barrados.\n'
