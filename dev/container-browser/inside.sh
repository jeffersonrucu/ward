#!/bin/bash -p
# Inside the browser container (Phase 65a; `dev/tests/browser-container.sh`), as the
# user's uid, without network: starts the in-container side of the service relay
# (127.0.0.1:8081–8084 → the Unix sockets in /relay), checks the guard (this Chrome sees no
# Ward) and runs the suite as on the host.
# Usage: inside.sh <nm|extension|login> <root> <suite arguments...>
export PATH=/usr/bin:/bin
set -euo pipefail
[ -f /.dockerenv ] || { echo "inside.sh: só no contêiner" >&2; exit 2; }
SUITE="${1:?uso: inside.sh <nm|extension|login> <raiz> <argumentos>}"
ROOT="${2:?uso: inside.sh <nm|extension|login> <raiz> <argumentos>}"
shift 2
cd "$ROOT"
/usr/bin/python3 -I -B "$ROOT/dev/tests/chrome_guard.py" check
/usr/bin/python3 -I -B "$ROOT/dev/tests/service_relay.py" inside /relay > /tmp/relay.log 2>&1 &
RELAY_PID=$!
trap 'kill "$RELAY_PID" 2>/dev/null || true' EXIT
for _ in $(seq 1 100); do grep -qs pronto /tmp/relay.log && break; sleep 0.05; done
grep -qs pronto /tmp/relay.log || { echo "inside.sh: o relay não subiu" >&2; cat /tmp/relay.log >&2; exit 1; }
export WARD_TEST_IN_CONTAINER=1 WARD_TEST_SERVICE_RELAY=/relay/service.sock
echo "contêiner: $(/opt/google/chrome/chrome --version), uid $(id -u), sem rede"
case "$SUITE" in
    nm) dev/tests/nm.sh "$@" ;;
    extension) dev/tests/extension.sh "$@" ;;
    login) dev/tests/login.sh "$@" ;;
    *) echo "inside.sh: suíte desconhecida: $SUITE" >&2; exit 2 ;;
esac
