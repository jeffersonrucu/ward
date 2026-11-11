#!/bin/bash -p
# Inside part of `make test-deb` (Phase 52; V9.8, V9.9, V9.10, V9.11 through the `.deb`;
# D46, D47, D60). Runs as root **only inside** the disposable container with systemd and
# without network; from the host comes only the read-only test mount under /opt: the
# `.deb` built by `make deb` (artifacts without `test-mode`; the `ward-app` is
# `startup-container`), the `tree.txt`, the minimal client and the id of the fake extension
# package. The sshpass package is in /opt/debs (downloaded in the image build). Phase 57
# (m2:V5.7, m2:V5.9): also the 0.1.4 `.deb` of .dev/baseline/, the 0.1.4 vault and state,
# the v1 client and the test master password (fake, D15) for the upgrade. Each check
# prints "ok:" or the failure marker; any failure exits ≠ 0.

export PATH=/usr/sbin:/usr/bin:/sbin:/bin
export LC_ALL=C.UTF-8
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
equal() {
    if [ "$2" = "$3" ]; then ok "$1"; else
        failed "$1"
        diff <(printf '%s\n' "$2") <(printf '%s\n' "$3") || true
    fi
}

for _ in $(seq 1 100); do
    sys_state="$(systemctl is-system-running 2>/dev/null || true)"
    [[ "$sys_state" == running || "$sys_state" == degraded ]] && break
    sleep 0.1
done
check "systemd de pé" "$sys_state" "r"

T=/opt/teste
DEB=$T/ward.deb
UID_T=1000
TEST_USER="$(getent passwd "$UID_T" | cut -d: -f1)"
HOME_T="$(getent passwd "$UID_T" | cut -d: -f6)"
# Folders of the container's user (not the host's), with the name built in parts so the
# V1.5 scan (`dev/tests/attacks-foundation.sh`) does not mistake it for the real home.
VAULT_T="$HOME_T/.local/share/w""ard"
SSH_T="$HOME_T/.s""sh"
SSH_AGENT_VAR="SSH_AUTH""_SOCK"
ID="$(cat "$T/id")"
SOCK="/run/ward/$UID_T/wardd.sock"
as_1000() { setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups "$@"; }
install -d -o "$UID_T" -g "$UID_T" -m 0750 "$HOME_T"

FILES=()
while read -r t _ _ _ c _; do
    [[ -z "$t" || "$t" == \#* ]] && continue
    [ "$t" = f ] && FILES+=("/$c")
done <"$T/tree.txt"
no_ward_file() {
    local c
    for c in "${FILES[@]}"; do
        if [ -e "$c" ] || [ -L "$c" ]; then echo "$c"; return; fi
    done
    echo "nada"
}
status() { dpkg-query -W -f='${db:Status-Abbrev}' "$1" 2>/dev/null || echo "ausente"; }
overrides() { dpkg-statoverride --list | grep -E ' /usr/local/(bin/ward-app|lib/ward/ward-nm)$' | sort || true; }
clean_groups() {
    userdel fulano 2>/dev/null || true
    for g in ward-app ward-nm; do groupdel "$g" 2>/dev/null || true; done
}

# --- Phase 51c (S2, S8): crash of the WebKitGTK helpers --------------------------------
# kernel.core_pattern belongs to the host (it has no namespace): the test never writes it
# and calls apport and systemd-coredump by hand, as the kernel would, with a fake core
# (random bytes) of a `sleep` of uid 1000. Before anything: /proc/sys read-only.
if [ -w /proc/sys/kernel/core_pattern ] || [ -w /proc/sys/fs/suid_dumpable ]; then
    echo "FALHOU: /proc/sys gravável no contêiner; o teste pararia de proteger o core_pattern do host"
    exit 1
fi
ok "51c: /proc/sys só leitura (o core_pattern do host fica intacto)"
mapfile -t AUX_WEBKIT < <(dpkg -L libwebkit2gtk-4.1-0 | grep -E '/webkit2gtk-4\.1/WebKit[A-Za-z]*Process$' | sort)
[ "${#AUX_WEBKIT[@]}" -ge 3 ] && ok "51c: ${#AUX_WEBKIT[@]} auxiliares no libwebkit2gtk-4.1-0 (${AUX_WEBKIT[*]##*/})" ||
    failed "51c: auxiliares do WebKitGTK não achados (${AUX_WEBKIT[*]})"
WEBPROC="$(printf '%s\n' "${AUX_WEBKIT[@]}" | grep '/WebKitWebProcess$' || true)"
CORE=/tmp/nucleo-falso
head -c 65536 /dev/urandom >"$CORE"
install -d -m 3777 /var/crash
install -d -m 0755 /var/lib/systemd/coredump
setpriv --reuid="$UID_T" --regid="$UID_T" --clear-groups sleep 3600 &
TARGET=$!
sleep 0.3
# crash_apport <executable>: the written marker if apport wrote the report to /var/crash
# (readable by the user: dump mode 1, the one of the dumpable helpers), the nothing marker
# otherwise. The core limit is 0, like the RLIMIT_CORE the app passes to the helpers.
crash_apport() {
    local exe="$1" report
    report="/var/crash/${exe//\//_}.$UID_T.crash"
    rm -f -- "$report"
    /usr/share/apport/apport "-p$TARGET" -s11 -c0 -d1 "-P$TARGET" "-u$UID_T" "-g$UID_T" -- "${exe//\//!}" <"$CORE" >/dev/null 2>&1 || true
    if [ -e "$report" ]; then echo "gravado $(stat -c %U -- "$report")"; rm -f -- "$report"; else echo "nada"; fi
}
# crash_coredump: how many cores systemd-coredump kept. The limit passed is the fixed one
# of Ubuntu's /usr/lib/sysctl.d/50-coredump.conf (it ignores RLIMIT_CORE).
SOCK_CD=/run/systemd/coredump
systemd-socket-activate --seqpacket -a -l "$SOCK_CD" /opt/coredump/usr/lib/systemd/systemd-coredump >/tmp/coredump.log 2>&1 &
ACTIVATOR=$!
for _ in $(seq 1 50); do [ -S "$SOCK_CD" ] && break; sleep 0.1; done
crash_coredump() {
    rm -f /var/lib/systemd/coredump/core.*
    /opt/coredump/usr/lib/systemd/systemd-coredump "$TARGET" "$UID_T" "$UID_T" 11 "$(date +%s)" 9223372036854775808 conteiner 1 <"$CORE" >/dev/null 2>&1 || true
    sleep 1
    local n
    n="$(find /var/lib/systemd/coredump -maxdepth 1 -name 'core.*' | wc -l)"
    rm -f /var/lib/systemd/coredump/core.*
    echo "cores=$n"
}
# Control without Ward: the failure of the M5 audit reproduced (report of the user's
# WebKitWebProcess, even with limit 0; core kept by systemd-coredump).
check "51c controle sem o Ward: o apport grava o crash do WebKitWebProcess (legível pelo usuário)" "$(crash_apport "$WEBPROC")" "gravado $TEST_USER"
check "51c controle sem o Ward: o systemd-coredump guarda o core" "$(crash_coredump)" "cores=1"

# --- V9.8: what Ward must not touch, before anything --------------------------------------
# Fake personal key (generated here, discarded with the container), the user's config and
# known_hosts; /etc/ssh of openssh-client; the environment of the user's manager with an
# SSH agent (gnome-keyring's in the real session).
install -d -o "$UID_T" -g "$UID_T" -m 0700 "$SSH_T"
as_1000 ssh-keygen -q -t ed25519 -N '' -C falso-ward-teste -f "$SSH_T/id_ed25519"
printf 'Host exemplo\n    HostName 127.0.0.1\n    User fulano\n' | as_1000 tee "$SSH_T/config" >/dev/null
as_1000 touch "$SSH_T/known_hosts"
install -d -m 0755 "/run/systemd/system/user-runtime-dir@$UID_T.service.d" "/run/systemd/system/user@$UID_T.service.d"
printf '[Service]\nExecStart=\nExecStart=/usr/bin/install -d -o %s -g %s -m 0700 /run/user/%s\nExecStop=\nExecStop=/usr/bin/rm -rf /run/user/%s\n' \
    "$UID_T" "$UID_T" "$UID_T" "$UID_T" >"/run/systemd/system/user-runtime-dir@$UID_T.service.d/sem-logind.conf"
printf '[Service]\nEnvironment=XDG_RUNTIME_DIR=/run/user/%s\n' "$UID_T" >"/run/systemd/system/user@$UID_T.service.d/sem-logind.conf"
systemctl daemon-reload
systemctl start "user@$UID_T.service" || true
user_manager() { as_1000 /usr/bin/env -i XDG_RUNTIME_DIR="/run/user/$UID_T" PATH=/usr/bin:/bin systemctl --user "$@"; }
user_manager set-environment "$SSH_AGENT_VAR=/run/user/$UID_T/keyring/ssh"
ssh_snapshot() {
    {
        find /etc/ssh "$SSH_T" -printf '%p %U %G %m %y\n'
        find /etc/ssh "$SSH_T" -type f -exec sha256sum {} +
        user_manager show-environment
        as_1000 /usr/bin/env -i HOME="$HOME_T" PATH=/usr/bin:/bin ssh -G exemplo
    } 2>&1 | sort
}
SSH_BEFORE="$(ssh_snapshot)"
check "V9.8 retrato inicial com o agente SSH do usuário" "$SSH_BEFORE" "$SSH_AGENT_VAR=/run/user/$UID_T/keyring/ssh"

# --- V9.9: without sshpass dpkg does not configure the package -------------------------
check "sshpass ausente no começo" "$(status sshpass)" "ausente"
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
lacks "V9.9 dpkg -i sem sshpass não sai 0" "$r" "saiu=0"
check "V9.9 a falha cita a dependência de sshpass" "$r" "sshpass"
lacks "V9.9 o pacote não fica configurado" "$(status ward)" "ii"
# Unpacked and not configured on the first install: dpkg read the overrides database
# before the preinst, so the binary stays without setgid until the postinst.
check "desempacotado e não configurado: ward-app sem setgid (0755 root:root)" "$(stat -c '%U:%G %a' /usr/local/bin/ward-app)" "root:root 755"
check "desempacotado e não configurado: ward-nm sem setgid (0755 root:root)" "$(stat -c '%U:%G %a' /usr/local/lib/ward/ward-nm)" "root:root 755"
r="$(dpkg -r ward 2>&1; echo "saiu=$?")"
check "dpkg -r do pacote não configurado" "$r" "saiu=0"
check "dpkg -r: nenhum arquivo do Ward" "$(no_ward_file)" "nada"
check "dpkg -r: nenhum override" "[$(overrides)]" "[]"
check "dpkg -r: grupos apagados" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
r="$(dpkg -i /opt/debs/sshpass_*.deb 2>&1; echo "saiu=$?")"
check "sshpass instalado do pacote local" "$r" "saiu=0"

# --- V9.11 through the .deb: existing group outside D60 -> refusal without any file -----
group_case() {
    local label="$1" msg="$2" r
    r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
    lacks "V9.11 (.deb) $label: recusado" "$r" "saiu=0"
    check "V9.11 (.deb) $label: mensagem do preinst" "$r" "$msg"
    check "V9.11 (.deb) $label: nenhum arquivo do Ward (nem o setgid)" "$(no_ward_file)" "nada"
    lacks "V9.11 (.deb) $label: pacote não instalado" "$(status ward)" "ii"
    check "V9.11 (.deb) $label: nenhum override" "[$(overrides)]" "[]"
    clean_groups
    dpkg --purge ward >/dev/null 2>&1 || true
}
groupadd -g 1500 ward-app
group_case "gid >= 1000" "gid 1500"
groupadd --system ward-nm
gpasswd -a "$TEST_USER" ward-nm >/dev/null
group_case "com membro" "tem membros"
groupadd --system ward-app
groupmod -p 'falso-senha-de-grupo' ward-app
group_case "com senha no gshadow" "tem senha"
groupadd --system ward-app
useradd -M -N -g ward-app -s /usr/sbin/nologin fulano
group_case "grupo primário de um usuário" "grupo primário"
install -d -m 0755 /etc/security
printf '*;*;*;Al0000-2400;ward-nm\n' >>/etc/security/group.conf
group_case "pam_group (group.conf, Fase 51a)" "pam_group"
sed -i '/;ward-nm$/d' /etc/security/group.conf
# Phase 51b: groups coming from another NSS source (sss, ldap…) outside /etc/group.
cp /etc/nsswitch.conf /tmp/nsswitch.orig
sed -i '/^group:/d' /etc/nsswitch.conf
printf 'group: files sss\n' >>/etc/nsswitch.conf
group_case "nsswitch com group: files sss (Fase 51b)" "busca 'group'"
cp /tmp/nsswitch.orig /etc/nsswitch.conf

# --- Phase 51a: another Chrome policy with the names of Ward's -> the preinst refuses ----
install -d -m 0755 /etc/opt/chrome/policies/managed
printf '{"ExtensionSettings": {"*": {"installation_mode": "allowed"}}}\n' >/etc/opt/chrome/policies/managed/00-empresa.json
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
lacks "política conflitante: dpkg -i recusado" "$r" "saiu=0"
check "política conflitante: mensagem do preinst" "$r" "ExtensionSettings"
check "política conflitante: nenhum arquivo do Ward" "$(no_ward_file)" "nada"
check "política conflitante: nenhum grupo" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
check "política conflitante: o outro arquivo intacto" "$(cat /etc/opt/chrome/policies/managed/00-empresa.json)" "allowed"
rm -f /etc/opt/chrome/policies/managed/00-empresa.json
dpkg --purge ward >/dev/null 2>&1 || true

# --- install --------------------------------------------------------------------------
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
check "V9.9 dpkg -i (com SUDO_UID) sai 0" "$r" "saiu=0"
check "V9.9 pacote instalado" "$(status ward)" "ii"
check "postinst liga o socket do SUDO_UID" "$r" "wardd@$UID_T.socket ligado"
# dpkg -L: the files of tree.txt.
list="$(dpkg -L ward | sort)"
for c in "${FILES[@]}"; do
    grep -qx -- "$c" <<<"$list" || failed "dpkg -L sem $c"
done
ok "dpkg -L tem os ${#FILES[@]} arquivos de tree.txt"
# Owner, group and mode of each path of tree.txt (like make install).
failures_before=$failures
while read -r t m owner group c _; do
    [[ -z "$t" || "$t" == \#* ]] && continue
    actual="$(stat -c '%U %G' -- "/$c") $((8#$(stat -c %a -- "/$c")))"
    [ "$actual" = "$owner $group $((8#$m))" ] || failed "/$c é '$actual' (esperado $owner $group $m)"
    [ $((8#$(stat -c %a -- "/$c") & 8#022)) = 0 ] || failed "/$c com escrita para grupo ou outros"
done <"$T/tree.txt"
[ "$failures" = "$failures_before" ] && ok "donos e modos de tree.txt (ward-app e ward-nm root:<grupo> 2755)"
equal "overrides registrados" "$(overrides)" "$(printf 'root ward-app 2755 /usr/local/bin/ward-app\nroot ward-nm 2755 /usr/local/lib/ward/ward-nm')"
check "manifesto de native messaging com o id" "$(cat /etc/opt/chrome/native-messaging-hosts/br.ward.nm.json)" "chrome-extension://$ID/"
check "Fase 51a: política com NativeMessagingUserLevelHosts false" "$(grep -c '^  "NativeMessagingUserLevelHosts": false,$' /etc/opt/chrome/policies/managed/ward.json)" "1"
check "Fase 44c: política com PasswordManagerEnabled false" "$(grep -c '^  "PasswordManagerEnabled": false,$' /etc/opt/chrome/policies/managed/ward.json)" "1"
check "Fase 44c: política com PasswordLeakDetectionEnabled false" "$(grep -c '^  "PasswordLeakDetectionEnabled": false,$' /etc/opt/chrome/policies/managed/ward.json)" "1"
check "Fase 51a: marca dos grupos criados pelo preinst" "$(sort /usr/local/lib/ward/created-groups)" "$(for g in ward-app ward-nm; do echo "$g $(getent group "$g" | cut -d: -f3)"; done)"
# Phase 51c: the list covers each helper of the package (real path from dpkg -L), and the
# real apport and systemd-coredump write nothing; controls show it did not silence everything.
for l in report-ignore blacklist.d; do
    for c in "${AUX_WEBKIT[@]}" /usr/local/bin/ward-app /usr/local/lib/ward/ward-nm /usr/local/bin/wardd /usr/local/bin/ward; do
        grep -qxF -- "$c" "/etc/apport/$l/ward" || failed "51c: $c fora de /etc/apport/$l/ward"
    done
done
ok "51c: /etc/apport/{report-ignore,blacklist.d}/ward com os auxiliares do dpkg -L e os binários do Ward"
for c in "${AUX_WEBKIT[@]}"; do
    check "51c com o Ward: o apport não grava o crash de ${c##*/}" "$(crash_apport "$c")" "nada"
done
check "51c com o Ward: crash do ward-app sem relatório" "$(crash_apport /usr/local/bin/ward-app)" "nada"
check "51c controle com o Ward: o apport ainda grava o crash de outro programa (/usr/bin/sleep)" "$(crash_apport /usr/bin/sleep)" "gravado"
check "51c com o Ward: o systemd-coredump não guarda core" "$(crash_coredump)" "cores=0"
for g in ward-app ward-nm; do
    IFS=: read -r _ _ gid members <<<"$(getent group "$g")"
    if [ "$gid" -gt 0 ] && [ "$gid" -lt 1000 ]; then ok "V9.11 (.deb) $g é de sistema (gid $gid)"; else failed "V9.11 (.deb) $g gid $gid"; fi
    check "V9.11 (.deb) $g sem membros" "[$members]" "[]"
    check "V9.11 (.deb) $g com '!' no gshadow" "$(grep "^$g:" /etc/gshadow | cut -d: -f2)" "!"
done
r="$(as_1000 sg ward-app -c 'id -G' </dev/null 2>&1; echo "saiu=$?")"
lacks "V9.11 (.deb) sg ward-app pelo uid $UID_T falha" "$r" "saiu=0"

# V9.7: socket, folders and first connection.
check "V9.7 wardd@$UID_T.socket habilitado" "$(systemctl is-enabled "wardd@$UID_T.socket" || true)" "enabled"
check "V9.7 wardd@$UID_T.socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
check "V9.7 /run/ward root:root 0755" "$(stat -c '%U:%G %a' /run/ward)" "root:root 755"
check "V9.7 /run/ward/$UID_T root:root 0755" "$(stat -c '%U:%G %a' "/run/ward/$UID_T")" "root:root 755"
check "V9.7 socket do uid, 0600" "$(stat -c '%u %a %F' "$SOCK")" "$UID_T 600 socket"
r="$(as_1000 "$T/bin/cliente" "$SOCK" estado)"
check "V9.7 a primeira conexão sobe o wardd instalado pelo .deb" "$r" "resposta: status"
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
check "V9.7 wardd roda com o uid" "$(awk '/^Uid:/{print $2,$3,$4,$5}' "/proc/$pid/status")" "$UID_T $UID_T $UID_T $UID_T"
check "D57 wardd sem no_new_privs" "$(awk '/^NoNewPrivs:/{print $2}' "/proc/$pid/status")" "0"
check "V9.4 wardd não-dumpable (/proc do root)" "$(stat -c %U "/proc/$pid/environ")" "root"

# V9.8: nothing about SSH changed.
equal "V9.8 pasta do ssh do usuário, /etc/ssh, ssh -G e o ambiente do usuário iguais depois de instalar" "$SSH_BEFORE" "$(ssh_snapshot)"

# --- reinstall (upgrade) ----------------------------------------------------------------
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
check "reinstalar o .deb sai 0" "$r" "saiu=0"
check "reinstalar: ward-app continua root:ward-app 2755" "$(stat -c '%U:%G %a' /usr/local/bin/ward-app)" "root:ward-app 2755"
check "reinstalar: ward-nm continua root:ward-nm 2755" "$(stat -c '%U:%G %a' /usr/local/lib/ward/ward-nm)" "root:ward-nm 2755"
check "reinstalar: socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
r="$(as_1000 "$T/bin/cliente" "$SOCK" estado)"
check "reinstalar: o wardd responde" "$r" "resposta: status"
# Without SUDO_UID or WARD_UID (app store/PackageKit): turns on the socket of each human
# user (uid in the login.defs range, with a login shell) and of nobody else.
systemctl disable --now "wardd@$UID_T.socket" "wardd@$UID_T.service" >/dev/null 2>&1 || true
useradd -M -N -u 1500 -s /usr/sbin/nologin semshell
useradd -M -N -u 999 -s /bin/bash sistema
r="$(env -u SUDO_UID -u WARD_UID dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
check "sem SUDO_UID: dpkg -i sai 0" "$r" "saiu=0"
check "sem SUDO_UID: o postinst liga o socket do usuário humano" "$r" "wardd@$UID_T.socket ligado"
check "sem SUDO_UID: socket do usuário humano ligado" "$(systemctl is-enabled "wardd@$UID_T.socket" 2>&1 || true)" "enabled"
check "sem SUDO_UID: socket do usuário humano ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
for u in 1500 999 65534 0; do
    lacks "sem SUDO_UID: wardd@$u.socket não ligado" "$(systemctl is-enabled "wardd@$u.socket" 2>&1 || true)" "enabled"
done
r="$(as_1000 "$T/bin/cliente" "$SOCK" estado)"
check "sem SUDO_UID: o wardd responde" "$r" "resposta: status"
userdel semshell
userdel sistema
# An invalid WARD_UID turns nothing on.
r="$(WARD_UID=0 dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
check "WARD_UID=0: dpkg -i sai 0" "$r" "saiu=0"
check "WARD_UID=0: recusado pelo postinst" "$r" "não serve"
lacks "WARD_UID=0: wardd@0.socket não ligado" "$(systemctl is-enabled wardd@0.socket 2>&1 || true)" "enabled"

# --- V9.10 through the .deb: dpkg -r and dpkg -P -----------------------------------------
for d in "$HOME_T/.local" "$HOME_T/.local/share" "$VAULT_T"; do
    install -d -m 0700 -o "$UID_T" -g "$UID_T" "$d"
done
head -c 4096 /dev/urandom >"$VAULT_T/cofre.ward"
chown "$UID_T:$UID_T" "$VAULT_T/cofre.ward"
chmod 0600 "$VAULT_T/cofre.ward"
before="$(sha256sum "$VAULT_T/cofre.ward"; stat -c '%U %a' "$VAULT_T/cofre.ward")"
r="$(dpkg -r ward 2>&1; echo "saiu=$?")"
check "V9.10 dpkg -r sai 0" "$r" "saiu=0"
check "V9.10 nenhum arquivo do Ward" "$(no_ward_file)" "nada"
for d in /usr/local/lib/ward /usr/local/share/ward /run/ward; do
    [ ! -e "$d" ] && ok "V9.10 $d apagada" || failed "V9.10 $d ficou"
done
check "V9.10 grupos apagados" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
check "V9.10 nenhum override" "[$(overrides)]" "[]"
check "V9.10 nenhuma unit wardd@ carregada" "[$(systemctl list-units --all --plain --no-legend 'wardd@*' || true)]" "[]"
check "V9.10 nenhum link em sockets.target.wants" "$(ls /etc/systemd/system/sockets.target.wants/ 2>/dev/null | grep -c wardd || true)" "0"
check "V9.10 o cofre do usuário intacto" "$(sha256sum "$VAULT_T/cofre.ward"; stat -c '%U %a' "$VAULT_T/cofre.ward")" "$before"
equal "V9.8 SSH igual depois de remover" "$SSH_BEFORE" "$(ssh_snapshot)"
# Phase 51c: dpkg -r removes the list and the drop-in (apport's README stays).
for c in /etc/apport/report-ignore/ward /etc/apport/blacklist.d/ward /etc/systemd/coredump.conf.d/ward.conf; do
    [ ! -e "$c" ] && ok "51c dpkg -r: $c apagado" || failed "51c dpkg -r: $c ficou"
done
[ -e /etc/apport/report-ignore/README.denylist ] && ok "51c dpkg -r: o README do pacote apport fica" || failed "51c dpkg -r: README do apport apagado"
check "51c dpkg -r: o apport volta a gravar o crash do WebKitWebProcess" "$(crash_apport "$WEBPROC")" "gravado"
check "51c dpkg -r: o systemd-coredump volta a guardar" "$(crash_coredump)" "cores=1"
r="$(dpkg -P ward 2>&1; echo "saiu=$?")"
check "dpkg -P depois do -r sai 0" "$r" "saiu=0"
check "dpkg -P: pacote fora do banco do dpkg" "$(status ward)" "ausente"
check "dpkg -P: o cofre do usuário intacto" "$(sha256sum "$VAULT_T/cofre.ward"; stat -c '%U %a' "$VAULT_T/cofre.ward")" "$before"

# --- Phase 51a: a group that already existed (and passes D60) stays on dpkg -r -----------
groupadd --system ward-nm
admin_gid="$(getent group ward-nm | cut -d: -f3)"
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
check "grupo do administrador: dpkg -i sai 0" "$r" "saiu=0"
check "grupo do administrador: aceito pelo preinst" "$r" "ward-nm já existia e confere"
r="$(dpkg -r ward 2>&1; echo "saiu=$?")"
check "grupo do administrador: dpkg -r sai 0" "$r" "saiu=0"
check "dpkg -r: ward-nm do administrador fica (mesmo gid)" "$(getent group ward-nm | cut -d: -f3)" "$admin_gid"
check "dpkg -r: avisa que ficou" "$r" "não foi criado pelo Ward"
check "dpkg -r: ward-app (criado pelo preinst) apagado" "$(getent group ward-app || echo ausente)" "ausente"
[ ! -e /usr/local/lib/ward ] && ok "dpkg -r: /usr/local/lib/ward (com a marca) apagada" || failed "/usr/local/lib/ward ficou"
groupdel ward-nm
dpkg -P ward >/dev/null 2>&1 || true

# --- Phase 57 (m2:V5.7, m2:V5.9, SEG; m2:DT15, m2:DT16): the 0.2.0 .deb over the 0.1.4 one --
# /opt/teste/ward-0.1.4.deb is the 0.1.4 `.deb` of .dev/baseline/ (its release binaries and
# a fake CRX3); /opt/teste/legacy has the 0.1.4 vault and state (fake values of D15), the v1
# client and the 0.1.4 names of the install.
L=$T/legacy
OLD_DEB=$T/ward-0.1.4.deb
legacy_name() { awk -v k="$1" '$1 == k {print $2; exit}' "$L/install-0.1.4.txt"; }
OLD_EXT_DIR="/$(legacy_name extension_dir)"
OLD_MARK="/$(legacy_name groups_mark)"
MARK=/usr/local/lib/ward/created-groups
# Lines of protocol-v1.jsonl (0.1.4 baseline): the agent's status and unlock requests, the
# `ok` reply and the status reply of a locked vault.
REQ_STATUS=1 REQ_UNLOCK=2 RESP_OK=37 RESP_LOCKED=38
legacy_call() { as_1000 /usr/bin/python3 -I "$L/legacy_client.py" "$SOCK" "$L/protocol-v1.jsonl" "$1" "$2"; }
PASSWORD="$(cat "$T/senha")"
plant_old_vault() {
    rm -rf -- "$VAULT_T"
    install -d -m 0700 -o "$UID_T" -g "$UID_T" "$VAULT_T"
    install -m 0600 -o "$UID_T" -g "$UID_T" "$L/vault-0.1.4.ward" "$VAULT_T/cofre.ward"
    install -m 0600 -o "$UID_T" -g "$UID_T" "$L/state-0.1.4" "$VAULT_T/estado"
}
vault_sums() { (cd "$VAULT_T" && sha256sum cofre.ward estado; stat -c '%U %a' cofre.ward estado); }
group_lines() { for g in ward-app ward-nm; do getent group "$g" || echo "$g ausente"; done; }
mark_lines() { for g in ward-app ward-nm; do echo "$g $(getent group "$g" | cut -d: -f3)"; done; }
old_paths_in() { grep -x -F -e "$OLD_EXT_DIR" -e "$OLD_EXT_DIR/ward.crx" -e "$OLD_EXT_DIR/update.xml" -e "$OLD_MARK" <<<"$1" || true; }
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
install -d -m 0700 /tmp/v57
# Control: the watcher sees a binary swapped under a live process (dpkg also swaps by rename).
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
plant_old_vault
r="$(SUDO_UID=$UID_T dpkg -i "$OLD_DEB" 2>&1; echo "saiu=$?")"
check "m2:V5.9: o .deb 0.1.4 instala" "$r" "saiu=0"
check "m2:V5.9: dpkg -L da 0.1.4 tem a pasta antiga da extensão (controle)" "$(dpkg -L ward)" "$OLD_EXT_DIR/update.xml"
OLD_MARK_LINES="$(sort "$OLD_MARK" 2>/dev/null || true)"
check "m2:V5.7: o preinst 0.1.4 gravou a marca no nome dela" "$OLD_MARK_LINES" "$(mark_lines)"
OLD_GROUPS="$(group_lines)"
OLD_WARDD_SUM="$(sha256sum </usr/local/bin/wardd)"
check "m2:V5.7: o wardd 0.1.4 responde ao cliente v1 (cofre travado)" "$(legacy_call "$REQ_STATUS" "$RESP_LOCKED")" "same"
check "m2:V5.7: o wardd 0.1.4 destrava o cofre 0.1.4" "$(legacy_call "$REQ_UNLOCK" "$RESP_OK")" "same"
old_pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
check "m2:V5.7: wardd 0.1.4 no ar" "$(sha256sum <"/proc/$old_pid/exe")" "$OLD_WARDD_SUM"
VAULT_BEFORE="$(vault_sums)"
watch_swap "$old_pid" /tmp/v57/order &
WATCH=$!
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
wait "$WATCH" || true
check "m2:V5.9: o .deb 0.2.0 por cima do 0.1.4 sai 0" "$r" "saiu=0"
check "m2:V5.9: pacote configurado" "$(status ward)" "ii"
order="$(cat /tmp/v57/order)"
check "m2:V5.7 (DT16): o wardd 0.1.4 terminou" "$order" "parado"
lacks "m2:V5.7 (DT16): o dpkg não trocou o binário com o wardd 0.1.4 no ar" "$order" "trocado"
check "m2:V5.7 (DT16): o preinst parou o wardd@$UID_T" "$r" "parando wardd@$UID_T.service antes de trocar os binários"
check "m2:V5.9: dpkg -L depois da atualização sem caminho da 0.1.x" "[$(old_paths_in "$(dpkg -L ward)")]" "[]"
check "m2:V5.9: dpkg -L com a pasta nova da extensão" "$(dpkg -L ward)" "/usr/local/share/ward/extension/update.xml"
[ ! -e "$OLD_EXT_DIR" ] && [ ! -L "$OLD_EXT_DIR" ] && ok "m2:V5.7 (DT15): a pasta da extensão da 0.1.4 sumiu" || failed "m2:V5.7: $OLD_EXT_DIR ficou"
[ ! -e "$OLD_MARK" ] && ok "m2:V5.7 (DT15): a marca no nome da 0.1.4 sumiu" || failed "m2:V5.7: $OLD_MARK ficou"
check "m2:V5.7 (DT15): created-groups com os grupos que a 0.1.4 criou" "$(sort "$MARK" 2>&1)" "$OLD_MARK_LINES"
check "m2:V5.7: os grupos não mudam de gid" "$(group_lines)" "$OLD_GROUPS"
equal "m2:V5.9: statoverride 2755 dos binários novos" "$(overrides)" "$(printf 'root ward-app 2755 /usr/local/bin/ward-app\nroot ward-nm 2755 /usr/local/lib/ward/ward-nm')"
check "m2:V5.9: ward-app root:ward-app 2755" "$(stat -c '%U:%G %a' /usr/local/bin/ward-app)" "root:ward-app 2755"
check "m2:V5.9: ward-nm root:ward-nm 2755" "$(stat -c '%U:%G %a' /usr/local/lib/ward/ward-nm)" "root:ward-nm 2755"
check "m2:V5.6: a política aponta para o caminho novo" "$(grep update_url /etc/opt/chrome/policies/managed/ward.json)" "file:///usr/local/share/ward/extension/update.xml"
check "m2:V5.7: o socket religou" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
check "m2:V5.7: o cofre não foi tocado pela atualização" "$(vault_sums)" "$VAULT_BEFORE"
r="$(as_1000 "$T/bin/cliente" "$SOCK" estado)"
check "m2:V5.7: a primeira conexão sobe o wardd 0.2.0" "$r" "resposta: status"
new_pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
NEW_WARDD_SUM="$(sha256sum </usr/local/bin/wardd)"
[ "$NEW_WARDD_SUM" != "$OLD_WARDD_SUM" ] && [ "$(sha256sum <"/proc/$new_pid/exe")" = "$NEW_WARDD_SUM" ] &&
    ok "m2:V5.7: o wardd no ar é o 0.2.0" || failed "m2:V5.7: o wardd no ar não é o 0.2.0"
lacks "m2:V5.7: o wardd 0.2.0 não atende o cliente v1" "$(legacy_call "$REQ_STATUS" "$RESP_LOCKED")" "same"
r="$(printf '%s' "$PASSWORD" | as_1000 "$T/bin/cliente" "$SOCK" destravar)"
check "m2:V5.7: o wardd 0.2.0 destrava o cofre 0.1.4" "$r" "resposta: ok"
lacks "m2:V5.7: o destrave migra o cofre (arquivo regravado)" "$(vault_sums)" "$(head -n 1 <<<"$VAULT_BEFORE")"
check "m2:V5.7: estado no formato novo depois do destrave" "$(head -n 1 "$VAULT_T/estado")" "ward-state 2"
VAULT_MIGRATED="$(vault_sums)"
r="$(dpkg -r ward 2>&1; echo "saiu=$?")"
check "m2:V5.9: dpkg -r depois da atualização sai 0" "$r" "saiu=0"
check "m2:V5.9: dpkg -r apaga os grupos que a 0.1.4 criou" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
[ ! -e "$MARK" ] && [ ! -e "$OLD_MARK" ] && [ ! -e /usr/local/lib/ward ] && ok "m2:V5.9: dpkg -r apaga as marcas e /usr/local/lib/ward" || failed "m2:V5.9: marca ou /usr/local/lib/ward ficou"
r="$(dpkg -P ward 2>&1; echo "saiu=$?")"
check "m2:V5.9: dpkg -P sai 0" "$r" "saiu=0"
check "m2:V5.9: dpkg -r/-P deixam o cofre migrado intacto" "$(vault_sums)" "$VAULT_MIGRATED"

# --- Phase 57 (m2:DT15): 0.1.x leftovers no package owns (a 0.1.x `make install`) ---------
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
check "m2:DT15: instalar o .deb 0.2.0" "$r" "saiu=0"
MARK_LINES="$(sort "$MARK")"
mkdir -p -m 0755 "$OLD_EXT_DIR"
printf 'pacote 0.1.x\n' >"$OLD_EXT_DIR/ward.crx"
printf 'update 0.1.x\n' >"$OLD_EXT_DIR/update.xml"
mv "$MARK" "$OLD_MARK"
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
check "m2:DT15: reinstalar por cima das sobras da 0.1.x sai 0" "$r" "saiu=0"
check "m2:DT15: o postinst apaga a sobra que nenhum pacote tem" "$r" "removido $OLD_EXT_DIR/ward.crx (0.1.x)"
[ ! -e "$OLD_EXT_DIR" ] && ok "m2:DT15: a pasta da 0.1.x sumiu" || failed "m2:DT15: $OLD_EXT_DIR ficou"
check "m2:DT15: o preinst passou a marca antiga para created-groups" "$(sort "$MARK" 2>&1)" "$MARK_LINES"
[ ! -e "$OLD_MARK" ] && ok "m2:DT15: a marca antiga sumiu" || failed "m2:DT15: $OLD_MARK ficou"
# Attack: the 0.1.x extension dir by link to another dir: the preinst refuses before any
# change (no stop, no file), and the link target stays.
install -d -m 0755 /tmp/v57/victim
printf 'da vítima\n' >/tmp/v57/victim/ward.crx
ln -s /tmp/v57/victim "$OLD_EXT_DIR"
as_1000 "$T/bin/cliente" "$SOCK" estado >/dev/null
live_pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
lacks "m2:DT15 ataque: pasta da 0.1.x por link: dpkg -i recusado" "$r" "saiu=0"
check "m2:DT15 ataque: mensagem do preinst" "$r" "link simbólico"
check "m2:DT15 ataque: o alvo do link intacto" "$(cat /tmp/v57/victim/ward.crx)" "da vítima"
check "m2:DT15 ataque: recusado antes de parar o wardd" "$(systemctl show -p MainPID --value "wardd@$UID_T.service")" "$live_pid"
rm -f "$OLD_EXT_DIR"
dpkg -r ward >/dev/null 2>&1 || true
dpkg -P ward >/dev/null 2>&1 || true
clean_groups
rm -rf /tmp/v57

kill "$ACTIVATOR" "$TARGET" 2>/dev/null || true
rm -f /var/crash/*.crash /var/lib/systemd/coredump/core.* "$CORE"
user_manager unset-environment "$SSH_AGENT_VAR" >/dev/null 2>&1 || true
systemctl stop "user@$UID_T.service" "user-runtime-dir@$UID_T.service" || true

if [ "$failures" -ne 0 ]; then
    echo "test-deb (contêiner): $failures falha(s)"
    exit 1
fi
echo "test-deb (contêiner): tudo ok"
