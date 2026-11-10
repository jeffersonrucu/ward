#!/bin/bash -p
# Parte de dentro do `make test-install` (Fase 51; V9.2, V9.3, V9.4, V9.7, V9.10, V9.11;
# D46, D47, D57, D60, D62, D63). Roda como root **só dentro** do contêiner descartável
# com systemd e sem rede; do host só vem /opt/teste (somente leitura): uma cópia do
# Makefile e de install/, os artefatos sem `test-mode` (o `ward-app` é o
# `partida-conteiner`: a partida do app real sem GTK), o `wardd` de teste (para a
# recusa do V9.2), o cliente mínimo, o ajudante `plataforma` (sockets Wayland/barramento
# falsos e o ambiente de D57), o servidor falso `falso`, a `libmarca.so`, um pacote CRX3
# falso, o cofre de teste e a senha de teste (falsa, D15). Cada conferência imprime
# "ok:" ou "FALHOU:"; qualquer falha sai ≠ 0.

export PATH=/usr/sbin:/usr/bin:/sbin:/bin
set -euo pipefail

falhas=0
ok() { echo "ok: $*"; }
falhou() { echo "FALHOU: $*"; falhas=$((falhas + 1)); }
confere() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else falhou "$1 (esperado '$3', veio '$2')"; fi
}
nao_tem() {
    if [[ "$2" != *"$3"* ]]; then ok "$1"; else falhou "$1 (não devia ter '$3': $2)"; fi
}

for _ in $(seq 1 100); do
    estado="$(systemctl is-system-running 2>/dev/null || true)"
    [[ "$estado" == running || "$estado" == degraded ]] && break
    sleep 0.1
done
confere "systemd de pé" "$estado" "r"

T=/opt/teste
B=$T/bin
UID_T=1000
USUARIO="$(getent passwd "$UID_T" | cut -d: -f1)"
HOME_T="$(getent passwd "$UID_T" | cut -d: -f6)"
# Pastas do usuário do contêiner (não do host), com o nome montado por partes para a
# varredura do V1.5 (`dev/tests/attacks-foundation.sh`) não confundir com o home real.
COFRE_T="$HOME_T/.local/share/w""ard"
ID="$(cat "$T/id")"
SOCK="/run/ward/$UID_T/wardd.sock"
como_1000() { setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups "$@"; }

# Árvore (caminhos dos arquivos do Ward) lida da cópia de install/tree.txt.
ARQUIVOS=()
while read -r t _ _ _ c _; do
    [[ -z "$t" || "$t" == \#* ]] && continue
    [ "$t" = f ] && ARQUIVOS+=("/$c")
done <"$T/repo/install/tree.txt"
nada_do_ward() {
    local c
    for c in "${ARQUIVOS[@]}"; do
        if [ -e "$c" ] || [ -L "$c" ]; then echo "$c"; return; fi
    done
    echo "nada"
}

instalar() {
    make -s -C "$T/repo" install WARD_UID="$UID_T" EXTENSION_ID="$ID" EXTENSION_CRX="$T/ward.crx" \
        BIN_WARD="$B/ward" BIN_WARDD="$B/wardd" BIN_NM="$B/ward-nm" BIN_APP="$B/ward-app" "$@"
}
limpar_grupos() {
    userdel fulano 2>/dev/null || true
    for g in ward-app ward-nm outro; do groupdel "$g" 2>/dev/null || true; done
}

confere "o systemd lê /usr/local/lib/systemd/system" "$(systemd-analyze unit-paths)" "/usr/local/lib/systemd/system"

# --- V9.11: grupo existente fora das regras de D60 -> recusa, sem binário setgid -------
cp /etc/subgid /tmp/subgid.orig
cp /etc/nsswitch.conf /tmp/nsswitch.orig
install -d -m 0755 /etc/security /etc/sudoers.d
[ -e /etc/security/group.conf ] && cp /etc/security/group.conf /tmp/group.conf.orig || : >/tmp/group.conf.orig
caso_grupo() {
    local rotulo="$1" msg="$2" r codigo=0
    r="$(instalar 2>&1)" || codigo=$?
    if [ "$codigo" -ne 0 ]; then ok "V9.11 $rotulo: recusado"; else falhou "V9.11 $rotulo: instalou"; fi
    confere "V9.11 $rotulo: mensagem" "$r" "$msg"
    confere "V9.11 $rotulo: nenhum arquivo do Ward (nem o setgid)" "$(nada_do_ward)" "nada"
    limpar_grupos
    cp /tmp/subgid.orig /etc/subgid
    cp /tmp/nsswitch.orig /etc/nsswitch.conf
    cp /tmp/group.conf.orig /etc/security/group.conf
    rm -f /etc/sudoers.d/ward-teste /etc/sudoers.d/incluido /etc/sudoers-extra
}
groupadd -g 1500 ward-app
caso_grupo "gid >= 1000" "gid 1500"
groupadd --system ward-app
gpasswd -a "$USUARIO" ward-app >/dev/null
caso_grupo "com membro" "tem membros"
groupadd --system ward-nm
groupmod -p 'falso-senha-de-grupo' ward-nm
caso_grupo "com senha no gshadow" "tem senha"
groupadd --system ward-app
gpasswd -A "$USUARIO" ward-app >/dev/null
caso_grupo "com administrador no gshadow" "administradores"
groupadd --system ward-app
groupadd -o -g "$(getent group ward-app | cut -d: -f3)" outro
caso_grupo "outro grupo com o mesmo gid" "outro grupo"
groupadd --system ward-app
useradd -M -N -g ward-app -s /usr/sbin/nologin fulano
caso_grupo "grupo primário de um usuário" "grupo primário"
printf 'falso:900:50\n' >>/etc/subgid
groupadd -g 920 ward-app
caso_grupo "gid numa faixa do /etc/subgid" "subgid"
# Recém-criado pela instalação e caindo no /etc/subgid: apagado e recusado.
printf 'falso:1:999\n' >>/etc/subgid
caso_grupo "grupo novo cairia no /etc/subgid" "subgid"
confere "V9.11 o grupo recém-criado e recusado foi apagado" "$(getent group ward-app || echo ausente)" "ausente"
cp /tmp/subgid.orig /etc/subgid
# Fase 51a: outros caminhos para o gid do grupo.
printf '*;*;*;Al0000-2400;ward-app\n' >>/etc/security/group.conf
caso_grupo "pam_group (group.conf) dá o grupo no login" "pam_group"
printf '%s ALL=(:ward-nm) NOPASSWD: /usr/bin/id\n' "$USUARIO" >/etc/sudoers.d/ward-teste
caso_grupo "sudoers.d com sudo -g ward-nm" "sudoers"
groupadd --system ward-app
printf '%s ALL=(:#%s) NOPASSWD: /usr/bin/id\n' "$USUARIO" "$(getent group ward-app | cut -d: -f3)" >/etc/sudoers.d/incluido
mv /etc/sudoers.d/incluido /etc/sudoers-extra
printf '@include /etc/sudoers-extra\n' >/etc/sudoers.d/ward-teste
caso_grupo "sudoers por @include com #<gid>" "sudoers"
printf 'subid: sss\n' >>/etc/nsswitch.conf
caso_grupo "nsswitch com subid: sss" "subid"
printf 'sudoers: files ldap\n' >>/etc/nsswitch.conf
caso_grupo "nsswitch com sudoers: files ldap" "sudoers"
# Fase 51b: grupos vindos de outra fonte do NSS (sss, ldap…) fora do /etc/group.
sed -i '/^group:/d' /etc/nsswitch.conf
printf 'group: files sss\n' >>/etc/nsswitch.conf
caso_grupo "nsswitch com group: files sss" "busca 'group'"
printf 'initgroups: files ldap\n' >>/etc/nsswitch.conf
caso_grupo "nsswitch com initgroups: files ldap" "busca 'initgroups'"
confere "V9.11 (51a) os grupos recém-criados e recusados foram apagados" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
rm -rf /usr/local/lib/ward

# --- Fase 51a: outra política do Chrome com os nomes da do Ward -> recusa ---------------
install -d -m 0755 /etc/opt/chrome/policies/managed
printf '{"NativeMessagingUserLevelHosts": true}\n' >/etc/opt/chrome/policies/managed/outra.json
r="$(instalar 2>&1)" && falhou "política conflitante: instalou" || ok "política conflitante (NativeMessagingUserLevelHosts em outro arquivo): recusada"
confere "política conflitante: mensagem" "$r" "NativeMessagingUserLevelHosts"
confere "política conflitante: nenhum arquivo do Ward" "$(nada_do_ward)" "nada"
confere "política conflitante: nenhum grupo criado" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
rm -f /etc/opt/chrome/policies/managed/outra.json

# --- V9.2: artefato test-mode -> recusa antes de grupo e arquivo ---------------------
r="$(instalar BIN_WARDD="$B/wardd-test-mode" 2>&1)" && falhou "V9.2: instalou o wardd de teste" || ok "V9.2: wardd com test-mode recusado"
confere "V9.2: mensagem" "$r" "modo-teste"
confere "V9.2: nenhum arquivo do Ward" "$(nada_do_ward)" "nada"
confere "V9.2: nenhum grupo criado" "$(getent group ward-app ward-nm || echo ausente)" "ausente"

# --- instalação de verdade ------------------------------------------------------------
r="$(instalar 2>&1)" || { printf '%s\n' "$r"; falhou "make install"; }
ok "make install (root, sem DESTDIR) saiu 0"
# V9.1 (como root): dono, grupo e modo de cada caminho de tree.txt.
while read -r t m dono grupo c _; do
    [[ -z "$t" || "$t" == \#* ]] && continue
    real="$(stat -c '%U %G %a' -- "/$c")"
    esperado="$dono $grupo $((8#$m))"
    [ "$(stat -c '%U %G' -- "/$c") $((8#$(stat -c %a -- "/$c")))" = "$esperado" ] || falhou "V9.1: /$c é '$real' (esperado $dono $grupo $m)"
    [ $((8#$(stat -c %a -- "/$c") & 8#022)) = 0 ] || falhou "V9.1: /$c com escrita para grupo ou outros"
done <"$T/repo/install/tree.txt"
ok "V9.1: donos e modos de tree.txt (root:root, ward-app e ward-nm root:<grupo> 2755)"
confere "V9.6: manifesto de native messaging do root" "$(stat -c '%U %a' /etc/opt/chrome/native-messaging-hosts/br.ward.nm.json)" "root 644"
confere "V9.6: allowed_origins só com o id" "$(grep allowed_origins /etc/opt/chrome/native-messaging-hosts/br.ward.nm.json)" "[\"chrome-extension://$ID/\"]"
confere "Fase 51a/44c: política com NativeMessagingUserLevelHosts, PasswordManagerEnabled e PasswordLeakDetectionEnabled false" "$(python3 -I -c 'import json,sys; p=json.load(open(sys.argv[1])); print(p["NativeMessagingUserLevelHosts"] is False and p["PasswordManagerEnabled"] is False and p["PasswordLeakDetectionEnabled"] is False, sorted(p))' /etc/opt/chrome/policies/managed/ward.json)" "True ['ExtensionSettings', 'NativeMessagingUserLevelHosts', 'PasswordLeakDetectionEnabled', 'PasswordManagerEnabled']"
igual_marca="$(for g in ward-app ward-nm; do echo "$g $(getent group "$g" | cut -d: -f3)"; done)"
confere "Fase 51a: marca dos grupos criados (nome e gid)" "$(sort /usr/local/lib/ward/grupos-criados)" "$igual_marca"
# Fase 51c (S2, S8): sem o apport e o libwebkit2gtk na imagem, as pastas são criadas e a
# lista sai com os caminhos do multiarch; o systemd lê o drop-in do systemd-coredump.
confere "51c: sem o libwebkit2gtk-4.1-0, a instalação avisa" "$r" "libwebkit2gtk-4.1-0 ($(dpkg --print-architecture)) não está instalado"
case "$(dpkg --print-architecture)" in arm64) MA=aarch64-linux-gnu ;; *) MA=x86_64-linux-gnu ;; esac
for l in report-ignore blacklist.d; do
    confere "51c: /etc/apport/$l/ward com o WebKitWebProcess" "$(cat "/etc/apport/$l/ward")" "/usr/lib/$MA/webkit2gtk-4.1/WebKitWebProcess"
    confere "51c: /etc/apport/$l/ward com o ward-app" "$(cat "/etc/apport/$l/ward")" "/usr/local/bin/ward-app"
done
cc="$(systemd-analyze cat-config systemd/coredump.conf 2>&1 || true)"
confere "51c: systemd-analyze cat-config lê o drop-in do Ward" "$cc" "# /etc/systemd/coredump.conf.d/ward.conf"
confere "51c: Storage=none" "$cc" "Storage=none"
confere "51c: ProcessSizeMax=0" "$cc" "ProcessSizeMax=0"
confere "Fase 51a: marca root 0644" "$(stat -c '%U %a' /usr/local/lib/ward/grupos-criados)" "root 644"

# V9.11: os grupos criados.
for g in ward-app ward-nm; do
    IFS=: read -r _ _ gid membros <<<"$(getent group "$g")"
    if [ "$gid" -gt 0 ] && [ "$gid" -lt 1000 ]; then ok "V9.11 $g é de sistema (gid $gid)"; else falhou "V9.11 $g gid $gid"; fi
    confere "V9.11 $g sem membros" "[$membros]" "[]"
    IFS=: read -r _ senha admins gmembros <<<"$(grep "^$g:" /etc/gshadow)"
    confere "V9.11 $g com '!' no gshadow" "$senha" "!"
    confere "V9.11 $g sem administradores nem membros no gshadow" "[$admins][$gmembros]" "[][]"
    fora=1
    while IFS=: read -r _ ini qtd; do
        [[ "$ini" =~ ^[0-9]+$ ]] || continue
        if [ "$gid" -ge "$ini" ] && [ "$gid" -lt $((ini + qtd)) ]; then fora=0; fi
    done </etc/subgid
    [ "$fora" = 1 ] && ok "V9.11 $g fora das faixas do /etc/subgid" || falhou "V9.11 $g dentro do /etc/subgid"
done
# newgrp de quem não é do grupo: falha (senha travada).
r="$(como_1000 newgrp ward-app </dev/null 2>&1; echo "saiu=$?")"
nao_tem "V9.11 newgrp ward-app pelo uid $UID_T falha" "$r" "saiu=0"
r="$(como_1000 sg ward-app -c 'id -G' </dev/null 2>&1; echo "saiu=$?")"
nao_tem "V9.11 sg ward-app pelo uid $UID_T falha" "$r" "saiu=0"
nao_tem "V9.11 o uid $UID_T não ganha o gid de ward-app" "$r" " $(getent group ward-app | cut -d: -f3)"

# V9.7: socket ativo e habilitado, pastas do root, socket do uid.
confere "V9.7 wardd@$UID_T.socket habilitado" "$(systemctl is-enabled "wardd@$UID_T.socket" || true)" "enabled"
confere "V9.7 wardd@$UID_T.socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
confere "V9.7 serviço ainda parado (sobe na primeira conexão)" "$(systemctl is-active "wardd@$UID_T.service" || true)" "inactive"
confere "V9.7 /run/ward root:root 0755 (tmpfiles.d)" "$(stat -c '%U:%G %a' /run/ward)" "root:root 755"
confere "V9.7 /run/ward/$UID_T root:root 0755" "$(stat -c '%U:%G %a' "/run/ward/$UID_T")" "root:root 755"
confere "V9.7 socket do uid, 0600" "$(stat -c '%u %a %F' "$SOCK")" "$UID_T 600 socket"

# D57 / V9.11: units sem no_new_privs nem RestrictSUIDSGID.
r="$(systemctl show -p NoNewPrivileges -p RestrictSUIDSGID "wardd@$UID_T.service")"
confere "V9.11 systemctl show: NoNewPrivileges=no" "$r" "NoNewPrivileges=no"
confere "V9.11 systemctl show: RestrictSUIDSGID=no" "$r" "RestrictSUIDSGID=no"
sec="$(systemd-analyze security --no-pager "wardd@$UID_T.service" 2>&1 || true)"
for d in NoNewPrivileges= RestrictSUIDSGID=; do
    linha="$(grep -F -- "$d" <<<"$sec" | head -n 1 || true)"
    confere "V9.11 systemd-analyze security: $d não ligado" "$linha" "✗"
done

# Primeira conexão sobe o wardd como o usuário (V9.7), com o par uid 0 que a CLI confere.
r="$(como_1000 "$B/cliente" "$SOCK" estado)"
confere "V9.7 a primeira conexão sobe o wardd e ele responde" "$r" "resposta: estado"
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
st="/proc/$pid/status"
confere "V9.7 wardd roda com o uid" "$(awk '/^Uid:/{print $2,$3,$4,$5}' "$st")" "$UID_T $UID_T $UID_T $UID_T"
confere "D57 wardd sem no_new_privs" "$(awk '/^NoNewPrivs:/{print $2}' "$st")" "0"
confere "V9.4 core 0" "$(awk '/^Max core file size/{print $5, $6}' "/proc/$pid/limits")" "0 0"
confere "D62 LimitMEMLOCK=infinity" "$(awk '/^Max locked memory/{print $4, $5}' "/proc/$pid/limits")" "unlimited unlimited"
# V9.4: não-dumpable: o /proc do wardd é do root e o próprio uid não lê o environ.
confere "V9.4 /proc/<wardd>/environ do root (dumpable 0)" "$(stat -c %U "/proc/$pid/environ")" "root"
r="$(como_1000 cat "/proc/$pid/environ" 2>&1 >/dev/null || true)"
confere "V9.4 o uid $UID_T não lê o environ do wardd" "$r" "Permission denied"
r="$(como_1000 cat "/proc/$pid/mem" 2>&1 >/dev/null || true)"
confere "V9.4 o uid $UID_T não abre o mem do wardd" "$r" "Permission denied"

# V9.4: `systemctl --user set-environment LD_PRELOAD=…` do atacante não alcança o wardd
# do sistema. O gerente do usuário é o `user@1000.service` de verdade; como o contêiner
# não tem logind nem barramento do sistema, dois drop-ins só do contêiner (em /run) fazem
# o papel do pam_systemd: criam /run/user/1000 e passam o XDG_RUNTIME_DIR. O
# `systemctl set-environment` do gerente do sistema o uid nem consegue.
rm -f /tmp/ward-marca
r="$(como_1000 systemctl set-environment "LD_PRELOAD=$B/libmarca.so" 2>&1; echo "saiu=$?")"
nao_tem "V9.4 o uid $UID_T não muda o ambiente do gerente do sistema" "$r" "saiu=0"
nao_tem "V9.4 o gerente do sistema continua sem LD_PRELOAD" "$(systemctl show-environment)" "LD_PRELOAD"
install -d -m 0755 "/run/systemd/system/user-runtime-dir@$UID_T.service.d" "/run/systemd/system/user@$UID_T.service.d"
printf '[Service]\nExecStart=\nExecStart=/usr/bin/install -d -o %s -g %s -m 0700 /run/user/%s\nExecStop=\nExecStop=/usr/bin/rm -rf /run/user/%s\n' \
    "$UID_T" "$UID_T" "$UID_T" "$UID_T" >"/run/systemd/system/user-runtime-dir@$UID_T.service.d/sem-logind.conf"
printf '[Service]\nEnvironment=XDG_RUNTIME_DIR=/run/user/%s\n' "$UID_T" >"/run/systemd/system/user@$UID_T.service.d/sem-logind.conf"
systemctl daemon-reload
systemctl start "user@$UID_T.service" || true
confere "V9.4 gerente do usuário (user@$UID_T.service) no ar" "$(systemctl is-active "user@$UID_T.service" || true)" "active"
usuario() { como_1000 /usr/bin/env -i XDG_RUNTIME_DIR="/run/user/$UID_T" PATH=/usr/bin:/bin systemctl --user "$@"; }
r="$(usuario set-environment "LD_PRELOAD=$B/libmarca.so" 2>&1; echo "saiu=$?")"
confere "V9.4 o atacante põe LD_PRELOAD no gerente do usuário" "$r" "saiu=0"
confere "V9.4 o gerente do usuário tem o LD_PRELOAD" "$(usuario show-environment 2>&1 || true)" "LD_PRELOAD=$B/libmarca.so"
# Força o wardd a subir de novo: mata e conecta (com o LD_PRELOAD também no cliente).
kill -9 "$pid"
sleep 1.5
r="$(como_1000 env "LD_PRELOAD=$B/libmarca.so" "$B/cliente" "$SOCK" estado 2>&1)"
confere "V9.4 depois de morto, a conexão sobe outro wardd" "$r" "resposta: estado"
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
amb="$(tr '\0' '\n' <"/proc/$pid/environ" | cut -d= -f1 | sort | tr '\n' ' ')"
nao_tem "V9.4 o wardd novo não tem LD_PRELOAD (environ lido como root: $amb)" "$amb" "LD_"
r="$(grep -c libmarca "/proc/$pid/maps" || true)"
confere "V9.4 a lib do atacante não está no wardd" "maps=$r" "maps=0"
confere "V9.4 o wardd novo também é não-dumpable" "$(stat -c %U "/proc/$pid/environ")" "root"
usuario unset-environment LD_PRELOAD >/dev/null 2>&1 || true
systemctl stop "user@$UID_T.service" "user-runtime-dir@$UID_T.service" || true
rm -rf "/run/systemd/system/user-runtime-dir@$UID_T.service.d" "/run/systemd/system/user@$UID_T.service.d" "/run/user/$UID_T"
systemctl daemon-reload

# --- papel app: o app aberto pelo daemon recebe o papel (D47, D57, D63; V9.11) -------
REL=/tmp/ward-app-relatorio.txt
rm -f "$REL"
install -d -o "$UID_T" -g "$UID_T" -m 0700 "/run/user/$UID_T"
rm -f "/run/user/$UID_T/bus" "/run/user/$UID_T"/wayland-*
como_1000 "$B/plataforma" escutar "/run/user/$UID_T/wayland-1" >/dev/null 2>&1 &
W1=$!
como_1000 "$B/plataforma" escutar "/run/user/$UID_T/bus" >/dev/null 2>&1 &
BUS=$!
for _ in $(seq 50); do [ -S "/run/user/$UID_T/wayland-1" ] && [ -S "/run/user/$UID_T/bus" ] && break; sleep 0.1; done
amb="$(como_1000 "$B/plataforma" ambiente)"
inicio="$(date '+%Y-%m-%d %H:%M:%S')"
# O ward-app aberto pelo usuário (o do menu): pede `abrir_app` ao daemon e sai.
# shellcheck disable=SC2086
como_1000 /usr/bin/bash -c "/usr/bin/env -i $(echo $amb) /usr/local/bin/ward-app; true"
for _ in $(seq 100); do grep -q '^depois ' "$REL" 2>/dev/null && break; sleep 0.1; done
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
r="$(grep '^erro ' "$REL" | head -n 1 || true)"
confere "o ward-app aberto pelo usuário pede ao daemon e sai (pai não é o daemon)" "$r" "pedido ao daemon"
r="$(grep '^depois ' "$REL" | head -n 1 || true)"
confere "V9.11 o daemon abriu o ward-app setgid e ele recebeu o papel app" "$r" "pai=$pid"
confere "V9.11 o app largou o grupo depois de conectar" "$r" "gid=$UID_T,$UID_T,$UID_T,$UID_T"
confere "V9.11 o app ficou não-dumpable" "$r" "dumpable=0"
APID="$(sed -E 's/^depois pid=([0-9]+) .*/\1/' <<<"$r")"
r="$(grep "^inicio pid=$APID " "$REL" | head -n 1 || true)"
GID_APP="$(getent group ward-app | cut -d: -f3)"
confere "V9.11 o exec aplicou o setgid (units sem no_new_privs, D57)" "$r" "gid=$UID_T,$GID_APP,$GID_APP,$GID_APP"
jr="$(journalctl -u "wardd@$UID_T.service" --no-pager -q --since "$inicio" 2>/dev/null || true)"
nao_tem "V9.11 o daemon não recusou o papel app ao app que ele abriu" "$jr" "papel app recusado (pid $APID)"
systemctl stop "wardd@$UID_T.socket" "wardd@$UID_T.service"
kill "$W1" "$BUS" 2>/dev/null || true
wait "$W1" "$BUS" 2>/dev/null || true

# --- V9.3: wardd copiado e servidor falso do uid: a CLI recusa antes da senha ---------
# (o socket do sistema está parado desde o fim do caso acima)
install -d -o "$UID_T" -g "$UID_T" -m 0700 "/run/user/$UID_T/ward" "$HOME_T/bin" /tmp/v93
como_1000 cp /usr/local/bin/wardd "$HOME_T/bin/wardd"
r="$(como_1000 /usr/bin/timeout 10 /usr/bin/env -i HOME="$HOME_T" PATH=/usr/bin:/bin XDG_RUNTIME_DIR="/run/user/$UID_T" \
    "$HOME_T/bin/wardd" 2>&1; echo "saiu=$?")"
nao_tem "V9.3 o wardd copiado para ~/bin não sobe sem o systemd" "$r" "saiu=0"
nao_tem "V9.3 o wardd copiado não fica no ar" "$r" "saiu=124"
REG=/tmp/v93/registro
como_1000 touch "$REG"
como_1000 "$B/falso" "/run/user/$UID_T/ward/wardd.sock" "$REG" 30 >/tmp/v93/falso.out 2>&1 &
FPID=$!
for _ in $(seq 50); do grep -q pronto /tmp/v93/falso.out 2>/dev/null && break; sleep 0.1; done
confere "V9.3 servidor falso do uid escutando em /run/user/$UID_T/ward" "$(cat /tmp/v93/falso.out)" "pronto"
SENHA="$(cat "$T/senha")"
r="$(printf '%s\n' "$SENHA" | como_1000 /usr/bin/env -i HOME="$HOME_T" PATH=/usr/bin:/bin LANG=C.UTF-8 \
    XDG_RUNTIME_DIR="/run/user/$UID_T" WARD_SOCKET="/run/user/$UID_T/ward/wardd.sock" \
    /usr/bin/script -qec "/usr/local/bin/ward destravar" /dev/null 2>&1; echo "saiu=$?")"
nao_tem "V9.3 ward destravar não sai 0" "$r" "saiu=0"
nao_tem "V9.3 ward destravar não pede a senha" "$r" "Senha mestra"
confere "V9.3 o servidor falso não recebeu conexão nem byte" "[$(cat "$REG")]" "[]"
kill "$FPID" 2>/dev/null || true
wait "$FPID" 2>/dev/null || true
systemctl start "wardd@$UID_T.socket"

# --- reinstalar por cima e o cofre do usuário -------------------------------------------
for d in "$HOME_T/.local" "$HOME_T/.local/share" "$COFRE_T"; do
    install -d -m 0700 -o "$UID_T" -g "$UID_T" "$d"
done
install -m 0600 -o "$UID_T" -g "$UID_T" "$T/cofre.ward" "$COFRE_T/cofre.ward"
printf 'ward-estado 1\nvista 0\nqueimada 0\n' >"$COFRE_T/estado"
chown "$UID_T:$UID_T" "$COFRE_T/estado"
chmod 0600 "$COFRE_T/estado"
r="$(printf '%s' "$SENHA" | como_1000 "$B/cliente" "$SOCK" destravar)"
confere "o wardd instalado destrava o cofre de teste" "$r" "resposta: ok"
antes="$(cd "$COFRE_T" && sha256sum cofre.ward estado; stat -c '%U %a' cofre.ward)"
r="$(instalar 2>&1)" || { printf '%s\n' "$r"; falhou "reinstalar"; }
ok "reinstalar por cima sai 0 (grupos já existentes conferem)"
confere "reinstalar mantém o socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
r="$(como_1000 "$B/cliente" "$SOCK" estado)"
confere "depois de reinstalar, o wardd responde" "$r" "resposta: estado"

# --- V9.10: uninstall ----------------------------------------------------------------
r="$(make -s -C "$T/repo" uninstall 2>&1)" || { printf '%s\n' "$r"; falhou "make uninstall"; }
ok "make uninstall saiu 0"
confere "V9.10 nenhum arquivo do Ward" "$(nada_do_ward)" "nada"
for d in /usr/local/lib/ward /usr/local/share/ward /run/ward; do
    [ ! -e "$d" ] && ok "V9.10 $d apagada" || falhou "V9.10 $d ficou"
done
nao_tem "51c: depois do uninstall, o systemd-coredump não lê mais o drop-in" "$(systemd-analyze cat-config systemd/coredump.conf 2>&1 || true)" "ward.conf"
confere "V9.10 grupos apagados" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
confere "V9.10 nenhuma unit wardd@ carregada" "[$(systemctl list-units --all --plain --no-legend 'wardd@*' || true)]" "[]"
nao_tem "V9.10 wardd@$UID_T.socket não está mais habilitado" "$(systemctl is-enabled "wardd@$UID_T.socket" 2>&1 || true)" "enabled"
confere "V9.10 nenhum link em sockets.target.wants" "$(ls /etc/systemd/system/sockets.target.wants/ 2>/dev/null | grep -c wardd || true)" "0"
confere "V9.10 o cofre do usuário intacto" "$(cd "$COFRE_T" && sha256sum cofre.ward estado; stat -c '%U %a' cofre.ward)" "$antes"
r="$(make -s -C "$T/repo" uninstall 2>&1)" && ok "uninstall de novo sai 0" || { printf '%s\n' "$r"; falhou "uninstall repetido"; }

# --- Fase 51a: grupo que já existia (e confere com D60) não é apagado no uninstall ------
groupadd --system ward-app
gid_admin="$(getent group ward-app | cut -d: -f3)"
r="$(instalar 2>&1)" || { printf '%s\n' "$r"; falhou "instalar com ward-app do administrador"; }
confere "grupo do administrador: aceito" "$r" "ward-app já existia e confere"
nao_tem "grupo do administrador: fora da marca" "$(cat /usr/local/lib/ward/grupos-criados)" "ward-app"
r="$(make -s -C "$T/repo" uninstall 2>&1)" || { printf '%s\n' "$r"; falhou "uninstall"; }
confere "uninstall: ward-app do administrador fica (mesmo gid)" "$(getent group ward-app | cut -d: -f3)" "$gid_admin"
confere "uninstall: avisa que ficou" "$r" "não foi criado pelo Ward"
confere "uninstall: ward-nm (criado pelo Ward) apagado" "$(getent group ward-nm || echo ausente)" "ausente"
[ ! -e /usr/local/lib/ward ] && ok "uninstall: /usr/local/lib/ward (com a marca) apagada" || falhou "/usr/local/lib/ward ficou"
groupdel ward-app

if [ "$falhas" -ne 0 ]; then
    echo "test-install (contêiner): $falhas falha(s)"
    exit 1
fi
echo "test-install (contêiner): tudo ok"
