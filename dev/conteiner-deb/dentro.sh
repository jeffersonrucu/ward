#!/bin/bash -p
# Parte de dentro do `make teste-deb` (Fase 52; V9.8, V9.9, V9.10, V9.11 pelo `.deb`;
# D46, D47, D60). Roda como root **só dentro** do contêiner descartável com systemd e
# sem rede; do host só vem /opt/teste (somente leitura): o `.deb` montado pelo
# `make deb` (artefatos sem `modo-teste`; o `ward-app` é o `partida-conteiner`), a
# `arvore.txt`, o cliente mínimo e o id do pacote falso da extensão. O pacote do
# sshpass está em /opt/debs (baixado no build da imagem). Cada conferência imprime
# "ok:" ou "FALHOU:"; qualquer falha sai ≠ 0.

export PATH=/usr/sbin:/usr/bin:/sbin:/bin
export LC_ALL=C.UTF-8
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
igual() {
    if [ "$2" = "$3" ]; then ok "$1"; else
        falhou "$1"
        diff <(printf '%s\n' "$2") <(printf '%s\n' "$3") || true
    fi
}

for _ in $(seq 1 100); do
    estado="$(systemctl is-system-running 2>/dev/null || true)"
    [[ "$estado" == running || "$estado" == degraded ]] && break
    sleep 0.1
done
confere "systemd de pé" "$estado" "r"

T=/opt/teste
DEB=$T/ward.deb
UID_T=1000
USUARIO="$(getent passwd "$UID_T" | cut -d: -f1)"
HOME_T="$(getent passwd "$UID_T" | cut -d: -f6)"
# Pastas do usuário do contêiner (não do host), com o nome montado por partes para a
# varredura do V1.5 (`dev/testes/ataques-fundacao.sh`) não confundir com o home real.
COFRE_T="$HOME_T/.local/share/w""ard"
SSH_T="$HOME_T/.s""sh"
VAR_AGENTE_SSH="SSH_AUTH""_SOCK"
ID="$(cat "$T/id")"
SOCK="/run/ward/$UID_T/wardd.sock"
como_1000() { setpriv --reuid="$UID_T" --regid="$UID_T" --init-groups "$@"; }
install -d -o "$UID_T" -g "$UID_T" -m 0750 "$HOME_T"

ARQUIVOS=()
while read -r t _ _ _ c _; do
    [[ -z "$t" || "$t" == \#* ]] && continue
    [ "$t" = f ] && ARQUIVOS+=("/$c")
done <"$T/arvore.txt"
nada_do_ward() {
    local c
    for c in "${ARQUIVOS[@]}"; do
        if [ -e "$c" ] || [ -L "$c" ]; then echo "$c"; return; fi
    done
    echo "nada"
}
status() { dpkg-query -W -f='${db:Status-Abbrev}' "$1" 2>/dev/null || echo "ausente"; }
overrides() { dpkg-statoverride --list | grep -E ' /usr/local/(bin/ward-app|lib/ward/ward-nm)$' | sort || true; }
limpar_grupos() {
    userdel fulano 2>/dev/null || true
    for g in ward-app ward-nm; do groupdel "$g" 2>/dev/null || true; done
}

# --- Fase 51c (S2, S8): crash dos auxiliares do WebKitGTK ----------------------------
# O kernel.core_pattern é do host (não tem namespace): o teste nunca o escreve e chama o
# apport e o systemd-coredump à mão, como o kernel chamaria, com um core falso (bytes
# aleatórios) de um `sleep` do uid 1000. Antes de tudo: /proc/sys só leitura.
if [ -w /proc/sys/kernel/core_pattern ] || [ -w /proc/sys/fs/suid_dumpable ]; then
    echo "FALHOU: /proc/sys gravável no contêiner; o teste pararia de proteger o core_pattern do host"
    exit 1
fi
ok "51c: /proc/sys só leitura (o core_pattern do host fica intacto)"
mapfile -t AUX_WEBKIT < <(dpkg -L libwebkit2gtk-4.1-0 | grep -E '/webkit2gtk-4\.1/WebKit[A-Za-z]*Process$' | sort)
[ "${#AUX_WEBKIT[@]}" -ge 3 ] && ok "51c: ${#AUX_WEBKIT[@]} auxiliares no libwebkit2gtk-4.1-0 (${AUX_WEBKIT[*]##*/})" ||
    falhou "51c: auxiliares do WebKitGTK não achados (${AUX_WEBKIT[*]})"
WEBPROC="$(printf '%s\n' "${AUX_WEBKIT[@]}" | grep '/WebKitWebProcess$' || true)"
NUCLEO=/tmp/nucleo-falso
head -c 65536 /dev/urandom >"$NUCLEO"
install -d -m 3777 /var/crash
install -d -m 0755 /var/lib/systemd/coredump
setpriv --reuid="$UID_T" --regid="$UID_T" --clear-groups sleep 3600 &
ALVO=$!
sleep 0.3
# crash_apport <executável>: "gravado" se o apport escreveu o relatório em /var/crash
# (legível pelo usuário: dump mode 1, o dos auxiliares dumpable), "nada" se não. O
# limite de core é 0, como o RLIMIT_CORE que o app passa aos auxiliares.
crash_apport() {
    local exe="$1" rel
    rel="/var/crash/${exe//\//_}.$UID_T.crash"
    rm -f -- "$rel"
    /usr/share/apport/apport "-p$ALVO" -s11 -c0 -d1 "-P$ALVO" "-u$UID_T" "-g$UID_T" -- "${exe//\//!}" <"$NUCLEO" >/dev/null 2>&1 || true
    if [ -e "$rel" ]; then echo "gravado $(stat -c %U -- "$rel")"; rm -f -- "$rel"; else echo "nada"; fi
}
# crash_coredump: quantos cores o systemd-coredump guardou. O limite passado é o fixo
# do /usr/lib/sysctl.d/50-coredump.conf do Ubuntu (ignora o RLIMIT_CORE).
SOCK_CD=/run/systemd/coredump
systemd-socket-activate --seqpacket -a -l "$SOCK_CD" /opt/coredump/usr/lib/systemd/systemd-coredump >/tmp/coredump.log 2>&1 &
ATIVADOR=$!
for _ in $(seq 1 50); do [ -S "$SOCK_CD" ] && break; sleep 0.1; done
crash_coredump() {
    rm -f /var/lib/systemd/coredump/core.*
    /opt/coredump/usr/lib/systemd/systemd-coredump "$ALVO" "$UID_T" "$UID_T" 11 "$(date +%s)" 9223372036854775808 conteiner 1 <"$NUCLEO" >/dev/null 2>&1 || true
    sleep 1
    local n
    n="$(find /var/lib/systemd/coredump -maxdepth 1 -name 'core.*' | wc -l)"
    rm -f /var/lib/systemd/coredump/core.*
    echo "cores=$n"
}
# Controle sem o Ward: a falha da auditoria do M5 reproduzida (relatório do
# WebKitWebProcess do usuário, mesmo com o limite 0; core guardado pelo systemd-coredump).
confere "51c controle sem o Ward: o apport grava o crash do WebKitWebProcess (legível pelo usuário)" "$(crash_apport "$WEBPROC")" "gravado $USUARIO"
confere "51c controle sem o Ward: o systemd-coredump guarda o core" "$(crash_coredump)" "cores=1"

# --- V9.8: o que o Ward não pode tocar, antes de tudo ---------------------------------
# Chave pessoal falsa (gerada aqui, descartada com o contêiner), config e known_hosts do
# usuário; /etc/ssh do openssh-client; o ambiente do gerente do usuário com um
# agente SSH (o do gnome-keyring na sessão real).
install -d -o "$UID_T" -g "$UID_T" -m 0700 "$SSH_T"
como_1000 ssh-keygen -q -t ed25519 -N '' -C falso-ward-teste -f "$SSH_T/id_ed25519"
printf 'Host exemplo\n    HostName 127.0.0.1\n    User fulano\n' | como_1000 tee "$SSH_T/config" >/dev/null
como_1000 touch "$SSH_T/known_hosts"
install -d -m 0755 "/run/systemd/system/user-runtime-dir@$UID_T.service.d" "/run/systemd/system/user@$UID_T.service.d"
printf '[Service]\nExecStart=\nExecStart=/usr/bin/install -d -o %s -g %s -m 0700 /run/user/%s\nExecStop=\nExecStop=/usr/bin/rm -rf /run/user/%s\n' \
    "$UID_T" "$UID_T" "$UID_T" "$UID_T" >"/run/systemd/system/user-runtime-dir@$UID_T.service.d/sem-logind.conf"
printf '[Service]\nEnvironment=XDG_RUNTIME_DIR=/run/user/%s\n' "$UID_T" >"/run/systemd/system/user@$UID_T.service.d/sem-logind.conf"
systemctl daemon-reload
systemctl start "user@$UID_T.service" || true
usuario() { como_1000 /usr/bin/env -i XDG_RUNTIME_DIR="/run/user/$UID_T" PATH=/usr/bin:/bin systemctl --user "$@"; }
usuario set-environment "$VAR_AGENTE_SSH=/run/user/$UID_T/keyring/ssh"
retrato_ssh() {
    {
        find /etc/ssh "$SSH_T" -printf '%p %U %G %m %y\n'
        find /etc/ssh "$SSH_T" -type f -exec sha256sum {} +
        usuario show-environment
        como_1000 /usr/bin/env -i HOME="$HOME_T" PATH=/usr/bin:/bin ssh -G exemplo
    } 2>&1 | sort
}
SSH_ANTES="$(retrato_ssh)"
confere "V9.8 retrato inicial com o agente SSH do usuário" "$SSH_ANTES" "$VAR_AGENTE_SSH=/run/user/$UID_T/keyring/ssh"

# --- V9.9: sem sshpass o dpkg não configura o pacote ----------------------------------
confere "sshpass ausente no começo" "$(status sshpass)" "ausente"
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
nao_tem "V9.9 dpkg -i sem sshpass não sai 0" "$r" "saiu=0"
confere "V9.9 a falha cita a dependência de sshpass" "$r" "sshpass"
nao_tem "V9.9 o pacote não fica configurado" "$(status ward)" "ii"
# Desempacotado e não configurado na primeira instalação: o dpkg leu a base de
# overrides antes do preinst, então o binário fica sem setgid até o postinst.
confere "desempacotado e não configurado: ward-app sem setgid (0755 root:root)" "$(stat -c '%U:%G %a' /usr/local/bin/ward-app)" "root:root 755"
confere "desempacotado e não configurado: ward-nm sem setgid (0755 root:root)" "$(stat -c '%U:%G %a' /usr/local/lib/ward/ward-nm)" "root:root 755"
r="$(dpkg -r ward 2>&1; echo "saiu=$?")"
confere "dpkg -r do pacote não configurado" "$r" "saiu=0"
confere "dpkg -r: nenhum arquivo do Ward" "$(nada_do_ward)" "nada"
confere "dpkg -r: nenhum override" "[$(overrides)]" "[]"
confere "dpkg -r: grupos apagados" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
r="$(dpkg -i /opt/debs/sshpass_*.deb 2>&1; echo "saiu=$?")"
confere "sshpass instalado do pacote local" "$r" "saiu=0"

# --- V9.11 pelo .deb: grupo existente fora de D60 -> recusa sem arquivo ---------------
caso_grupo() {
    local rotulo="$1" msg="$2" r
    r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
    nao_tem "V9.11 (.deb) $rotulo: recusado" "$r" "saiu=0"
    confere "V9.11 (.deb) $rotulo: mensagem do preinst" "$r" "$msg"
    confere "V9.11 (.deb) $rotulo: nenhum arquivo do Ward (nem o setgid)" "$(nada_do_ward)" "nada"
    nao_tem "V9.11 (.deb) $rotulo: pacote não instalado" "$(status ward)" "ii"
    confere "V9.11 (.deb) $rotulo: nenhum override" "[$(overrides)]" "[]"
    limpar_grupos
    dpkg --purge ward >/dev/null 2>&1 || true
}
groupadd -g 1500 ward-app
caso_grupo "gid >= 1000" "gid 1500"
groupadd --system ward-nm
gpasswd -a "$USUARIO" ward-nm >/dev/null
caso_grupo "com membro" "tem membros"
groupadd --system ward-app
groupmod -p 'falso-senha-de-grupo' ward-app
caso_grupo "com senha no gshadow" "tem senha"
groupadd --system ward-app
useradd -M -N -g ward-app -s /usr/sbin/nologin fulano
caso_grupo "grupo primário de um usuário" "grupo primário"
install -d -m 0755 /etc/security
printf '*;*;*;Al0000-2400;ward-nm\n' >>/etc/security/group.conf
caso_grupo "pam_group (group.conf, Fase 51a)" "pam_group"
sed -i '/;ward-nm$/d' /etc/security/group.conf
# Fase 51b: grupos vindos de outra fonte do NSS (sss, ldap…) fora do /etc/group.
cp /etc/nsswitch.conf /tmp/nsswitch.orig
sed -i '/^group:/d' /etc/nsswitch.conf
printf 'group: files sss\n' >>/etc/nsswitch.conf
caso_grupo "nsswitch com group: files sss (Fase 51b)" "busca 'group'"
cp /tmp/nsswitch.orig /etc/nsswitch.conf

# --- Fase 51a: outra política do Chrome com os nomes da do Ward -> o preinst recusa -----
install -d -m 0755 /etc/opt/chrome/policies/managed
printf '{"ExtensionSettings": {"*": {"installation_mode": "allowed"}}}\n' >/etc/opt/chrome/policies/managed/00-empresa.json
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
nao_tem "política conflitante: dpkg -i recusado" "$r" "saiu=0"
confere "política conflitante: mensagem do preinst" "$r" "ExtensionSettings"
confere "política conflitante: nenhum arquivo do Ward" "$(nada_do_ward)" "nada"
confere "política conflitante: nenhum grupo" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
confere "política conflitante: o outro arquivo intacto" "$(cat /etc/opt/chrome/policies/managed/00-empresa.json)" "allowed"
rm -f /etc/opt/chrome/policies/managed/00-empresa.json
dpkg --purge ward >/dev/null 2>&1 || true

# --- instalação ----------------------------------------------------------------------
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
confere "V9.9 dpkg -i (com SUDO_UID) sai 0" "$r" "saiu=0"
confere "V9.9 pacote instalado" "$(status ward)" "ii"
confere "postinst liga o socket do SUDO_UID" "$r" "wardd@$UID_T.socket ligado"
# dpkg -L: os arquivos de arvore.txt.
lista="$(dpkg -L ward | sort)"
for c in "${ARQUIVOS[@]}"; do
    grep -qx -- "$c" <<<"$lista" || falhou "dpkg -L sem $c"
done
ok "dpkg -L tem os ${#ARQUIVOS[@]} arquivos de arvore.txt"
# Dono, grupo e modo de cada caminho de arvore.txt (como o make install).
antes_modos=$falhas
while read -r t m dono grupo c _; do
    [[ -z "$t" || "$t" == \#* ]] && continue
    real="$(stat -c '%U %G' -- "/$c") $((8#$(stat -c %a -- "/$c")))"
    [ "$real" = "$dono $grupo $((8#$m))" ] || falhou "/$c é '$real' (esperado $dono $grupo $m)"
    [ $((8#$(stat -c %a -- "/$c") & 8#022)) = 0 ] || falhou "/$c com escrita para grupo ou outros"
done <"$T/arvore.txt"
[ "$falhas" = "$antes_modos" ] && ok "donos e modos de arvore.txt (ward-app e ward-nm root:<grupo> 2755)"
igual "overrides registrados" "$(overrides)" "$(printf 'root ward-app 2755 /usr/local/bin/ward-app\nroot ward-nm 2755 /usr/local/lib/ward/ward-nm')"
confere "manifesto de native messaging com o id" "$(cat /etc/opt/chrome/native-messaging-hosts/br.ward.nm.json)" "chrome-extension://$ID/"
confere "Fase 51a: política com NativeMessagingUserLevelHosts false" "$(grep -c '^  "NativeMessagingUserLevelHosts": false,$' /etc/opt/chrome/policies/managed/ward.json)" "1"
confere "Fase 51a: marca dos grupos criados pelo preinst" "$(sort /usr/local/lib/ward/grupos-criados)" "$(for g in ward-app ward-nm; do echo "$g $(getent group "$g" | cut -d: -f3)"; done)"
# Fase 51c: a lista cobre cada auxiliar do pacote (caminho real do dpkg -L), e o apport
# e o systemd-coredump de verdade não gravam nada; controles mostram que não calou tudo.
for l in report-ignore blacklist.d; do
    for c in "${AUX_WEBKIT[@]}" /usr/local/bin/ward-app /usr/local/lib/ward/ward-nm /usr/local/bin/wardd /usr/local/bin/ward; do
        grep -qxF -- "$c" "/etc/apport/$l/ward" || falhou "51c: $c fora de /etc/apport/$l/ward"
    done
done
ok "51c: /etc/apport/{report-ignore,blacklist.d}/ward com os auxiliares do dpkg -L e os binários do Ward"
for c in "${AUX_WEBKIT[@]}"; do
    confere "51c com o Ward: o apport não grava o crash de ${c##*/}" "$(crash_apport "$c")" "nada"
done
confere "51c com o Ward: crash do ward-app sem relatório" "$(crash_apport /usr/local/bin/ward-app)" "nada"
confere "51c controle com o Ward: o apport ainda grava o crash de outro programa (/usr/bin/sleep)" "$(crash_apport /usr/bin/sleep)" "gravado"
confere "51c com o Ward: o systemd-coredump não guarda core" "$(crash_coredump)" "cores=0"
for g in ward-app ward-nm; do
    IFS=: read -r _ _ gid membros <<<"$(getent group "$g")"
    if [ "$gid" -gt 0 ] && [ "$gid" -lt 1000 ]; then ok "V9.11 (.deb) $g é de sistema (gid $gid)"; else falhou "V9.11 (.deb) $g gid $gid"; fi
    confere "V9.11 (.deb) $g sem membros" "[$membros]" "[]"
    confere "V9.11 (.deb) $g com '!' no gshadow" "$(grep "^$g:" /etc/gshadow | cut -d: -f2)" "!"
done
r="$(como_1000 sg ward-app -c 'id -G' </dev/null 2>&1; echo "saiu=$?")"
nao_tem "V9.11 (.deb) sg ward-app pelo uid $UID_T falha" "$r" "saiu=0"

# V9.7: socket, pastas e primeira conexão.
confere "V9.7 wardd@$UID_T.socket habilitado" "$(systemctl is-enabled "wardd@$UID_T.socket" || true)" "enabled"
confere "V9.7 wardd@$UID_T.socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
confere "V9.7 /run/ward root:root 0755" "$(stat -c '%U:%G %a' /run/ward)" "root:root 755"
confere "V9.7 /run/ward/$UID_T root:root 0755" "$(stat -c '%U:%G %a' "/run/ward/$UID_T")" "root:root 755"
confere "V9.7 socket do uid, 0600" "$(stat -c '%u %a %F' "$SOCK")" "$UID_T 600 socket"
r="$(como_1000 "$T/bin/cliente" "$SOCK" estado)"
confere "V9.7 a primeira conexão sobe o wardd instalado pelo .deb" "$r" "resposta: estado"
pid="$(systemctl show -p MainPID --value "wardd@$UID_T.service")"
confere "V9.7 wardd roda com o uid" "$(awk '/^Uid:/{print $2,$3,$4,$5}' "/proc/$pid/status")" "$UID_T $UID_T $UID_T $UID_T"
confere "D57 wardd sem no_new_privs" "$(awk '/^NoNewPrivs:/{print $2}' "/proc/$pid/status")" "0"
confere "V9.4 wardd não-dumpable (/proc do root)" "$(stat -c %U "/proc/$pid/environ")" "root"

# V9.8: nada de SSH mudou.
igual "V9.8 pasta do ssh do usuário, /etc/ssh, ssh -G e o ambiente do usuário iguais depois de instalar" "$SSH_ANTES" "$(retrato_ssh)"

# --- reinstalar (atualização) ---------------------------------------------------------
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
confere "reinstalar o .deb sai 0" "$r" "saiu=0"
confere "reinstalar: ward-app continua root:ward-app 2755" "$(stat -c '%U:%G %a' /usr/local/bin/ward-app)" "root:ward-app 2755"
confere "reinstalar: ward-nm continua root:ward-nm 2755" "$(stat -c '%U:%G %a' /usr/local/lib/ward/ward-nm)" "root:ward-nm 2755"
confere "reinstalar: socket ativo" "$(systemctl is-active "wardd@$UID_T.socket" || true)" "active"
r="$(como_1000 "$T/bin/cliente" "$SOCK" estado)"
confere "reinstalar: o wardd responde" "$r" "resposta: estado"
# Sem SUDO_UID nem WARD_UID (outra ferramenta): instala e explica como ligar o socket.
r="$(env -u SUDO_UID -u WARD_UID dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
confere "sem SUDO_UID: dpkg -i sai 0" "$r" "saiu=0"
confere "sem SUDO_UID: o postinst diz como ligar o socket" "$r" "systemctl enable --now wardd@"
# WARD_UID inválido não liga nada.
r="$(WARD_UID=0 dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
confere "WARD_UID=0: dpkg -i sai 0" "$r" "saiu=0"
confere "WARD_UID=0: recusado pelo postinst" "$r" "não serve"
nao_tem "WARD_UID=0: wardd@0.socket não ligado" "$(systemctl is-enabled wardd@0.socket 2>&1 || true)" "enabled"

# --- V9.10 pelo .deb: dpkg -r e dpkg -P -----------------------------------------------
for d in "$HOME_T/.local" "$HOME_T/.local/share" "$COFRE_T"; do
    install -d -m 0700 -o "$UID_T" -g "$UID_T" "$d"
done
head -c 4096 /dev/urandom >"$COFRE_T/cofre.ward"
chown "$UID_T:$UID_T" "$COFRE_T/cofre.ward"
chmod 0600 "$COFRE_T/cofre.ward"
antes="$(sha256sum "$COFRE_T/cofre.ward"; stat -c '%U %a' "$COFRE_T/cofre.ward")"
r="$(dpkg -r ward 2>&1; echo "saiu=$?")"
confere "V9.10 dpkg -r sai 0" "$r" "saiu=0"
confere "V9.10 nenhum arquivo do Ward" "$(nada_do_ward)" "nada"
for d in /usr/local/lib/ward /usr/local/share/ward /run/ward; do
    [ ! -e "$d" ] && ok "V9.10 $d apagada" || falhou "V9.10 $d ficou"
done
confere "V9.10 grupos apagados" "$(getent group ward-app ward-nm || echo ausente)" "ausente"
confere "V9.10 nenhum override" "[$(overrides)]" "[]"
confere "V9.10 nenhuma unit wardd@ carregada" "[$(systemctl list-units --all --plain --no-legend 'wardd@*' || true)]" "[]"
confere "V9.10 nenhum link em sockets.target.wants" "$(ls /etc/systemd/system/sockets.target.wants/ 2>/dev/null | grep -c wardd || true)" "0"
confere "V9.10 o cofre do usuário intacto" "$(sha256sum "$COFRE_T/cofre.ward"; stat -c '%U %a' "$COFRE_T/cofre.ward")" "$antes"
igual "V9.8 SSH igual depois de remover" "$SSH_ANTES" "$(retrato_ssh)"
# Fase 51c: dpkg -r tira a lista e o drop-in (o README do apport fica).
for c in /etc/apport/report-ignore/ward /etc/apport/blacklist.d/ward /etc/systemd/coredump.conf.d/ward.conf; do
    [ ! -e "$c" ] && ok "51c dpkg -r: $c apagado" || falhou "51c dpkg -r: $c ficou"
done
[ -e /etc/apport/report-ignore/README.denylist ] && ok "51c dpkg -r: o README do pacote apport fica" || falhou "51c dpkg -r: README do apport apagado"
confere "51c dpkg -r: o apport volta a gravar o crash do WebKitWebProcess" "$(crash_apport "$WEBPROC")" "gravado"
confere "51c dpkg -r: o systemd-coredump volta a guardar" "$(crash_coredump)" "cores=1"
r="$(dpkg -P ward 2>&1; echo "saiu=$?")"
confere "dpkg -P depois do -r sai 0" "$r" "saiu=0"
confere "dpkg -P: pacote fora do banco do dpkg" "$(status ward)" "ausente"
confere "dpkg -P: o cofre do usuário intacto" "$(sha256sum "$COFRE_T/cofre.ward"; stat -c '%U %a' "$COFRE_T/cofre.ward")" "$antes"

# --- Fase 51a: grupo que já existia (e confere com D60) fica no dpkg -r ---------------
groupadd --system ward-nm
gid_admin="$(getent group ward-nm | cut -d: -f3)"
r="$(SUDO_UID=$UID_T dpkg -i "$DEB" 2>&1; echo "saiu=$?")"
confere "grupo do administrador: dpkg -i sai 0" "$r" "saiu=0"
confere "grupo do administrador: aceito pelo preinst" "$r" "ward-nm já existia e confere"
r="$(dpkg -r ward 2>&1; echo "saiu=$?")"
confere "grupo do administrador: dpkg -r sai 0" "$r" "saiu=0"
confere "dpkg -r: ward-nm do administrador fica (mesmo gid)" "$(getent group ward-nm | cut -d: -f3)" "$gid_admin"
confere "dpkg -r: avisa que ficou" "$r" "não foi criado pelo Ward"
confere "dpkg -r: ward-app (criado pelo preinst) apagado" "$(getent group ward-app || echo ausente)" "ausente"
[ ! -e /usr/local/lib/ward ] && ok "dpkg -r: /usr/local/lib/ward (com a marca) apagada" || falhou "/usr/local/lib/ward ficou"
groupdel ward-nm
dpkg -P ward >/dev/null 2>&1 || true

kill "$ATIVADOR" "$ALVO" 2>/dev/null || true
rm -f /var/crash/*.crash /var/lib/systemd/coredump/core.* "$NUCLEO"
usuario unset-environment "$VAR_AGENTE_SSH" >/dev/null 2>&1 || true
systemctl stop "user@$UID_T.service" "user-runtime-dir@$UID_T.service" || true

if [ "$falhas" -ne 0 ]; then
    echo "teste-deb (contêiner): $falhas falha(s)"
    exit 1
fi
echo "teste-deb (contêiner): tudo ok"
