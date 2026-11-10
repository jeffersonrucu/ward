# `make test-units` (Fase 24; V3.21): ubuntu:24.04 (a mesma digest das Fases 21b e 22)
# com o systemd, para subir o `wardd@.socket`/`wardd@.service` de verdade. A rede só é
# usada no build (apt do Ubuntu); o contêiner roda com `--network none`.
FROM ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends systemd \
    && rm -rf /var/lib/apt/lists/* \
    && systemctl mask systemd-logind.service getty.target console-getty.service \
         systemd-networkd.service systemd-resolved.service systemd-timesyncd.service
COPY start.sh /usr/local/sbin/iniciar
ENV container=docker
STOPSIGNAL SIGRTMIN+3
CMD ["/usr/local/sbin/iniciar"]
