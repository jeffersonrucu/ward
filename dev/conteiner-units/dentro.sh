#!/bin/bash -p
# Parte de dentro do `make teste-units` (Fase 24; V3.21; D46, D57, D62). Roda como root
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
