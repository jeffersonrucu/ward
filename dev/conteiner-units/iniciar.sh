#!/bin/sh
# PID 1 do contêiner do `make teste-units`. Com `--cgroupns=private`, o
# /sys/fs/cgroup de dentro é só a subárvore do próprio contêiner, mas o Docker o monta
# somente leitura; o systemd precisa escrever nela. O remount pede CAP_SYS_ADMIN (dado
# só a este contêiner descartável, sem rede).
set -eu
mount -o remount,rw /sys/fs/cgroup
exec /usr/lib/systemd/systemd --log-target=console
