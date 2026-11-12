#!/bin/bash -p
# Phase 63 (m2): inside part of manual-run.sh. Root only inside the disposable container.
# V2.6 (release wardd refuses LD_PRELOAD/LD_AUDIT/LD_LIBRARY_PATH, not dumpable, gdb and
# strace of the same uid fail, SIGSEGV leaves no core), V2.7 (ward lock kills the
# `ward run` and its `setsid` descendant, exit 137 and the message, root's dump of the
# daemon after the lock without patterns; positive control on the child's memory) and
# V4.4 (the 0.1.4 vault migrating on unlock under inotify and strace: every file created
# has no pattern, no type name in clear, vault files start with WARDCOFR).
# Prints only counts and names, never a pattern.
export PATH=/usr/sbin:/usr/bin:/bin
set -uo pipefail

failures=0
ok() { echo "ok: $*"; }
failed() { echo "FALHOU: $*"; failures=$((failures + 1)); }
check() { if [[ "$2" == *"$3"* ]]; then ok "$1"; else failed "$1 (esperado '$3', veio '$2')"; fi; }
lacks() { if [[ "$2" == *"$3"* ]]; then failed "$1 (não devia ter '$3': $2)"; else ok "$1"; fi; }
T=/opt/teste
V="$T/variations.txt"
UID_T=1000
SOCK="/run/ward/$UID_T/wardd.sock"
PASSWORD="$(cat "$T/senha")" # D15 test master password (fake), from dev/fixtures/entries.json

for _ in $(seq 1 100); do
    s="$(systemctl is-system-running 2>/dev/null || true)"
    [[ "$s" == running || "$s" == degraded ]] && break
    sleep 0.1
done
check "systemd de pé" "$s" "r"
echo "info: ptrace_scope=$(cat /proc/sys/kernel/yama/ptrace_scope) suid_dumpable=$(cat /proc/sys/fs/suid_dumpable)"
echo "info: core_pattern=$(head -c 120 /proc/sys/kernel/core_pattern)"

HOME_T="$(getent passwd "$UID_T" | cut -d: -f6)"
VAULT_T="$HOME_T/.local/share/w""ard"
install -m 0755 -o root -g root "$T/wardd" /usr/local/bin/wardd
install -m 0755 -o root -g root "$T/ward" /usr/local/bin/ward
install -m 0644 "$T/wardd@.socket" "$T/wardd@.service" /etc/systemd/system/
install -d -m 0700 -o "$UID_T" -g "$UID_T" "/run/user/$UID_T"
for d in "$HOME_T/.local" "$HOME_T/.local/share" "$VAULT_T"; do install -d -m 0700 -o "$UID_T" -g "$UID_T" "$d"; done
install -m 0600 -o "$UID_T" -g "$UID_T" "$T/cofre.ward" "$VAULT_T/cofre.ward"
printf 'ward-state 2\nseen 0\nburned 0\nagent_failures 0\n' >"$VAULT_T/estado"
chown "$UID_T:$UID_T" "$VAULT_T/estado"; chmod 0600 "$VAULT_T/estado"
systemctl daemon-reload

as_user() {
    setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups \
        env -i PATH=/usr/local/bin:/usr/bin:/bin HOME="$HOME_T" LANG=C.UTF-8 XDG_RUNTIME_DIR="/run/user/$UID_T" "$@"
}
client() { as_user "$T/cliente" "$SOCK" "$@"; }
main_pid() { systemctl show -p MainPID --value "wardd@$UID_T.service"; }
count_hits() { grep -a -F -o -f "$V" -- "$@" 2>/dev/null | wc -l; }

# --- V2.6: LD_PRELOAD, LD_AUDIT, LD_LIBRARY_PATH at startup ------------------------------
DROP="/etc/systemd/system/wardd@$UID_T.service.d"
for var in LD_PRELOAD=/tmp/x.so LD_AUDIT=/tmp/x.so LD_LIBRARY_PATH=/tmp/x; do
    name="${var%%=*}"
    install -d -m 0755 "$DROP"
    printf '[Service]\nEnvironment=%s\nRestart=no\n' "$var" >"$DROP/fase63.conf"
    systemctl daemon-reload
    systemctl start "wardd@$UID_T.socket"
    since="$(date '+%Y-%m-%d %H:%M:%S')"
    r="$(timeout 15 setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups "$T/cliente" "$SOCK" status 2>&1 || true)"
    sleep 1
    lacks "V2.6 $name: o daemon não responde" "$r" "resposta:"
    st="$(systemctl is-active "wardd@$UID_T.service" || true)"
    lacks "V2.6 $name: o serviço não fica ativo ($st)" "$st" "active"
    j="$(journalctl -u "wardd@$UID_T.service" --no-pager -q --since "$since" 2>/dev/null | grep 'wardd\[' | tail -n 2)"
    echo "info: $name journal: $(sed 's/^.*wardd\[[0-9]*\]: //' <<<"$j" | tr '\n' '|')"
    check "V2.6 $name: o journal registra a recusa" "$j" "$name"
    systemctl stop "wardd@$UID_T.service" "wardd@$UID_T.socket" 2>/dev/null || true
    systemctl reset-failed "wardd@$UID_T.service" "wardd@$UID_T.socket" 2>/dev/null || true
    rm -rf -- "$DROP"
done
systemctl daemon-reload

# --- Normal start: not dumpable, ptrace of the same uid fails -------------------------
systemctl start "wardd@$UID_T.socket"
check "o daemon sobe na primeira conexão" "$(client status)" "resposta: status"
pid="$(main_pid)"
check "V2.6 /proc/<pid>/environ do wardd é do root (não-dumpable)" "$(stat -c '%U' "/proc/$pid/environ")" "root"
as_user sleep 30 &
ctl=$!
sleep 0.3
ctl_pid="$(pgrep -u "$UID_T" -x sleep | head -n 1)"
check "controle: /proc/<pid>/environ de um processo comum do uid é dele" "$(stat -c '%u' "/proc/$ctl_pid/environ")" "$UID_T"
kill "$ctl" 2>/dev/null; kill "$ctl_pid" 2>/dev/null || true
r="$(as_user cat "/proc/$pid/environ" 2>&1 | head -c 200 || true)"
check "V2.6 o uid não lê o environ do wardd" "$r" "Permission denied"
r="$(as_user timeout 20 strace -p "$pid" -e trace=none 2>&1 | head -n 3 || true)"
check "V2.6 strace -p do mesmo uid falha" "$r" "Operation not permitted"
r="$(as_user timeout 30 gdb -p "$pid" -batch -nx -ex 'info proc' 2>&1 | grep -i -E 'ptrace|permitted|attach' | head -n 3 || true)"
check "V2.6 gdb -p do mesmo uid falha" "$r" "Could not attach"
echo "info: gdb: $(tr '\n' '|' <<<"$r")"

# --- V2.7: ward run in progress with a setsid descendant; ward lock --------------------
check "destrava o cofre de teste" "$(printf '%s' "$PASSWORD" | client unlock)" "resposta: ok"
rm -f /tmp/f63-desc /tmp/f63-child
as_user /usr/local/bin/ward run acme-pg-dev -- /bin/bash -c 'setsid sleep 300 & echo $! >/tmp/f63-desc; echo $$ >/tmp/f63-child; sleep 300' \
    >/tmp/f63-run.out 2>&1 &
run_pid=$!
for _ in $(seq 50); do [ -s /tmp/f63-desc ] && [ -s /tmp/f63-child ] && break; sleep 0.1; done
desc="$(cat /tmp/f63-desc 2>/dev/null || true)"
child="$(cat /tmp/f63-child 2>/dev/null || true)"
[ -n "$desc" ] && [ -d "/proc/$desc" ] && ok "V2.7 descendente com setsid no ar ($desc)" || failed "V2.7 descendente não subiu"
check "V2.7 o descendente está em outra sessão" "$(ps -o sid= -p "$desc" | tr -d ' ')" "$desc"
# Positive control of the dump: the child of `ward run` has the password in its environment.
r="$(python3 -I "$T/dump.py" "$child" "$V")"
echo "info: controle (filho do ward run): $r"
lacks "controle: a varredura acha o segredo no filho do ward run" "$r" "padrões_achados=0"
r="$(python3 -I "$T/dump.py" "$pid" "$V")"
echo "info: wardd destravado, com o ward run no ar: $r"
r="$(as_user /usr/local/bin/ward lock 2>&1)"
echo "info: ward lock: $r"
wait "$run_pid"
run_code=$?
check "V2.7 a CLI do ward run sai 137" "$run_code" "137"
check "V2.7 mensagem do ward run" "$(cat /tmp/f63-run.out)" "o cofre travou; o comando foi encerrado"
sleep 0.5
[ ! -d "/proc/$child" ] && ok "V2.7 o filho do ward run morreu" || failed "V2.7 o filho do ward run ficou"
[ ! -d "/proc/$desc" ] && ok "V2.7 o descendente com setsid morreu" || failed "V2.7 o descendente com setsid ficou"
check "V2.7 o daemon continua no ar (mesmo pid)" "$(main_pid)" "$pid"
check "V2.7 o cofre está travado" "$(client status)" "resposta: status"
echo "info: status depois da trava: $(client status | head -c 200)"
sleep 1
r="$(python3 -I "$T/dump.py" "$pid" "$V")"
echo "info: dump do wardd depois da trava (root): $r"
check "V2.7 dump da memória do wardd depois da trava sem padrão" "$r" "padrões_achados=0"

# --- V2.6: SIGSEGV leaves no core ------------------------------------------------------
touch /tmp/f63-mark
sleep 1
cwd="$(readlink "/proc/$pid/cwd")"
echo "info: cwd do wardd: $cwd; core limit: $(awk '/^Max core file size/{print $5, $6}' "/proc/$pid/limits")"
kill -SEGV "$pid"
sleep 3
cores="$(find / -xdev \( -path /proc -o -path /sys \) -prune -o \( -name 'core' -o -name 'core.*' -o -name '*.crash' \) -newer /tmp/f63-mark -print 2>/dev/null || true)"
check "V2.6 kill -SEGV: nenhum core no contêiner" "[$cores]" "[]"
check "V2.6 kill -SEGV: nenhum core na pasta do daemon" "[$(find "$cwd" -maxdepth 1 -name 'core*' -newer /tmp/f63-mark 2>/dev/null)]" "[]"
echo "info: journal depois do SEGV: $(journalctl -u "wardd@$UID_T.service" --no-pager -q -n 3 | sed 's/^.*\]: //' | tr '\n' '|')"
systemctl stop "wardd@$UID_T.service" 2>/dev/null || true
systemctl reset-failed "wardd@$UID_T.service" 2>/dev/null || true

# --- V4.4: the 0.1.4 vault migrating on unlock, under inotify and strace ---------------
rm -rf -- "$VAULT_T"
install -d -m 0700 -o "$UID_T" -g "$UID_T" "$VAULT_T"
install -m 0600 -o "$UID_T" -g "$UID_T" "$T/vault-0.1.4.ward" "$VAULT_T/cofre.ward"
install -m 0600 -o "$UID_T" -g "$UID_T" "$T/state-0.1.4" "$VAULT_T/estado"
check "V4.4: o cofre 0.1.4 começa v4 (WARDCOFR)" "$(head -c 8 "$VAULT_T/cofre.ward")" "WARDCOFR"
check "o wardd sobe travado com o cofre 0.1.4" "$(client status)" "resposta: status"
pid="$(main_pid)"
CAP=/tmp/f63-cap
install -d -m 0700 "$CAP"
(
    n=0
    inotifywait -m -r -q -e create,close_write,moved_to,modify --format '%e|%w%f' "$VAULT_T" | while IFS='|' read -r ev f; do
        n=$((n + 1))
        echo "$ev $f" >>/tmp/f63-inotify.log
        [ -f "$f" ] && cp -- "$f" "$CAP/$(printf '%04d' "$n")-${f##*/}" 2>/dev/null || true
    done
) &
inw=$!
strace -f -p "$pid" -e trace=openat,write,rename,renameat,renameat2,unlinkat,fsync -s 256 -o /tmp/f63-strace.log 2>/dev/null &
str=$!
sleep 1.5
check "V4.4: destrave do cofre 0.1.4 (migra)" "$(printf '%s' "$PASSWORD" | client unlock)" "resposta: ok"
sleep 2
kill "$str" 2>/dev/null || true
pkill -f "inotifywait -m -r -q" 2>/dev/null || true
kill "$inw" 2>/dev/null || true
wait 2>/dev/null || true
echo "info: eventos inotify: $(wc -l </tmp/f63-inotify.log)"
sed "s#$VAULT_T#<cofre>#" /tmp/f63-inotify.log | sort -u -k2 | head -n 40 | sed 's/^/info:   /'
echo "info: arquivos capturados: $(ls "$CAP" | wc -l)"
check "V4.4: o cofre virou v5 (WARDCOFR)" "$(head -c 8 "$VAULT_T/cofre.ward")" "WARDCOFR"
check "V4.4: estado novo" "$(head -n 1 "$VAULT_T/estado")" "ward-state 2"
TYPES='database ftp_sftp secure_note banco servidor nota_segura "login" "server" "smtp" "generic" generico'
all_files=("$CAP"/* )
while IFS= read -r -d '' f; do all_files+=("$f"); done < <(find "$VAULT_T" -type f -print0)
for f in "${all_files[@]}"; do
    [ -f "$f" ] || continue
    label="${f#$CAP/}"; label="${label#$VAULT_T/}"
    h="$(count_hits "$f")"
    [ "$h" = 0 ] && ok "V4.4: $label sem padrão" || failed "V4.4: $label com $h ocorrência(s) de padrão"
    found=""
    for t in $TYPES; do grep -a -q -F -- "$t" "$f" && found="$found $t"; done
    case "$label" in
        *estado* | *state*)
            check "V4.4: $label é o estado" "$(head -n 1 "$f")" "ward-state" ;;
        *)
            [ -z "$found" ] && ok "V4.4: $label sem nome de tipo em claro" || failed "V4.4: $label com nome de tipo em claro:$found"
            if [ -s "$f" ]; then
                check "V4.4: $label começa com WARDCOFR" "$(head -c 8 "$f")" "WARDCOFR"
            fi ;;
    esac
done
h="$(count_hits /tmp/f63-strace.log)"
check "V4.4: o strace do wardd (openat/write/rename) sem padrão" "$h" "0"
echo "info: linhas do strace: $(wc -l </tmp/f63-strace.log); renames: $(grep -c -E 'rename' /tmp/f63-strace.log)"
grep -E 'rename|openat\(.*(O_CREAT|O_TMPFILE)' /tmp/f63-strace.log | sed -e "s#$VAULT_T#<cofre>#g" -e 's/^[0-9]* *//' | cut -c1-160 | head -n 20 | sed 's/^/info:   /'
rm -rf "$CAP" /tmp/f63-strace.log /tmp/f63-inotify.log

echo
if [ "$failures" -ne 0 ]; then echo "fase63 manual: $failures falha(s)"; exit 1; fi
echo "fase63 manual: tudo ok"
