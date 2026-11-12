#!/bin/bash -p
# V5.32 (D48, D57, D64; S8): the real WebKitGTK with the D57 environment under `strace -f`.
#
# The real `ward-app` does not reach GTK in this test: the startup (D47) refuses a
# debugger and requires the daemon installed with setgid. So the script builds, only in
# `.dev/`, a copy of `app/src-tauri` whose `main` skips the startup and opens the window
# with the service in error (the rest is the same: same Tauri, same configuration, same
# interface from `app/dist`). No code that skips the startup enters the repository.
#
# What it verifies (exits 1 if something fails):
# - the environment passed is exactly the D57 one, with the values of `launcher.rs`;
# - no process does `open`/`connect` on `/tmp/nvidia-mps`, and an "attacker" listener on
#   `/tmp/nvidia-mps/control` (SOCK_SEQPACKET, the MPS client type) gets no connection;
# - a mark plugin in `~/.local/share/gstreamer-1.0/plugins` does not load;
# - lists the `open`/`connect`/`bind`/`mkdir` calls in `/tmp`, `/var/tmp`, `/dev/shm`,
#   `/run/user/<uid>` (outside `wayland-<n>` and `bus`), in HOME and the abstract
#   sockets, for the handoff (D64: a new channel goes back to the planner).
# Decoys for the channels found in Phase 29b (code in the HOME plugin directories that
# `gst-plugin-scanner` scans, and the libcuda `/dev/shm/cuda_injection_path_shm`),
# closed in Phase 39c by `GST_PLUGIN_LOADING_WHITELIST` and `__GL_APPLICATION_PROFILE=0`:
# if any of them loads, or if some process opens `~/.nv/nvidia-application-profiles-rc*`,
# `~/.mfx_trace`, `/dev/shm/GFX_MEDIA_TRACE` or `/dev/shm/cuda_injection_path_shm`,
# V5.32 fails (exits 1). A new channel in the final list goes back to the planner (D64).
#
# Opens an app window in the user's Wayland session for ~20 s. Requires WebKitGTK 4.1
# (the development libraries to compile: `libwebkit2gtk-4.1-dev` or PKG_CONFIG_PATH,
# R32), `strace`, `gcc`, a session with a single `wayland-<n>` and `make app-front`
# first. HOME is fake, in `.dev/webkit-real/`; the vault is not touched.
# Usage: dev/tests/webkit-real.sh <cargo> [--no-mps] [--no-39c]  (or `make test-webkit-real`)
#   --no-mps: runs without `CUDA_MPS_PIPE_DIRECTORY`, to see the attack D64 closes.
#   --no-39c: runs without the two Phase 39c variables, to see the decoys load.
# The two flags can go together; any other argument is refused.

set -euo pipefail
export PATH=/usr/bin:/bin
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$ROOT/dev/env.sh"

CARGO="${1:?uso: webkit-real.sh <cargo> [--no-mps] [--no-39c]}"
NO_MPS=0
NO_39C=0
for arg in "${@:2}"; do
    case "$arg" in
        --no-mps) NO_MPS=1 ;;
        --no-39c) NO_39C=1 ;;
        *) echo "webkit-real: opção desconhecida: $arg (uso: webkit-real.sh <cargo> [--no-mps] [--no-39c])" >&2; exit 2 ;;
    esac
done
[[ -x "$CARGO" ]] || { echo "webkit-real: cargo não encontrado em $CARGO" >&2; exit 1; }
for f in strace gcc python3 timeout; do
    command -v "$f" > /dev/null || { echo "webkit-real: falta $f" >&2; exit 1; }
done
[[ -f "$ROOT/app/dist/index.html" ]] || { echo "webkit-real: rode 'make app-front' antes" >&2; exit 1; }

UID_="$(id -u)"
RT="/run/user/$UID_"
[[ -d "$RT" && ! -L "$RT" ]] || { echo "webkit-real: sem $RT" >&2; exit 1; }
mapfile -t WAYLANDS < <(find "$RT" -maxdepth 1 -type s -regex '.*/wayland-[0-9]+' -printf '%f\n')
[[ ${#WAYLANDS[@]} -eq 1 ]] || { echo "webkit-real: precisa de exatamente um wayland-<n> em $RT (achou ${#WAYLANDS[@]})" >&2; exit 1; }
[[ -S "$RT/bus" && ! -L "$RT/bus" ]] || { echo "webkit-real: sem $RT/bus" >&2; exit 1; }

# Decoys outside .dev: nothing may exist beforehand (overwrites nothing of anyone's).
MPS=/tmp/nvidia-mps
SHM_CUDA=/dev/shm/cuda_injection_path_shm
[[ ! -e "$MPS" && ! -L "$MPS" ]] || { echo "webkit-real: $MPS já existe; não mexo" >&2; exit 1; }
[[ ! -e "$SHM_CUDA" && ! -L "$SHM_CUDA" ]] || { echo "webkit-real: $SHM_CUDA já existe; não mexo" >&2; exit 1; }

BASE="$ROOT/.dev/webkit-real"
mkdir -p -m 0700 "$BASE"
chmod 0700 "$BASE"

# --- D57 values: the fixed ones come from launcher.rs (a single source) ------------
LAUNCHER="$ROOT/crates/ward-platform/src/launcher.rs"
fixed_value() { # variable name in FIXED_VARIABLES
    python3 -I - "$LAUNCHER" "$1" "$(uname -m)" <<'PY'
import re, sys
s = open(sys.argv[1]).read()
consts = {}
# Constant with `#[cfg(target_arch = …)]`: only the one for this machine's architecture.
for cfg, name, val in re.findall(r'(?:#\[cfg\(target_arch = "(\w+)"\)\]\s*)?pub const (\w+): &str = "([^"]*)";', s):
    if not cfg or cfg == sys.argv[3]:
        consts[name] = val
block = s[s.index("pub const FIXED_VARIABLES"):]
block = block[:block.index("];")]
for name, val in re.findall(r'\("(\w+)",\s*("[^"]*"|\w+)\)', block):
    if name == sys.argv[2]:
        print(val.strip('"') if val.startswith('"') else consts[val])
        sys.exit(0)
sys.exit(1)
PY
}
GST="$(fixed_value GST_PLUGIN_SYSTEM_PATH)"
[[ "$GST" == "/usr/lib/$(uname -m)-linux-gnu/gstreamer-1.0" && -d "$GST" ]] \
    || { echo "webkit-real: GST_PLUGIN_SYSTEM_PATH inesperado ou ausente: $GST" >&2; exit 1; }

# --- Harness: copy of the app in .dev, without the startup ---------------------------
H="$BASE/src-tauri"
rm -rf "$H"
mkdir -p "$H"
tar -C "$ROOT/app/src-tauri" --exclude=./target -cf - . | tar -C "$H" -xf -
# Phase 65a: the harness tray icon goes to a folder of this test in .dev
# ($BASE/tray-runtime, 0700), never to $XDG_RUNTIME_DIR/ward-tray, the folder of the
# installed app (checked under strace below).
TRAY_RT="$BASE/tray-runtime"
rm -rf "$TRAY_RT"
mkdir -m 0700 "$TRAY_RT"
python3 -I - "$H" "$ROOT" "$TRAY_RT" "$UID_" <<'PY'
import json, sys
h, root, tray_rt, uid = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
p = h + "/Cargo.toml"
s = open(p).read()
assert s.count('path = "../core"') == 2, "Cargo.toml do app mudou"
s = s.replace('path = "../core"', 'path = "%s/app/core"' % root)
s = s.replace("lto = true", "lto = false").replace("codegen-units = 1", "codegen-units = 16")
open(p, "w").write(s)
p = h + "/tauri.conf.json"
s = open(p).read()
assert '"frontendDist": "../dist"' in s, "tauri.conf.json do app mudou"
open(p, "w").write(s.replace('"frontendDist": "../dist"', '"frontendDist": "%s/app/dist"' % root))
p = h + "/src/main.rs"
s = open(p).read()
# Removes only the startup block; the rest of main (new variables included) stays.
def once(t):
    assert s.count(t) == 1, "main.rs do app mudou: %r" % t
    return s.index(t)
# 1) the startup call with its error closure, up to the closing `});`
a = once("    let mut app_startup = startup::start()")
b = s.index("\n    });\n", a) + len("\n    });\n")
s = s[:a] + "    // webkit-real.sh: sem a partida (só em .dev).\n" + s[b:]
# 2) daemon: none, as if the startup had not handed over the connection
t = "app_startup.take_daemon().ok_or(AppError::ConnectionLost)"
once(t)
s = s.replace(t, "Err::<DaemonSocket, AppError>(AppError::ConnectionLost)")
# 3) the startup watch call with its closure, up to the closing `});`
a = once("    app_startup.watch(move || {")
b = s.index("\n    });\n", a) + len("\n    });\n")
assert "for_watcher" in s[a:b], "main.rs do app mudou: watch"
s = s[:a] + "    drop(for_watcher);\n" + s[b:]
# 4) tray icon folder (Phase 65a): the test one, not the installed app's in XDG_RUNTIME_DIR
t = "tray::session_icons_dir()"
once(t)
s = s.replace(t, "tray::icons_dir(std::path::Path::new(%s), %d)" % (json.dumps(tray_rt), uid))
once("use ward_app_core::startup;\n")
s = s.replace("use ward_app_core::startup;\n", "")
import re
assert not re.search(r"\b(partida|app_startup|startup)\s*[.:(]", re.sub(r"//[^\n]*", "", s)), "main.rs do app mudou: partida restante"
open(p, "w").write(s)
PY
echo "==> compilando o harness em .dev/webkit-real (release, sem LTO)"
"$CARGO" build --quiet --release --locked --manifest-path "$H/Cargo.toml"
BIN="$H/target/release/ward-app"

# --- Decoys --------------------------------------------------------------------------
OUT_DIR="$BASE/out"
rm -rf "$OUT_DIR"
mkdir -m 0700 "$OUT_DIR"
FAKE_HOME="$OUT_DIR/home"
MARKS="$OUT_DIR/marks"
cat > "$OUT_DIR/mark.c" <<EOF
#include <fcntl.h>
#include <unistd.h>
__attribute__((constructor)) static void mark(void) {
    int fd = open("$MARKS", O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (fd >= 0) { write(fd, LABEL "\\n", sizeof(LABEL "\\n") - 1); close(fd); }
}
EOF
decoy() { # label destination
    mkdir -p "$(dirname "$2")"
    gcc -shared -fPIC -DLABEL="\"$1\"" -o "$2" "$OUT_DIR/mark.c"
}
decoy gst "$FAKE_HOME/.local/share/gstreamer-1.0/plugins/libgstmark.so"
decoy frei0r "$FAKE_HOME/.frei0r-1/lib/mark.so"
decoy ladspa "$FAKE_HOME/.ladspa/mark.so"
decoy libvisual "$FAKE_HOME/.libvisual/actor/libactor_mark.so"
decoy lv2 "$FAKE_HOME/.lv2/mark.lv2/mark.so"
printf '@prefix lv2: <http://lv2plug.in/ns/lv2core#> .\n<urn:ward:mark> a lv2:Plugin ; lv2:binary <mark.so> .\n' \
    > "$FAKE_HOME/.lv2/mark.lv2/manifest.ttl"
decoy cuda "$OUT_DIR/cuda-mark.so"

LISTENER_PID=""
cleanup() {
    [[ -n "$LISTENER_PID" ]] && kill "$LISTENER_PID" 2> /dev/null || true
    rm -f "$MPS/control"
    rmdir "$MPS" 2> /dev/null || true
    rm -f "$SHM_CUDA"
}
trap cleanup EXIT

(umask 077 && printf '%s' "$OUT_DIR/cuda-mark.so" > "$SHM_CUDA")
mkdir -m 0700 "$MPS"
python3 -I -c '
import socket, sys
s = socket.socket(socket.AF_UNIX, socket.SOCK_SEQPACKET)
s.bind("/tmp/nvidia-mps/control"); s.listen(8)
log = open(sys.argv[1], "a", buffering=1)
log.write("pronto\n")
while True:
    c, _ = s.accept()
    log.write("conexao\n")
    c.close()
' "$OUT_DIR/listener.log" &
LISTENER_PID=$!
for _ in $(seq 50); do grep -qs pronto "$OUT_DIR/listener.log" && break; sleep 0.1; done
grep -qs pronto "$OUT_DIR/listener.log" || { echo "webkit-real: listener não subiu" >&2; exit 1; }

# --- D57 environment -----------------------------------------------------------------
LOCALE="${LANG:-C.UTF-8}"
[[ "$LOCALE" =~ ^[A-Za-z0-9_.@-]{1,64}$ ]] || LOCALE=C.UTF-8
VARS=(
    "XDG_RUNTIME_DIR=$RT"
    "WAYLAND_DISPLAY=${WAYLANDS[0]}"
    "DBUS_SESSION_BUS_ADDRESS=unix:path=$RT/bus"
    "XDG_SESSION_TYPE=$(fixed_value XDG_SESSION_TYPE)"
    "GDK_BACKEND=$(fixed_value GDK_BACKEND)"
    "HOME=$FAKE_HOME"
    "PATH=$(fixed_value PATH)"
    "LANG=$LOCALE"
    "GST_PLUGIN_SYSTEM_PATH=$GST"
    "NO_AT_BRIDGE=$(fixed_value NO_AT_BRIDGE)"
    "WEBKIT_A11Y_BUS_ADDRESS=$(fixed_value WEBKIT_A11Y_BUS_ADDRESS)"
)
[[ $NO_MPS -eq 1 ]] || VARS+=("CUDA_MPS_PIPE_DIRECTORY=$(fixed_value CUDA_MPS_PIPE_DIRECTORY)")
if [[ $NO_39C -eq 0 ]]; then
    VARS+=("GST_PLUGIN_LOADING_WHITELIST=$(fixed_value GST_PLUGIN_LOADING_WHITELIST)")
    VARS+=("__GL_APPLICATION_PROFILE=$(fixed_value __GL_APPLICATION_PROFILE)")
fi

echo "==> abrindo o app (WebKitGTK real) sob strace -f por 20 s"
set +e
(cd / && timeout -s TERM 20 strace -f -qq -s 300 -e trace=%file,%network,%ipc -o "$OUT_DIR/strace.log" \
    env -i "${VARS[@]}" "$BIN" > "$OUT_DIR/app.out" 2>&1)
set -e
sleep 1
kill "$LISTENER_PID" 2> /dev/null || true
wait "$LISTENER_PID" 2> /dev/null || true
LISTENER_PID=""

# --- Verification --------------------------------------------------------------------
failed=0
ok() { echo "ok - $1"; }
not_ok() { echo "FALHOU - $1"; failed=1; }

grep -q 'execve("[^"]*WebKitWebProcess' "$OUT_DIR/strace.log" && ok "o WebKitWebProcess subiu" \
    || not_ok "o WebKitWebProcess não subiu (o app abriu a janela?)"
# `env -i` gets the variables in argv; the app's `execve` shows how many it got.
ARGV_ENV="$(grep -m1 -E '^[0-9]+ execve\("/usr/bin/env"' "$OUT_DIR/strace.log" || true)"
APP_EXECVE_LINE="$(grep -m1 -E '^[0-9]+ execve\("[^"]*/ward-app"' "$OUT_DIR/strace.log" || true)"
for v in "${VARS[@]}"; do
    grep -qF "\"$v\"" <<< "$ARGV_ENV" || not_ok "a variável ${v%%=*} não foi passada ao app"
done
n_env="$(grep -o '/\* [0-9]* vars \*/' <<< "$APP_EXECVE_LINE" | tr -dc 0-9)"
[[ "$n_env" == "${#VARS[@]}" ]] && ok "o ambiente do app tem só as ${#VARS[@]} variáveis de D57" \
    || not_ok "o ambiente do app tem $n_env variáveis, esperado ${#VARS[@]}"

# Without the GStreamer scan the decoys prove nothing: the registry has to have been
# built from the system directory.
grep -q "\"$GST\", O_RDONLY|O_NONBLOCK|O_CLOEXEC|O_DIRECTORY) = [0-9]" "$OUT_DIR/strace.log" \
    && ok "o GStreamer varreu a pasta de plugins do sistema" \
    || not_ok "o GStreamer não varreu $GST (o teste não provaria nada)"
if grep -q '/tmp/nvidia-mps' "$OUT_DIR/strace.log"; then
    not_ok "algum processo do app fez open/connect em /tmp/nvidia-mps"
else
    ok "nenhum processo do app tocou /tmp/nvidia-mps"
fi
if grep -q conexao "$OUT_DIR/listener.log"; then
    not_ok "o listener do atacante em /tmp/nvidia-mps/control recebeu conexão"
else
    ok "o listener do atacante em /tmp/nvidia-mps/control não recebeu conexão"
fi
if grep -qx gst "$MARKS" 2> /dev/null; then
    not_ok "o plugin-marca de ~/.local/share/gstreamer-1.0/plugins carregou"
else
    ok "o plugin-marca de ~/.local/share/gstreamer-1.0/plugins não carregou"
fi
# Phase 39c: the 29b channels are now part of V5.32.
# Phase 65a: the installed app's tray folder is never touched (the harness uses its own).
if grep -qF "\"$RT/ward-tray" "$OUT_DIR/strace.log"; then
    not_ok "algum processo do app tocou $RT/ward-tray (pasta do app instalado)"
else
    ok "nenhum processo do app tocou /run/user/<uid>/ward-tray (pasta do app instalado)"
fi
grep -qF "\"$TRAY_RT/ward-tray" "$OUT_DIR/strace.log" \
    && ok "o ícone da bandeja do harness ficou em .dev/webkit-real/tray-runtime" \
    || not_ok "o harness não criou a pasta do ícone em .dev/webkit-real/tray-runtime (a bandeja não subiu?)"
for label in frei0r ladspa libvisual lv2 cuda; do
    if grep -qx "$label" "$MARKS" 2> /dev/null; then
        not_ok "código da isca '$label' carregou num processo do app (canal da 29b, Fase 39c)"
    else
        ok "a isca '$label' não carregou"
    fi
done
for channel in "$FAKE_HOME/.nv/nvidia-application-profiles-rc" "$FAKE_HOME/.mfx_trace" \
    /dev/shm/GFX_MEDIA_TRACE "$SHM_CUDA" "$FAKE_HOME/.nv/ComputeCache"; do
    if grep -qF "\"$channel" "$OUT_DIR/strace.log"; then
        not_ok "algum processo do app abriu ${channel/#$FAKE_HOME/\~} (canal da 29b, Fase 39c)"
    else
        ok "nenhum processo do app abriu ${channel/#$FAKE_HOME/\~}"
    fi
done

echo "==> open/connect/bind/mkdir em pastas graváveis fora do root (para o handoff)"
python3 -I - "$OUT_DIR/strace.log" "$FAKE_HOME" "$RT" "${WAYLANDS[0]}" "$ROOT" <<'PY'
import collections, re, sys
log, home, rt, wl, root = sys.argv[1:6]
r = re.compile(r'^\d+ (open|openat|openat2|creat|connect|bind|mkdir|mkdirat|unlink|unlinkat|rename\w*|symlink\w*|link\w*)\((.*?)(?:\) = |\s+<unfinished)(.*)$')
cats = collections.OrderedDict()
for line in open(log, errors="replace"):
    m = r.match(line)
    if not m:
        continue
    sc, args, ret = m.groups()
    ok = "erro " + ret.split()[1] if ret.startswith("-1") else "ok"
    for p in re.findall(r'sun_path=@"([^"]*)"', args):
        cats.setdefault("socket abstrato", collections.Counter())[(sc, "@" + re.sub(r"\d+", "N", p.rstrip("\\0")), ok)] += 1
    for p in re.findall(r'"(/[^"]*)"', args):
        if p.startswith(home):
            c, p = "HOME (falso)", "~" + p[len(home):]
        elif p.startswith(root):
            continue
        elif re.match(r"^/(tmp|var/tmp|dev/shm)(/|$)", p):
            c = "/tmp, /var/tmp, /dev/shm"
        elif p.startswith(rt + "/") or p == rt:
            if p in (rt + "/" + wl, rt + "/bus"):
                continue
            c = "/run/user/<uid>"
        else:
            continue
        cats.setdefault(c, collections.Counter())[(sc, re.sub(r"[0-9a-f]{20,}", "<hash>", p), ok)] += 1
for c, cnt in cats.items():
    print("## " + c)
    for (sc, p, ok), n in sorted(cnt.items(), key=lambda x: (x[0][1], x[0][0])):
        print("  %-9s %-14s x%-3d %s" % (sc, ok, n, p))
PY

if grep -q "GStreamer element .* not found" "$OUT_DIR/app.out"; then
    echo "aviso - o WebKit avisou elemento do GStreamer ausente (a lista da Fase 39c tirou um que ele procura):"
    grep -o "GStreamer element .* not found" "$OUT_DIR/app.out" | sort -u | sed 's/^/  /'
fi
if [[ $failed -ne 0 ]]; then
    echo "webkit-real: V5.32 falhou (strace em $OUT_DIR/strace.log)" >&2
    exit 1
fi
echo "webkit-real: V5.32 ok"
