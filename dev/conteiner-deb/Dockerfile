# `make teste-deb` (Fase 52): ubuntu:24.04 (a mesma digest das Fases 21b, 22, 24 e 51)
# com o systemd e as dependências do `.deb` já instaladas (openssh-client e
# libwebkit2gtk-4.1-0), menos o sshpass: o pacote dele fica baixado em /opt/debs para o
# teste ver o `dpkg -i` falhar sem ele e passar com ele. A rede só é usada no build (apt
# do Ubuntu); o contêiner roda com `--network none`. Contexto do build:
# dev/conteiner-units (o `iniciar.sh` de lá).
#
# Fase 51c: o apport instalado (o teste chama o /usr/share/apport/apport à mão, como o
# kernel chamaria) e o systemd-coredump só extraído em /opt/coredump (sem postinst: nada
# mexe no kernel.core_pattern, que é do host), com o usuário dele. As units do apport
# ficam mascaradas: o `apport --start` escreveria o core_pattern (o /proc/sys do contêiner
# é só leitura, e o teste confere isso antes de tudo).
FROM ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
         systemd openssh-client libwebkit2gtk-4.1-0 apport \
    && mkdir -m 0755 /opt/debs && cd /opt/debs && apt-get download sshpass \
    && chmod 0644 /opt/debs/*.deb \
    && mkdir -m 0755 /opt/coredump-deb && cd /opt/coredump-deb && apt-get download systemd-coredump \
    && dpkg-deb -x /opt/coredump-deb/systemd-coredump_*.deb /opt/coredump \
    && rm -rf /opt/coredump-deb \
    && systemd-sysusers /opt/coredump/usr/lib/sysusers.d/systemd-coredump.conf \
    && rm -rf /var/lib/apt/lists/* \
    && systemctl mask systemd-logind.service getty.target console-getty.service \
         systemd-networkd.service systemd-resolved.service systemd-timesyncd.service \
         apport.service apport-forward.socket apport-autoreport.path apport-autoreport.service \
         apport-autoreport.timer
COPY iniciar.sh /usr/local/sbin/iniciar
ENV container=docker
STOPSIGNAL SIGRTMIN+3
CMD ["/usr/local/sbin/iniciar"]
