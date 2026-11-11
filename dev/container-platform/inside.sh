#!/bin/bash -p
# Inside part of `make test-docker-platform` (Phases 21b, 29a, 28 and 40; D5, D9,
# D46, D47, D57, D65 e; V3.5, V3.23, V3.24, V5.31, the 2nd part of V4.2, V6.1 and V6.2).
# Runs as root **only inside** the disposable `ubuntu:24.04` container; nothing from the
# host is mounted besides the read-only test mount under /opt (the test binary, the attack library, the
# release `ward` CLI and `wardd`, the fake server of V4.2 and, in Phase 40, the test
# `wardd`, the release `ward-nm`, the fake "Chrome", the minimal client and a copy of the
# test vault) and this script.
#
# Builds what the install (Phase 51) will build: system groups `ward-app`/`ward-nm`
# without members, `ward-app` and `ward-nm` root:<group> 2755 in /usr/local/bin and the
# folder /run/ward/1000 root:root 0755. The role of the system systemd is played by
# activation subcommand, which creates the socket as root and passes it to the test "wardd" (uid 1000)
# through LISTEN_FDS. Each check prints "ok:" or the failure marker; any failure exits ≠ 0.

export PATH=/usr/sbin:/usr/bin:/bin
set -euo pipefail

BIN=/opt/teste/ward-teste
LIB=/opt/teste/libmarca.so
SOCK=/run/ward/1000/wardd.sock
LOG=/tmp/servidor.log
failures=0

ok() { echo "ok: $*"; }
failed() { echo "FALHOU: $*"; failures=$((failures + 1)); }
# Checks that the output $2 contains the text $3 (description in $1).
check() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else failed "$1 (esperado '$3', veio '$2')"; fi
}
# Checks that the output $2 does NOT contain the text $3.
lacks() {
    if [[ "$2" != *"$3"* ]]; then ok "$1"; else failed "$1 (não devia ter '$3': '$2')"; fi
}
# As uid 1000 without any group (the test user; on the host it would be the user).
as_1000() { setpriv --reuid=1000 --regid=1000 --clear-groups -- "$@"; }

[ "$(id -u)" = 0 ] || { echo "inside.sh roda como root, só no contêiner" >&2; exit 1; }
[ -f /.dockerenv ] || { echo "inside.sh só roda dentro do contêiner" >&2; exit 1; }

# --- Test install ---------------------------------------------------------------------
groupadd --system ward-app
groupadd --system ward-nm
install -o root -g root -m 0755 "$BIN" /usr/local/bin/ward-servidor
install -o root -g ward-app -m 2755 "$BIN" /usr/local/bin/ward-app
install -o root -g ward-nm -m 2755 "$BIN" /usr/local/bin/ward-nm
install -d -o root -g root -m 0755 /run/ward /run/ward/1000
install -d -o 1000 -g 1000 -m 0700 /run/user/1000
GID_APP="$(getent group ward-app | cut -d: -f3)"
GID_NM="$(getent group ward-nm | cut -d: -f3)"

# --- D46: socket created by root, handed over through LISTEN_FDS ------------------------
# The daemon opens the app as a child (the app's parent is the daemon, D47).
/usr/local/bin/ward-servidor activate "$SOCK" 1000 -- \
    /usr/local/bin/ward-servidor server --chrome /usr/bin/bash \
    --launch /usr/local/bin/ward-app client >"$LOG" 2>&1 &
SRV=$!
for _ in $(seq 100); do grep -q "servidor: pronto" "$LOG" 2>/dev/null && break; sleep 0.1; done
ready_line="$(grep "servidor: pronto" "$LOG" || true)"
check "V3.23: o wardd de teste aceitou o socket do systemd (LISTEN_FDS)" "$ready_line" "servidor: pronto"
check "D46: o fd 3 original foi fechado" "$ready_line" "fd3_aberto=false"
check "D46: o socket que escuta tem O_CLOEXEC (o ward run não herda)" "$ready_line" "cloexec=true"
check "D47: grupos lidos do /etc/group" "$ready_line" "gid_app=$GID_APP gid_nm=$GID_NM"
check "D47: política do kernel aceita (ptrace_scope ≥ 1, suid_dumpable ≠ 1)" "$ready_line" "politica=Ok(())"
modes="$(stat -c '%U:%G %a %F' /run/ward/1000) | $(stat -c '%u %a %F' "$SOCK")"
check "D46: pasta root:root 0755 e socket 0600 do uid 1000" "$modes" "root:root 755 directory | 1000 600 socket"

# --- Common client: server check (peer uid 0) and agent role -------------------------
r="$(as_1000 /usr/local/bin/ward-servidor client)"
check "V3.23: a conferência do cliente aceita o par uid 0 em pasta do root" "$r" "servidor=ok"
check "D47: cliente sem grupo especial fica agente" "$r" "papel=agente motivo=CommonGroup"

# --- App opened by the daemon: app role ------------------------------------------------
for _ in $(seq 50); do grep -q "app-filho:" "$LOG" && break; sleep 0.1; done
r="$(grep "app-filho:" "$LOG" || true)"
check "V3.24: ward-app setgid aberto pelo daemon recebe o papel app" "$r" "papel=app"
# With fs.suid_dumpable=2 the kernel sets dumpable to 2 (SUID_DUMP_ROOT) on the setgid
# exec: ptrace and /proc/<pid>/mem need CAP_SYS_PTRACE all the same. Only 1 would be unsafe.
lacks "V3.24: ward-app setgid não é dumpable para o usuário desde o exec" "$r" "dumpable=1"
check "V3.24: o egid do ward-app é o grupo ward-app" "$r" "gid=1000,$GID_APP"

# --- ward-app launched by another process: agent ---------------------------------------
r="$(as_1000 /usr/local/bin/ward-app client)"
check "D47, D57: ward-app lançado por quem não é o daemon fica agente" \
    "$r" "papel=agente motivo=AppParentNotAllowed"

# --- ward-nm with "Chrome" as parent: extension role -----------------------------------
# The test "Chrome" is /usr/bin/bash (root, from the system); the `; true` keeps bash as parent.
r="$(as_1000 /usr/bin/bash -c '/usr/local/bin/ward-nm client; true')"
check "V3.24: ward-nm setgid com o Chrome de pai recebe o papel extensão" "$r" "papel=extensao"
r="$(as_1000 /usr/local/bin/ward-nm client)"
check "D47: ward-nm sem o Chrome de pai fica agente" "$r" "papel=agente motivo=ParentNotAllowed"

# --- V3.5 (form of D47): a copy of the binary loses the setgid -------------------------
as_1000 cp /usr/local/bin/ward-app /run/user/1000/ward-app-copia
r="$(as_1000 /run/user/1000/ward-app-copia client)"
check "V3.5: a cópia do ward-app perde o setgid e fica agente" "$r" "papel=agente motivo=CommonGroup"

# --- V3.24: LD_PRELOAD with a mark -----------------------------------------------------
rm -f /tmp/ward-marca
# Control: without setgid, the library loads and writes the mark.
as_1000 env LD_PRELOAD="$LIB" /run/user/1000/ward-app-copia client >/dev/null
if [ -f /tmp/ward-marca ]; then ok "controle: sem setgid o LD_PRELOAD carrega"; else failed "controle do LD_PRELOAD não carregou"; fi
rm -f /tmp/ward-marca
r="$(as_1000 /usr/bin/bash -c "LD_PRELOAD=$LIB /usr/local/bin/ward-nm client; true")"
if [ ! -e /tmp/ward-marca ]; then ok "V3.24: no ward-nm setgid o LD_PRELOAD é ignorado (sem marca)"; else failed "a marca do LD_PRELOAD apareceu no setgid"; fi
check "V3.24: o ward-nm com LD_PRELOAD no ambiente segue com o papel" "$r" "papel=extensao"
rm -f /tmp/ward-marca
as_1000 env LD_PRELOAD="$LIB" /usr/local/bin/ward-app client >/dev/null
if [ ! -e /tmp/ward-marca ]; then ok "V3.24: no ward-app setgid o LD_PRELOAD é ignorado (sem marca)"; else failed "a marca do LD_PRELOAD apareceu no ward-app"; fi

# --- V3.24: ptrace after the exec and under ptrace from before -------------------------
r="$(as_1000 /usr/local/bin/ward-servidor attach /usr/local/bin/ward-app client --pause)"
check "V3.24: PTRACE_ATTACH no ward-app logo depois do exec dá EPERM" "$r" "attach=EPERM"
check "V3.24: PTRACE_SEIZE no ward-app dá EPERM" "$r" "seize=EPERM"
check "V3.24: /proc/<pid>/mem do ward-app dá EACCES" "$r" "mem=EACCES"
check "V3.24: o ward-app pausado já está no grupo ward-app" "$r" "pronto gid=1000,$GID_APP"
r="$(as_1000 /usr/local/bin/ward-servidor attach /usr/local/bin/ward-nm client --pause)"
check "V3.24: PTRACE_ATTACH no ward-nm dá EPERM" "$r" "attach=EPERM"
check "V3.24: PTRACE_SEIZE no ward-nm dá EPERM" "$r" "seize=EPERM"
# Control: without setgid, the same parent attaches (Yama ptrace_scope=1 allows the ancestor).
r="$(as_1000 /usr/local/bin/ward-servidor attach /run/user/1000/ward-app-copia client --pause)"
check "controle: sem setgid o pai consegue PTRACE_ATTACH" "$r" "attach=ok"
r="$(as_1000 /usr/local/bin/ward-servidor traceme /usr/local/bin/ward-nm client)"
check "V3.24: preso antes do exec, o kernel não aplica o setgid" "$r" "gid_no_exec=1000,1000,1000,1000"
check "V3.24: preso antes do exec, o ward-nm fica agente" "$r" "papel=agente motivo=CommonGroup"

# --- V3.24: another gid of the same uid stays agent ------------------------------------
r="$(setpriv --reuid=1000 --regid=4242 --clear-groups -- /usr/local/bin/ward-servidor client)"
check "V3.24: uid de teste com outro gid fica agente" "$r" "papel=agente motivo=CommonGroup"

# --- V3.23: fake server of the same uid ------------------------------------------------
install -d -o 1000 -g 1000 -m 0700 /run/user/1000/falso
r="$(as_1000 /usr/local/bin/ward-servidor bind /run/user/1000/falso/wardd.sock)"
check "controle: o uid 1000 cria socket na pasta dele" "$r" "bind=ok"
# Fake server really listening (the binary itself, as uid 1000, without systemd).
as_1000 /usr/local/bin/ward-servidor activate /run/user/1000/falso/wardd.sock 1000 -- /usr/bin/sleep 30 2>/dev/null &
FAKE=$!
sleep 0.5
r="$(as_1000 /usr/local/bin/ward-servidor client --system-at /run/user/1000/falso/wardd.sock)"
check "V3.23: servidor falso do mesmo uid é recusado antes de qualquer byte" "$r" "servidor=UntrustedDir"
kill "$FAKE" 2>/dev/null || true

# --- V3.23: the attacker kills the wardd and tries to take its place --------------------
as_1000 kill "$SRV"
wait "$SRV" 2>/dev/null || true
r="$(as_1000 /usr/local/bin/ward-servidor bind /run/ward/1000/falso.sock)"
check "V3.23: o uid 1000 não cria socket em /run/ward/1000 (EACCES)" "$r" "bind=EACCES"
if as_1000 rm -f "$SOCK" 2>/dev/null; then
    [ -S "$SOCK" ] && ok "V3.23: o uid 1000 não apaga o socket do systemd" || failed "o uid 1000 apagou o socket"
else
    ok "V3.23: o uid 1000 não apaga o socket do systemd"
fi
r="$(as_1000 /usr/local/bin/ward-servidor client)"
check "com o wardd morto, o cliente vê o daemon indisponível" "$r" "servidor=Missing"

# === Phase 29a: daemon launcher and app startup (D47, D57; V5.31) =====================
# The `ward-app` is now the test app of `app/core` (the real app's startup, without GTK),
# and the test daemon is called `wardd` (the app checks the parent's name).
APP29=/opt/teste/ward-app-teste
REPORT=/tmp/ward-app-relatorio.txt
LOG2=/tmp/servidor-29a.log
install -o root -g root -m 0755 "$BIN" /usr/local/bin/wardd
install -o root -g ward-app -m 2755 "$APP29" /usr/local/bin/ward-app
# Session of uid 1000: a Wayland socket and the bus, of the uid (the launcher checks).
as_1000 /usr/local/bin/wardd listen /run/user/1000/wayland-1 >/dev/null &
W1=$!
as_1000 /usr/local/bin/wardd listen /run/user/1000/bus >/dev/null &
BUS=$!
as_1000 touch /run/user/1000/wayland-1.lock
for _ in $(seq 50); do [ -S /run/user/1000/wayland-1 ] && [ -S /run/user/1000/bus ] && break; sleep 0.1; done
rm -f "$REPORT"
# Line of the app report (start, after, error or drop marker) for pid $2.
app_line() { grep -E "^$1 pid=$2( |$)" "$REPORT" 2>/dev/null || true; }
# pid of the last app execution with the error prefix.
last_error_pid() { grep '^erro ' "$REPORT" | tail -1 | sed -E 's/^erro pid=([0-9]+) .*/\1/'; }

# The daemon starts with a "dirty" environment (an attacker's `systemctl --user
# set-environment` would arrive like this): none of it may reach the app (D57: environment built from scratch).
/usr/local/bin/wardd activate "$SOCK" 1000 -- /usr/bin/env \
    WAYLAND_DISPLAY=/tmp/proxy.sock GTK_MODULES=/tmp/x.so GST_PLUGIN_PATH=/tmp/gst \
    GIO_EXTRA_MODULES=/tmp/gio DISPLAY=:0 DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/bus \
    WEBKIT_INJECTED_BUNDLE_PATH=/tmp/wk LANG=C.UTF-8 \
    /usr/local/bin/wardd server --chrome /usr/bin/bash \
    --open-app /usr/local/bin/ward-app \
    --open-app-with /usr/local/bin/ward-app WAYLAND_DISPLAY=/tmp/proxy.sock \
    --open-app-with /usr/local/bin/ward-app WAYLAND_DISPLAY=../x \
    --open-app-with /usr/local/bin/ward-app DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/bus \
    --open-app-with /usr/local/bin/ward-app XDG_RUNTIME_DIR=/tmp/x \
    --open-app-with /usr/local/bin/ward-app GTK_MODULES=/tmp/x.so \
    >"$LOG2" 2>&1 &
SRV2=$!
for _ in $(seq 100); do [ "$(grep -c '^forcado: aberto' "$LOG2" 2>/dev/null)" = 5 ] && break; sleep 0.1; done
for _ in $(seq 50); do [ "$(grep -c '^queda ' "$REPORT" 2>/dev/null)" -ge 1 ] && [ "$(grep -c '^erro ' "$REPORT" 2>/dev/null)" -ge 5 ] && break; sleep 0.1; done
sleep 0.5

r="$(grep '^lancador:' "$LOG2" || true)"
check "D57: o lançador do daemon abriu o ward-app setgid" "$r" "lancador: aberto pid="
check "D57: uma instância por vez (o segundo pedido não abre outra)" "$r" "lancador: segundo AlreadyOpen"
APID="$(grep '^lancador: aberto' "$LOG2" | head -1 | sed -E 's/.*pid=([0-9]+).*/\1/')"
r="$(app_line inicio "$APID" | head -1)"
check "V5.31: desde o exec, o ward-app está no grupo ward-app" "$r" "gid=1000,$GID_APP,$GID_APP,$GID_APP"
lacks "V5.31: desde o exec, Dumpable ≠ 1 (2 com suid_dumpable=2)" "$r" "dumpable=1"
check "V5.31: desde o exec, TracerPid 0" "$r" "tracer=0"
check "D57: o app recebe só 0–2, ligados a /dev/null (fds ≥ 3 fechados)" "$r" "fds=0,1,2,3 std=/dev/null,/dev/null,/dev/null"
r="$(app_line depois "$APID" | head -1)"
check "V5.31: depois da partida, os gids voltam ao real" "$r" "gid=1000,1000,1000,1000"
check "V5.31: depois da partida, Dumpable 0" "$r" "dumpable=0"
check "V5.31: depois da partida, TracerPid 0" "$r" "tracer=0"
# All sockets (the connection, the watcher's copy and those of the transport of D65 a; the
# reader drops its own when the "daemon" closes, so the number varies) have CLOEXEC.
check "V5.31: o socket do daemon tem CLOEXEC" "$r" "socket_cloexec=true/"
check "V3.34: o app lançado tem VmLck 0 kB (o mlockall do daemon não passa pelo execve)" "$r" "vmlck=0,kB"
lacks "V3.34: o daemon de teste travou a memória (mlockall com LimitMEMLOCK infinito)" "$(grep '^servidor: pronto' "$LOG2" || true)" "vmlck=0,kB"
check "V5.31: o ambiente do app é exatamente o de D57 (sem DISPLAY, LISTEN_*, GTK_MODULES, GST_PLUGIN_PATH, GIO_EXTRA_MODULES, WEBKIT_INJECTED_BUNDLE_PATH)" \
    "$r" "env=CUDA_MPS_PIPE_DIRECTORY,DBUS_SESSION_BUS_ADDRESS,GDK_BACKEND,GST_PLUGIN_LOADING_WHITELIST,GST_PLUGIN_SYSTEM_PATH,HOME,LANG,NO_AT_BRIDGE,PATH,WAYLAND_DISPLAY,WEBKIT_A11Y_BUS_ADDRESS,XDG_RUNTIME_DIR,XDG_SESSION_TYPE,__GL_APPLICATION_PROFILE"
r="$(grep "^servidor: pid=$APID " "$LOG2" || true)"
check "V5.31: o app aberto pelo daemon conectou antes de largar o grupo e recebeu o papel app" "$r" "gid=$GID_APP papel=app"
check "D47: com a conexão caída e o pai ainda o daemon, o app se executa de novo" "$(app_line queda "$APID")" "queda pid=$APID"
n="$(grep -c "^servidor: pid=$APID gid=$GID_APP papel=app" "$LOG2" || true)"
check "D47: depois de se executar de novo, o app reconecta com o papel app" "conexoes=$n" "conexoes=2"
# Variables forced into the environment of the app opened by the daemon: the app's checker
# exits before connecting.
n_forced=0
while read -r fpid name; do
    n_forced=$((n_forced + 1))
    r="$(app_line erro "$fpid")"
    check "V5.31: app aberto pelo daemon com $name forçado sai com mensagem" "$r" "variável $name"
    r="$(grep "^servidor: pid=$fpid " "$LOG2" || true)"
    if [ -z "$r" ]; then ok "V5.31: com $name forçado, o app nem conecta"; else failed "o app com $name forçado conectou: $r"; fi
done < <(grep '^forcado: aberto' "$LOG2" | sed -E 's/^forcado: aberto pid=([0-9]+) (.*)$/\1 \2/')
check "V5.31: os cinco ambientes forçados foram tentados" "forcados=$n_forced" "forcados=5"

# Launched by any process (the menu, with the session environment): drops the group
# before connecting, asks the daemon to open the app as a common client and exits.
as_1000 /usr/local/bin/ward-app || true
PPID_MENU="$(last_error_pid)"
r="$(app_line erro "$PPID_MENU")"
check "V5.31: o ward-app aberto por outro processo, com o ambiente dele, pede ao daemon e sai" "$r" "pedido ao daemon"
check "V5.31: o ward-app aberto por outro processo conecta sem o grupo do app" \
    "$(grep "^servidor: pid=$PPID_MENU " "$LOG2" || true)" "papel=agente motivo=CommonGroup"
# Launched by any process with the environment of D57 copied: same path (the parent is
# not the daemon), without the app role.
app_env="$(as_1000 /usr/local/bin/wardd environment)"
# The `bash -c '…; true'` stays as parent (without it, the parent would be the container's pid 1).
# shellcheck disable=SC2086
as_1000 /usr/bin/bash -c "/usr/bin/env -i $(echo $app_env) /usr/local/bin/ward-app; true"
QPID="$(last_error_pid)"
check "V5.31: o ward-app aberto por outro processo sai com mensagem" "$(app_line erro "$QPID")" "pedido ao daemon"
check "V5.31: o ward-app aberto por outro processo não recebe o papel app" \
    "$(grep "^servidor: pid=$QPID " "$LOG2" || true)" "papel=agente motivo=CommonGroup"
# Launched by something that imitates `systemd --user` (like a unit of ~/.config/systemd/user/).
install -d -o 1000 -g 1000 -m 0755 /tmp/gerente
install -o 1000 -g 1000 -m 0755 "$BIN" /tmp/gerente/systemd
rm -f /tmp/gerente.log
as_1000 /usr/bin/env WARD_GERENTE_PROGRAMA=/usr/local/bin/ward-app /usr/bin/bash -c \
    '( exec -a /usr/lib/systemd/systemd /tmp/gerente/systemd --user >/tmp/gerente.log 2>&1 & )'
for _ in $(seq 50); do grep -q "gerente: Ok" /tmp/gerente.log 2>/dev/null && break; sleep 0.1; done
r="$(cat /tmp/gerente.log 2>/dev/null || true)"
check "V5.31: o pai falso imita o gerente (Name, PPid 1 e cmdline)" "$r" "gerente: nome=systemd ppid=1 cmdline=/usr/lib/systemd/systemd --user"
GPID="$(last_error_pid)"
check "V5.31: o ward-app aberto pelo gerente falso sai com mensagem" "$(app_line erro "$GPID")" "pedido ao daemon"
check "V5.31: o ward-app aberto pelo gerente falso não recebe o papel app" \
    "$(grep "^servidor: pid=$GPID " "$LOG2" || true)" "papel=agente motivo=CommonGroup"

# Wrong session: the launcher does not open and answers "approval unavailable" (D21).
as_1000 /usr/local/bin/wardd listen /run/user/1000/wayland-0 >/dev/null &
W0=$!
for _ in $(seq 50); do [ -S /run/user/1000/wayland-0 ] && break; sleep 0.1; done
r="$(as_1000 /usr/local/bin/wardd launch-app /usr/local/bin/ward-app)"
check "V5.31: com dois wayland-<n>, o lançador não abre (aprovação indisponível)" "$r" "Session(SeveralWayland) indisponivel=true"
kill "$W0" 2>/dev/null || true; wait "$W0" 2>/dev/null || true
rm -f /run/user/1000/wayland-0
as_1000 /usr/local/bin/wardd listen /run/user/1000/proxy.sock >/dev/null &
PROXY=$!
for _ in $(seq 50); do [ -S /run/user/1000/proxy.sock ] && break; sleep 0.1; done
mv /run/user/1000/wayland-1 /run/user/1000/wayland-1.real
as_1000 ln -s /run/user/1000/proxy.sock /run/user/1000/wayland-1
r="$(as_1000 /usr/local/bin/wardd launch-app /usr/local/bin/ward-app)"
check "V5.31: com o wayland-<n> trocado por um link, o lançador não abre" "$r" "Session(UntrustedWayland) indisponivel=true"
rm -f /run/user/1000/wayland-1
mv /run/user/1000/wayland-1.real /run/user/1000/wayland-1
kill "$PROXY" 2>/dev/null || true
# Binary that is not the installed one (user's copy, without setgid): the launcher does not open.
as_1000 cp /usr/local/bin/ward-app /run/user/1000/ward-app-29a
r="$(as_1000 /usr/local/bin/wardd launch-app /run/user/1000/ward-app-29a)"
check "D57: o lançador só abre o ward-app do root, setgid do grupo ward-app" "$r" "UntrustedBinary"

kill "$SRV2" "$W1" "$BUS" 2>/dev/null || true
wait "$SRV2" 2>/dev/null || true

# === Phase 28: V4.2, 2nd part (D46, D65 e; S5) ==========================================
# Build WITHOUT `test-mode` (the release one): the `ward` CLI only connects in
# /run/ward/<uid>/ (root's folder) and requires peer uid 0. The real `wardd`, copied to
# /tmp, and a non-dumpable fake (`fake.c`, which would pass the extra check of `test-mode`)
# run as uid 1000 in folders of that uid; neither receives any byte, the CLI exits ≠ 0
# without asking for the password, and uid 1000 does not create a socket in /run/ward/1000/.
CLI=/opt/teste/ward-release
WARDD_REL=/opt/teste/wardd-release
FAKE=/opt/teste/falso
install -o root -g root -m 0755 "$CLI" /usr/local/bin/ward
install -d -o 1000 -g 1000 -m 0700 /run/user/1000/ward /run/user/1000/ward-teste /run/user/1000/copia /tmp/v42
RECORD=/tmp/v42/registro
as_1000 touch "$RECORD"

# The real wardd copied by the user: does not listen outside /run/ward/<uid>/ (the
# LISTEN_FDS socket in a folder of the uid is refused; without LISTEN_FDS, it does not start).
as_1000 cp "$WARDD_REL" /tmp/v42/wardd
as_1000 chmod 0755 /tmp/v42/wardd
# `timeout`: if the copy started, it would stay up (124 counts as a failure).
r="$(as_1000 /usr/bin/timeout 10 /usr/bin/env -i HOME=/tmp/v42 PATH=/usr/bin:/bin "$FAKE" --activate /run/user/1000/copia/wardd.sock /tmp/v42/wardd 2>&1; echo "saiu=$?")"
lacks "V4.2: o wardd copiado não fica escutando numa pasta do uid 1000" "$r" "saiu=0"
lacks "V4.2: o wardd copiado não fica no ar com o socket de uma pasta do uid 1000" "$r" "saiu=124"
echo "   (wardd copiado com LISTEN_FDS de /run/user/1000/copia: $(tr '\n' ' ' <<<"$r"))"
r="$(as_1000 /usr/bin/timeout 10 /usr/bin/env -i HOME=/tmp/v42 PATH=/usr/bin:/bin XDG_RUNTIME_DIR=/run/user/1000 /tmp/v42/wardd 2>&1; echo "saiu=$?")"
lacks "V4.2: o wardd copiado sem o systemd não sobe" "$r" "saiu=0"
lacks "V4.2: o wardd copiado sem o systemd não fica no ar" "$r" "saiu=124"
echo "   (wardd copiado sem LISTEN_FDS: $(tr '\n' ' ' <<<"$r"))"

# The non-dumpable fakes of uid 1000: at the old path (/run/user/<uid>/ward/), at the
# test daemon's folder and at the copy's.
FPIDS=()
for d in ward ward-teste copia; do
    as_1000 "$FAKE" "/run/user/1000/$d/wardd.sock" "$RECORD" 40 >"/tmp/v42/falso-$d.out" 2>&1 &
    FPIDS+=("$!")
done
for d in ward ward-teste copia; do
    for _ in $(seq 50); do grep -q pronto "/tmp/v42/falso-$d.out" 2>/dev/null && break; sleep 0.1; done
    check "V4.2: falso não-dumpable do uid 1000 escutando em /run/user/1000/$d" "$(cat "/tmp/v42/falso-$d.out")" "pronto dumpable=0"
done
# Control: the record catches a real connection (then it is emptied).
as_1000 "$FAKE" --connect /run/user/1000/ward/wardd.sock x
for _ in $(seq 50); do grep -q "bytes 1" "$RECORD" && break; sleep 0.1; done
check "controle: o falso registra conexão e bytes" "$(tr '\n' ' ' <"$RECORD")" "conexao 0 bytes 1"
: >"$RECORD"
r="$(as_1000 "$FAKE" /run/ward/1000/falso-v42.sock "$RECORD" 1 2>&1; echo "saiu=$?")"
check "V4.2: o falso do uid 1000 não faz bind em /run/ward/1000/ (EACCES)" "$r" "bind=EACCES"
# With root's old socket at the wardd path: uid 1000 neither deletes nor takes it.
r="$(as_1000 "$FAKE" "$SOCK" "$RECORD" 1 2>&1; echo "saiu=$?")"
check "V4.2: com o socket velho do root, o falso não toma o caminho do wardd" "$r" "saiu=3"
[ -S "$SOCK" ] && [ "$(stat -c %U /run/ward/1000)" = root ] && ok "V4.2: o socket velho continua lá, na pasta do root" || failed "o socket velho sumiu ou a pasta mudou de dono"

# The release CLI in a pty (`script`), with the (fake) password typed anyway and the
# environment pointing to the fakes: exits ≠ 0 without showing the prompt.
PASSWORD_V42="falso-mestra-ward-01"
cli_pty() {
    printf '%s\n' "$PASSWORD_V42" | as_1000 /usr/bin/env -i HOME=/tmp/v42 PATH=/usr/bin:/bin LANG=C.UTF-8 \
        XDG_RUNTIME_DIR=/run/user/1000 WARD_DEV_SOCKET_DIR=/run/user/1000/ward-teste \
        /usr/bin/script -qec "/usr/local/bin/ward $1" /dev/null 2>&1
    echo "saiu=$?"
}
verify_cli() {
    local label="$1" r="$2"
    lacks "V4.2 ($label): a CLI de release não sai 0" "$r" "saiu=0"
    if [[ "$r" == *"o daemon não está rodando"* || "$r" == *"servidor não é o wardd"* ]]; then
        ok "V4.2 ($label): a CLI de release recusa ('o daemon não está rodando' ou 'servidor não é o wardd')"
    else
        failed "V4.2 ($label): mensagem inesperada: $r"
    fi
    lacks "V4.2 ($label): a CLI não pede a senha" "$r" "Senha mestra"
}
# With root's old socket in /run/ward/1000 (nobody listening) and without it.
for scenario in socket-velho sem-socket; do
    if [ "$scenario" = sem-socket ]; then
        rm -f "$SOCK"
        r="$(as_1000 "$FAKE" "$SOCK" "$RECORD" 1 2>&1; echo "saiu=$?")"
        check "V4.2: sem o socket, o falso do uid 1000 não faz bind em $SOCK (EACCES)" "$r" "bind=EACCES"
        r="$(as_1000 /usr/local/bin/ward-servidor bind "$SOCK")"
        check "V4.2: nem o binário de teste (bind em $SOCK)" "$r" "bind=EACCES"
    fi
    for cmd in destravar lista travar; do
        verify_cli "$scenario, ward $cmd" "$(cli_pty "$cmd")"
    done
done
# Root deletes the folder /run/ward/1000: uid 1000 cannot create it either.
rmdir /run/ward/1000
r="$(as_1000 mkdir /run/ward/1000 2>&1; echo "saiu=$?")"
lacks "V4.2: o uid 1000 não cria /run/ward/1000" "$r" "saiu=0"
verify_cli "sem a pasta, ward destravar" "$(cli_pty destravar)"
install -d -o root -g root -m 0755 /run/ward/1000

kill "${FPIDS[@]}" 2>/dev/null || true
wait "${FPIDS[@]}" 2>/dev/null || true
if [ -s "$RECORD" ]; then
    failed "V4.2: um falso recebeu conexão ou bytes: $(tr '\n' ' ' <"$RECORD")"
else
    ok "V4.2: nenhum dos falsos recebeu conexão nem byte"
fi

# === Phase 40: ward-nm with the real test wardd (D5, D9, D47; V6.1, V6.2) =============
# The `ward-nm` is the release one (without `test-mode`): it only talks to
# /run/ward/1000/wardd.sock in root's folder and peer uid 0. The daemon is the test `wardd`
# (test vault, a copy) with the "systemd" socket through LISTEN_FDS. The "Chrome" is the
# `fake-chrome` installed at /opt/google/chrome/chrome (root, 0755): it opens the host as a direct child, like Chrome.
LOG40=/tmp/wardd-40.log
DEV40=/tmp/ward-dev-40
install -d -o 1000 -g 1000 -m 0700 "$DEV40"
install -o 1000 -g 1000 -m 0600 /opt/teste/test-vault.ward "$DEV40/test-vault.ward"
printf 'ward-state 2\nseen 0\nburned 0\nagent_failures 0\n' > "$DEV40/test-vault.state"
chown 1000:1000 "$DEV40/test-vault.state"
chmod 0600 "$DEV40/test-vault.state"
install -o root -g root -m 0755 /opt/teste/wardd-real /usr/local/bin/wardd-real
install -o root -g ward-nm -m 2755 /opt/teste/ward-nm-real /usr/local/bin/ward-nm
install -d -o root -g root -m 0755 /opt/google /opt/google/chrome
install -o root -g root -m 0755 /opt/teste/fake-chrome /opt/google/chrome/chrome
/usr/local/bin/ward-servidor activate "$SOCK" 1000 -- /usr/bin/env HOME="$DEV40" \
    XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 WARD_DEV_DIR="$DEV40" RUST_LOG=trace \
    /usr/local/bin/wardd-real >"$LOG40" 2>&1 &
SRV40=$!
for _ in $(seq 100); do grep -q "pronto" "$LOG40" 2>/dev/null && break; sleep 0.1; done
check "Fase 40: o wardd de teste real subiu com o socket do systemd" "$(grep -c pronto "$LOG40" || true)" "1"
r="$(as_1000 /opt/teste/wardd-test-client "$SOCK" destravar < /opt/teste/senha)"
check "Fase 40: destravou a cópia do cofre de teste" "$r" "resposta: ok"

M=/tmp/msgs-40.jsonl
cat > "$M" <<'FIM'
{"v":2,"op":"status"}
{"v":2,"op":"suggest","data":{"origin":"http://localhost:8081/wp-login.php"}}
{"v":2,"op":"fill","data":{"entry":"acme-wp-dev","origin":"http://localhost:8081/wp-login.php"}}
{"v":2,"op":"fill","data":{"entry":"acme-wp-dev","origin":"http://127.0.0.1:8081/wp-login.php"}}
{"v":2,"op":"suggest","data":{"origin":"http://127.0.0.1:8081/wp-login.php"}}
{"v":2,"op":"reveal","data":{"entry":"acme-wp-dev","field":"password"}}
{"v":2,"op":"list"}
FIM
chmod 0644 "$M"
# Line $1 of the output in $r.
l() { sed -n "${1}p" <<<"$r"; }
r="$(as_1000 /opt/google/chrome/chrome /usr/local/bin/ward-nm "$M")"
check "D47: ward-nm setgid aberto pelo Chrome instalado recebe o papel extensão" "$(l 1)" '"role":"extension"'
check "D9: suggest em localhost:8081 traz acme-wp-dev" "$(l 2)" '"name":"acme-wp-dev"'
check "D9: suggest em localhost:8081 traz acme-wp-prod" "$(l 2)" '"name":"acme-wp-prod"'
# Phase 54 (44c/44e): http (even on localhost) asks for the origin confirmation in the
# approval window; here nobody confirms (the host ends stdin and exits): the request is
# denied and nothing comes out. The daemon log must show the requested confirmation (checked at the end).
# Without app or session in the container, the window does not open: the daemon denies or
# answers "approval unavailable"; both refuse the fill (checked right below).
case "$(l 3)" in
    *'"code":"denied"'* | *'"code":"approval_unavailable"'*) ok "D9/44c: fill acme-wp-dev em http://localhost:8081 sem confirmar: recusado" ;;
    *) failed "D9/44c: fill acme-wp-dev em http://localhost:8081 sem confirmar: recusado (veio '$(l 3)')" ;;
esac
lacks "D9/44c: sem a confirmação, nada de preenchimento" "$(l 3)" '"type":"fill"'
lacks "D9/44c: sem a confirmação, nada de senha" "$(l 3)" '"password"'
check "V6.2: fill acme-wp-dev em 127.0.0.1:8081 é recusado pelo daemon" "$(l 4)" '"code":"origin_mismatch"'
lacks "V6.2: a recusa de origem não leva valor" "$(l 4)" '"password"'
check "V6.2: suggest em 127.0.0.1:8081 não traz acme-wp-*" "$(l 5)" '"items":[]'
check "papel extensão: reveal (operação do app) recusado" "$(l 6)" '"code":"unauthorized"'
check "papel extensão: list (operação do agente e do app) recusada" "$(l 7)" '"code":"unauthorized"'
check "o host sai 0 no fim do stdin" "$(l 8)" "saida=0"

# V6.1: the same ward-nm executed by a user process (the parent is not Chrome).
printf '{"v":2,"op":"fill","data":{"entry":"acme-wp-dev","origin":"http://localhost:8081/wp-login.php"}}' > /tmp/v61.json
printf "$(printf '\\x%02x\\x00\\x00\\x00' "$(stat -c %s /tmp/v61.json)")" > /tmp/v61.bin
cat /tmp/v61.json >> /tmp/v61.bin
chmod 0644 /tmp/v61.bin
r="$(as_1000 /usr/bin/bash -c '/usr/local/bin/ward-nm chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/ < /tmp/v61.bin | tr -d "\000-\037"; true')"
check "V6.1: ward-nm executado direto: o daemon recusa (unauthorized)" "$r" "unauthorized"
lacks "V6.1: ward-nm executado direto: nada de senha no stdout" "$r" "password"
# A copy of "Chrome" outside the installed path, the user's: agent.
as_1000 cp /opt/google/chrome/chrome /run/user/1000/chrome
r="$(as_1000 /run/user/1000/chrome /usr/local/bin/ward-nm "$M")"
check "V6.1: pai que é cópia do Chrome fora de /opt/google/chrome: status unauthorized" "$(l 1)" '"code":"unauthorized"'
check "V6.1: pai que é cópia do Chrome: fill recusado" "$(l 3)" '"code":"unauthorized"'
lacks "V6.1: pai que é cópia do Chrome: nada de preenchimento" "$r" '"type":"fill"'
# The installed Chrome with LD_PRELOAD in the environment (Chrome is not setgid): agent.
r="$(as_1000 /usr/bin/env LD_PRELOAD="$LIB" /opt/google/chrome/chrome /usr/local/bin/ward-nm "$M")"
check "D5: Chrome com LD_PRELOAD de pai: fill recusado" "$(l 3)" '"code":"unauthorized"'
lacks "D5: Chrome com LD_PRELOAD de pai: nada de preenchimento" "$r" '"type":"fill"'
# A copy of ward-nm (without setgid) with the installed Chrome as parent: agent.
as_1000 cp /usr/local/bin/ward-nm /run/user/1000/ward-nm-copia
r="$(as_1000 /opt/google/chrome/chrome /run/user/1000/ward-nm-copia "$M")"
check "V3.5/D47: cópia do ward-nm sem setgid, com o Chrome de pai: fill recusado" "$(l 3)" '"code":"unauthorized"'
lacks "V3.5/D47: cópia do ward-nm: nada de preenchimento" "$r" '"type":"fill"'
# Message above the limit through Chrome.
printf '{"v":2,"op":"suggest","data":{"origin":"http://localhost:8081/%s"}}\n' "$(head -c 70000 /dev/zero | tr '\0' a)" > /tmp/grande-40.jsonl
chmod 0644 /tmp/grande-40.jsonl
r="$(as_1000 /opt/google/chrome/chrome /usr/local/bin/ward-nm /tmp/grande-40.jsonl)"
check "mensagem de mais de 64 KiB: invalid_frame" "$r" '"code":"invalid_frame"'
check "mensagem de mais de 64 KiB: o host sai ≠ 0" "$r" "saida=1"

kill "$SRV40" 2>/dev/null || true
wait "$SRV40" 2>/dev/null || true
n="$(grep -c 'falso-' "$LOG40" || true)"
check "S13: o log do wardd (RUST_LOG=trace) não tem valor falso do cofre" "falso=$n" "falso=0"
check "D9: o daemon registrou a recusa de origem (só o nome da entrada)" "$(grep 'origem diferente' "$LOG40" || true)" "preencher acme-wp-dev recusado: origem diferente"
check "D9/44c: o daemon pediu a confirmação da origem local" "$(grep 'confirmação na janela' "$LOG40" || true)" "preencher acme-wp-dev: origem local, confirmação na janela"
lacks "S13: o log não tem a origem pedida" "$(cat "$LOG40")" "127.0.0.1:8081"

echo
if [ "$failures" -ne 0 ]; then
    echo "test-docker-platform: $failures conferência(s) falharam"
    echo "--- log do servidor ---"; cat "$LOG"
    echo "--- log do servidor (29a) ---"; cat "$LOG2" 2>/dev/null || true
    echo "--- relatório do app (29a) ---"; cat "$REPORT" 2>/dev/null || true
    echo "--- log do wardd (40) ---"; grep -v 'falso-' "$LOG40" 2>/dev/null || true
    exit 1
fi
echo "test-docker-platform: tudo certo"
