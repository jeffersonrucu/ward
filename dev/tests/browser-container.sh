#!/bin/bash -p
# Phase 65a (m2; handoff of Phase 65, problem 1): `make test-nm`, `make test-extension`
# and `make test-login` in a disposable `ubuntu:24.04` container with Google Chrome
# (image built from `dev/container-browser/Dockerfile`; the build uses the
# network, the test does not).
#
# Why: on a host with Ward installed, the Chrome policy force-installs the real extension
# in every profile, and the system manifest `br.ward.nm.json` sends the Ward id to
# `/usr/local/bin/ward-nm`, the installed `wardd` and the real vault. In the container,
# Chrome sees no policy and no system manifest (the guard `dev/tests/chrome_guard.py`
# runs inside before the suite), and nothing of the host's `/etc`, `/usr/local`, `/run`
# or home is mounted: only the repository (read-only), its `.dev/` and
# `docs/mission/evidence/` (read-write: the suites' work folders, logs and screenshots)
# and the relay folder.
#
# - The container: `--network none`, the user's uid and gid, no capabilities,
#   `no-new-privileges`, its own `/tmp` and `/run/user/<uid>` (tmpfs), memlock without a
#   limit (the test `wardd` does the `mlockall`), process and memory ceilings.
# - The test services (8081–8084 of `dev/compose.yml`, already up on the host) are
#   reached through `dev/tests/service_relay.py`: Unix sockets in a 0700 folder of
#   `$XDG_RUNTIME_DIR` relay to `127.0.0.1:<port>`; the two fixed reads the suites make
#   with `compose.sh exec` (phishing `posts.log`, WordPress's `zxcvbn.min.js`) go through
#   the same relay. Never starts, stops nor recreates the services.
# - The test `wardd`, the test `ward-nm` and the suite run inside, exactly as on the host
#   (`dev/tests/nm.sh`, `extension.sh`, `login.sh`).
# - m2:Phase 66c: `make test-playwright` and `make test-chrome-policy` run here too. Their
#   Chrome is opened without `--no-sandbox` (`ward playwright` never passes it, and its
#   tests check so), and Chrome's sandbox needs an unprivileged user namespace, which the
#   default seccomp profile of Docker refuses. For these two suites the container gets
#   `dev/container-browser/seccomp.json`: every syscall allowed (`clone`/`unshare` with
#   the namespace flags included) except a fixed list of the ones that only matter with a
#   privilege or that open more of the kernel (mount, bpf, keyctl, ptrace, setns,
#   io_uring, kexec, modules...), refused with EPERM. The rest stays: no network, no
#   capabilities, `no-new-privileges`, the user's uid. The host Chrome makes the same
#   namespaces.
#   - playwright: the test binary of `crates/wardd/tests/playwright.rs` (built on the host
#     by the Makefile) runs here with `--ignored`; `target/tmp` (its `CARGO_TARGET_TMPDIR`)
#     is mounted read-write and the Python Playwright (`PW_PYTHONPATH`) read-only;
#   - chrome-policy: the first argument is the work dir of `chrome-policy.sh`
#     (`.dev/chrome-policy.*`), and its `etc-chrome` is mounted on /etc/opt/chrome.
# - Nothing stays up at the end: the container is removed, the relay killed and its
#   folder removed. The image stays in Docker, like the images of the other container tests.
#
# Usage: dev/tests/browser-container.sh <nm|extension|login|playwright|chrome-policy> <arguments...>

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"
unset MAKEFLAGS MAKELEVEL MFLAGS
cd "$ROOT"

SUITE="${1:?uso: browser-container.sh <nm|extension|login|playwright|chrome-policy> <argumentos da suíte>}"
shift
case "$SUITE" in nm|extension|login|playwright|chrome-policy) ;; *) echo "suíte desconhecida: $SUITE" >&2; exit 2 ;; esac
command -v docker >/dev/null || { echo "falta o docker" >&2; exit 1; }

UID_="$(id -u)"
GID_="$(id -g)"
RT="${XDG_RUNTIME_DIR:-/run/user/$UID_}"
[[ "$RT" == /run/user/"$UID_" && -d "$RT" && ! -L "$RT" ]] || { echo "XDG_RUNTIME_DIR inesperado: $RT" >&2; exit 1; }

IMAGE="ward-teste/browser:1"
echo "==> imagem $IMAGE (o build usa a rede; o teste não)"
docker build --quiet -t "$IMAGE" "$ROOT/dev/container-browser" >/dev/null

umask 077
RELAY="$(mktemp -d "$RT/ward-browser-relay.XXXXXX")"
NAME="ward-teste-browser-$$"
RELAY_PID=""
cleanup() {
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    if [ -n "$RELAY_PID" ]; then
        kill "$RELAY_PID" 2>/dev/null || true
        wait "$RELAY_PID" 2>/dev/null || true
    fi
    rm -rf -- "$RELAY"
}
trap cleanup EXIT
/usr/bin/python3 -I -B "$ROOT/dev/tests/service_relay.py" host "$RELAY" &
RELAY_PID=$!
for _ in $(seq 1 100); do [ -f "$RELAY/host.ready" ] && break; sleep 0.05; done
[ -f "$RELAY/host.ready" ] || { echo "o relay dos serviços não subiu" >&2; exit 1; }

mkdir -p "$ROOT/docs/mission/evidence"
# m2:Phase 66c: what only the playwright and chrome-policy suites get.
EXTRA=()
RT_SIZE=64m
case "$SUITE" in
    playwright|chrome-policy)
        SECCOMP="$ROOT/dev/container-browser/seccomp.json"
        [ -f "$SECCOMP" ] && [ ! -L "$SECCOMP" ] || { echo "falta $SECCOMP" >&2; exit 1; }
        EXTRA+=(--security-opt "seccomp=$SECCOMP")
        ;;
esac
case "$SUITE" in
    playwright)
        # Chrome profiles and the Python Playwright's TMPDIR live in <rt>.
        RT_SIZE=512m
        mkdir -p -m 0700 "$ROOT/target/tmp"
        EXTRA+=(--mount "type=bind,src=$ROOT/target/tmp,dst=$ROOT/target/tmp")
        PW="${PW_PYTHONPATH:-}"
        if [ -n "$PW" ]; then
            [[ "$PW" == /* && "$PW" != *..* && -d "$PW/playwright" ]] \
                || { echo "PW_PYTHONPATH=<site-packages com o playwright> (absoluto)" >&2; exit 1; }
            case "$PW" in
                "$ROOT"/.dev/*) ;;
                *) EXTRA+=(--mount "type=bind,src=$PW,dst=$PW,readonly") ;;
            esac
            EXTRA+=(-e "PW_PYTHONPATH=$PW")
        fi
        EXTRA+=(-e "NO_PLAYWRIGHT_PYTHON=${NO_PLAYWRIGHT_PYTHON:-}")
        ;;
    chrome-policy)
        WORK="${1:?uso: browser-container.sh chrome-policy <pasta de trabalho> <argumentos>}"
        case "$WORK" in "$ROOT"/.dev/chrome-policy.*) ;; *) echo "pasta de trabalho fora de .dev/chrome-policy.*" >&2; exit 2 ;; esac
        [[ "$WORK" != *..* && -d "$WORK/etc-chrome" && ! -L "$WORK/etc-chrome" ]] \
            || { echo "falta $WORK/etc-chrome" >&2; exit 1; }
        EXTRA+=(--mount "type=bind,src=$WORK/etc-chrome,dst=/etc/opt/chrome")
        ;;
esac
echo "==> make test-$SUITE no contêiner (sem rede; Chrome sem política nem host do Ward)"
set +e
docker run --rm --name "$NAME" --network none --pull never \
    --user "$UID_:$GID_" --cap-drop ALL --security-opt no-new-privileges \
    --ulimit memlock=-1:-1 --pids-limit 4096 --memory 6g --shm-size 1g \
    --tmpfs /tmp:rw,nosuid,nodev,size=512m,mode=1777 \
    --tmpfs "/run/user/$UID_:rw,nosuid,nodev,size=$RT_SIZE,mode=0700,uid=$UID_,gid=$GID_" \
    --mount "type=bind,src=$ROOT,dst=$ROOT,readonly" \
    --mount "type=bind,src=$ROOT/.dev,dst=$ROOT/.dev" \
    --mount "type=bind,src=$ROOT/docs/mission/evidence,dst=$ROOT/docs/mission/evidence" \
    --mount "type=bind,src=$RELAY,dst=/relay" \
    -e HOME=/tmp -e LANG=C.UTF-8 -e "XDG_RUNTIME_DIR=/run/user/$UID_" \
    "${EXTRA[@]}" \
    "$IMAGE" /bin/bash -p "$ROOT/dev/container-browser/inside.sh" "$SUITE" "$ROOT" "$@"
rc=$?
set -e
exit "$rc"
