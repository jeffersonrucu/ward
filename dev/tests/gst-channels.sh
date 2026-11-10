#!/bin/bash -p
# Canais da Fase 29b fechados na Fase 39c (D57, D64; S8), sem o WebKitGTK.
#
# O WebKitGTK real (V5.32, `make test-webkit-real`) precisa do app compilado; este
# script confere as duas variáveis novas de D57 com o que o processo web usa de baixo:
#
# - GStreamer: o `gst-inspect-1.0` monta o registro do zero (HOME falso) e, como o
#   processo web, roda o `gst-plugin-scanner` sobre `GST_PLUGIN_SYSTEM_PATH`. Iscas em
#   `~/.frei0r-1/lib`, `~/.ladspa`, `~/.libvisual/actor`, `~/.lv2` e
#   `~/.local/share/gstreamer-1.0/plugins` gravam uma marca se forem carregadas. Também
#   olha `open` de `/dev/shm/cuda_injection_path_shm`, `/dev/shm/GFX_MEDIA_TRACE`,
#   `~/.mfx_trace` e `~/.nv/ComputeCache`.
# - Driver da GPU: `eglInitialize` (o que o WebKitGTK faz para desenhar) com um
#   `~/.nv/nvidia-application-profile-globals-rc` plantado com `enabled: true`; os
#   perfis em `~/.nv/nvidia-application-profiles-rc` não podem ser lidos.
#
# Roda cada parte duas vezes: sem as variáveis da 39c (controle: o ataque tem de
# aparecer, senão o teste não provaria nada) e com o ambiente de D57 inteiro, com os
# valores lidos do `lancador.rs`. Sem driver da NVIDIA, a parte do EGL é pulada com
# aviso. Nada é escrito fora de `.dev/gst-channels/`; nenhum cofre é tocado.
# Uso: dev/tests/gst-channels.sh  (ou `make test-gst-channels`)

set -euo pipefail
export PATH=/usr/bin:/bin
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$RAIZ/dev/env.sh"

for f in gst-inspect-1.0 strace gcc python3 timeout; do
    command -v "$f" > /dev/null || { echo "canais-gst: falta $f" >&2; exit 1; }
done

BASE="$RAIZ/.dev/gst-channels"
rm -rf "$BASE"
mkdir -p -m 0700 "$BASE"

LANCADOR="$RAIZ/crates/ward-platform/src/lancador.rs"
valor_fixo() { # nome da variável em VARIAVEIS_FIXAS (mesmo leitor do webkit-real.sh)
    python3 -I - "$LANCADOR" "$1" "$(uname -m)" <<'PY'
import re, sys
s = open(sys.argv[1]).read()
consts = {}
for cfg, nome, val in re.findall(r'(?:#\[cfg\(target_arch = "(\w+)"\)\]\s*)?pub const (\w+): &str = "([^"]*)";', s):
    if not cfg or cfg == sys.argv[3]:
        consts[nome] = val
bloco = s[s.index("pub const VARIAVEIS_FIXAS"):]
bloco = bloco[:bloco.index("];")]
for nome, val in re.findall(r'\("(\w+)",\s*("[^"]*"|\w+)\)', bloco):
    if nome == sys.argv[2]:
        print(val.strip('"') if val.startswith('"') else consts[val])
        sys.exit(0)
sys.exit(1)
PY
}
GST="$(valor_fixo GST_PLUGIN_SYSTEM_PATH)"
LISTA="$(valor_fixo GST_PLUGIN_LOADING_WHITELIST)"
GL="$(valor_fixo __GL_APPLICATION_PROFILE)"
MPS="$(valor_fixo CUDA_MPS_PIPE_DIRECTORY)"
[[ -d "$GST" ]] || { echo "canais-gst: sem a pasta de plugins $GST" >&2; exit 1; }
[[ -n "$LISTA" && "$GL" == "0" ]] || { echo "canais-gst: variáveis da 39c ausentes do lancador.rs" >&2; exit 1; }

cat > "$BASE/mark.c" <<'EOF'
#include <fcntl.h>
#include <unistd.h>
__attribute__((constructor)) static void marca(void) {
    int fd = open(MARCA, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (fd >= 0) { write(fd, ROT "\n", sizeof(ROT "\n") - 1); close(fd); }
}
EOF

falhou=0
ok() { echo "ok - $1"; }
nao() { echo "FALHOU - $1"; falhou=1; }

# --- GStreamer -------------------------------------------------------------------------
gst() { # rótulo [VAR=valor…]; imprime as marcas e deixa o strace em $BASE/<rótulo>.strace
    local rot="$1"
    shift
    local casa="$BASE/$rot/casa" marca="$BASE/$rot/marcas"
    mkdir -p -m 0700 "$BASE/$rot"
    isca() {
        mkdir -p "$(dirname "$2")"
        gcc -shared -fPIC -DROT="\"$1\"" -DMARCA="\"$marca\"" -o "$2" "$BASE/mark.c"
    }
    isca gst "$casa/.local/share/gstreamer-1.0/plugins/libgstmarca.so"
    isca frei0r "$casa/.frei0r-1/lib/marca.so"
    isca ladspa "$casa/.ladspa/marca.so"
    isca libvisual "$casa/.libvisual/actor/libactor_marca.so"
    isca lv2 "$casa/.lv2/marca.lv2/marca.so"
    printf '@prefix lv2: <http://lv2plug.in/ns/lv2core#> .\n<urn:ward:marca> a lv2:Plugin ; lv2:binary <marca.so> .\n' \
        > "$casa/.lv2/marca.lv2/manifest.ttl"
    (cd / && timeout 120 strace -f -qq -e trace=%file,%network -o "$BASE/$rot.strace" \
        env -i HOME="$casa" PATH="$(valor_fixo PATH)" LANG=C.UTF-8 \
        GST_PLUGIN_SYSTEM_PATH="$GST" CUDA_MPS_PIPE_DIRECTORY="$MPS" "$@" \
        gst-inspect-1.0 > "$BASE/$rot.saida" 2>&1) || true
    if [[ -f "$marca" ]]; then sort -u "$marca" | tr "\n" " "; fi
}

echo "==> GStreamer sem as variáveis da 39c (controle)"
marcas="$(gst gst-sem)"
echo "  marcas: ${marcas:-nenhuma}"
for rot in frei0r ladspa libvisual; do
    [[ " $marcas " == *" $rot "* ]] && ok "controle: a isca '$rot' carregou sem a lista (o teste enxerga o ataque)" \
        || nao "controle: a isca '$rot' não carregou nem sem a lista (o teste não provaria nada)"
done
grep -q "\"$GST\", O_RDONLY|O_NONBLOCK|O_CLOEXEC|O_DIRECTORY) = [0-9]" "$BASE/gst-sem.strace" \
    && ok "controle: o scanner varreu a pasta do sistema" || nao "o scanner não varreu $GST"

echo "==> GStreamer com o ambiente de D57 (GST_PLUGIN_LOADING_WHITELIST)"
marcas="$(gst gst-com GST_PLUGIN_LOADING_WHITELIST="$LISTA" __GL_APPLICATION_PROFILE="$GL")"
CASA="$BASE/gst-com/casa"
[[ -z "$marcas" ]] && ok "nenhuma isca carregou (frei0r, LADSPA, libvisual, LV2, plugin do HOME)" \
    || nao "iscas carregaram com a lista: $marcas"
for canal in /dev/shm/cuda_injection_path_shm /dev/shm/GFX_MEDIA_TRACE "$CASA/.mfx_trace" "$CASA/.nv/ComputeCache"; do
    if grep -qF "\"$canal" "$BASE/gst-com.strace"; then
        nao "o scanner abriu ${canal/#$CASA/\~}"
    else
        ok "o scanner não abriu ${canal/#$CASA/\~}"
    fi
done
grep -q '@cuda-uvmfd' "$BASE/gst-com.strace" && nao "o libcuda fez bind do socket abstrato @cuda-uvmfd" \
    || ok "nenhum bind de @cuda-uvmfd (o libcuda não carregou)"
grep -q '^app:' "$BASE/gst-com.saida" && grep -q '^coreelements:' "$BASE/gst-com.saida" \
    && ok "o appsink/appsrc e os elementos básicos continuam lá" \
    || nao "a lista tirou o app ou o coreelements"

# --- Driver da GPU (EGL) ----------------------------------------------------------------
egl() { # rótulo [VAR=valor…]
    local rot="$1"
    shift
    local casa="$BASE/$rot/casa"
    mkdir -p -m 0700 "$casa/.nv"
    printf '{"enabled": true}\n' > "$casa/.nv/nvidia-application-profile-globals-rc"
    printf '{"rules": [], "profiles": []}\n' > "$casa/.nv/nvidia-application-profiles-rc"
    (cd / && timeout 60 strace -f -qq -e trace=openat -o "$BASE/$rot.strace" \
        env -i HOME="$casa" PATH="$(valor_fixo PATH)" LANG=C.UTF-8 "$@" \
        python3 -I -c '
import ctypes
e = ctypes.CDLL("libEGL.so.1")
e.eglGetDisplay.restype = ctypes.c_void_p
d = e.eglGetDisplay(ctypes.c_void_p(0))
e.eglInitialize(ctypes.c_void_p(d), None, None)
' > "$BASE/$rot.saida" 2>&1) || true
    grep -c "/.nv/nvidia-application-profiles-rc\", O_RDONLY) = [0-9]" "$BASE/$rot.strace" || true
}

echo "==> EGL com perfis da NVIDIA plantados no HOME"
n="$(egl egl-sem)"
if [[ "$n" -eq 0 ]]; then
    echo "aviso - sem driver da NVIDIA que leia ~/.nv (o EGL não leu os perfis nem sem a variável); parte pulada"
else
    ok "controle: sem a variável o driver leu ~/.nv/nvidia-application-profiles-rc"
    n="$(egl egl-com __GL_APPLICATION_PROFILE="$GL")"
    [[ "$n" -eq 0 ]] && ok "com __GL_APPLICATION_PROFILE=0 o driver não lê os perfis (mesmo com enabled: true plantado)" \
        || nao "o driver leu ~/.nv/nvidia-application-profiles-rc com __GL_APPLICATION_PROFILE=0"
fi

if [[ $falhou -ne 0 ]]; then
    echo "canais-gst: falhou (strace em $BASE)" >&2
    exit 1
fi
echo "canais-gst: canais da 29b fechados"
