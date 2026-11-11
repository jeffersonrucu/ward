#!/bin/sh
# Entrypoint of the test SSH servers. Builds the sshd_config of the mode and starts sshd.
# Passwords read from /run/ward/ and passed to chpasswd through standard input (never argv).
set -eu

mode="${WARD_MODE:?WARD_MODE não definido}"
secrets=/run/ward

# The host key comes from .dev/ (stable across recreations; rotated in V4.28).
install -m 0600 -o root -g root "$secrets/ssh_host_ed25519_key" /etc/ssh/ssh_host_ed25519_key

set_password() { # user file
    printf '%s:%s\n' "$1" "$(cat "$2")" | chpasswd >/dev/null 2>&1
}
# Account that only gets in by key: password "*" (no valid password, but not locked).
key_only() {
    printf '%s:*\n' "$1" | chpasswd -e >/dev/null 2>&1
}
authorize_key() { # user
    install -m 0644 -o root -g root "$secrets/autorizada.pub" "/etc/ssh/autorizadas/$1"
}

cat >/etc/ssh/sshd_config <<CONF
Port 22
HostKey /etc/ssh/ssh_host_ed25519_key
PermitRootLogin no
AuthorizedKeysFile /etc/ssh/autorizadas/%u
StrictModes yes
MaxAuthTries 3
LoginGraceTime 20
# Without the per-source penalties of OpenSSH 10 (PerSourcePenalties): every client comes
# through the same docker gateway, and after a deliberately wrong password sshd dropped
# the following connections for 16 s. The "refused" tests passed because of the drop,
# not because of authentication, and the positive tunnel sometimes failed (Phase 12a).
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

case "$mode" in
password)
    # 2222: deploy by password (and tunnel to postgres); sftpuser sftp only, in a chroot.
    set_password deploy "$secrets/senha-deploy"
    set_password sftpuser "$secrets/senha-sftpuser"
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
    # 2223: deploy by key only; password refused.
    key_only deploy
    authorize_key deploy
    cat >>/etc/ssh/sshd_config <<CONF
AllowUsers deploy
PubkeyAuthentication yes
PasswordAuthentication no
AuthenticationMethods publickey
AllowTcpForwarding no
CONF
    ;;
jump)
    # 2224: the jump user by password; forwards only to port 22 of the internal host (ProxyJump).
    set_password salto "$secrets/senha-salto"
    cat >>/etc/ssh/sshd_config <<CONF
AllowUsers salto
PubkeyAuthentication no
PasswordAuthentication yes
AllowTcpForwarding local
PermitOpen ssh-interno:22
CONF
    ;;
internal)
    # No port on the host; only the internal compose network reaches it (through the jump, 2224).
    key_only deploy
    authorize_key deploy
    cat >>/etc/ssh/sshd_config <<CONF
AllowUsers deploy
PubkeyAuthentication yes
PasswordAuthentication no
AuthenticationMethods publickey
AllowTcpForwarding no
CONF
    ;;
*)
    echo "WARD_MODE desconhecido: $mode" >&2
    exit 1
    ;;
esac

/usr/sbin/sshd -t
exec /usr/sbin/sshd -D -e
