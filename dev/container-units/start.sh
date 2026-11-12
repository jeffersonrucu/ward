#!/bin/sh
# PID 1 of the `make test-units` container. With `--cgroupns=private`, the
# /sys/fs/cgroup inside is only the container's own subtree, but Docker mounts it
# read-only; systemd needs to write to it. The remount needs CAP_SYS_ADMIN (given only to
# this disposable container, without network).
set -eu
mount -o remount,rw /sys/fs/cgroup
exec /usr/lib/systemd/systemd --log-target=console
