# `make test-install` (Phase 51): ubuntu:24.04 (the same digest as Phases 21b, 22 and
# 24) with systemd, make (the real `make install`) and python3 (the install's `crx.py`).
# The network is only used in the build (Ubuntu apt); the container runs with
# `--network none`. Build context: dev/container-units (its `start.sh`).
FROM ubuntu:24.04@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends systemd make python3 \
    && rm -rf /var/lib/apt/lists/* \
    && systemctl mask systemd-logind.service getty.target console-getty.service \
         systemd-networkd.service systemd-resolved.service systemd-timesyncd.service
COPY start.sh /usr/local/sbin/start
ENV container=docker
STOPSIGNAL SIGRTMIN+3
CMD ["/usr/local/sbin/start"]
