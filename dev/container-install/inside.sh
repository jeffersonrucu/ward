#!/bin/bash -p
# Inside part of `make test-install` (Phase 51; V9.2, V9.3, V9.4, V9.7, V9.10, V9.11;
# D46, D47, D57, D60, D62, D63). Runs as root **only inside** the disposable container
# with systemd and without network; from the host comes only the read-only test mount
# under /opt: a copy of the Makefile and of install/, the artifacts without `test-mode`
# (the `ward-app` is `startup-container`: the real app's startup without GTK), the test
# `wardd` (for the refusal of V9.2), the minimal client, the platform helper (fake
# Wayland/bus sockets and the environment of D57), the fake server, the mark library, a
# fake CRX3 package, the test vault and the test password (fake, D15). Phase 57 (m2:V5.7,
# m2:V5.8): also the 0.1.4 installer of the v0.1.4 tag with its release binaries and fake
# package (`old/` of the mount) and the 0.1.4 vault, state and v1 client (`legacy/`).
# Each check prints "ok:" or the failure marker; any failure exits ≠ 0.

export PATH=/usr/sbin:/usr/bin:/sbin:/bin
set -euo pipefail

failures=0
ok() { echo "ok: $*"; }
failed() { echo "FALHOU: $*"; failures=$((failures + 1)); }
check() {
    if [[ "$2" == *"$3"* ]]; then ok "$1"; else failed "$1 (esperado '$3', veio '$2')"; fi
}
lacks() {
    if [[ "$2" != *"$3"* ]]; then ok "$1"; else failed "$1 (não devia ter '$3': $2)"; fi
}

for _ in $(seq 1 100); do
    sys_state="$(systemctl is-system-running 2>/dev/null || true)"
    [[ "$sys_state" == running || "$sys_state" == degraded ]] && break
    sleep 0.1
done
check "systemd de pé" "$sys_state" "r"

T=/opt/teste
B=$T/bin
UID_T=1000
TEST_USER="$(getent passwd "$UID_T" | cut -d: -f1)"
HOME_T="$(getent passwd "$UID_T" | cut -d: -f6)"
# Folders of the container's user (not the host's), with the name built in parts so the
# V1.5 scan (`dev/tests/attacks-foundation.sh`) does not mistake it for the real home.
VAULT_T="$HOME_T/.local/share/w""ard"
ID="$(cat "$T/id")"
SOCK="/run/ward/$UID_T/wardd.sock"
as_1000() { setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups "$@"; }

# Tree (paths of the Ward files) read from the copy of install/tree.txt.
FILES=()
while read -r t _ _ _ c _; do
    [[ -z "$t" || "$t" == \#* ]] && continue
    [ "$t" = f ] && FILES+=("/$c")
done <"$T/repo/install/tree.txt"
no_ward_file() {
    local c
    for c in "${FILES[@]}"; do
        if [ -e "$c" ] || [ -L "$c" ]; then echo "$c"; return; fi
    done
    echo "nada"
}

install_ward() {
    make -s -C "$T/repo" install WARD_UID="$UID_T" EXTENSION_ID="$ID" EXTENSION_CRX="$T/ward.crx" \
        BIN_WARD="$B/ward" BIN_WARDD="$B/wardd" BIN_NM="$B/ward-nm" BIN_APP="$B/ward-app" "$@"
}
clean_groups() {
    userdel fulano 2>/dev/null || true
    for g in ward-app ward-nm outro; do groupdel "$g" 2>/dev/null || true; done
}

check "o systemd lê /usr/local/lib/systemd/system" "$(systemd-analyze unit-paths)" "/usr/local/lib/systemd/system"

# --- V9.11: existing group outside the rules of D60 -> refusal, without a setgid binary ---
cp /etc/subgid /tmp/subgid.orig
cp /etc/nsswitch.conf /tmp/nsswitch.orig
install -d -m 0755 /etc/security /etc/sudoers.d
[ -e /etc/security/group.conf ] && cp /etc/security/group.conf /tmp/group.conf.orig || : >/tmp/group.conf.orig
group_case() {
    local label="$1" msg="$2" r code=0
    r="$(install_ward 2>&1)" || code=$?
    if [ "$code" -ne 0 ]; then ok "V9.11 $label: recusado"; else failed "V9.11 $label: instalou"; fi
    check "V9.11 $label: mensagem" "$r" "$msg"
    check "V9.11 $label: nenhum arquivo do Ward (nem o setgid)" "$(no_ward_file)" "nada"
    clean_groups
    cp /tmp/subgid.orig /etc/subgid
    cp /tmp/nsswitch.orig /etc/nsswitch.conf
    cp /tmp/group.conf.orig /etc/security/group.conf
    rm -f /etc/sudoers.d/ward-teste /etc/sudoers.d/incluido /etc/sudoers-extra
}
groupadd -g 1500 ward-app
group_case "gid >= 1000" "gid 1500"
groupadd --system ward-app
gpasswd -a "$TEST_USER" ward-app >/dev/null
group_case "com membro" "tem membros"
groupadd --system ward-nm
groupmod -p 'falso-senha-de-grupo' ward-nm
group_case "com senha no gshadow" "tem senha"
groupadd --system ward-app
gpasswd -A "$TEST_USER" ward-app >/dev/null
group_case "com administrador no gshadow" "administradores"
groupadd --system ward-app
groupadd -o -g "$(getent group ward-app | cut -d: -f3)" outro
group_case "outro grupo com o mesmo gid" "outro grupo"
groupadd --system ward-app
useradd -M -N -g ward-app -s /usr/sbin/nologin fulano
group_case "grupo primário de um usuário" "grupo primário"
printf 'falso:900:50\n' >>/etc/subgid
groupadd -g 920 ward-app
group_case "gid numa faixa do /etc/subgid" "subgid"
# Just created by the install and falling in /etc/subgid: deleted and refused.
printf 'falso:1:999\n' >>/etc/subgid
group_case "grupo novo cairia no /etc/subgid" "subgid"
check "V9.11 o grupo recém-criado e recusado foi apagado" "$(getent group ward-app || echo ausente)" "ausente"
cp /tmp/subgid.orig /etc/subgid
# Phase 51a: other paths to the group's gid.
printf '*;*;*;Al0000-2400;ward-app\n' >>/etc/security/group.conf
group_case "pam_group (group.conf) dá o grupo no login" "pam_group"
printf '%s ALL=(:ward-nm) NOPASSWD: /usr/bin/id\n' "$TEST_USER" >/etc/sudoers.d/ward-teste
group_case "sudoers.d com sudo -g ward-nm" "sudoers"
groupadd --system ward-app
printf '%s ALL=(:#%s) NOPASSWD: /usr/bin/id\n' "$TEST_USER" "$(getent group ward-app | cut -d: -f3)" >/etc/sudoers.d/incluido
mv /etc/sudoers.d/incluido /etc/sudoers-extra
printf '@include /etc/sudoers-extra\n' >/etc/sudoers.d/ward-teste
group_case "sudoers por @include com #<gid>" "sudoers"
printf 'subid: sss\n' >>/etc/nsswitch.conf
group_case "nsswitch com subid: sss" "subid"
printf 'sudoers: files ldap\n' >>/etc/nsswitch.conf
group_case "nsswitch com sudoers: files ldap" "sudoers"
# Phase 51b: groups coming from another NSS source (sss, ldap…) outside /etc/group.
sed -i '/^group:/d' /etc/nsswitch.conf
printf 'group: files sss\n' >>/etc/nsswitch.conf
group_case "nsswitch com group: files sss" "busca 'group'"
printf 'initgroups: files ldap\n' >>/etc/nsswitch.conf
group_case "nsswitch com initgroups: files ldap" "busca 'initgroups'"
check "V9.11 (51a) os grupos recém-criados e recusados foram apagados" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
rm -rf /usr/local/lib/ward

# --- Phase 51a: another Chrome policy with the names of Ward's -> refusal ----------------
install -d -m 0755 /etc/opt/chrome/policies/managed
printf '{"NativeMessagingUserLevelHosts": true}\n' >/etc/opt/chrome/policies/managed/outra.json
r="$(install_ward 2>&1)" && failed "política conflitante: instalou" || ok "política conflitante (NativeMessagingUserLevelHosts em outro arquivo): recusada"
check "política conflitante: mensagem" "$r" "NativeMessagingUserLevelHosts"
check "política conflitante: nenhum arquivo do Ward" "$(no_ward_file)" "nada"
check "política conflitante: nenhum grupo criado" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
rm -f /etc/opt/chrome/policies/managed/outra.json

# --- V9.2: test-mode artifact -> refusal before any group and file -------------------
r="$(install_ward BIN_WARDD="$B/wardd-test-mode" 2>&1)" && failed "V9.2: instalou o wardd de teste" || ok "V9.2: wardd com test-mode recusado"
check "V9.2: mensagem" "$r" "compilado com test-mode"
check "V9.2: nenhum arquivo do Ward" "$(no_ward_file)" "nada"
check "V9.2: nenhum grupo criado" "$(getent group ward-app ward-nm || echo ausente)" "ausente"

# --- real install ---------------------------------------------------------------------
r="$(install_ward 2>&1)" || { printf '%s\n' "$r"; failed "make install"; }
ok "make install (root, sem DESTDIR) saiu 0"
# V9.1 (as root): owner, group and mode of each path of tree.txt.
while read -r t m owner group c _; do
    [[ -z "$t" || "$t" == \#* ]] && continue
    actual="$(stat -c '%U %G %a' -- "/$c")"
    expected="$owner $group $((8#$m))"
    [ "$(stat -c '%U %G' -- "/$c") $((8#$(stat -c %a -- "/$c")))" = "$expected" ] || failed "V9.1: /$c é '$actual' (esperado $owner $group $m)"
    [ $((8#$(stat -c %a -- "/$c") & 8#022)) = 0 ] || failed "V9.1: /$c com escrita para grupo ou outros"
done <"$T/repo/install/tree.txt"
ok "V9.1: donos e modos de tree.txt (root:root, ward-app e ward-nm root:<grupo> 2755)"
check "V9.6: manifesto de native messaging do root" "$(stat -c '%U %a' /etc/opt/chrome/native-messaging-hosts/br.ward.nm.json)" "root 644"
check "V9.6: allowed_origins só com o id" "$(grep allowed_origins /etc/opt/chrome/native-messaging-hosts/br.ward.nm.json)" "[\"chrome-extension://$ID/\"]"
check "Fase 51a/44c: política com NativeMessagingUserLevelHosts, PasswordManagerEnabled e PasswordLeakDetectionEnabled false" "$(python3 -I -c 'import json,sys; p=json.load(open(sys.argv[1])); print(p["NativeMessagingUserLevelHosts"] is False and p["PasswordManagerEnabled"] is False and p["PasswordLeakDetectionEnabled"] is False, sorted(p))' /etc/opt/chrome/policies/managed/ward.json)" "True ['ExtensionSettings', 'NativeMessagingUserLevelHosts', 'PasswordLeakDetectionEnabled', 'PasswordManagerEnabled']"
groups_mark="$(for g in ward-app ward-nm; do echo "$g $(getent group "$g" | cut -d: -f3)"; done)"
check "Fase 51a: marca dos grupos criados (nome e gid)" "$(sort /usr/local/lib/ward/created-groups)" "$groups_mark"
# Phase 51c (S2, S8): without apport and libwebkit2gtk in the image, the folders are
# created and the list comes out with the multiarch paths; systemd reads the drop-in of systemd-coredump.
check "51c: sem o libwebkit2gtk-4.1-0, a instalação avisa" "$r" "libwebkit2gtk-4.1-0 ($(dpkg --print-architecture)) não está instalado"
case "$(dpkg --print-architecture)" in arm64) MA=aarch64-linux-gnu ;; *) MA=x86_64-linux-gnu ;; esac
for l in report-ignore blacklist.d; do
    check "51c: /etc/apport/$l/ward com o WebKitWebProcess" "$(cat "/etc/apport/$l/ward")" "/usr/lib/$MA/webkit2gtk-4.1/WebKitWebProcess"
    check "51c: /etc/apport/$l/ward com o ward-app" "$(cat "/etc/apport/$l/ward")" "/usr/local/bin/ward-app"
done
cc="$(systemd-analyze cat-config systemd/coredump.conf 2>&1 || true)"
check "51c: systemd-analyze cat-config lê o drop-in do Ward" "$cc" "# /etc/systemd/coredump.conf.d/ward.conf"
check "51c: Storage=none" "$cc" "Storage=none"
check "51c: ProcessSizeMax=0" "$cc" "ProcessSizeMax=0"
check "Fase 51a: marca root 0644" "$(stat -c '%U %a' /usr/local/lib/ward/created-groups)" "root 644"

# V9.11: the created groups.
for g in ward-app ward-nm; do
    IFS=: read -r _ _ gid members <<<"$(getent group "$g")"
    if [ "$gid" -gt 0 ] && [ "$gid" -lt 1000 ]; then ok "V9.11 $g é de sistema (gid $gid)"; else failed "V9.11 $g gid $gid"; fi
    check "V9.11 $g sem membros" "[$members]" "[]"
    IFS=: read -r _ password admins gmembers <<<"$(grep "^$g:" /etc/gshadow)"
    check "V9.11 $g com '!' no gshadow" "$password" "!"
    check "V9.11 $g sem administradores nem membros no gshadow" "[$admins][$gmembers]" "[][]"
    outside=1
    while IFS=: read -r _ start count; do
        [[ "$start" =~ ^[0-9]+$ ]] || continue
        if [ "$gid" -ge "$start" ] && [ "$gid" -lt $((start + count)) ]; then outside=0; fi
    done </etc/subgid
    [ "$outside" = 1 ] && ok "V9.11 $g fora das faixas do /etc/subgid" || failed "V9.11 $g dentro do /etc/subgid"
done
# newgrp by someone outside the group: fails (locked password).
r="$(as_1000 newgrp ward-app </dev/null 2>&1; echo "saiu=$?")"
lacks "V9.11 newgrp ward-app pelo uid $UID_T falha" "$r" "saiu=0"
r="$(as_1000 sg ward-app -c 'id -G' </dev/null 2>&1; echo "saiu=$?")"
lacks "V9.11 sg ward-app pelo uid $UID_T falha" "$r" "saiu=0"
lacks "V9.11 o uid $UID_T não ganha o gid de ward-app" "$r" " $(getent group ward-app | cut -d: -f3)"

# V9.7: socket active and enabled, root's folders, the uid's socket.
check "V9.7 wardd@$UID_T.socket habilitado" "$(systemctl is-enabled "wardd@$UID_T.socket" || true)" "enabled"
check "V9.7 wardd@$UID_T.socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
check "V9.7 serviço ainda parado (sobe na primeira conexão)" "$(systemctl is-active "wardd@$UID_T.service" || true)" "inactive"
check "V9.7 /run/ward root:root 0755 (tmpfiles.d)" "$(stat -c '%U:%G %a' /run/ward)" "root:root 755"
check "V9.7 /run/ward/$UID_T root:root 0755" "$(stat -c '%U:%G %a' "/run/ward/$UID_T")" "root:root 755"
check "V9.7 socket do uid, 0600" "$(stat -c '%u %a %F' "$SOCK")" "$UID_T 600 socket"

# D57 / V9.11: units without no_new_privs or RestrictSUIDSGID.
r="$(systemctl show -p NoNewPrivileges -p RestrictSUIDSGID "wardd@$UID_T.service")"
check "V9.11 systemctl show: NoNewPrivileges=no" "$r" "NoNewPrivileges=no"
check "V9.11 systemctl show: RestrictSUIDSGID=no" "$r" "RestrictSUIDSGID=no"
sec="$(systemd-analyze security --no-pager "wardd@$UID_T.service" 2>&1 || true)"
for d in NoNewPrivileges= RestrictSUIDSGID=; do
    line="$(grep -F -- "$d" <<<"$sec" | head -n 1 || true)"
    check "V9.11 systemd-analyze security: $d não ligado" "$line" "✗"
done

# The first connection starts the wardd as the user (V9.7), with the peer uid 0 the CLI checks.
r="$(as_1000 "$B/cliente" "$SOCK" estado)"
check "V9.7 a primeira conexão sobe o wardd e ele responde" "$r" "resposta: status"
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
st="/proc/$pid/status"
check "V9.7 wardd roda com o uid" "$(awk '/^Uid:/{print $2,$3,$4,$5}' "$st")" "$UID_T $UID_T $UID_T $UID_T"
check "D57 wardd sem no_new_privs" "$(awk '/^NoNewPrivs:/{print $2}' "$st")" "0"
check "V9.4 core 0" "$(awk '/^Max core file size/{print $5, $6}' "/proc/$pid/limits")" "0 0"
check "D62 LimitMEMLOCK=infinity" "$(awk '/^Max locked memory/{print $4, $5}' "/proc/$pid/limits")" "unlimited unlimited"
# V9.4: non-dumpable: the wardd's /proc belongs to root and the uid itself cannot read the environ.
check "V9.4 /proc/<wardd>/environ do root (dumpable 0)" "$(stat -c %U "/proc/$pid/environ")" "root"
r="$(as_1000 cat "/proc/$pid/environ" 2>&1 >/dev/null || true)"
check "V9.4 o uid $UID_T não lê o environ do wardd" "$r" "Permission denied"
r="$(as_1000 cat "/proc/$pid/mem" 2>&1 >/dev/null || true)"
check "V9.4 o uid $UID_T não abre o mem do wardd" "$r" "Permission denied"

# V9.4: the attacker's `systemctl --user set-environment LD_PRELOAD=…` does not reach the
# system wardd. The user's manager is the real `user@1000.service`; since the container
# has neither logind nor a system bus, two container-only drop-ins (in /run) play the
# role of pam_systemd: they create /run/user/1000 and pass XDG_RUNTIME_DIR. The uid
# cannot even run `systemctl set-environment` on the system manager.
rm -f /tmp/ward-marca
r="$(as_1000 systemctl set-environment "LD_PRELOAD=$B/libmarca.so" 2>&1; echo "saiu=$?")"
lacks "V9.4 o uid $UID_T não muda o ambiente do gerente do sistema" "$r" "saiu=0"
lacks "V9.4 o gerente do sistema continua sem LD_PRELOAD" "$(systemctl show-environment)" "LD_PRELOAD"
install -d -m 0755 "/run/systemd/system/user-runtime-dir@$UID_T.service.d" "/run/systemd/system/user@$UID_T.service.d"
printf '[Service]\nExecStart=\nExecStart=/usr/bin/install -d -o %s -g %s -m 0700 /run/user/%s\nExecStop=\nExecStop=/usr/bin/rm -rf /run/user/%s\n' \
    "$UID_T" "$UID_T" "$UID_T" "$UID_T" >"/run/systemd/system/user-runtime-dir@$UID_T.service.d/sem-logind.conf"
printf '[Service]\nEnvironment=XDG_RUNTIME_DIR=/run/user/%s\n' "$UID_T" >"/run/systemd/system/user@$UID_T.service.d/sem-logind.conf"
systemctl daemon-reload
systemctl start "user@$UID_T.service" || true
check "V9.4 gerente do usuário (user@$UID_T.service) no ar" "$(systemctl is-active "user@$UID_T.service" || true)" "active"
user_manager() { as_1000 /usr/bin/env -i XDG_RUNTIME_DIR="/run/user/$UID_T" PATH=/usr/bin:/bin systemctl --user "$@"; }
r="$(user_manager set-environment "LD_PRELOAD=$B/libmarca.so" 2>&1; echo "saiu=$?")"
check "V9.4 o atacante põe LD_PRELOAD no gerente do usuário" "$r" "saiu=0"
check "V9.4 o gerente do usuário tem o LD_PRELOAD" "$(user_manager show-environment 2>&1 || true)" "LD_PRELOAD=$B/libmarca.so"
# Forces the wardd to start again: kills it and connects (with LD_PRELOAD in the client too).
kill -9 "$pid"
sleep 1.5
r="$(as_1000 env "LD_PRELOAD=$B/libmarca.so" "$B/cliente" "$SOCK" estado 2>&1)"
check "V9.4 depois de morto, a conexão sobe outro wardd" "$r" "resposta: status"
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
app_env="$(tr '\0' '\n' <"/proc/$pid/environ" | cut -d= -f1 | sort | tr '\n' ' ')"
lacks "V9.4 o wardd novo não tem LD_PRELOAD (environ lido como root: $app_env)" "$app_env" "LD_"
r="$(grep -c libmarca "/proc/$pid/maps" || true)"
check "V9.4 a lib do atacante não está no wardd" "maps=$r" "maps=0"
check "V9.4 o wardd novo também é não-dumpable" "$(stat -c %U "/proc/$pid/environ")" "root"
user_manager unset-environment LD_PRELOAD >/dev/null 2>&1 || true
systemctl stop "user@$UID_T.service" "user-runtime-dir@$UID_T.service" || true
rm -rf "/run/systemd/system/user-runtime-dir@$UID_T.service.d" "/run/systemd/system/user@$UID_T.service.d" "/run/user/$UID_T"
systemctl daemon-reload

# --- app role: the app opened by the daemon gets the role (D47, D57, D63; V9.11) -------
REPORT=/tmp/ward-app-relatorio.txt
rm -f "$REPORT"
install -d -o "$UID_T" -g "$UID_T" -m 0700 "/run/user/$UID_T"
rm -f "/run/user/$UID_T/bus" "/run/user/$UID_T"/wayland-*
as_1000 "$B/plataforma" listen "/run/user/$UID_T/wayland-1" >/dev/null 2>&1 &
W1=$!
as_1000 "$B/plataforma" listen "/run/user/$UID_T/bus" >/dev/null 2>&1 &
BUS=$!
for _ in $(seq 50); do [ -S "/run/user/$UID_T/wayland-1" ] && [ -S "/run/user/$UID_T/bus" ] && break; sleep 0.1; done
app_env="$(as_1000 "$B/plataforma" environment)"
since="$(date '+%Y-%m-%d %H:%M:%S')"
# The ward-app opened by the user (the menu one): asks the daemon to open the app and exits.
# shellcheck disable=SC2086
as_1000 /usr/bin/bash -c "/usr/bin/env -i $(echo $app_env) /usr/local/bin/ward-app; true"
for _ in $(seq 100); do grep -q '^depois ' "$REPORT" 2>/dev/null && break; sleep 0.1; done
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
r="$(grep '^erro ' "$REPORT" | head -n 1 || true)"
check "o ward-app aberto pelo usuário pede ao daemon e sai (pai não é o daemon)" "$r" "pedido ao daemon"
r="$(grep '^depois ' "$REPORT" | head -n 1 || true)"
check "V9.11 o daemon abriu o ward-app setgid e ele recebeu o papel app" "$r" "pai=$pid"
check "V9.11 o app largou o grupo depois de conectar" "$r" "gid=$UID_T,$UID_T,$UID_T,$UID_T"
check "V9.11 o app ficou não-dumpable" "$r" "dumpable=0"
APID="$(sed -E 's/^depois pid=([0-9]+) .*/\1/' <<<"$r")"
r="$(grep "^inicio pid=$APID " "$REPORT" | head -n 1 || true)"
GID_APP="$(getent group ward-app | cut -d: -f3)"
check "V9.11 o exec aplicou o setgid (units sem no_new_privs, D57)" "$r" "gid=$UID_T,$GID_APP,$GID_APP,$GID_APP"
journal="$(journalctl -u "wardd@$UID_T.service" --no-pager -q --since "$since" 2>/dev/null || true)"
lacks "V9.11 o daemon não recusou o papel app ao app que ele abriu" "$journal" "papel app recusado (pid $APID)"
systemctl stop "wardd@$UID_T.socket" "wardd@$UID_T.service"
kill "$W1" "$BUS" 2>/dev/null || true
wait "$W1" "$BUS" 2>/dev/null || true

# --- V9.3: copied wardd and fake server of the uid: the CLI refuses before the password ---
# (the system socket is stopped since the end of the case above)
install -d -o "$UID_T" -g "$UID_T" -m 0700 "/run/user/$UID_T/ward" "$HOME_T/bin" /tmp/v93
as_1000 cp /usr/local/bin/wardd "$HOME_T/bin/wardd"
r="$(as_1000 /usr/bin/timeout 10 /usr/bin/env -i HOME="$HOME_T" PATH=/usr/bin:/bin XDG_RUNTIME_DIR="/run/user/$UID_T" \
    "$HOME_T/bin/wardd" 2>&1; echo "saiu=$?")"
lacks "V9.3 o wardd copiado para ~/bin não sobe sem o systemd" "$r" "saiu=0"
lacks "V9.3 o wardd copiado não fica no ar" "$r" "saiu=124"
RECORD=/tmp/v93/registro
as_1000 touch "$RECORD"
as_1000 "$B/falso" "/run/user/$UID_T/ward/wardd.sock" "$RECORD" 30 >/tmp/v93/falso.out 2>&1 &
FPID=$!
for _ in $(seq 50); do grep -q pronto /tmp/v93/falso.out 2>/dev/null && break; sleep 0.1; done
check "V9.3 servidor falso do uid escutando em /run/user/$UID_T/ward" "$(cat /tmp/v93/falso.out)" "pronto"
PASSWORD="$(cat "$T/senha")"
r="$(printf '%s\n' "$PASSWORD" | as_1000 /usr/bin/env -i HOME="$HOME_T" PATH=/usr/bin:/bin LANG=C.UTF-8 \
    XDG_RUNTIME_DIR="/run/user/$UID_T" WARD_SOCKET="/run/user/$UID_T/ward/wardd.sock" \
    /usr/bin/script -qec "/usr/local/bin/ward unlock" /dev/null 2>&1; echo "saiu=$?")"
lacks "V9.3 ward unlock não sai 0" "$r" "saiu=0"
lacks "V9.3 ward unlock não pede a senha" "$r" "Senha mestra"
# m2:DT10: the refusal comes from the server check, not from an unknown subcommand.
lacks "V9.3 ward unlock é um subcomando conhecido" "$r" "comando desconhecido"
check "V9.3 o servidor falso não recebeu conexão nem byte" "[$(cat "$RECORD")]" "[]"
kill "$FPID" 2>/dev/null || true
wait "$FPID" 2>/dev/null || true
systemctl start "wardd@$UID_T.socket"

# --- reinstall on top and the user's vault -----------------------------------------------
for d in "$HOME_T/.local" "$HOME_T/.local/share" "$VAULT_T"; do
    install -d -m 0700 -o "$UID_T" -g "$UID_T" "$d"
done
install -m 0600 -o "$UID_T" -g "$UID_T" "$T/cofre.ward" "$VAULT_T/cofre.ward"
printf 'ward-state 2\nseen 0\nburned 0\nagent_failures 0\n' >"$VAULT_T/estado"
chown "$UID_T:$UID_T" "$VAULT_T/estado"
chmod 0600 "$VAULT_T/estado"
r="$(printf '%s' "$PASSWORD" | as_1000 "$B/cliente" "$SOCK" destravar)"
check "o wardd instalado destrava o cofre de teste" "$r" "resposta: ok"
before="$(cd "$VAULT_T" && sha256sum cofre.ward estado; stat -c '%U %a' cofre.ward)"
r="$(install_ward 2>&1)" || { printf '%s\n' "$r"; failed "reinstalar"; }
ok "reinstalar por cima sai 0 (grupos já existentes conferem)"
check "reinstalar mantém o socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
r="$(as_1000 "$B/cliente" "$SOCK" estado)"
check "depois de reinstalar, o wardd responde" "$r" "resposta: status"

# --- V9.10: uninstall ----------------------------------------------------------------
r="$(make -s -C "$T/repo" uninstall 2>&1)" || { printf '%s\n' "$r"; failed "make uninstall"; }
ok "make uninstall saiu 0"
check "V9.10 nenhum arquivo do Ward" "$(no_ward_file)" "nada"
for d in /usr/local/lib/ward /usr/local/share/ward /run/ward; do
    [ ! -e "$d" ] && ok "V9.10 $d apagada" || failed "V9.10 $d ficou"
done
lacks "51c: depois do uninstall, o systemd-coredump não lê mais o drop-in" "$(systemd-analyze cat-config systemd/coredump.conf 2>&1 || true)" "ward.conf"
check "V9.10 grupos apagados" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
check "V9.10 nenhuma unit wardd@ carregada" "[$(systemctl list-units --all --plain --no-legend 'wardd@*' || true)]" "[]"
lacks "V9.10 wardd@$UID_T.socket não está mais habilitado" "$(systemctl is-enabled "wardd@$UID_T.socket" 2>&1 || true)" "enabled"
check "V9.10 nenhum link em sockets.target.wants" "$(ls /etc/systemd/system/sockets.target.wants/ 2>/dev/null | grep -c wardd || true)" "0"
check "V9.10 o cofre do usuário intacto" "$(cd "$VAULT_T" && sha256sum cofre.ward estado; stat -c '%U %a' cofre.ward)" "$before"
r="$(make -s -C "$T/repo" uninstall 2>&1)" && ok "uninstall de novo sai 0" || { printf '%s\n' "$r"; failed "uninstall repetido"; }

# --- Phase 51a: a group that already existed (and passes D60) is not deleted on uninstall --
groupadd --system ward-app
admin_gid="$(getent group ward-app | cut -d: -f3)"
r="$(install_ward 2>&1)" || { printf '%s\n' "$r"; failed "instalar com ward-app do administrador"; }
check "grupo do administrador: aceito" "$r" "ward-app já existia e confere"
lacks "grupo do administrador: fora da marca" "$(cat /usr/local/lib/ward/created-groups)" "ward-app"
r="$(make -s -C "$T/repo" uninstall 2>&1)" || { printf '%s\n' "$r"; failed "uninstall"; }
check "uninstall: ward-app do administrador fica (mesmo gid)" "$(getent group ward-app | cut -d: -f3)" "$admin_gid"
check "uninstall: avisa que ficou" "$r" "não foi criado pelo Ward"
check "uninstall: ward-nm (criado pelo Ward) apagado" "$(getent group ward-nm || echo ausente)" "ausente"
[ ! -e /usr/local/lib/ward ] && ok "uninstall: /usr/local/lib/ward (com a marca) apagada" || failed "/usr/local/lib/ward ficou"
groupdel ward-app

# --- Phase 57 (m2:V5.7, SEG; m2:DT15, m2:DT16): 0.2.0 over the 0.1.4 `make install` ------
# 0.1.4 comes from /opt/teste/old (its installer from the v0.1.4 tag, its release binaries
# and fake CRX3 package from .dev/baseline/) and /opt/teste/legacy (the 0.1.4 vault and
# state, fake values of D15, the v1 client and the 0.1.4 names of the install).
O=$T/old
L=$T/legacy
legacy_name() { awk -v k="$1" '$1 == k {print $2; exit}' "$L/install-0.1.4.txt"; }
OLD_EXT_DIR="/$(legacy_name extension_dir)"
OLD_MARK="/$(legacy_name groups_mark)"
MARK=/usr/local/lib/ward/created-groups
# Lines of protocol-v1.jsonl (0.1.4 baseline): the agent's status and unlock requests, the
# `ok` reply and the status reply of a locked vault.
REQ_STATUS=1 REQ_UNLOCK=2 RESP_OK=37 RESP_LOCKED=38
legacy_call() { as_1000 /usr/bin/python3 -I "$L/legacy_client.py" "$SOCK" "$L/protocol-v1.jsonl" "$1" "$2"; }
# The 0.1.4 installer with its own interface (`make install`/`make uninstall` of the tag).
install_old() {
    /usr/bin/env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin LC_ALL=C WARD_UID="$UID_T" PREFIX=/usr/local DESTDIR= \
        "$(legacy_name var_extension_id)=$(cat "$O/id")" "$(legacy_name var_extension_crx)=$O/ward.crx" \
        BIN_WARD="$O/bin/ward" BIN_WARDD="$O/bin/wardd" BIN_NM="$O/bin/ward-nm" BIN_APP="$O/bin/ward-app" \
        "$O/repo/$(legacy_name script)" "$(legacy_name "action_$1")"
}
# The vault of the container's user = the 0.1.4 one (v4 body, `ward-estado 1` state).
plant_old_vault() {
    rm -rf -- "$VAULT_T"
    install -d -m 0700 -o "$UID_T" -g "$UID_T" "$VAULT_T"
    install -m 0600 -o "$UID_T" -g "$UID_T" "$L/vault-0.1.4.ward" "$VAULT_T/cofre.ward"
    install -m 0600 -o "$UID_T" -g "$UID_T" "$L/state-0.1.4" "$VAULT_T/estado"
}
vault_sums() { (cd "$VAULT_T" && sha256sum cofre.ward estado; stat -c '%U %a' cofre.ward estado); }
group_lines() { for g in ward-app ward-nm; do getent group "$g" || echo "$g ausente"; done; }
# m2:DT16: "trocado" if the binary of the live process `$1` was replaced (its
# /proc/<pid>/exe says "(deleted)") before it ended; "parado" when it ends. Polls every 10 ms.
watch_swap() {
    local pid="$1" out="$2" exe
    : >"$out"
    while [ -d "/proc/$pid" ]; do
        exe="$(readlink "/proc/$pid/exe" 2>/dev/null || true)"
        if [[ "$exe" == *' (deleted)' ]]; then
            echo "trocado com o processo no ar" >>"$out"
            break
        fi
        sleep 0.01
    done
    echo "parado" >>"$out"
}
# Control: the watcher sees a binary swapped under a live process.
install -d -m 0700 /tmp/v57
cp /usr/bin/sleep /tmp/v57/sleep
/tmp/v57/sleep 30 &
CTL=$!
sleep 0.2
watch_swap "$CTL" /tmp/v57/control &
CTL_WATCH=$!
sleep 0.2
cp /usr/bin/sleep /tmp/v57/sleep.new && mv -f /tmp/v57/sleep.new /tmp/v57/sleep
sleep 0.2
kill "$CTL" 2>/dev/null || true
wait "$CTL" 2>/dev/null || true
wait "$CTL_WATCH" || true
check "m2:V5.7 controle: o vigia vê um binário trocado com o processo no ar" "$(cat /tmp/v57/control)" "trocado com o processo no ar"

clean_groups
rm -rf /usr/local/lib/ward /usr/local/share/ward
plant_old_vault
r="$(install_old install 2>&1)" || { printf '%s\n' "$r"; failed "m2:V5.7: make install da 0.1.4"; }
check "m2:V5.7: a 0.1.4 instalou a extensão no caminho dela" "$(cat "$OLD_EXT_DIR/update.xml" 2>&1)" "$(cat "$O/id")"
OLD_MARK_LINES="$(sort "$OLD_MARK" 2>/dev/null || true)"
check "m2:V5.7: a 0.1.4 gravou a marca dos grupos no nome dela" "$OLD_MARK_LINES" \
    "$(for g in ward-app ward-nm; do echo "$g $(getent group "$g" | cut -d: -f3)"; done)"
OLD_GROUPS="$(group_lines)"
cmp -s "$O/bin/wardd" /usr/local/bin/wardd && ok "m2:V5.7: /usr/local/bin/wardd é o 0.1.4" || failed "m2:V5.7: wardd instalado não é o 0.1.4"
check "m2:V5.7: o wardd 0.1.4 responde ao cliente v1 (cofre travado)" "$(legacy_call "$REQ_STATUS" "$RESP_LOCKED")" "same"
check "m2:V5.7: o wardd 0.1.4 destrava o cofre 0.1.4" "$(legacy_call "$REQ_UNLOCK" "$RESP_OK")" "same"
old_pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
check "m2:V5.7: wardd 0.1.4 no ar" "$(cmp -s "/proc/$old_pid/exe" "$O/bin/wardd" && echo 0.1.4)" "0.1.4"
VAULT_BEFORE="$(vault_sums)"
watch_swap "$old_pid" /tmp/v57/order &
WATCH=$!
r="$(install_ward 2>&1)" || { printf '%s\n' "$r"; failed "m2:V5.7: make install da 0.2.0 por cima da 0.1.4"; }
wait "$WATCH" || true
order="$(cat /tmp/v57/order)"
check "m2:V5.7 (DT16): o wardd 0.1.4 terminou" "$order" "parado"
lacks "m2:V5.7 (DT16): o binário não foi trocado com o wardd 0.1.4 no ar" "$order" "trocado"
stop_line="$(grep -n "parando wardd@$UID_T.service antes de trocar os binários" <<<"$r" | head -n 1 | cut -d: -f1)"
files_line="$(grep -n '^==> arquivos em' <<<"$r" | head -n 1 | cut -d: -f1)"
[ -n "$stop_line" ] && [ -n "$files_line" ] && [ "$stop_line" -lt "$files_line" ] &&
    ok "m2:V5.7 (DT16): a instalação parou o wardd@$UID_T antes dos arquivos" || failed "m2:V5.7 (DT16): sem parada antes dos arquivos ($stop_line/$files_line)"
[ ! -e "$OLD_EXT_DIR" ] && [ ! -L "$OLD_EXT_DIR" ] && ok "m2:V5.7 (DT15): a pasta da extensão da 0.1.4 sumiu" || failed "m2:V5.7: $OLD_EXT_DIR ficou"
[ ! -e "$OLD_MARK" ] && ok "m2:V5.7 (DT15): a marca no nome da 0.1.4 sumiu" || failed "m2:V5.7: $OLD_MARK ficou"
check "m2:V5.7 (DT15): created-groups com os grupos que a 0.1.4 criou" "$(sort "$MARK" 2>&1)" "$OLD_MARK_LINES"
check "m2:V5.7 (DT15): created-groups root 0644" "$(stat -c '%U %a' "$MARK")" "root 644"
check "m2:V5.7: os grupos não mudam de gid" "$(group_lines)" "$OLD_GROUPS"
check "m2:V5.7: ward-app novo root:ward-app 2755" "$(stat -c '%U:%G %a' /usr/local/bin/ward-app)" "root:ward-app 2755"
check "m2:V5.7: ward-nm novo root:ward-nm 2755" "$(stat -c '%U:%G %a' /usr/local/lib/ward/ward-nm)" "root:ward-nm 2755"
cmp -s "$B/ward-app" /usr/local/bin/ward-app && cmp -s "$B/wardd" /usr/local/bin/wardd && cmp -s "$B/ward-nm" /usr/local/lib/ward/ward-nm &&
    ok "m2:V5.7: binários da 0.2.0 no lugar" || failed "m2:V5.7: binário da 0.2.0 fora do lugar"
check "m2:V5.6: a política aponta para o caminho novo" "$(grep update_url /etc/opt/chrome/policies/managed/ward.json)" "file:///usr/local/share/ward/extension/update.xml"
check "m2:V5.7: o socket religou" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
check "m2:V5.7: nenhum wardd no ar antes da primeira conexão" "$(systemctl is-active "wardd@$UID_T.service" || true)" "inactive"
check "m2:V5.7: o cofre não foi tocado pela instalação" "$(vault_sums)" "$VAULT_BEFORE"
r="$(as_1000 "$B/cliente" "$SOCK" estado)"
check "m2:V5.7: a primeira conexão sobe o wardd 0.2.0" "$r" "resposta: status"
new_pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
[ "$new_pid" != "$old_pid" ] && cmp -s "/proc/$new_pid/exe" "$B/wardd" && ok "m2:V5.7: o wardd no ar é o 0.2.0" || failed "m2:V5.7: o wardd no ar não é o 0.2.0"
lacks "m2:V5.7: o wardd 0.2.0 não atende o cliente v1" "$(legacy_call "$REQ_STATUS" "$RESP_LOCKED")" "same"
check "m2:V5.7: o cofre continua intacto até destravar" "$(vault_sums)" "$VAULT_BEFORE"
r="$(printf '%s\n' "$PASSWORD" | as_1000 /usr/bin/env -i HOME="$HOME_T" PATH=/usr/bin:/bin LANG=C.UTF-8 \
    XDG_RUNTIME_DIR="/run/user/$UID_T" /usr/bin/script -qec "/usr/local/bin/ward unlock" /dev/null 2>&1; echo "saiu=$?")"
check "m2:V5.7: \`ward unlock\` 0.2.0 destrava o cofre 0.1.4" "$r" "cofre destravado"
check "m2:V5.7: \`ward unlock\` sai 0" "$r" "saiu=0"
lacks "m2:V5.7: o destrave migra o cofre (arquivo regravado)" "$(vault_sums)" "$(head -n 1 <<<"$VAULT_BEFORE")"
check "m2:V5.7: estado no formato novo depois do destrave" "$(head -n 1 "$VAULT_T/estado")" "ward-state 2"
check "m2:V5.7: cofre migrado continua 0600 do usuário" "$(stat -c '%U %a' "$VAULT_T/cofre.ward")" "$TEST_USER 600"

# --- Phase 57 (m2:V5.8, SEG): 0.2.0 uninstall over a 0.1.4 install (only the old mark) ----
r="$(make -s -C "$T/repo" uninstall 2>&1)" || { printf '%s\n' "$r"; failed "m2:V5.8: limpar"; }
groupadd --system ward-app
admin_gid="$(getent group ward-app | cut -d: -f3)"
plant_old_vault
r="$(install_old install 2>&1)" || { printf '%s\n' "$r"; failed "m2:V5.8: make install da 0.1.4"; }
check "m2:V5.8: a 0.1.4 aceita o ward-app do administrador" "$r" "ward-app"
lacks "m2:V5.8: marca da 0.1.4 sem o ward-app do administrador" "$(cat "$OLD_MARK" 2>&1)" "ward-app"
check "m2:V5.8: marca da 0.1.4 com o ward-nm que ela criou" "$(cat "$OLD_MARK" 2>&1)" "ward-nm $(getent group ward-nm | cut -d: -f3)"
[ ! -e "$MARK" ] && ok "m2:V5.8: só a marca no nome da 0.1.4" || failed "m2:V5.8: created-groups antes da 0.2.0"
VAULT_BEFORE="$(vault_sums)"
r="$(make -s -C "$T/repo" uninstall 2>&1)" || { printf '%s\n' "$r"; failed "m2:V5.8: make uninstall da 0.2.0"; }
check "m2:V5.8: o ward-app do administrador fica (mesmo gid)" "$(getent group ward-app | cut -d: -f3)" "$admin_gid"
check "m2:V5.8: avisa que o ward-app ficou" "$r" "não foi criado pelo Ward"
check "m2:V5.8: o ward-nm da marca antiga é apagado" "$(getent group ward-nm || echo ausente)" "ausente"
[ ! -e "$OLD_MARK" ] && [ ! -e "$MARK" ] && ok "m2:V5.8: as duas marcas apagadas" || failed "m2:V5.8: marca ficou"
for d in "$OLD_EXT_DIR" /usr/local/share/ward /usr/local/lib/ward; do
    [ ! -e "$d" ] && ok "m2:V5.8: $d apagada" || failed "m2:V5.8: $d ficou"
done
check "m2:V5.8: nenhum arquivo do Ward" "$(no_ward_file)" "nada"
check "m2:V5.8: o cofre do usuário intacto" "$(vault_sums)" "$VAULT_BEFORE"
groupdel ward-app
rm -rf /tmp/v57

if [ "$failures" -ne 0 ]; then
    echo "test-install (contêiner): $failures falha(s)"
    exit 1
fi
echo "test-install (contêiner): tudo ok"
