# Funções comuns de dev/servicos/conferir.sh e dev/testes/ataques-servicos.sh.
# Uso: `. dev/servicos/comum.sh` (com RAIZ definido).
#
# Clientes de teste isolados (R19): `env -i` com HOME falso em .dev/servicos/, ssh com
# `-F /dev/null`, known_hosts próprio, sem agente e sem as identidades padrão (só a
# chave pedida com -i). Senha e passphrase sempre por `sshpass -f <arquivo>`: nunca em
# argv nem no ambiente.

# PATH fixo, git sem config do usuário (dev/ambiente.sh).
# shellcheck source=dev/ambiente.sh
. "$RAIZ/dev/ambiente.sh"

DS="$RAIZ/.dev/servicos"
SEG="$DS/segredos"

# Docker sem nada do usuário (Fase 12b). O docker lê `~/.docker/config.json` (credential
# helpers, que ele executa, e `cliPluginsExtraDirs`), carrega plugins de
# `~/.docker/cli-plugins` (um `docker-compose` plantado ali rodaria no lugar do
# compose) e segue DOCKER_HOST/DOCKER_CONTEXT para outro daemon. Aqui:
# - DOCKER_CONFIG aponta para `.dev/docker/`, recriado vazio (só `{}`) a cada script:
#   nada plantado lá dentro (plugin, contexto, builder do buildx) sobrevive;
# - DOCKER_*, COMPOSE_*, BUILDX_* e BUILDKIT_* herdados saem do ambiente (o compose
#   usa o socket padrão e o projeto do `dev/compose.yml`).
while IFS= read -r _var; do unset "$_var"; done \
    < <(compgen -e | grep -E '^(DOCKER_|COMPOSE_|BUILDX_|BUILDKIT_)' || true)
unset _var
DOCKER_VAZIO="$RAIZ/.dev/docker"
rm -rf -- "$DOCKER_VAZIO"
mkdir -p -m 0700 -- "$RAIZ/.dev"
mkdir -m 0700 -- "$DOCKER_VAZIO"
printf '{}\n' >"$DOCKER_VAZIO/config.json"
chmod 0600 "$DOCKER_VAZIO/config.json"
export DOCKER_CONFIG="$DOCKER_VAZIO"
COMPOSE=(docker compose -f "$RAIZ/dev/compose.yml")
IMAGEM_PG="postgres:16-alpine@sha256:0cefaabd882e42bcb2a2f3dc56d3ce4770560ba62fd7f197f1a2e008fd5731ab"

exigir() {
    local p
    for p in "$@"; do
        command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
    done
    [ -f "$DS/known_hosts" ] || { echo "rode antes: make dev-servicos" >&2; exit 1; }
}

# Ambiente mínimo dos clientes: nada do usuário (sem agente, sem HOME real).
# O curl procura o .curlrc também no home do passwd quando não acha no $HOME: todo
# `cliente curl` começa com `-q` (tem de ser o primeiro argumento), que não lê nenhum.
cliente() {
    /usr/bin/env -i PATH=/usr/bin:/bin HOME="$DS/home-cliente" LANG=C.UTF-8 "$@"
}

# Opções de todo cliente ssh de teste. `montar_opcoes_ssh <known_hosts>` troca o
# known_hosts (o teste de chave de host trocada usa uma cópia antiga).
#
# O ssh acha as identidades e certificados padrão pelo home do passwd (getpwuid), não
# por $HOME: sem `IdentityFile=none`, todo cliente sem `-i` (e o salto do ProxyCommand)
# abria as chaves padrão do usuário real. `IdentityFile=none` tira as padrão (o `-i`
# continua valendo). `CertificateFile=/dev/null` fixa a lista de certificados num
# caminho absoluto: o OpenSSH 9.6 não trata `CertificateFile=none` como "nenhum" e
# abria um arquivo `none` relativo ao diretório atual. `dev/testes/home-real.sh` prova
# sob strace, com o home coberto por tmpfs, que nenhum caminho do home é aberto.
montar_opcoes_ssh() {
    OPCOES_SSH=(
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
montar_opcoes_ssh "$DS/known_hosts"

# ssh_senha <arquivo-senha> <porta> <usuario> [comando...]
ssh_senha() {
    local arq="$1" porta="$2" usuario="$3"
    shift 3
    cliente sshpass -f "$arq" ssh "${OPCOES_SSH[@]}" \
        -o PubkeyAuthentication=no -o PreferredAuthentications=password,keyboard-interactive \
        -o NumberOfPasswordPrompts=1 -p "$porta" "$usuario@127.0.0.1" "$@"
}

# ssh_chave <porta> <usuario> [comando...] (chave do deploy, passphrase por sshpass -f)
ssh_chave() {
    local porta="$1" usuario="$2"
    shift 2
    cliente sshpass -f "$SEG/chave-passphrase" -P passphrase ssh "${OPCOES_SSH[@]}" \
        -o PasswordAuthentication=no -o KbdInteractiveAuthentication=no \
        -i "$DS/chaves/acme-deploy_ed25519" -p "$porta" "$usuario@127.0.0.1" "$@"
}

# ssh_interno [comando...]: deploy@ssh-interno por ProxyCommand pelo salto (2224).
ssh_interno() {
    local salto
    printf -v salto 'sshpass -f %q ssh' "$SEG/ssh-salto"
    local o
    for o in "${OPCOES_SSH[@]}"; do printf -v salto '%s %q' "$salto" "$o"; done
    salto+=" -o PubkeyAuthentication=no -o NumberOfPasswordPrompts=1 -p 2224 -W %h:%p salto@127.0.0.1"
    cliente sshpass -f "$SEG/chave-passphrase" -P passphrase ssh "${OPCOES_SSH[@]}" \
        -o PasswordAuthentication=no -o KbdInteractiveAuthentication=no \
        -o ProxyCommand="$salto" \
        -i "$DS/chaves/acme-deploy_ed25519" deploy@ssh-interno "$@"
}

# psql_app <sql>: psql da imagem do postgres na rede do host, senha pelo ambiente
# (`-e PGPASSWORD` sem valor: o docker lê do ambiente, não vai em argv).
psql_app() {
    PGPASSWORD="$(cat "$SEG/pg-app")" docker run --rm --network host -e PGPASSWORD \
        "$IMAGEM_PG" psql -h 127.0.0.1 -p 55432 -U app -d app_teste -tAc "$1"
}

# wp_login <usuario> <arquivo-senha>: imprime os cabeçalhos da resposta do wp-login.php.
wp_login() {
    local usuario="$1" arq="$2"
    cliente curl -q -sS -o /dev/null -D - --max-time 15 \
        -b 'wordpress_test_cookie=WP%20Cookie%20check' \
        --data-urlencode "log=$usuario" --data-urlencode "pwd@$arq" \
        --data-urlencode 'testcookie=1' \
        http://localhost:8081/wp-login.php
}

# Espera o wpcli terminar (instala o WordPress e cria os usuários).
esperar_wordpress() {
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
