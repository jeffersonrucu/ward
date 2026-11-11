# `make test-deb` (Phase 52): ubuntu:24.04 (the same digest as Phases 21b, 22, 24 and
# 51) with systemd and the `.deb` dependencies already installed (openssh-client and
# libwebkit2gtk-4.1-0), except sshpass: its package is downloaded to /opt/debs so the
# test sees `dpkg -i` fail without it and pass with it. The network is only used in the
# build (Ubuntu apt); the container runs with `--network none`. Build context:
# dev/container-units (its `start.sh`).
#
# Phase 51c: apport installed (the test calls /usr/share/apport/apport by hand, as the
# kernel would) and systemd-coredump only extracted to /opt/coredump (no postinst: nothing
# touches kernel.core_pattern, which belongs to the host), with its user. The apport units
# stay masked: `apport --start` would write the core_pattern (the container's /proc/sys
# is read-only, and the test checks that before anything else).
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
COPY start.sh /usr/local/sbin/start
ENV container=docker
STOPSIGNAL SIGRTMIN+3
CMD ["/usr/local/sbin/start"]
