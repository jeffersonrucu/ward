#!/bin/bash -p
# Parte de dentro do `make teste-docker-plataforma` (Fase 21b; D46, D47; V3.5, V3.23,
# V3.24). Roda como root **só dentro** do contêiner `ubuntu:24.04` descartável; nada do
# host é montado além de /opt/teste (somente leitura: o binário de teste e a biblioteca
# de ataque) e deste script.
#
# Monta o que a instalação (Fase 51) vai montar: grupos de sistema `ward-app`/`ward-nm`
# sem membros, `ward-app` e `ward-nm` root:<grupo> 2755 em /usr/local/bin e a pasta
# /run/ward/1000 root:root 0755. O papel do systemd do sistema fica com `ativar`, que
# cria o socket como root e o passa ao "wardd" de teste (uid 1000) por LISTEN_FDS.
# Cada conferência imprime "ok:" ou "FALHOU:"; qualquer falha sai ≠ 0.

export PATH=/usr/sbin:/usr/bin:/bin
set -euo pipefail

BIN=/opt/teste/ward-teste
LIB=/opt/teste/libmarca.so
SOCK=/run/ward/1000/wardd.sock
LOG=/tmp/servidor.log
falhas=0

ok() { echo "ok: $*"; }
falhou() { echo "FALHOU: $*"; falhas=$((falhas + 1)); }
# Confere se a saída $2 contém o texto $3 (descrição em $1).
confere() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else falhou "$1 (esperado '$3', veio '$2')"; fi
}
# Confere se a saída $2 NÃO contém o texto $3.
nao_tem() {
    if [[ "$2" != *"$3"* ]]; then ok "$1"; else falhou "$1 (não devia ter '$3': '$2')"; fi
}
# Como o uid 1000 sem grupo nenhum (o usuário de teste; no host seria o usuário).
como_1000() { setpriv --reuid=1000 --regid=1000 --clear-groups -- "$@"; }

[ "$(id -u)" = 0 ] || { echo "dentro.sh roda como root, só no contêiner" >&2; exit 1; }
[ -f /.dockerenv ] || { echo "dentro.sh só roda dentro do contêiner" >&2; exit 1; }

# --- Instalação de teste -------------------------------------------------------------
groupadd --system ward-app
groupadd --system ward-nm
install -o root -g root -m 0755 "$BIN" /usr/local/bin/ward-servidor
install -o root -g ward-app -m 2755 "$BIN" /usr/local/bin/ward-app
install -o root -g ward-nm -m 2755 "$BIN" /usr/local/bin/ward-nm
install -d -o root -g root -m 0755 /run/ward /run/ward/1000
install -d -o 1000 -g 1000 -m 0700 /run/user/1000
GID_APP="$(getent group ward-app | cut -d: -f3)"
GID_NM="$(getent group ward-nm | cut -d: -f3)"

# --- D46: socket criado pelo root, entregue por LISTEN_FDS -----------------------------
# O daemon abre o app como filho (o pai do app é o daemon, D47).
/usr/local/bin/ward-servidor ativar "$SOCK" 1000 -- \
    /usr/local/bin/ward-servidor servidor --chrome /usr/bin/bash \
    --lancar /usr/local/bin/ward-app cliente >"$LOG" 2>&1 &
SRV=$!
for _ in $(seq 100); do grep -q "servidor: pronto" "$LOG" 2>/dev/null && break; sleep 0.1; done
pronto="$(grep "servidor: pronto" "$LOG" || true)"
confere "V3.23: o wardd de teste aceitou o socket do systemd (LISTEN_FDS)" "$pronto" "servidor: pronto"
confere "D46: o fd 3 original foi fechado" "$pronto" "fd3_aberto=false"
confere "D46: o socket que escuta tem O_CLOEXEC (o ward run não herda)" "$pronto" "cloexec=true"
confere "D47: grupos lidos do /etc/group" "$pronto" "gid_app=$GID_APP gid_nm=$GID_NM"
confere "D47: política do kernel aceita (ptrace_scope ≥ 1, suid_dumpable ≠ 1)" "$pronto" "politica=Ok(())"
modo="$(stat -c '%U:%G %a %F' /run/ward/1000) | $(stat -c '%u %a %F' "$SOCK")"
confere "D46: pasta root:root 0755 e socket 0600 do uid 1000" "$modo" "root:root 755 directory | 1000 600 socket"

# --- Cliente comum: conferência do servidor (uid 0 do par) e papel agente ------------
r="$(como_1000 /usr/local/bin/ward-servidor cliente)"
confere "V3.23: a conferência do cliente aceita o par uid 0 em pasta do root" "$r" "servidor=ok"
confere "D47: cliente sem grupo especial fica agente" "$r" "papel=agente motivo=GrupoComum"

# --- App aberto pelo daemon: papel app -----------------------------------------------
for _ in $(seq 50); do grep -q "app-filho:" "$LOG" && break; sleep 0.1; done
r="$(grep "app-filho:" "$LOG" || true)"
confere "V3.24: ward-app setgid aberto pelo daemon recebe o papel app" "$r" "papel=app"
# Com fs.suid_dumpable=2 o kernel põe o dumpable em 2 (SUID_DUMP_ROOT) no exec setgid:
# ptrace e /proc/<pid>/mem exigem CAP_SYS_PTRACE do mesmo jeito. Só o 1 seria inseguro.
nao_tem "V3.24: ward-app setgid não é dumpable para o usuário desde o exec" "$r" "dumpable=1"
confere "V3.24: o egid do ward-app é o grupo ward-app" "$r" "gid=1000,$GID_APP"

# --- ward-app lançado por outro processo: agente --------------------------------------
r="$(como_1000 /usr/local/bin/ward-app cliente)"
confere "D47: ward-app lançado por quem não é o daemon nem o systemd --user fica agente" \
    "$r" "papel=agente motivo=PaiDoAppNaoPermitido"

# --- ward-nm com o "Chrome" de pai: papel extensão ----------------------------------
# O "Chrome" do teste é o /usr/bin/bash (root, do sistema); o `; true` mantém o bash pai.
r="$(como_1000 /usr/bin/bash -c '/usr/local/bin/ward-nm cliente; true')"
confere "V3.24: ward-nm setgid com o Chrome de pai recebe o papel extensão" "$r" "papel=extensao"
r="$(como_1000 /usr/local/bin/ward-nm cliente)"
confere "D47: ward-nm sem o Chrome de pai fica agente" "$r" "papel=agente motivo=PaiNaoPermitido"

# --- V3.5 (forma de D47): cópia do binário perde o setgid ----------------------------
como_1000 cp /usr/local/bin/ward-app /run/user/1000/ward-app-copia
r="$(como_1000 /run/user/1000/ward-app-copia cliente)"
confere "V3.5: a cópia do ward-app perde o setgid e fica agente" "$r" "papel=agente motivo=GrupoComum"

# --- V3.24: LD_PRELOAD com marca ------------------------------------------------------
rm -f /tmp/ward-marca
# Controle: sem setgid, a biblioteca carrega e grava a marca.
como_1000 env LD_PRELOAD="$LIB" /run/user/1000/ward-app-copia cliente >/dev/null
if [ -f /tmp/ward-marca ]; then ok "controle: sem setgid o LD_PRELOAD carrega"; else falhou "controle do LD_PRELOAD não carregou"; fi
rm -f /tmp/ward-marca
r="$(como_1000 /usr/bin/bash -c "LD_PRELOAD=$LIB /usr/local/bin/ward-nm cliente; true")"
if [ ! -e /tmp/ward-marca ]; then ok "V3.24: no ward-nm setgid o LD_PRELOAD é ignorado (sem marca)"; else falhou "a marca do LD_PRELOAD apareceu no setgid"; fi
confere "V3.24: o ward-nm com LD_PRELOAD no ambiente segue com o papel" "$r" "papel=extensao"
rm -f /tmp/ward-marca
como_1000 env LD_PRELOAD="$LIB" /usr/local/bin/ward-app cliente >/dev/null
if [ ! -e /tmp/ward-marca ]; then ok "V3.24: no ward-app setgid o LD_PRELOAD é ignorado (sem marca)"; else falhou "a marca do LD_PRELOAD apareceu no ward-app"; fi

# --- V3.24: ptrace depois do exec e sob ptrace desde antes ----------------------------
r="$(como_1000 /usr/local/bin/ward-servidor prender /usr/local/bin/ward-app cliente --pausar)"
confere "V3.24: PTRACE_ATTACH no ward-app logo depois do exec dá EPERM" "$r" "attach=EPERM"
confere "V3.24: PTRACE_SEIZE no ward-app dá EPERM" "$r" "seize=EPERM"
confere "V3.24: /proc/<pid>/mem do ward-app dá EACCES" "$r" "mem=EACCES"
confere "V3.24: o ward-app pausado já está no grupo ward-app" "$r" "pronto gid=1000,$GID_APP"
r="$(como_1000 /usr/local/bin/ward-servidor prender /usr/local/bin/ward-nm cliente --pausar)"
confere "V3.24: PTRACE_ATTACH no ward-nm dá EPERM" "$r" "attach=EPERM"
confere "V3.24: PTRACE_SEIZE no ward-nm dá EPERM" "$r" "seize=EPERM"
# Controle: sem setgid, o mesmo pai se prende (o Yama ptrace_scope=1 deixa o ancestral).
r="$(como_1000 /usr/local/bin/ward-servidor prender /run/user/1000/ward-app-copia cliente --pausar)"
confere "controle: sem setgid o pai consegue PTRACE_ATTACH" "$r" "attach=ok"
r="$(como_1000 /usr/local/bin/ward-servidor traceme /usr/local/bin/ward-nm cliente)"
confere "V3.24: preso antes do exec, o kernel não aplica o setgid" "$r" "gid_no_exec=1000,1000,1000,1000"
confere "V3.24: preso antes do exec, o ward-nm fica agente" "$r" "papel=agente motivo=GrupoComum"

# --- V3.24: outro gid do mesmo uid fica agente ---------------------------------------
r="$(setpriv --reuid=1000 --regid=4242 --clear-groups -- /usr/local/bin/ward-servidor cliente)"
confere "V3.24: uid de teste com outro gid fica agente" "$r" "papel=agente motivo=GrupoComum"

# --- V3.23: servidor falso do mesmo uid -----------------------------------------------
install -d -o 1000 -g 1000 -m 0700 /run/user/1000/falso
r="$(como_1000 /usr/local/bin/ward-servidor bind /run/user/1000/falso/wardd.sock)"
confere "controle: o uid 1000 cria socket na pasta dele" "$r" "bind=ok"
# Servidor falso escutando de verdade (o próprio binário, como uid 1000, sem systemd).
como_1000 /usr/local/bin/ward-servidor ativar /run/user/1000/falso/wardd.sock 1000 -- /usr/bin/sleep 30 2>/dev/null &
FALSO=$!
sleep 0.5
r="$(como_1000 /usr/local/bin/ward-servidor cliente --sistema-em /run/user/1000/falso/wardd.sock)"
confere "V3.23: servidor falso do mesmo uid é recusado antes de qualquer byte" "$r" "servidor=PastaNaoConfiavel"
kill "$FALSO" 2>/dev/null || true

# --- V3.23: atacante mata o wardd e tenta tomar o lugar -------------------------------
como_1000 kill "$SRV"
wait "$SRV" 2>/dev/null || true
r="$(como_1000 /usr/local/bin/ward-servidor bind /run/ward/1000/falso.sock)"
confere "V3.23: o uid 1000 não cria socket em /run/ward/1000 (EACCES)" "$r" "bind=EACCES"
if como_1000 rm -f "$SOCK" 2>/dev/null; then
    [ -S "$SOCK" ] && ok "V3.23: o uid 1000 não apaga o socket do systemd" || falhou "o uid 1000 apagou o socket"
else
    ok "V3.23: o uid 1000 não apaga o socket do systemd"
fi
r="$(como_1000 /usr/local/bin/ward-servidor cliente)"
confere "com o wardd morto, o cliente vê o daemon indisponível" "$r" "servidor=Ausente"

echo
if [ "$falhas" -ne 0 ]; then
    echo "teste-docker-plataforma: $falhas conferência(s) falharam"
    echo "--- log do servidor ---"; cat "$LOG"
    exit 1
fi
echo "teste-docker-plataforma: tudo certo"
