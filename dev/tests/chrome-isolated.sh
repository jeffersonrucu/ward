#!/bin/bash -p
# Phase 65a (m2): the Chrome of `make test-app-e2e` (the Python Playwright launches it as
# its `executable_path`, with `--disable-extensions` and the CDP pipe on fds 3 and 4).
#
# On a host with Ward installed, Chrome would read the Ward policy in /etc/opt/chrome
# (force-installed extension) and the system manifest `br.ward.nm.json` (the way to
# /usr/local/bin/ward-nm and the real vault). Here Chrome runs in a bwrap where
# `/etc/opt`, `/usr/local` and, when they exist, `/etc/chromium` and `/run/ward` are empty
# tmpfs: it sees no policy, no host manifest, no installed `ward-nm` and no socket of the
# installed `wardd`. The guard (`dev/tests/chrome_guard.py check`) runs inside the bwrap,
# before Chrome, and refuses (exit 3) if anything of Ward is still visible. Nothing of the
# system changes and no sudo.
#
# Usage: dev/tests/chrome-isolated.sh <Chrome arguments...>
export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
command -v bwrap >/dev/null || { echo "chrome-isolated: falta o bwrap" >&2; exit 1; }
ARGS=(--dev-bind / / --tmpfs /etc/opt --tmpfs /usr/local --die-with-parent)
for d in /etc/chromium /run/ward; do
    if [ -e "$d" ]; then ARGS+=(--tmpfs "$d"); fi
done
# shellcheck disable=SC2016
exec bwrap "${ARGS[@]}" -- /bin/bash -p -c \
    '/usr/bin/python3 -I -B "$1" check >&2 || exit 3; shift; exec /opt/google/chrome/chrome "$@"' \
    chrome-isolated "$ROOT/dev/tests/chrome_guard.py" "$@"
