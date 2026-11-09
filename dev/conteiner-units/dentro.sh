#!/bin/bash -p
# Parte de dentro do `make teste-units` (Fase 24; V3.21, V3.34; D46, D57, D62). Roda como root
# **só dentro** do contêiner descartável com systemd; do host só vêm /opt/teste
# (somente leitura: `wardd` sem `modo-teste`, o cliente mínimo e as duas units) e este
# script. Cada conferência imprime "ok:" ou "FALHOU:"; qualquer falha sai ≠ 0.

export PATH=/usr/sbin:/usr/bin:/bin
set -euo pipefail

falhas=0
ok() { echo "ok: $*"; }
falhou() { echo "FALHOU: $*"; falhas=$((falhas + 1)); }
confere() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else falhou "$1 (esperado '$3', veio '$2')"; fi
}

# O systemd do contêiner precisa estar de pé.
for _ in $(seq 1 100); do
    estado="$(systemctl is-system-running 2>/dev/null || true)"
    [[ "$estado" == running || "$estado" == degraded ]] && break
    sleep 0.1
done
confere "systemd de pé" "$estado" "r"

UID_T=1000
id "$UID_T" >/dev/null
install -m 0755 -o root -g root /opt/teste/wardd /usr/local/bin/wardd
install -m 0644 /opt/teste/wardd@.socket /opt/teste/wardd@.service /etc/systemd/system/
systemctl daemon-reload

systemctl start "wardd@$UID_T.socket"
confere "socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
confere "serviço ainda parado" "$(systemctl is-active "wardd@$UID_T.service" || true)" "inactive"
confere "/run/ward root:root 0755" "$(stat -c '%U:%G %a' /run/ward)" "root:root 755"
confere "/run/ward/$UID_T root:root 0755" "$(stat -c '%U:%G %a' /run/ward/$UID_T)" "root:root 755"
confere "socket do uid, 0600" "$(stat -c '%u %a %F' /run/ward/$UID_T/wardd.sock)" "$UID_T 600 socket"

# Primeira conexão sobe o wardd (resposta do daemon, como o uid 1000).
r="$(setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" estado)"
confere "primeira conexão responde" "$r" "resposta: estado"
confere "serviço subiu" "$(systemctl is-active "wardd@$UID_T.service" || true)" "active"

pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
st="/proc/$pid/status"
confere "wardd roda com o uid" "$(awk '/^Uid:/{print $2,$3,$4,$5}' "$st")" "$UID_T $UID_T $UID_T $UID_T"
confere "sem no_new_privs (D57)" "$(awk '/^NoNewPrivs:/{print $2}' "$st")" "0"
confere "LimitCORE=0" "$(awk '/^Max core file size/{print $5, $6}' "/proc/$pid/limits")" "0 0"
confere "LimitMEMLOCK=infinity (D62)" "$(awk '/^Max locked memory/{print $4, $5}' "/proc/$pid/limits")" "unlimited unlimited"
amb="$(tr '\0' '\n' <"/proc/$pid/environ" | cut -d= -f1 | sort | tr '\n' ' ')"
if [[ "$amb" == *LD_* ]]; then falhou "LD_* no ambiente: $amb"; else ok "sem LD_* no ambiente ($amb)"; fi

# V3.34 (D62): com o cofre de teste (valores falsos) destravado e uma operação, toda a
# memória do wardd está travada (`MCL_ONFAULT`: `VmLck` conta as regiões inteiras).
HOME_T="$(getent passwd "$UID_T" | cut -d: -f6)"
for d in "$HOME_T/.local" "$HOME_T/.local/share" "$HOME_T/.local/share/ward"; do
    install -d -m 0700 -o "$UID_T" -g "$UID_T" "$d"
done
install -m 0600 -o "$UID_T" -g "$UID_T" /opt/teste/cofre.ward "$HOME_T/.local/share/ward/cofre.ward"
printf 'ward-estado 1\nvista 0\nqueimada 0\n' >"$HOME_T/.local/share/ward/estado"
chown "$UID_T:$UID_T" "$HOME_T/.local/share/ward/estado"
chmod 0600 "$HOME_T/.local/share/ward/estado"
# Senha mestra de teste (D15, falsa) pelo stdin, nunca em argv.
r="$(printf '%s' 'falso-mestra-ward-01' | setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" destravar)"
confere "destrava o cofre de teste" "$r" "resposta: ok"
r="$(setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" lista)"
confere "lista com o cofre destravado" "$r" "resposta: lista"

# V4.38 (Fase 26, D63): grupos da instalação e cópias setgid do cliente mínimo. O
# `wardd` lê os grupos na partida: reinicia para ligar os papéis.
groupadd --system ward-app
groupadd --system ward-nm
install -m 2755 -o root -g ward-app /opt/teste/cliente /usr/local/bin/ward-app
install -m 2755 -o root -g ward-nm /opt/teste/cliente /usr/local/bin/ward-nm
install -m 0755 -o root -g root /opt/teste/ward /usr/local/bin/ward
systemctl stop "wardd@$UID_T.service"
r="$(printf '%s' 'falso-mestra-ward-01' | setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" destravar)"
confere "V4.38 destrava de novo, com os grupos" "$r" "resposta: ok"
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
st="/proc/$pid/status"
# Ambiente de uma sessão (D57), mais o que o `ward run` repassa.
como_usuario() {
    setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups \
        env -i PATH=/usr/local/bin:/usr/bin:/bin HOME="$HOME_T" LANG=C.UTF-8 \
        XDG_RUNTIME_DIR="/run/user/$UID_T" WAYLAND_DISPLAY=wayland-0 "$@"
}
inicio="$(date '+%Y-%m-%d %H:%M:%S')"
r="$(como_usuario /usr/local/bin/ward run acme-pg-dev -- /usr/local/bin/ward-app "/run/ward/$UID_T/wardd.sock" estado 2>&1)" || true
confere "V4.38 ward run -- ward-app: papel agente" "$r" "resposta: estado papel=agente"
r="$(como_usuario /usr/local/bin/ward run acme-pg-dev -- /usr/local/bin/ward-app "/run/ward/$UID_T/wardd.sock" lista 2>&1)" || true
confere "V4.38 ward run -- ward-app: lista do agente" "$r" "resposta: lista"
r="$(como_usuario /usr/local/bin/ward run acme-pg-dev -- /usr/local/bin/ward-nm "/run/ward/$UID_T/wardd.sock" estado 2>&1)" || true
confere "V4.38 ward run -- ward-nm: papel agente" "$r" "resposta: estado papel=agente"
r="$(como_usuario /usr/local/bin/ward run acme-pg-dev -- /bin/sh -c 'printf %s "$PGPASSWORD"' 2>&1)" || true
confere "ward run de release: a senha sai mascarada" "$r" "••••"
sleep 0.5
jr="$(journalctl -u "wardd@$UID_T.service" --no-pager -q --since "$inicio" 2>/dev/null || true)"
confere "V4.38 o daemon recusou o papel app ao filho (D63)" "$jr" "papel app recusado"
if [[ "$jr" == *falso-* ]]; then falhou "valor falso no journal"; else ok "journal sem valor do cofre"; fi
lck="$(awk '/^VmLck:/{print $2}' "$st")"
rss="$(awk '/^VmRSS:/{print $2}' "$st")"
if [[ "$lck" =~ ^[0-9]+$ && "$rss" =~ ^[0-9]+$ && "$lck" -gt 0 && "$lck" -ge "$rss" ]]; then
    ok "VmLck ${lck} kB cobre o VmRSS ${rss} kB (mlockall, D62)"
else
    falhou "VmLck '${lck}' kB não cobre o VmRSS '${rss}' kB"
fi
# O mesmo binário com RLIMIT_MEMLOCK de 8 MiB: falha fechada, com mensagem.
codigo=0
saida="$(prlimit --memlock=8388608:8388608 setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups \
    env -i PATH=/usr/bin:/bin HOME="$HOME_T" /usr/local/bin/wardd 2>&1)" || codigo=$?
if [ "$codigo" -ne 0 ]; then ok "8 MiB de memlock: sai $codigo"; else falhou "8 MiB de memlock: saiu 0"; fi
confere "8 MiB de memlock: mensagem" "$saida" "LimitMEMLOCK=infinity"
if [[ "$saida" == *pronto* ]]; then falhou "8 MiB de memlock: chegou a subir"; else ok "8 MiB de memlock: não chegou ao socket"; fi

# Matar o wardd: a próxima conexão sobe outro (Restart/ativação).
kill -9 "$pid"
sleep 1.5
r="$(setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /opt/teste/cliente "/run/ward/$UID_T/wardd.sock" estado)"
confere "depois de morto, a conexão sobe de novo" "$r" "resposta: estado"
novo="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
if [[ "$novo" != "$pid" && "$novo" != 0 ]]; then ok "pid novo ($novo)"; else falhou "pid não mudou"; fi

# O uid 1000 não troca o socket nem cria outro na pasta do root.
r="$(setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups /bin/sh -c "rm -f /run/ward/$UID_T/wardd.sock 2>&1; touch /run/ward/$UID_T/x 2>&1" || true)"
confere "uid $UID_T não mexe em /run/ward/$UID_T" "$r" "Permission denied"

journalctl -u "wardd@$UID_T.service" --no-pager -q 2>/dev/null | tail -5 || true

if [ "$falhas" -ne 0 ]; then
    echo "teste-units: $falhas falha(s)"
    exit 1
fi
echo "teste-units: tudo ok"
