#!/bin/bash -p
# Parte de dentro do `make teste-docker-plataforma` (Fases 21b, 29a e 40; D46, D47, D57; V3.5,
# V3.23, V3.24, V5.31). Roda como root **só dentro** do contêiner `ubuntu:24.04` descartável; nada do
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
confere "D47, D57: ward-app lançado por quem não é o daemon fica agente" \
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

# === Fase 29a: lançador do daemon e partida do app (D47, D57; V5.31) ==================
# O `ward-app` agora é o app de teste do `app/nucleo` (a partida do app real, sem GTK),
# e o daemon de teste se chama `wardd` (o app confere o nome do pai).
APP29=/opt/teste/ward-app-teste
REL=/tmp/ward-app-relatorio.txt
LOG2=/tmp/servidor-29a.log
install -o root -g root -m 0755 "$BIN" /usr/local/bin/wardd
install -o root -g ward-app -m 2755 "$APP29" /usr/local/bin/ward-app
# Sessão do uid 1000: um socket Wayland e o barramento, do uid (o lançador confere).
como_1000 /usr/local/bin/wardd escutar /run/user/1000/wayland-1 >/dev/null &
W1=$!
como_1000 /usr/local/bin/wardd escutar /run/user/1000/bus >/dev/null &
BUS=$!
como_1000 touch /run/user/1000/wayland-1.lock
for _ in $(seq 50); do [ -S /run/user/1000/wayland-1 ] && [ -S /run/user/1000/bus ] && break; sleep 0.1; done
rm -f "$REL"
# Linha do relatório do app (`inicio`/`depois`/`erro`/`queda`) para o pid $2.
linha_app() { grep -E "^$1 pid=$2( |$)" "$REL" 2>/dev/null || true; }
# pid da última execução do app com o prefixo `erro`.
ultimo_erro_pid() { grep '^erro ' "$REL" | tail -1 | sed -E 's/^erro pid=([0-9]+) .*/\1/'; }

# O daemon sobe com o ambiente "sujo" (um `systemctl --user set-environment` do atacante
# chegaria assim): nada disso pode chegar ao app (D57: ambiente montado do zero).
/usr/local/bin/wardd ativar "$SOCK" 1000 -- /usr/bin/env \
    WAYLAND_DISPLAY=/tmp/proxy.sock GTK_MODULES=/tmp/x.so GST_PLUGIN_PATH=/tmp/gst \
    GIO_EXTRA_MODULES=/tmp/gio DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/bus \
    WEBKIT_INJECTED_BUNDLE_PATH=/tmp/wk LANG=C.UTF-8 \
    /usr/local/bin/wardd servidor --chrome /usr/bin/bash \
    --abrir-app /usr/local/bin/ward-app \
    --abrir-app-com /usr/local/bin/ward-app WAYLAND_DISPLAY=/tmp/proxy.sock \
    --abrir-app-com /usr/local/bin/ward-app WAYLAND_DISPLAY=../x \
    --abrir-app-com /usr/local/bin/ward-app DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/bus \
    --abrir-app-com /usr/local/bin/ward-app XDG_RUNTIME_DIR=/tmp/x \
    --abrir-app-com /usr/local/bin/ward-app GTK_MODULES=/tmp/x.so \
    >"$LOG2" 2>&1 &
SRV2=$!
for _ in $(seq 100); do [ "$(grep -c '^forcado: aberto' "$LOG2" 2>/dev/null)" = 5 ] && break; sleep 0.1; done
for _ in $(seq 50); do [ "$(grep -c '^queda ' "$REL" 2>/dev/null)" -ge 1 ] && [ "$(grep -c '^erro ' "$REL" 2>/dev/null)" -ge 5 ] && break; sleep 0.1; done
sleep 0.5

r="$(grep '^lancador:' "$LOG2" || true)"
confere "D57: o lançador do daemon abriu o ward-app setgid" "$r" "lancador: aberto pid="
confere "D57: uma instância por vez (o segundo pedido não abre outra)" "$r" "lancador: segundo JaAberto"
APID="$(grep '^lancador: aberto' "$LOG2" | head -1 | sed -E 's/.*pid=([0-9]+).*/\1/')"
r="$(linha_app inicio "$APID" | head -1)"
confere "V5.31: desde o exec, o ward-app está no grupo ward-app" "$r" "gid=1000,$GID_APP,$GID_APP,$GID_APP"
nao_tem "V5.31: desde o exec, Dumpable ≠ 1 (2 com suid_dumpable=2)" "$r" "dumpable=1"
confere "V5.31: desde o exec, TracerPid 0" "$r" "tracer=0"
confere "D57: o app recebe só 0–2, ligados a /dev/null (fds ≥ 3 fechados)" "$r" "fds=0,1,2,3 std=/dev/null,/dev/null,/dev/null"
r="$(linha_app depois "$APID" | head -1)"
confere "V5.31: depois da partida, os gids voltam ao real" "$r" "gid=1000,1000,1000,1000"
confere "V5.31: depois da partida, Dumpable 0" "$r" "dumpable=0"
confere "V5.31: depois da partida, TracerPid 0" "$r" "tracer=0"
# Todos os sockets (a conexão, a cópia da vigia e as do transporte de D65 a; a leitora
# solta a dela quando o "daemon" fecha, então o número varia) têm CLOEXEC.
confere "V5.31: o socket do daemon tem CLOEXEC" "$r" "socket_cloexec=true/"
confere "V3.34: o app lançado tem VmLck 0 kB (o mlockall do daemon não passa pelo execve)" "$r" "vmlck=0,kB"
nao_tem "V3.34: o daemon de teste travou a memória (mlockall com LimitMEMLOCK infinito)" "$(grep '^servidor: pronto' "$LOG2" || true)" "vmlck=0,kB"
confere "V5.31: o ambiente do app é exatamente o de D57 (sem DISPLAY, LISTEN_*, GTK_MODULES, GST_PLUGIN_PATH, GIO_EXTRA_MODULES, WEBKIT_INJECTED_BUNDLE_PATH)" \
    "$r" "env=CUDA_MPS_PIPE_DIRECTORY,DBUS_SESSION_BUS_ADDRESS,GDK_BACKEND,GST_PLUGIN_SYSTEM_PATH,HOME,LANG,NO_AT_BRIDGE,PATH,WAYLAND_DISPLAY,WEBKIT_A11Y_BUS_ADDRESS,XDG_RUNTIME_DIR,XDG_SESSION_TYPE"
r="$(grep "^servidor: pid=$APID " "$LOG2" || true)"
confere "V5.31: o app aberto pelo daemon conectou antes de largar o grupo e recebeu o papel app" "$r" "gid=$GID_APP papel=app"
confere "D47: com a conexão caída e o pai ainda o daemon, o app se executa de novo" "$(linha_app queda "$APID")" "queda pid=$APID"
n="$(grep -c "^servidor: pid=$APID gid=$GID_APP papel=app" "$LOG2" || true)"
confere "D47: depois de se executar de novo, o app reconecta com o papel app" "conexoes=$n" "conexoes=2"
# Variáveis postas à força no ambiente do app aberto pelo daemon: o conferidor do app
# sai antes de conectar.
n_forcados=0
while read -r fpid nome; do
    n_forcados=$((n_forcados + 1))
    r="$(linha_app erro "$fpid")"
    confere "V5.31: app aberto pelo daemon com $nome forçado sai com mensagem" "$r" "variável $nome"
    r="$(grep "^servidor: pid=$fpid " "$LOG2" || true)"
    if [ -z "$r" ]; then ok "V5.31: com $nome forçado, o app nem conecta"; else falhou "o app com $nome forçado conectou: $r"; fi
done < <(grep '^forcado: aberto' "$LOG2" | sed -E 's/^forcado: aberto pid=([0-9]+) (.*)$/\1 \2/')
confere "V5.31: os cinco ambientes forçados foram tentados" "forcados=$n_forcados" "forcados=5"

# Lançado por um processo qualquer, com o próprio ambiente: sai antes de conectar.
como_1000 /usr/local/bin/ward-app || true
r="$(linha_app erro "$(ultimo_erro_pid)")"
confere "V5.31: o ward-app aberto por outro processo, com o ambiente dele, sai" "$r" "não é do ambiente do app"
# Lançado por um processo qualquer com o ambiente de D57 copiado: conecta, o daemon dá
# o papel agente (o pai não é ele) e o app sai.
amb="$(como_1000 /usr/local/bin/wardd ambiente)"
# O `bash -c '…; true'` fica de pai (sem ele, o pai seria o pid 1 do contêiner).
# shellcheck disable=SC2086
como_1000 /usr/bin/bash -c "/usr/bin/env -i $(echo $amb) /usr/local/bin/ward-app; true"
QPID="$(ultimo_erro_pid)"
confere "V5.31: o ward-app aberto por outro processo sai com mensagem" "$(linha_app erro "$QPID")" "não foi aberto pelo daemon"
confere "V5.31: o ward-app aberto por outro processo não recebe o papel app" \
    "$(grep "^servidor: pid=$QPID " "$LOG2" || true)" "papel=agente motivo=PaiDoAppNaoPermitido"
# Lançado por quem imita o `systemd --user` (como uma unit de ~/.config/systemd/user/).
install -d -o 1000 -g 1000 -m 0755 /tmp/gerente
install -o 1000 -g 1000 -m 0755 "$BIN" /tmp/gerente/systemd
rm -f /tmp/gerente.log
como_1000 /usr/bin/env WARD_GERENTE_PROGRAMA=/usr/local/bin/ward-app /usr/bin/bash -c \
    '( exec -a /usr/lib/systemd/systemd /tmp/gerente/systemd --user >/tmp/gerente.log 2>&1 & )'
for _ in $(seq 50); do grep -q "gerente: Ok" /tmp/gerente.log 2>/dev/null && break; sleep 0.1; done
r="$(cat /tmp/gerente.log 2>/dev/null || true)"
confere "V5.31: o pai falso imita o gerente (Name, PPid 1 e cmdline)" "$r" "gerente: nome=systemd ppid=1 cmdline=/usr/lib/systemd/systemd --user"
GPID="$(ultimo_erro_pid)"
confere "V5.31: o ward-app aberto pelo gerente falso sai com mensagem" "$(linha_app erro "$GPID")" "não foi aberto pelo daemon"
confere "V5.31: o ward-app aberto pelo gerente falso não recebe o papel app" \
    "$(grep "^servidor: pid=$GPID " "$LOG2" || true)" "papel=agente motivo=PaiDoAppNaoPermitido"

# Sessão errada: o lançador não abre e responde "aprovação indisponível" (D21).
como_1000 /usr/local/bin/wardd escutar /run/user/1000/wayland-0 >/dev/null &
W0=$!
for _ in $(seq 50); do [ -S /run/user/1000/wayland-0 ] && break; sleep 0.1; done
r="$(como_1000 /usr/local/bin/wardd lancar-app /usr/local/bin/ward-app)"
confere "V5.31: com dois wayland-<n>, o lançador não abre (aprovação indisponível)" "$r" "Sessao(VariosWayland) indisponivel=true"
kill "$W0" 2>/dev/null || true; wait "$W0" 2>/dev/null || true
rm -f /run/user/1000/wayland-0
como_1000 /usr/local/bin/wardd escutar /run/user/1000/proxy.sock >/dev/null &
PROXY=$!
for _ in $(seq 50); do [ -S /run/user/1000/proxy.sock ] && break; sleep 0.1; done
mv /run/user/1000/wayland-1 /run/user/1000/wayland-1.real
como_1000 ln -s /run/user/1000/proxy.sock /run/user/1000/wayland-1
r="$(como_1000 /usr/local/bin/wardd lancar-app /usr/local/bin/ward-app)"
confere "V5.31: com o wayland-<n> trocado por um link, o lançador não abre" "$r" "Sessao(WaylandNaoConfiavel) indisponivel=true"
rm -f /run/user/1000/wayland-1
mv /run/user/1000/wayland-1.real /run/user/1000/wayland-1
kill "$PROXY" 2>/dev/null || true
# Binário que não é o instalado (cópia do usuário, sem setgid): o lançador não abre.
como_1000 cp /usr/local/bin/ward-app /run/user/1000/ward-app-29a
r="$(como_1000 /usr/local/bin/wardd lancar-app /run/user/1000/ward-app-29a)"
confere "D57: o lançador só abre o ward-app do root, setgid do grupo ward-app" "$r" "BinarioNaoConfiavel"

kill "$SRV2" "$W1" "$BUS" 2>/dev/null || true
wait "$SRV2" 2>/dev/null || true

# === Fase 40: ward-nm com o wardd de teste real (D5, D9, D47; V6.1, V6.2) ============
# O `ward-nm` é o do release (sem `modo-teste`): só fala com /run/ward/1000/wardd.sock
# em pasta do root e par uid 0. O daemon é o `wardd` de teste (cofre de teste, cópia)
# com o socket do "systemd" por LISTEN_FDS. O "Chrome" é o `chrome-falso` instalado em
# /opt/google/chrome/chrome (root, 0755): abre o host como filho direto, como o Chrome.
LOG40=/tmp/wardd-40.log
DEV40=/tmp/ward-dev-40
install -d -o 1000 -g 1000 -m 0700 "$DEV40"
install -o 1000 -g 1000 -m 0600 /opt/teste/cofre-teste.ward "$DEV40/cofre-teste.ward"
printf 'ward-estado 1\nvista 0\nqueimada 0\n' > "$DEV40/cofre-teste.estado"
chown 1000:1000 "$DEV40/cofre-teste.estado"
chmod 0600 "$DEV40/cofre-teste.estado"
install -o root -g root -m 0755 /opt/teste/wardd-real /usr/local/bin/wardd-real
install -o root -g ward-nm -m 2755 /opt/teste/ward-nm-real /usr/local/bin/ward-nm
install -d -o root -g root -m 0755 /opt/google /opt/google/chrome
install -o root -g root -m 0755 /opt/teste/chrome-falso /opt/google/chrome/chrome
/usr/local/bin/ward-servidor ativar "$SOCK" 1000 -- /usr/bin/env HOME="$DEV40" \
    XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 WARD_DEV_DIR="$DEV40" RUST_LOG=trace \
    /usr/local/bin/wardd-real >"$LOG40" 2>&1 &
SRV40=$!
for _ in $(seq 100); do grep -q "pronto" "$LOG40" 2>/dev/null && break; sleep 0.1; done
confere "Fase 40: o wardd de teste real subiu com o socket do systemd" "$(grep -c pronto "$LOG40" || true)" "1"
r="$(como_1000 /opt/teste/wardd-cliente-teste "$SOCK" destravar < /opt/teste/senha)"
confere "Fase 40: destravou a cópia do cofre de teste" "$r" "resposta: ok"

M=/tmp/msgs-40.jsonl
cat > "$M" <<'FIM'
{"v":1,"op":"estado"}
{"v":1,"op":"sugerir","dados":{"origem":"http://localhost:8081/wp-login.php"}}
{"v":1,"op":"preencher","dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081/wp-login.php"}}
{"v":1,"op":"preencher","dados":{"entrada":"acme-wp-dev","origem":"http://127.0.0.1:8081/wp-login.php"}}
{"v":1,"op":"sugerir","dados":{"origem":"http://127.0.0.1:8081/wp-login.php"}}
{"v":1,"op":"revelar","dados":{"entrada":"acme-wp-dev","campo":"senha"}}
{"v":1,"op":"lista"}
FIM
chmod 0644 "$M"
# Linha $1 da saída em $r.
l() { sed -n "${1}p" <<<"$r"; }
r="$(como_1000 /opt/google/chrome/chrome /usr/local/bin/ward-nm "$M")"
confere "D47: ward-nm setgid aberto pelo Chrome instalado recebe o papel extensão" "$(l 1)" '"papel":"extensao"'
confere "D9: sugerir em localhost:8081 traz acme-wp-dev" "$(l 2)" '"nome":"acme-wp-dev"'
confere "D9: sugerir em localhost:8081 traz acme-wp-prod" "$(l 2)" '"nome":"acme-wp-prod"'
confere "D9: preencher acme-wp-dev na origem certa devolve o preenchimento" "$(l 3)" '"tipo":"preenchimento","dados":{"usuario":"wp_admin","senha":"'
confere "V6.2: preencher acme-wp-dev em 127.0.0.1:8081 é recusado pelo daemon" "$(l 4)" '"codigo":"origem_diferente"'
nao_tem "V6.2: a recusa de origem não leva valor" "$(l 4)" '"senha"'
confere "V6.2: sugerir em 127.0.0.1:8081 não traz acme-wp-*" "$(l 5)" '"itens":[]'
confere "papel extensão: revelar (operação do app) recusado" "$(l 6)" '"codigo":"nao_autorizado"'
confere "papel extensão: lista (operação do agente e do app) recusada" "$(l 7)" '"codigo":"nao_autorizado"'
confere "o host sai 0 no fim do stdin" "$(l 8)" "saida=0"

# V6.1: o mesmo ward-nm executado por um processo do usuário (pai não é o Chrome).
printf '{"v":1,"op":"preencher","dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081/wp-login.php"}}' > /tmp/v61.json
printf "$(printf '\\x%02x\\x00\\x00\\x00' "$(stat -c %s /tmp/v61.json)")" > /tmp/v61.bin
cat /tmp/v61.json >> /tmp/v61.bin
chmod 0644 /tmp/v61.bin
r="$(como_1000 /usr/bin/bash -c '/usr/local/bin/ward-nm chrome-extension://dkacdineofgfabdognjgeoakekaenaei/ < /tmp/v61.bin | tr -d "\000-\037"; true')"
confere "V6.1: ward-nm executado direto: o daemon recusa (nao_autorizado)" "$r" "nao_autorizado"
nao_tem "V6.1: ward-nm executado direto: nada de senha no stdout" "$r" "senha"
# Uma cópia do "Chrome" fora do caminho instalado, do usuário: agente.
como_1000 cp /opt/google/chrome/chrome /run/user/1000/chrome
r="$(como_1000 /run/user/1000/chrome /usr/local/bin/ward-nm "$M")"
confere "V6.1: pai que é cópia do Chrome fora de /opt/google/chrome: estado nao_autorizado" "$(l 1)" '"codigo":"nao_autorizado"'
confere "V6.1: pai que é cópia do Chrome: preencher recusado" "$(l 3)" '"codigo":"nao_autorizado"'
nao_tem "V6.1: pai que é cópia do Chrome: nada de preenchimento" "$r" "preenchimento"
# O Chrome instalado com LD_PRELOAD no ambiente (o Chrome não é setgid): agente.
r="$(como_1000 /usr/bin/env LD_PRELOAD="$LIB" /opt/google/chrome/chrome /usr/local/bin/ward-nm "$M")"
confere "D5: Chrome com LD_PRELOAD de pai: preencher recusado" "$(l 3)" '"codigo":"nao_autorizado"'
nao_tem "D5: Chrome com LD_PRELOAD de pai: nada de preenchimento" "$r" "preenchimento"
# Uma cópia do ward-nm (sem setgid) com o Chrome instalado de pai: agente.
como_1000 cp /usr/local/bin/ward-nm /run/user/1000/ward-nm-copia
r="$(como_1000 /opt/google/chrome/chrome /run/user/1000/ward-nm-copia "$M")"
confere "V3.5/D47: cópia do ward-nm sem setgid, com o Chrome de pai: preencher recusado" "$(l 3)" '"codigo":"nao_autorizado"'
nao_tem "V3.5/D47: cópia do ward-nm: nada de preenchimento" "$r" "preenchimento"
# Mensagem acima do limite pelo Chrome.
printf '{"v":1,"op":"sugerir","dados":{"origem":"http://localhost:8081/%s"}}\n' "$(head -c 70000 /dev/zero | tr '\0' a)" > /tmp/grande-40.jsonl
chmod 0644 /tmp/grande-40.jsonl
r="$(como_1000 /opt/google/chrome/chrome /usr/local/bin/ward-nm /tmp/grande-40.jsonl)"
confere "mensagem de mais de 64 KiB: quadro_invalido" "$r" '"codigo":"quadro_invalido"'
confere "mensagem de mais de 64 KiB: o host sai ≠ 0" "$r" "saida=1"

kill "$SRV40" 2>/dev/null || true
wait "$SRV40" 2>/dev/null || true
n="$(grep -c 'falso-' "$LOG40" || true)"
confere "S13: o log do wardd (RUST_LOG=trace) não tem valor falso do cofre" "falso=$n" "falso=0"
confere "D9: o daemon registrou a recusa de origem (só o nome da entrada)" "$(grep 'origem diferente' "$LOG40" || true)" "preencher acme-wp-dev recusado: origem diferente"
nao_tem "S13: o log não tem a origem pedida" "$(cat "$LOG40")" "127.0.0.1:8081"

echo
if [ "$falhas" -ne 0 ]; then
    echo "teste-docker-plataforma: $falhas conferência(s) falharam"
    echo "--- log do servidor ---"; cat "$LOG"
    echo "--- log do servidor (29a) ---"; cat "$LOG2" 2>/dev/null || true
    echo "--- relatório do app (29a) ---"; cat "$REL" 2>/dev/null || true
    echo "--- log do wardd (40) ---"; grep -v 'falso-' "$LOG40" 2>/dev/null || true
    exit 1
fi
echo "teste-docker-plataforma: tudo certo"
