#!/bin/bash -p
# Channels of Phase 29b closed in Phase 39c (D57, D64; S8), without WebKitGTK.
#
# The real WebKitGTK (V5.32, `make test-webkit-real`) needs the built app; this script
# checks the two new variables of D57 against what the web process uses underneath:
#
# - GStreamer: `gst-inspect-1.0` builds the registry from scratch (fake HOME) and, like
#   the web process, runs `gst-plugin-scanner` over `GST_PLUGIN_SYSTEM_PATH`. Decoys in
#   `~/.frei0r-1/lib`, `~/.ladspa`, `~/.libvisual/actor`, `~/.lv2` and
#   `~/.local/share/gstreamer-1.0/plugins` write a mark if they are loaded. It also
#   watches `open` of `/dev/shm/cuda_injection_path_shm`, `/dev/shm/GFX_MEDIA_TRACE`,
#   `~/.mfx_trace` and `~/.nv/ComputeCache`.
# - GPU driver: `eglInitialize` (what WebKitGTK does to draw) with a
#   `~/.nv/nvidia-application-profile-globals-rc` planted with `enabled: true`; the
#   profiles in `~/.nv/nvidia-application-profiles-rc` must not be read.
#
# Runs each part twice: without the 39c variables (control: the attack has to show up,
# otherwise the test would prove nothing) and with the whole D57 environment, with the
# values read from `launcher.rs`. Without an NVIDIA driver, the EGL part is skipped with
# a warning. Nothing is written outside `.dev/gst-channels/`; no vault is touched.
# Usage: dev/tests/gst-channels.sh  (or `make test-gst-channels`)

set -euo pipefail
export PATH=/usr/bin:/bin
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$ROOT/dev/env.sh"

for f in gst-inspect-1.0 strace gcc python3 timeout; do
    command -v "$f" > /dev/null || { echo "canais-gst: falta $f" >&2; exit 1; }
done

BASE="$ROOT/.dev/gst-channels"
rm -rf "$BASE"
mkdir -p -m 0700 "$BASE"

LAUNCHER="$ROOT/crates/ward-platform/src/launcher.rs"
fixed_value() { # variable name in FIXED_VARIABLES (same reader as webkit-real.sh)
    python3 -I - "$LAUNCHER" "$1" "$(uname -m)" <<'PY'
import re, sys
s = open(sys.argv[1]).read()
consts = {}
for cfg, name, val in re.findall(r'(?:#\[cfg\(target_arch = "(\w+)"\)\]\s*)?pub const (\w+): &str = "([^"]*)";', s):
    if not cfg or cfg == sys.argv[3]:
        consts[name] = val
bloco = s[s.index("pub const FIXED_VARIABLES"):]
bloco = bloco[:bloco.index("];")]
for name, val in re.findall(r'\("(\w+)",\s*("[^"]*"|\w+)\)', bloco):
    if name == sys.argv[2]:
        print(val.strip('"') if val.startswith('"') else consts[val])
        sys.exit(0)
sys.exit(1)
PY
}
GST="$(fixed_value GST_PLUGIN_SYSTEM_PATH)"
ALLOWLIST="$(fixed_value GST_PLUGIN_LOADING_WHITELIST)"
GL="$(fixed_value __GL_APPLICATION_PROFILE)"
MPS="$(fixed_value CUDA_MPS_PIPE_DIRECTORY)"
[[ -d "$GST" ]] || { echo "canais-gst: sem a pasta de plugins $GST" >&2; exit 1; }
[[ -n "$ALLOWLIST" && "$GL" == "0" ]] || { echo "canais-gst: variáveis da 39c ausentes do launcher.rs" >&2; exit 1; }

cat > "$BASE/mark.c" <<'EOF'
#include <fcntl.h>
#include <unistd.h>
__attribute__((constructor)) static void mark(void) {
    int fd = open(MARKS, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (fd >= 0) { write(fd, LABEL "\n", sizeof(LABEL "\n") - 1); close(fd); }
}
EOF

failed=0
ok() { echo "ok - $1"; }
not_ok() { echo "FALHOU - $1"; failed=1; }

# --- GStreamer -------------------------------------------------------------------------
gst() { # label [VAR=value…]; prints the marks and leaves the strace in $BASE/<label>.strace
    local label="$1"
    shift
    local home="$BASE/$label/home" marks="$BASE/$label/marks"
    mkdir -p -m 0700 "$BASE/$label"
    decoy() {
        mkdir -p "$(dirname "$2")"
        gcc -shared -fPIC -DLABEL="\"$1\"" -DMARKS="\"$marks\"" -o "$2" "$BASE/mark.c"
    }
    decoy gst "$home/.local/share/gstreamer-1.0/plugins/libgstmark.so"
    decoy frei0r "$home/.frei0r-1/lib/mark.so"
    decoy ladspa "$home/.ladspa/mark.so"
    decoy libvisual "$home/.libvisual/actor/libactor_mark.so"
    decoy lv2 "$home/.lv2/mark.lv2/mark.so"
    printf '@prefix lv2: <http://lv2plug.in/ns/lv2core#> .\n<urn:ward:mark> a lv2:Plugin ; lv2:binary <mark.so> .\n' \
        > "$home/.lv2/mark.lv2/manifest.ttl"
    (cd / && timeout 120 strace -f -qq -e trace=%file,%network -o "$BASE/$label.strace" \
        env -i HOME="$home" PATH="$(fixed_value PATH)" LANG=C.UTF-8 \
        GST_PLUGIN_SYSTEM_PATH="$GST" CUDA_MPS_PIPE_DIRECTORY="$MPS" "$@" \
        gst-inspect-1.0 > "$BASE/$label.out" 2>&1) || true
    if [[ -f "$marks" ]]; then sort -u "$marks" | tr "\n" " "; fi
}

echo "==> GStreamer sem as variáveis da 39c (controle)"
marks="$(gst gst-without)"
echo "  marcas: ${marks:-nenhuma}"
for label in frei0r ladspa libvisual; do
    [[ " $marks " == *" $label "* ]] && ok "controle: a isca '$label' carregou sem a lista (o teste enxerga o ataque)" \
        || not_ok "controle: a isca '$label' não carregou nem sem a lista (o teste não provaria nada)"
done
grep -q "\"$GST\", O_RDONLY|O_NONBLOCK|O_CLOEXEC|O_DIRECTORY) = [0-9]" "$BASE/gst-without.strace" \
    && ok "controle: o scanner varreu a pasta do sistema" || not_ok "o scanner não varreu $GST"

echo "==> GStreamer com o ambiente de D57 (GST_PLUGIN_LOADING_WHITELIST)"
marks="$(gst gst-with GST_PLUGIN_LOADING_WHITELIST="$ALLOWLIST" __GL_APPLICATION_PROFILE="$GL")"
GST_HOME="$BASE/gst-with/home"
[[ -z "$marks" ]] && ok "nenhuma isca carregou (frei0r, LADSPA, libvisual, LV2, plugin do HOME)" \
    || not_ok "iscas carregaram com a lista: $marks"
for channel in /dev/shm/cuda_injection_path_shm /dev/shm/GFX_MEDIA_TRACE "$GST_HOME/.mfx_trace" "$GST_HOME/.nv/ComputeCache"; do
    if grep -qF "\"$channel" "$BASE/gst-with.strace"; then
        not_ok "o scanner abriu ${channel/#$GST_HOME/\~}"
    else
        ok "o scanner não abriu ${channel/#$GST_HOME/\~}"
    fi
done
grep -q '@cuda-uvmfd' "$BASE/gst-with.strace" && not_ok "o libcuda fez bind do socket abstrato @cuda-uvmfd" \
    || ok "nenhum bind de @cuda-uvmfd (o libcuda não carregou)"
grep -q '^app:' "$BASE/gst-with.out" && grep -q '^coreelements:' "$BASE/gst-with.out" \
    && ok "o appsink/appsrc e os elementos básicos continuam lá" \
    || not_ok "a lista tirou o app ou o coreelements"

# --- GPU driver (EGL) -------------------------------------------------------------------
egl() { # label [VAR=value…]
    local label="$1"
    shift
    local home="$BASE/$label/home"
    mkdir -p -m 0700 "$home/.nv"
    printf '{"enabled": true}\n' > "$home/.nv/nvidia-application-profile-globals-rc"
    printf '{"rules": [], "profiles": []}\n' > "$home/.nv/nvidia-application-profiles-rc"
    (cd / && timeout 60 strace -f -qq -e trace=openat -o "$BASE/$label.strace" \
        env -i HOME="$home" PATH="$(fixed_value PATH)" LANG=C.UTF-8 "$@" \
        python3 -I -c '
import ctypes
e = ctypes.CDLL("libEGL.so.1")
e.eglGetDisplay.restype = ctypes.c_void_p
d = e.eglGetDisplay(ctypes.c_void_p(0))
e.eglInitialize(ctypes.c_void_p(d), None, None)
' > "$BASE/$label.out" 2>&1) || true
    grep -c "/.nv/nvidia-application-profiles-rc\", O_RDONLY) = [0-9]" "$BASE/$label.strace" || true
}

echo "==> EGL com perfis da NVIDIA plantados no HOME"
n="$(egl egl-without)"
if [[ "$n" -eq 0 ]]; then
    echo "aviso - sem driver da NVIDIA que leia ~/.nv (o EGL não leu os perfis nem sem a variável); parte pulada"
else
    ok "controle: sem a variável o driver leu ~/.nv/nvidia-application-profiles-rc"
    n="$(egl egl-with __GL_APPLICATION_PROFILE="$GL")"
    [[ "$n" -eq 0 ]] && ok "com __GL_APPLICATION_PROFILE=0 o driver não lê os perfis (mesmo com enabled: true plantado)" \
        || not_ok "o driver leu ~/.nv/nvidia-application-profiles-rc com __GL_APPLICATION_PROFILE=0"
fi

if [[ $failed -ne 0 ]]; then
    echo "canais-gst: falhou (strace em $BASE)" >&2
    exit 1
fi
echo "canais-gst: canais da 29b fechados"
