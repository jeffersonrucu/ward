# `make test-units` (Phase 24; V3.21): ubuntu:24.04 (the same digest as Phases 21b and
# 22) with systemd, to start the real `wardd@.socket`/`wardd@.service`. The network is
# only used in the build (Ubuntu apt); the container runs with `--network none`.
FROM ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends systemd \
    && rm -rf /var/lib/apt/lists/* \
    && systemctl mask systemd-logind.service getty.target console-getty.service \
         systemd-networkd.service systemd-resolved.service systemd-timesyncd.service
COPY start.sh /usr/local/sbin/start
ENV container=docker
STOPSIGNAL SIGRTMIN+3
CMD ["/usr/local/sbin/start"]
