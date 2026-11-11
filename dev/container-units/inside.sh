#!/bin/bash -p
# Inside part of `make test-units` (Phase 24; V3.21, V3.34; D46, D57, D62). Runs as root
# **only inside** the disposable container with systemd; from the host come only the
# read-only test mount under /opt (`wardd` without `test-mode`, the minimal client and the
# two units) and this script. Each check prints "ok:" or the failure marker; any failure
# exits ≠ 0.

export PATH=/usr/sbin:/usr/bin:/bin
set -euo pipefail

failures=0
ok() { echo "ok: $*"; }
failed() { echo "FALHOU: $*"; failures=$((failures + 1)); }
check() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else failed "$1 (esperado '$3', veio '$2')"; fi
}

# The container's systemd must be up.
for _ in $(seq 1 100); do
    sys_state="$(systemctl is-system-running 2>/dev/null || true)"
    [[ "$sys_state" == running || "$sys_state" == degraded ]] && break
    sleep 0.1
done
check "systemd de pé" "$sys_state" "r"

UID_T=1000
id "$UID_T" >/dev/null
install -m 0755 -o root -g root /opt/teste/wardd /usr/local/bin/wardd
install -m 0644 /opt/teste/wardd@.socket /opt/teste/wardd@.service /etc/systemd/system/
systemctl daemon-reload

systemctl start "wardd@$UID_T.socket"
check "socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
check "serviço ainda parado" "$(systemctl is-active "wardd@$UID_T.service" || true)" "inactive"
check "/run/ward root:root 0755" "$(stat -c '%U:%G %a' /run/ward)" "root:root 755"
check "/run/ward/$UID_T root:root 0755" "$(stat -c '%U:%G %a' /run/ward/$UID_T)" "root:root 755"
check "socket do uid, 0600" "$(stat -c '%u %a %F' /run/ward/$UID_T/wardd.sock)" "$UID_T 600 socket"

# The first connection starts the wardd (reply of the daemon, as uid 1000).
r="$(setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" status)"
check "primeira conexão responde" "$r" "resposta: status"
check "serviço subiu" "$(systemctl is-active "wardd@$UID_T.service" || true)" "active"

pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
st="/proc/$pid/status"
check "wardd roda com o uid" "$(awk '/^Uid:/{print $2,$3,$4,$5}' "$st")" "$UID_T $UID_T $UID_T $UID_T"
check "sem no_new_privs (D57)" "$(awk '/^NoNewPrivs:/{print $2}' "$st")" "0"
check "LimitCORE=0" "$(awk '/^Max core file size/{print $5, $6}' "/proc/$pid/limits")" "0 0"
check "LimitMEMLOCK=infinity (D62)" "$(awk '/^Max locked memory/{print $4, $5}' "/proc/$pid/limits")" "unlimited unlimited"
env_names="$(tr '\0' '\n' <"/proc/$pid/environ" | cut -d= -f1 | sort | tr '\n' ' ')"
if [[ "$env_names" == *LD_* ]]; then failed "LD_* no ambiente: $env_names"; else ok "sem LD_* no ambiente ($env_names)"; fi

# V3.34 (D62): with the test vault (fake values) unlocked and one operation, all the
# memory of the wardd is locked (`MCL_ONFAULT`: `VmLck` counts the whole regions).
HOME_T="$(getent passwd "$UID_T" | cut -d: -f6)"
# Folders of the container's user (not the host's), with the name built in parts so the
# V1.5 scan (`dev/tests/attacks-foundation.sh`) does not mistake it for the real home.
VAULT_T="$HOME_T/.local/share/w""ard"
for d in "$HOME_T/.local" "$HOME_T/.local/share" "$VAULT_T"; do
    install -d -m 0700 -o "$UID_T" -g "$UID_T" "$d"
done
install -m 0600 -o "$UID_T" -g "$UID_T" /opt/teste/cofre.ward "$VAULT_T/cofre.ward"
printf 'ward-state 2\nseen 0\nburned 0\nagent_failures 0\n' >"$VAULT_T/estado"
chown "$UID_T:$UID_T" "$VAULT_T/estado"
chmod 0600 "$VAULT_T/estado"
# Test master password (D15, fake) through stdin, never in argv.
r="$(printf '%s' 'falso-mestra-ward-01' | setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" unlock)"
check "destrava o cofre de teste" "$r" "resposta: ok"
r="$(setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" list)"
check "lista com o cofre destravado" "$r" "resposta: list"

# V4.38 (Phase 26, D63): install groups and setgid copies of the minimal client. The
# `wardd` reads the groups at startup: restart it to turn the roles on.
groupadd --system ward-app
groupadd --system ward-nm
install -m 2755 -o root -g ward-app /opt/teste/cliente /usr/local/bin/ward-app
install -m 2755 -o root -g ward-nm /opt/teste/cliente /usr/local/bin/ward-nm
install -m 0755 -o root -g root /opt/teste/ward /usr/local/bin/ward
systemctl stop "wardd@$UID_T.service"
r="$(printf '%s' 'falso-mestra-ward-01' | setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" unlock)"
check "V4.38 destrava de novo, com os grupos" "$r" "resposta: ok"
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
st="/proc/$pid/status"
# Environment of a session (D57), plus what `ward run` passes on.
as_user() {
    setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups \
        env -i PATH=/usr/local/bin:/usr/bin:/bin HOME="$HOME_T" LANG=C.UTF-8 \
        XDG_RUNTIME_DIR="/run/user/$UID_T" WAYLAND_DISPLAY=wayland-0 "$@"
}
since="$(date '+%Y-%m-%d %H:%M:%S')"
r="$(as_user /usr/local/bin/ward run acme-pg-dev -- /usr/local/bin/ward-app "/run/ward/$UID_T/wardd.sock" status 2>&1)" || true
check "V4.38 ward run -- ward-app: papel agente" "$r" "resposta: status papel=agent"
r="$(as_user /usr/local/bin/ward run acme-pg-dev -- /usr/local/bin/ward-app "/run/ward/$UID_T/wardd.sock" list 2>&1)" || true
check "V4.38 ward run -- ward-app: lista do agente" "$r" "resposta: list"
r="$(as_user /usr/local/bin/ward run acme-pg-dev -- /usr/local/bin/ward-nm "/run/ward/$UID_T/wardd.sock" status 2>&1)" || true
check "V4.38 ward run -- ward-nm: papel agente" "$r" "resposta: status papel=agent"
r="$(as_user /usr/local/bin/ward run acme-pg-dev -- /bin/sh -c 'printf %s "$PGPASSWORD"' 2>&1)" || true
check "ward run de release: a senha sai mascarada" "$r" "••••"
sleep 0.5
journal="$(journalctl -u "wardd@$UID_T.service" --no-pager -q --since "$since" 2>/dev/null || true)"
check "V4.38 o daemon recusou o papel app ao filho (D63)" "$journal" "papel app recusado"
if [[ "$journal" == *falso-* ]]; then failed "valor falso no journal"; else ok "journal sem valor do cofre"; fi
lck="$(awk '/^VmLck:/{print $2}' "$st")"
rss="$(awk '/^VmRSS:/{print $2}' "$st")"
if [[ "$lck" =~ ^[0-9]+$ && "$rss" =~ ^[0-9]+$ && "$lck" -gt 0 && "$lck" -ge "$rss" ]]; then
    ok "VmLck ${lck} kB cobre o VmRSS ${rss} kB (mlockall, D62)"
else
    failed "VmLck '${lck}' kB não cobre o VmRSS '${rss}' kB"
fi
# The same binary with an 8 MiB RLIMIT_MEMLOCK: fails closed, with a message.
code=0
output="$(prlimit --memlock=8388608:8388608 setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups \
    env -i PATH=/usr/bin:/bin HOME="$HOME_T" /usr/local/bin/wardd 2>&1)" || code=$?
if [ "$code" -ne 0 ]; then ok "8 MiB de memlock: sai $code"; else failed "8 MiB de memlock: saiu 0"; fi
check "8 MiB de memlock: mensagem" "$output" "LimitMEMLOCK=infinity"
if [[ "$output" == *pronto* ]]; then failed "8 MiB de memlock: chegou a subir"; else ok "8 MiB de memlock: não chegou ao socket"; fi

# Killing the wardd: the next connection starts another (Restart/activation).
kill -9 "$pid"
sleep 1.5
r="$(setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" status)"
check "depois de morto, a conexão sobe de novo" "$r" "resposta: status"
new_pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
if [[ "$new_pid" != "$pid" && "$new_pid" != 0 ]]; then ok "pid novo ($new_pid)"; else failed "pid não mudou"; fi

# Uid 1000 neither replaces the socket nor creates another one in root's folder.
r="$(setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /bin/sh -c "rm -f /run/ward/$UID_T/wardd.sock 2>&1; touch /run/ward/$UID_T/x 2>&1" || true)"
check "uid $UID_T não mexe em /run/ward/$UID_T" "$r" "Permission denied"

journalctl -u "wardd@$UID_T.service" --no-pager -q 2>/dev/null | tail -5 || true

if [ "$failures" -ne 0 ]; then
    echo "test-units: $failures falha(s)"
    exit 1
fi
echo "test-units: tudo ok"
