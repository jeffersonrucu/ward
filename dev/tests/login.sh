#!/bin/bash -p
# `make test-login` (Fase 43; R14, D9; V6.3, V6.10, V6.11, V6.15, V6.16): `ward login`
# de ponta a ponta. A extensão de `extension/dist/` no Chrome real (headless, perfil
# temporário em `.dev/login.*`), o `ward-nm` de teste aberto pelo service worker dela e
# um `wardd` de teste só deste script com `WARD_DEV_EXTENSAO=chrome` (o filho do Chrome
# instalado ganha o papel extensão sem o egid; só no `test-mode`). A CLI `ward` de teste
# fala com esse daemon por `WARD_DEV_SOCKET_DIR`.
#
# - V6.10: aba em `localhost:8081/wp-login.php`, `ward login acme-wp-prod` sai 0 e a aba
#   chega ao painel; página que recusa o login (servida pelo CDP na origem certa) e
#   página que não navega: os campos ficam vazios depois do envio e a CLI sai ≠ 0.
# - V6.3: aba no phishing (8082): sai ≠ 0 com "domínio não confere", `posts.log` sem
#   mudança e nada preenchido.
# - V6.15: `delta-redir` com a aba no 8084 (que vai ao 8082): o mesmo.
# - Ataques de formulário na origem certa: `action`/`formaction` para o phishing, `GET`,
#   `target`, dois campos de senha, `action` mascarado: nada enviado para fora.
# - V6.11 (lado do daemon): `acme-wp-dev` (com aprovação) negado na janela não preenche
#   nada.
# - Fase 44c (D9): toda entrada `http` passa pela janela, com a origem à vista. A janela é
#   a de teste (`wardd-aprovador-teste`, aberta pelo daemon por `WARD_DEV_APP`, que
#   responde o que está em `$TRAB/aprovador.modo`); o ataque `[::1]` da auditoria do M6
#   (`dev/tests/fake_v6.py` em `[::1]:8081`, o WordPress só em 127.0.0.1) com a
#   janela negada não leva nada ao falso.
# - V6.16: logs do daemon, do Chrome e do `ward-nm` e o console da extensão sem padrão de
#   `.dev/variations.txt`; o log do daemon sem as origens.
#
# Nunca toca o perfil real do Chrome, `~/.config/google-chrome` nem o daemon de
# `make dev-daemon`. Precisa de `make dev-vault`, `make dev-variations`,
# `make dev-services-up`, `make app-front` (fontes) e do Chrome em
# /opt/google/chrome/chrome; fora do `make check`.
#
# Uso: dev/tests/login.sh <wardd test-mode> <ward-nm test-mode> <ward test-mode>
#      <wardd-aprovador-teste>

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$RAIZ/dev/env.sh"
cd "$RAIZ"

abs() { case "$1" in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$RAIZ" "$1" ;; esac; }
USO="uso: login.sh <wardd> <ward-nm> <ward> <wardd-aprovador-teste>"
WARDD="$(abs "${1:?$USO}")"
NM="$(abs "${2:?$USO}")"
WARD="$(abs "${3:?$USO}")"
APROVADOR="$(abs "${4:?$USO}")"
for b in "$WARDD" "$NM" "$WARD" "$APROVADOR"; do [ -x "$b" ] || { echo "falta $b" >&2; exit 1; }; done
CHROME=/opt/google/chrome/chrome
[ -x "$CHROME" ] || { echo "falta o Chrome em $CHROME" >&2; exit 1; }
COFRE="$RAIZ/.dev/test-vault.ward"
LISTA="$RAIZ/.dev/variations.txt"
EXT="$RAIZ/extension/dist"
[ -f "$COFRE" ] && [ ! -L "$COFRE" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$LISTA" ] && [ ! -L "$LISTA" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }
[ -f "$EXT/manifest.json" ] || { echo "falta extension/dist: rode 'make extension'" >&2; exit 1; }
for porta in 8081 8082 8083 8084; do
    /usr/bin/python3 -I -c "import socket,sys; socket.create_connection(('127.0.0.1', int(sys.argv[1])), 2).close()" "$porta" 2>/dev/null \
        || { echo "serviço de teste na porta $porta fora do ar: rode 'make dev-services-up'" >&2; exit 1; }
done

UID_ATUAL="$(id -u)"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$UID_ATUAL}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
TRAB="$(mktemp -d "$RAIZ/.dev/login.XXXXXX")"
SOCKDIR="$RUNTIME/ward-login-teste-$$-${TRAB##*.}"
PID=""
limpar() {
    if [ -n "$PID" ] && kill -0 "$PID" 2>/dev/null; then
        kill -TERM "$PID" 2>/dev/null || true
        for _ in $(seq 1 50); do kill -0 "$PID" 2>/dev/null || break; sleep 0.1; done
        kill -KILL "$PID" 2>/dev/null || true
        wait "$PID" 2>/dev/null || true
    fi
    rm -rf -- "$TRAB" "$SOCKDIR"
}
trap limpar EXIT

cp -- "$COFRE" "$TRAB/test-vault.ward"
# Fase 44c: a janela de teste, dentro do "repositório" do daemon (a pasta acima de
# WARD_DEV_DIR, D63); começa confirmando a origem.
mkdir -m 0700 "$TRAB/logs"
cp -- "$APROVADOR" "$TRAB/wardd-aprovador-teste"
printf 'confirmar\n' > "$TRAB/aprovador.modo"
printf 'ward-estado 1\nvista 0\nqueimada 0\n' > "$TRAB/test-vault.state"
LOG="$TRAB/wardd.log"
env -i \
    HOME="$TRAB" \
    PATH=/usr/bin:/bin \
    XDG_RUNTIME_DIR="$RUNTIME" \
    LANG=C.UTF-8 \
    WARD_DEV_DIR="$TRAB" \
    WARD_DEV_SOCKET_DIR="$SOCKDIR" \
    WARD_DEV_EXTENSAO=chrome \
    WARD_DEV_APP="$TRAB/wardd-aprovador-teste" \
    RUST_LOG=trace \
    "$WARDD" </dev/null >"$LOG" 2>&1 &
PID=$!
SOCKET="$SOCKDIR/wardd.sock"
for _ in $(seq 1 200); do
    [ -S "$SOCKET" ] && break
    kill -0 "$PID" 2>/dev/null || { echo "o wardd de teste não subiu:" >&2; tail -n 20 -- "$LOG" >&2; exit 1; }
    sleep 0.05
done
[ -S "$SOCKET" ] || { echo "o wardd de teste não criou o socket" >&2; exit 1; }
echo "wardd de teste só deste script (WARD_DEV_EXTENSAO=chrome): pid $PID, socket $SOCKET"

falhas=0
/usr/bin/python3 -I -B "$RAIZ/dev/tests/login.py" "$RAIZ" "$TRAB" "$SOCKDIR" "$NM" "$CHROME" "$LISTA" "$EXT" "$WARD" \
    || falhas=$((falhas + 1))

kill -0 "$PID" 2>/dev/null || { echo "FALHA o wardd de teste morreu durante o teste" >&2; falhas=$((falhas + 1)); }
kill -TERM "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
PID=""
for f in "$LOG" "$TRAB/chrome.log" "$TRAB/console.log" "$TRAB/cli.log" "$TRAB/logs/wardd-aprovador-teste.log"; do
    if [ ! -f "$f" ]; then
        echo "FALHA ${f##*/} não existe" >&2
        falhas=$((falhas + 1))
    elif grep -qaF -f "$LISTA" -- "$f"; then
        echo "FALHA padrão de .dev/variations.txt em ${f##*/}" >&2
        falhas=$((falhas + 1))
    else
        echo "ok    ${f##*/} sem padrão de .dev/variations.txt"
    fi
done
# O log do daemon não leva a origem da aba (S13): nenhuma das origens usadas no teste.
if grep -qaE 'localhost:808[1-4]|127\.0\.0\.1:808[1-4]' -- "$LOG"; then
    echo "FALHA o log do daemon tem uma origem" >&2
    falhas=$((falhas + 1))
else
    echo "ok    log do daemon sem as origens"
fi
grep -qa 'login acme-wp-prod: formulário enviado' -- "$LOG" \
    && echo "ok    o daemon registrou o login enviado (só o nome da entrada)" \
    || { echo "FALHA o daemon não registrou o login enviado" >&2; falhas=$((falhas + 1)); }

if [ "$falhas" -ne 0 ]; then
    echo "test-login: $falhas parte(s) com falha" >&2
    exit 1
fi
echo "test-login: tudo certo"
