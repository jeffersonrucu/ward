#!/bin/bash -p
# Teste de ataque (Fase 12a): nenhum cliente de teste (ssh, sshpass, ssh-keygen, curl)
# abre caminho do home do usuário real. O ssh acha as identidades padrão pelo home do
# passwd (getpwuid), não por $HOME, então o HOME falso de `cliente` não basta.
#
# O script se reexecuta dentro de um bwrap com o home do passwd coberto por tmpfs (só o
# repositório volta por bind): mesmo que a proteção de `common.sh` regrida, o home real
# nunca é lido. Lá dentro roda cada cliente sob strace e falha se algum caminho do home
# (fora do repositório) aparecer numa chamada de arquivo. Desde a Fase 12b também roda
# o git e o docker como os scripts usam (dev/env.sh e common.sh), segue o diretório
# atual de cada processo (chdir/clone) e aplica mutações às opções de OPCOES_SSH: cada
# proteção tirada (IdentityFile=none, -F /dev/null, UserKnownHostsFile, os dois juntos
# para os *-cert.pub) ou trocada (CertificateFile=none) tem de ser acusada, assim como
# git e docker sem o isolamento; senão o detector ou a proteção não fazem efeito.
#
# Precisa dos serviços de pé, de bwrap (bubblewrap) e de strace. Sem eles, falha
# (fechado). Uso: dev/tests/real-home.sh  (ou `make test-real-home`); o
# `attacks-services.sh` e o `verify.sh` o rodam antes de qualquer outro cliente.

set -euo pipefail

# PATH fixo antes de qualquer comando (Fase 12b): nada do PATH do usuário roda aqui.
export PATH=/usr/bin:/bin
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CASA="$(getent passwd "$(id -u)" | cut -d: -f6)"
[ -n "$CASA" ] && [ "$CASA" != / ] || { echo "home do passwd indefinido" >&2; exit 1; }
for p in bwrap strace getent python3; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p (o teste falha fechado)" >&2; exit 1; }
done

# --- Fora do sandbox: reexecuta dentro dele -----------------------------------------------
if [ "${WARD_HOME_COBERTO:-}" != 1 ]; then
    args=(--die-with-parent --dev-bind / / --tmpfs "$CASA")
    # Só o repositório volta; /tmp também, se o tmpfs do home o tiver coberto.
    args+=(--bind "$RAIZ" "$RAIZ" --bind /tmp /tmp --setenv WARD_HOME_COBERTO 1 --chdir "$RAIZ")
    exec bwrap "${args[@]}" -- /bin/bash -p "$RAIZ/dev/tests/real-home.sh" "$@"
fi

# --- Dentro: confere que o home está mesmo coberto -----------------------------------------
case "$RAIZ/" in
    "$CASA"/*) ;;
    *) echo "aviso: o repositório não está dentro do home do passwd" >&2 ;;
esac
[ "$(stat -f -c %T "$CASA")" = tmpfs ] || { echo "o home não está coberto por tmpfs; abortando" >&2; exit 1; }
# Nada no home além do caminho até o repositório (o tmpfs começa vazio).
extra="$(find "$CASA" -mindepth 1 -maxdepth 1 ! -path "$CASA/$(cut -d/ -f1 <<<"${RAIZ#"$CASA"/}")" -print -quit)"
[ -z "$extra" ] || { echo "o home coberto tem conteúdo inesperado; abortando" >&2; exit 1; }

# shellcheck source=dev/services/common.sh
. "$RAIZ/dev/services/common.sh"
exigir sshpass ssh ssh-keygen curl git docker

TMP="$(mktemp -d)"
chmod 0700 "$TMP"
trap 'rm -rf "$TMP"' EXIT
# Diretório atual dos clientes com iscas: um caminho relativo (ex.: `none`) aberto pelo
# ssh apareceria como acesso a esta pasta e é acusado também.
mkdir -p "$TMP/cwd"
: >"$TMP/cwd/none"

MUT=() # opções mutadas dos controles (ver opcoes_mutadas)
falhas=0
ok() { printf 'ok    %s\n' "$1"; }
falhou() { printf 'FALHA %s\n' "$1" >&2; falhas=$((falhas + 1)); }

# rastrear <nome> <funcao> [args...]: roda a função sob strace (-f, chamadas de arquivo)
# com o PATH mínimo e o diretório atual em $TMP/cwd. O código de saída do cliente não
# importa aqui: só o que ele tentou abrir.
rastrear() {
    local nome="$1"
    shift
    (
        cd "$TMP/cwd"
        # As funções e variáveis de common.sh vão para o bash rastreado por declare.
        PATH=/usr/bin:/bin strace -f -qq -e trace=%file,%process,fchdir -o "$TMP/$nome.strace" \
            /bin/bash -p -c "$(declare -f); $(declare -p OPCOES_SSH MUT COMPOSE IMAGEM_PG DS SEG RAIZ TMP); \"\$@\"" \
            _ "$@" </dev/null >/dev/null 2>&1 && echo 0 >"$TMP/$nome.rc" || echo 1 >"$TMP/$nome.rc"
    )
}
# rastrear_ok: como rastrear, mas o cliente tem de dar certo (trace vazio de um cliente
# que nem conectou não prova nada).
rastrear_ok() {
    rastrear "$@"
    [ "$(cat "$TMP/$1.rc")" = 0 ] || falhou "$1: o cliente devia ter entrado e não entrou"
}

# acusados <arquivo.strace>: imprime os caminhos do home (fora do repositório) ou
# relativos ao diretório de iscas que aparecem no trace. Nunca imprime conteúdo.
acusados() {
    python3 -I -B - "$1" "$CASA" "$RAIZ" "$TMP/cwd" <<'PY'
import os, re, sys
trace, casa, raiz, cwd0 = sys.argv[1:5]
# Primeiro argumento entre aspas de cada chamada (o caminho). Caminho relativo é
# resolvido pelo diretório atual daquele processo: começa em cwd0, segue os chdir() que
# deram certo e passa de pai para filho no clone/fork (Fase 12b: o `git -C` muda de
# diretório). Depois de um fchdir(), ou com dirfd diferente de AT_FDCWD, não dá para
# resolver: acusa.
linha_re = re.compile(r'^(\d+)\s+(?:<\.\.\. (\w+) resumed>|(\w+)\()(.*)')
caminho_re = re.compile(r'"((?:[^"\\]|\\.)*)"')
retorno_re = re.compile(r'=\s+(-?\d+)(?:\s|$)')
COM_DIRFD = {
    'openat', 'openat2', 'newfstatat', 'fstatat64', 'faccessat', 'faccessat2', 'statx',
    'readlinkat', 'mkdirat', 'mknodat', 'unlinkat', 'renameat', 'renameat2', 'linkat',
    'symlinkat', 'fchmodat', 'fchmodat2', 'fchownat', 'futimesat', 'utimensat',
    'execveat', 'name_to_handle_at', 'open_tree', 'move_mount', 'fspick',
}
FORKS = {'clone', 'clone3', 'fork', 'vfork'}
def dentro(p, base):
    return p == base or p.startswith(base.rstrip('/') + '/')
linhas = []
pai_de = {}
for linha in open(trace, errors='replace'):
    m = linha_re.match(linha)
    if not m:
        continue
    pid, resumida, chamada, resto = m.group(1), m.group(2), m.group(3), m.group(4)
    nome = resumida or chamada
    r = retorno_re.search(resto.rsplit(')', 1)[-1] if ')' in resto else resto)
    ret = int(r.group(1)) if r else None
    if nome in FORKS and ret and ret > 0:
        pai_de[str(ret)] = pid
    linhas.append((pid, nome, bool(resumida), resto, ret))
cwd_de = {}
primeiro = None
acusados = set()
for pid, chamada, resumida, resto, ret in linhas:
    if pid not in cwd_de:
        if primeiro is None:
            primeiro = pid
            cwd_de[pid] = cwd0
        else:
            cwd_de[pid] = cwd_de.get(pai_de.get(pid), cwd0)
    atual = cwd_de[pid]
    if chamada == 'fchdir' and ret == 0:
        cwd_de[pid] = None
        continue
    if resumida:
        continue  # o caminho foi visto na linha <unfinished ...>
    if chamada in ('execve', 'execveat'):
        # argv pode citar caminhos do repositório; só o binário conta.
        resto = resto.split(',', 1)[0]
    caminhos = caminho_re.findall(resto)[:2]
    if chamada in ('readlink', 'readlinkat'):
        # O 2º texto entre aspas é o buffer de saída (o alvo lido), não um caminho aberto.
        caminhos = caminhos[:1]
    if chamada == 'chdir' and caminhos and ret == 0:
        c = caminhos[0]
        if atual is None and not c.startswith('/'):
            cwd_de[pid] = None
        else:
            cwd_de[pid] = os.path.normpath(os.path.join(atual or '/', c))
        alvo = cwd_de[pid]
        if alvo and dentro(alvo, casa) and not (dentro(alvo, raiz) or dentro(raiz, alvo)):
            acusados.add(alvo)
        continue
    for c in caminhos:
        if not c:
            continue
        if not c.startswith('/'):
            if chamada in COM_DIRFD and not resto.startswith('AT_FDCWD'):
                acusados.add('(relativo a um dirfd) ' + chamada)
                continue
            if atual is None:
                acusados.add('(relativo depois de fchdir) ' + chamada)
                continue
            c = os.path.join(atual, c)
        p = os.path.normpath(c)
        if p == cwd0:
            continue  # o próprio diretório de iscas (getcwd/stat de "."), não um arquivo nele
        if dentro(p, cwd0):
            acusados.add('(relativo ao diretório atual) ' + os.path.relpath(p, cwd0))
            continue
        if not dentro(p, casa):
            continue
        if dentro(p, raiz) or dentro(raiz, p):
            continue  # o repositório, ou um diretório no caminho até ele
        acusados.add(p)
for a in sorted(acusados):
    print(a)
PY
}

conferir() { # descrição nome
    local saida
    saida="$(acusados "$TMP/$2.strace")"
    if [ ! -s "$TMP/$2.strace" ] || ! grep -q 'execve("/usr/bin/\(ssh\|curl\|ssh-keygen\|git\|docker\|python3\)"' "$TMP/$2.strace"; then
        falhou "$1: o trace não pegou o cliente"
    elif [ -n "$saida" ]; then
        falhou "$1 abriu caminho fora do permitido:"
        sed 's/^/        /' <<<"$saida" >&2
    else
        ok "$1 não abre nada do home real"
    fi
}

senha_errada() { head -c 48 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24; }
senha_errada >"$TMP/senha-errada"
chmod 0600 "$TMP/senha-errada"

metodos() {
    cliente ssh -v "${OPCOES_SSH[@]}" -o BatchMode=yes -o PreferredAuthentications=none \
        -p 2223 ninguem@127.0.0.1 true
}
canal() { printf 'x' | ssh_senha "$SEG/ssh-deploy" 2222 deploy -W postgres:5432; }
sftp_ls() { printf 'ls\n' | ssh_senha "$SEG/ssh-sftpuser" 2222 sftpuser -s sftp; }
keygen_y() { ssh-keygen -y -P '' -f "$DS/keys/acme-deploy_ed25519"; }
# Como o generate.py gera as chaves (ssh-keygen -t … -f), numa pasta temporária.
keygen_novo() { ssh-keygen -q -t ed25519 -N '' -C ward-teste-home -f "$TMP/chave-nova"; }
wp() { wp_login wp_admin "$SEG/wp-admin"; }
# git e docker como os scripts usam (Fase 12b): ambiente de dev/env.sh e common.sh.
git_ignore() { git -C "$RAIZ" check-ignore -q .dev/services/secrets/pg-app; }
compose_ps() { "${COMPOSE[@]}" ps -q postgres; }
psql_um() { psql_app 'select 1'; }
# Cliente do Postgres em Python da Fase 28 (V4.29): senha por arquivo, nunca em argv.
pg_select_um() {
    cliente PGHOST=127.0.0.1 PGPORT=55432 PGUSER=app PGDATABASE=app_teste \
        WARD_TESTE_PG_SENHA_ARQUIVO="$SEG/pg-app" python3 -I -B "$RAIZ/dev/tests/pg_select.py" 'select 1'
}

rastrear_ok senha ssh_senha "$SEG/ssh-deploy" 2222 deploy whoami
conferir "ssh_senha (2222, deploy)" senha
rastrear errada ssh_senha "$TMP/senha-errada" 2224 salto true
conferir "ssh_senha com senha errada (2224)" errada
rastrear_ok chave ssh_chave 2223 deploy whoami
conferir "ssh_chave (2223, -i)" chave
rastrear_ok interno ssh_interno hostname
conferir "ssh_interno (ProxyCommand pelo salto)" interno
rastrear metodos metodos
conferir "ssh -v sem credencial (2223)" metodos
rastrear canal canal
conferir "ssh -W (túnel até o postgres)" canal
rastrear_ok sftp sftp_ls
conferir "sftp do sftpuser" sftp
rastrear keygen keygen_y
conferir "ssh-keygen -y da chave do deploy" keygen
rastrear_ok keygen-novo keygen_novo
conferir "ssh-keygen -t ed25519 -f (como no generate.py)" keygen-novo
rastrear_ok wp wp
conferir "curl no wp-login" wp
rastrear_ok git git_ignore
conferir "git check-ignore (sem config do usuário)" git
rastrear_ok compose compose_ps
conferir "docker compose ps (DOCKER_CONFIG vazio)" compose
rastrear_ok psql psql_um
conferir "docker run do psql_app" psql
rastrear_ok pg-select pg_select_um
conferir "dev/tests/pg_select.py (cliente Postgres da Fase 28)" pg-select

# Mutações das opções de OPCOES_SSH (Fase 12b): cada proteção, tirada ou trocada pela
# forma errada, TEM de ser acusada. Seguro: aqui o home é um tmpfs vazio.
# opcoes_mutadas <mutação>...: MUT = OPCOES_SSH com cada `-x valor` listado tirado
# ("-o IdentityFile=none") ou trocado ("-o CertificateFile=/dev/null=>CertificateFile=none").
opcoes_mutadas() {
    MUT=()
    local i par m feito aplicadas=0
    for ((i = 0; i < ${#OPCOES_SSH[@]}; i += 2)); do
        par="${OPCOES_SSH[i]} ${OPCOES_SSH[i + 1]}"
        feito=0
        for m in "$@"; do
            if [ "$m" = "$par" ]; then
                feito=1
            elif [[ "$m" == *"=>"* ]] && [ "${m%%=>*}" = "$par" ]; then
                feito=1
                MUT+=("${OPCOES_SSH[i]}" "${m#*=>}")
            fi
        done
        if [ "$feito" -eq 1 ]; then aplicadas=$((aplicadas + 1)); else MUT+=("${OPCOES_SSH[i]}" "${OPCOES_SSH[i + 1]}"); fi
    done
    [ "$aplicadas" -eq "$#" ] || { echo "mutação não casou com OPCOES_SSH: $*" >&2; exit 1; }
}
ssh_mutado() {
    cliente sshpass -f "$SEG/ssh-deploy" ssh "${MUT[@]}" -o PubkeyAuthentication=no \
        -p 2222 deploy@127.0.0.1 true
}
# mutacao <nome> <descrição> <padrão que tem de aparecer nos acusados> <mutação>...
mutacao() {
    local nome="$1" d="$2" padrao="$3"
    shift 3
    opcoes_mutadas "$@"
    rastrear "$nome" ssh_mutado
    if acusados "$TMP/$nome.strace" | grep -q -- "$padrao"; then
        ok "mutação acusada: $d"
    else
        falhou "mutação NÃO acusada (o detector ou a proteção não fazem efeito): $d"
    fi
}
mutacao m-ident "sem IdentityFile=none (abre as chaves padrão)" "/id_ed25519$" "-o IdentityFile=none"
mutacao m-cert-none "CertificateFile=none no lugar de /dev/null (abre ./none)" "relativo ao diretório atual" \
    "-o CertificateFile=/dev/null=>CertificateFile=none"
mutacao m-sem-os-dois "sem IdentityFile=none e sem CertificateFile=/dev/null (abre os -cert.pub)" "/id_ed25519-cert.pub$" \
    "-o IdentityFile=none" "-o CertificateFile=/dev/null"
mutacao m-config "sem -F /dev/null (lê a config do usuário)" "/config$" "-F /dev/null"
mutacao m-known "sem UserKnownHostsFile próprio (lê o known_hosts do usuário)" "/known_hosts$" \
    "-o UserKnownHostsFile=$DS/known_hosts"
# O CertificateFile=/dev/null é a segunda barreira: sem o IdentityFile=none mas com ele,
# nenhum *-cert.pub do home é procurado (só as chaves, que a mutação acima já acusa).
if acusados "$TMP/m-ident.strace" | grep -q -- '-cert.pub$'; then
    falhou "com CertificateFile=/dev/null o ssh ainda procura *-cert.pub no home"
else
    ok "CertificateFile=/dev/null tira os *-cert.pub do home mesmo sem IdentityFile=none"
fi

# Controles do git e do docker (Fase 12b): sem o isolamento, os dois leem o home.
controle_git() { /usr/bin/env -u GIT_CONFIG_GLOBAL -u GIT_CONFIG_COUNT -u GIT_CONFIG_NOSYSTEM git -C "$RAIZ" check-ignore -q .dev/x; }
rastrear controle-git controle_git
if [ -n "$(acusados "$TMP/controle-git.strace")" ]; then ok "controle: git sem o isolamento é acusado"; else falhou "controle: git sem o isolamento não foi acusado"; fi
controle_docker() { /usr/bin/env -u DOCKER_CONFIG docker compose -f "$RAIZ/dev/compose.yml" ps -q; }
rastrear controle-docker controle_docker
if [ -n "$(acusados "$TMP/controle-docker.strace")" ]; then ok "controle: docker sem DOCKER_CONFIG é acusado"; else falhou "controle: docker sem DOCKER_CONFIG não foi acusado"; fi

# Controle do curl: sem `-q`, ele procura o .curlrc no home do passwd e é acusado.
controle_curl() { cliente curl -s -o /dev/null --max-time 5 http://localhost:8083/login; }
rastrear curl controle_curl
if [ -n "$(acusados "$TMP/curl.strace")" ]; then
    ok "controle: curl sem -q é acusado"
else
    falhou "controle: curl sem -q não foi acusado"
fi

if [ "$falhas" -ne 0 ]; then
    printf '\n%d cliente(s) tocaram o home real ou o detector falhou.\n' "$falhas" >&2
    exit 1
fi
printf '\nNenhum cliente de teste abre caminho do home real.\n'
