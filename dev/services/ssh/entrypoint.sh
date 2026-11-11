#!/bin/sh
# Entrada dos servidores SSH de teste. Monta o sshd_config do modo e sobe o sshd.
# Senhas lidas de /run/ward/ e passadas ao chpasswd pela entrada padrão (nunca argv).
set -eu

modo="${WARD_MODE:?WARD_MODE não definido}"
segredos=/run/ward

# Chave de host vem de .dev/ (estável entre recriações; trocada em V4.28).
install -m 0600 -o root -g root "$segredos/ssh_host_ed25519_key" /etc/ssh/ssh_host_ed25519_key

definir_senha() { # usuário arquivo
    printf '%s:%s\n' "$1" "$(cat "$2")" | chpasswd >/dev/null 2>&1
}
# Conta que só entra por chave: senha "*" (sem senha válida, mas não travada).
so_chave() {
    printf '%s:*\n' "$1" | chpasswd -e >/dev/null 2>&1
}
autorizar_chave() { # usuário
    install -m 0644 -o root -g root "$segredos/autorizada.pub" "/etc/ssh/autorizadas/$1"
}

cat >/etc/ssh/sshd_config <<CONF
Port 22
HostKey /etc/ssh/ssh_host_ed25519_key
PermitRootLogin no
AuthorizedKeysFile /etc/ssh/autorizadas/%u
StrictModes yes
MaxAuthTries 3
LoginGraceTime 20
# Sem as penalidades por origem do OpenSSH 10 (PerSourcePenalties): todo cliente chega
# pelo mesmo gateway do docker, e depois de uma senha errada de propósito o sshd
# derrubava as conexões seguintes por 16 s. Os testes "recusado" passavam pela queda,
# não pela autenticação, e o túnel positivo falhava às vezes (Fase 12a).
PerSourcePenalties no
UseDNS no
PermitUserEnvironment no
AllowAgentForwarding no
X11Forwarding no
PermitTunnel no
GatewayPorts no
PrintMotd no
LogLevel VERBOSE
KbdInteractiveAuthentication no
Subsystem sftp internal-sftp
CONF

case "$modo" in
password)
    # 2222: deploy por senha (e túnel até o postgres); sftpuser só sftp, em chroot.
    definir_senha deploy "$segredos/senha-deploy"
    definir_senha sftpuser "$segredos/senha-sftpuser"
    cat >>/etc/ssh/sshd_config <<CONF
AllowUsers deploy sftpuser
PubkeyAuthentication no
PasswordAuthentication yes
AllowTcpForwarding local
PermitOpen postgres:5432
Match User sftpuser
    ForceCommand internal-sftp
    ChrootDirectory /srv/sftp
    AllowTcpForwarding no
    PermitTTY no
CONF
    ;;
key)
    # 2223: deploy só por chave; senha recusada.
    so_chave deploy
    autorizar_chave deploy
    cat >>/etc/ssh/sshd_config <<CONF
AllowUsers deploy
PubkeyAuthentication yes
PasswordAuthentication no
AuthenticationMethods publickey
AllowTcpForwarding no
CONF
    ;;
jump)
    # 2224: salto por senha; encaminha só para o ssh-interno:22 (ProxyJump).
    definir_senha salto "$segredos/senha-salto"
    cat >>/etc/ssh/sshd_config <<CONF
AllowUsers salto
PubkeyAuthentication no
PasswordAuthentication yes
AllowTcpForwarding local
PermitOpen ssh-interno:22
CONF
    ;;
internal)
    # Sem porta no host; só a rede interna do compose chega (pelo ssh-salto).
    so_chave deploy
    autorizar_chave deploy
    cat >>/etc/ssh/sshd_config <<CONF
AllowUsers deploy
PubkeyAuthentication yes
PasswordAuthentication no
AuthenticationMethods publickey
AllowTcpForwarding no
CONF
    ;;
*)
    echo "WARD_MODE desconhecido: $modo" >&2
    exit 1
    ;;
esac

/usr/sbin/sshd -t
exec /usr/sbin/sshd -D -e
