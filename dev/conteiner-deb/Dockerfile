# `make teste-deb` (Fase 52): ubuntu:24.04 (a mesma digest das Fases 21b, 22, 24 e 51)
# com o systemd e as dependências do `.deb` já instaladas (openssh-client e
# libwebkit2gtk-4.1-0), menos o sshpass: o pacote dele fica baixado em /opt/debs para o
# teste ver o `dpkg -i` falhar sem ele e passar com ele. A rede só é usada no build (apt
# do Ubuntu); o contêiner roda com `--network none`. Contexto do build:
# dev/conteiner-units (o `iniciar.sh` de lá).
FROM ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
         systemd openssh-client libwebkit2gtk-4.1-0 \
    && mkdir -m 0755 /opt/debs && cd /opt/debs && apt-get download sshpass \
    && chmod 0644 /opt/debs/*.deb \
    && rm -rf /var/lib/apt/lists/* \
    && systemctl mask systemd-logind.service getty.target console-getty.service \
         systemd-networkd.service systemd-resolved.service systemd-timesyncd.service
COPY iniciar.sh /usr/local/sbin/iniciar
ENV container=docker
STOPSIGNAL SIGRTMIN+3
CMD ["/usr/local/sbin/iniciar"]
