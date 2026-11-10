#!/bin/bash -p
# `make test-ssh-home` (Fase 28b, scrutiny do M4): V4.27 (R19, D13, S1) de ponta a
# ponta. A suíte de `ward ssh` e túnel não abre nada do home do passwd.
#
# O ssh acha a pasta dele no home pelo home do passwd (getpwuid), não por `$HOME`. Por isso o teste
# roda num bwrap com o home do passwd coberto por tmpfs (o home real nunca é visto; só
# o repositório volta por bind) e o `/tmp` também em tmpfs. No home coberto ficam as
# iscas, na pasta do ssh: `config` com `Host *`, `ProxyCommand touch /tmp/sentinela-ssh`,
# `LocalCommand` e `IdentityAgent` do agente falso; um `known_hosts` com chave de host
# trocada; um `id_ed25519` de sentinela; e FIFOs nos outros nomes que o ssh procura
# (`id_rsa`, `id_ecdsa`, `*-cert.pub`, `known_hosts2`, `authorized_keys`, `.pgpass`...).
#
# Três detectores independentes (o `wardd` recusa subir sob `strace`, então o rastreio é
# só nos programas que ele executa):
# - `/usr/bin/ssh`, `/usr/bin/sshpass` e `/usr/bin/sftp` são trocados, só dentro do
#   bwrap, por um invólucro que roda o original sob `strace -D -f` (o pid do programa
#   não muda: o agente de D12 e a ponte conferem o pid e o grupo de processos); toda
#   chamada de arquivo de cada `ssh`/`sshpass`/`sftp` é conferida contra o home;
# - inotify (ctypes, sem dependência nova) em todas as pastas do home coberto: qualquer
#   evento durante a suíte reprova;
# - escritores nas FIFOs anotam toda abertura, e um agente falso na variável do agente
#   do ssh (posta no ambiente do daemon e da CLI) conta as conexões.
# Controles positivos no fim provam que cada detector acusa um `ssh` sem o isolamento.
#
# O `wardd` de teste é só deste script: cópia do cofre de teste numa pasta própria em
# `.dev/ssh-home.XXXXXX` e socket próprio (`WARD_DEV_SOCKET_DIR`); nunca o daemon de
# `make dev-daemon`. O `known_hosts` do Ward fica nessa pasta (começa vazio).
#
# Precisa de bwrap, strace, `make dev-vault`, `make dev-variations` e dos serviços de pé
# (2222, 2223, 2224, 55432). Uso: dev/tests/ssh-home.sh <wardd> <ward> (test-mode).

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$RAIZ/dev/env.sh"
CASA="$CASA_PASSWD"
[ -n "$CASA" ] && [ "$CASA" != / ] || { echo "home do passwd indefinido" >&2; exit 1; }

WARDD="${1:?uso: ssh-home.sh <wardd> <ward>}"
WARD="${2:?uso: ssh-home.sh <wardd> <ward>}"
case "$WARDD" in /*) ;; *) WARDD="$RAIZ/$WARDD" ;; esac
case "$WARD" in /*) ;; *) WARD="$RAIZ/$WARD" ;; esac

# --- Dentro do bwrap: o driver em Python faz o resto -----------------------------------
if [ "${WARD_SSH_HOME_DENTRO:-}" = 1 ]; then
    [ "$(stat -f -c %T "$CASA")" = tmpfs ] || { echo "o home não está coberto por tmpfs; abortando" >&2; exit 1; }
    [ "$(stat -f -c %T /tmp)" = tmpfs ] || { echo "o /tmp não está em tmpfs; abortando" >&2; exit 1; }
    extra="$(find "$CASA" -mindepth 1 -maxdepth 1 ! -path "$CASA/$(cut -d/ -f1 <<<"${RAIZ#"$CASA"/}")" -print -quit)"
    [ -z "$extra" ] || { echo "o home coberto tem conteúdo inesperado; abortando" >&2; exit 1; }
    exec /usr/bin/python3 -I -B "$RAIZ/dev/tests/ssh_home.py" "$WARDD" "$WARD" "$RAIZ" "$CASA" "$WARD_SSH_HOME_TRAB"
fi

# --- Fora: prepara e entra no bwrap ----------------------------------------------------
for p in bwrap strace python3 ssh ssh-keygen sshpass sftp; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p (o teste falha fechado)" >&2; exit 1; }
done
[ -x "$WARDD" ] && [ -x "$WARD" ] || { echo "falta $WARDD ou $WARD" >&2; exit 1; }
COFRE="$RAIZ/.dev/test-vault.ward"
LISTA="$RAIZ/.dev/variations.txt"
[ -f "$COFRE" ] && [ ! -L "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$LISTA" ] && [ ! -L "$LISTA" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }
case "$RAIZ/" in
    "$CASA"/*) ;;
    *) echo "aviso: o repositório não está dentro do home do passwd" >&2 ;;
esac

umask 077
TRAB="$(mktemp -d "$RAIZ/.dev/ssh-home.XXXXXX")"
trap 'rm -rf -- "$TRAB"' EXIT
mkdir -m 0700 "$TRAB/tracos"

# Invólucro: o programa original sob `strace -D -f` (só chamadas de arquivo e de
# processo; o `execve` sai com o argv cortado em 32 bytes e sem o ambiente, e o argv do
# ssh não tem segredo, V4.14). Já rastreado (o `ssh` filho do `sshpass`), executa o
# original direto. Grava o diretório atual de partida ao lado do traço (caminhos
# relativos). Lido só por `read` do bash: nenhum programa a mais no rastro.
cat >"$TRAB/envolver" <<EOF
#!/bin/bash -p
n="\${0##*/}"
real="/tmp/ward-real/\$n"
t=0
while read -r k v; do [ "\$k" = TracerPid: ] && t="\$v"; done </proc/self/status
if [ "\$t" = 0 ]; then
    printf '%s\n' "\$PWD" >"$TRAB/tracos/\$n.\$\$.cwd"
    exec /usr/bin/strace -D -f -qq -e trace=%file,%process,fchdir -o "$TRAB/tracos/\$n.\$\$" -- "\$real" "\$@"
fi
exec "\$real" "\$@"
EOF
chmod 0755 "$TRAB/envolver"

# Raiz do bwrap: o tmpfs novo dele (do usuário, 0755), com cada pasta de `/` de volta
# por bind (links como links). Assim a pasta do socket de teste (`/ward-ssh-home/sock`)
# tem só pastas do usuário acima, como o `wardd` de teste exige (D7): dentro do
# namespace de usuário as pastas do root aparecem como de `nobody` e seriam recusadas.
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
args+=(--tmpfs "$CASA" --bind "$RAIZ" "$RAIZ" --tmpfs /tmp --dir /tmp/ward-real
    --perms 0755 --dir /ward-ssh-home)
for p in ssh sshpass sftp; do
    args+=(--ro-bind "/usr/bin/$p" "/tmp/ward-real/$p" --ro-bind "$TRAB/envolver" "/usr/bin/$p")
done
args+=(--setenv WARD_SSH_HOME_DENTRO 1 --setenv WARD_SSH_HOME_TRAB "$TRAB" --chdir "$RAIZ")
rc=0
/usr/bin/env -u WARD_HOME_COBERTO bwrap "${args[@]}" -- /bin/bash -p "$RAIZ/dev/tests/ssh-home.sh" "$WARDD" "$WARD" || rc=$?
exit "$rc"
