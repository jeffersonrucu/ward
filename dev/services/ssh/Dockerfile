# Servidor SSH de teste do Ward (Fase 12). Uma imagem para ssh-senha, ssh-chave,
# ssh-salto e ssh-interno; o modo vem de WARD_MODO. Só credenciais falsas, montadas
# de .dev/services/ em /run/ward/ (somente leitura).
FROM alpine:3.22@sha256:49340b346798875b5592e5d420241f2f9cfba5f47e62ebb1446a1b34734e898f
RUN apk add --no-cache openssh-server openssh-sftp-server \
    && rm -f /etc/ssh/ssh_host_* \
    && adduser -D -s /bin/sh deploy \
    && adduser -D -s /bin/sh salto \
    && adduser -D -H -s /sbin/nologin sftpuser \
    && mkdir -p /srv/sftp/arquivos /etc/ssh/autorizadas \
    && chown sftpuser:sftpuser /srv/sftp/arquivos
COPY entrypoint.sh /usr/local/bin/ward-ssh-entrada
RUN chmod 0755 /usr/local/bin/ward-ssh-entrada
EXPOSE 22
ENTRYPOINT ["/usr/local/bin/ward-ssh-entrada"]
