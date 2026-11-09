# `make teste-install` (Fase 51): ubuntu:24.04 (a mesma digest das Fases 21b, 22 e 24)
# com o systemd, o make (o `make install` de verdade) e o python3 (o `crx.py` da
# instalação). A rede só é usada no build (apt do Ubuntu); o contêiner roda com
# `--network none`. Contexto do build: dev/conteiner-units (o `iniciar.sh` de lá).
FROM ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends systemd make python3 \
    && rm -rf /var/lib/apt/lists/* \
    && systemctl mask systemd-logind.service getty.target console-getty.service \
         systemd-networkd.service systemd-resolved.service systemd-timesyncd.service
COPY iniciar.sh /usr/local/sbin/iniciar
ENV container=docker
STOPSIGNAL SIGRTMIN+3
CMD ["/usr/local/sbin/iniciar"]
