#!/bin/bash -p
# V5.32 (D48, D57, D64; S8): o WebKitGTK real com o ambiente de D57 sob `strace -f`.
#
# O `ward-app` de verdade não chega ao GTK neste teste: a partida (D47) recusa depurador
# e exige o daemon instalado com o setgid. Por isso o script monta, só em `.dev/`, uma
# cópia do `app/src-tauri` em que o `main` pula a partida e abre a janela com o serviço
# em erro (o resto é igual: mesmo Tauri, mesma configuração, mesma interface de
# `app/dist`). Nenhum código que pule a partida entra no repositório.
#
# O que confere (sai 1 se algo falhar):
# - o ambiente passado é exatamente o de D57, com os valores do `lancador.rs`;
# - nenhum processo faz `open`/`connect` em `/tmp/nvidia-mps`, e um listener do
#   "atacante" em `/tmp/nvidia-mps/control` (SOCK_SEQPACKET, o tipo do cliente MPS)
#   não recebe conexão;
# - um plugin-marca em `~/.local/share/gstreamer-1.0/plugins` não carrega;
# - lista os `open`/`connect`/`bind`/`mkdir` em `/tmp`, `/var/tmp`, `/dev/shm`,
#   `/run/user/<uid>` (fora do `wayland-<n>` e do `bus`), no HOME e os sockets
#   abstratos, para o handoff (D64: canal novo volta ao planner).
# Iscas dos canais achados na Fase 29b (código nas pastas de plugin do HOME que o
# `gst-plugin-scanner` varre, e o `/dev/shm/cuda_injection_path_shm` do libcuda): se
# alguma carregar, sai 2 ("canal pendente no planner"), separado da falha de V5.32.
#
# Abre uma janela do app na sessão Wayland do usuário por ~20 s. Exige o WebKitGTK 4.1
# (as bibliotecas de desenvolvimento para compilar: `libwebkit2gtk-4.1-dev` ou
# PKG_CONFIG_PATH, R32), `strace`, `gcc`, uma sessão com um único `wayland-<n>` e
# `make app-front` antes. O HOME é falso, em `.dev/webkit-real/`; o cofre não é tocado.
# Uso: dev/testes/webkit-real.sh <cargo> [--sem-mps]  (ou `make teste-webkit-real`)
#   --sem-mps: roda sem `CUDA_MPS_PIPE_DIRECTORY`, para ver o ataque que D64 fecha.

set -euo pipefail
export PATH=/usr/bin:/bin
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$RAIZ/dev/ambiente.sh"

CARGO="${1:?uso: webkit-real.sh <cargo> [--sem-mps]}"
SEM_MPS=0
[[ "${2:-}" == "--sem-mps" ]] && SEM_MPS=1
[[ -x "$CARGO" ]] || { echo "webkit-real: cargo não encontrado em $CARGO" >&2; exit 1; }
for f in strace gcc python3 timeout; do
    command -v "$f" > /dev/null || { echo "webkit-real: falta $f" >&2; exit 1; }
done
[[ -f "$RAIZ/app/dist/index.html" ]] || { echo "webkit-real: rode 'make app-front' antes" >&2; exit 1; }

UID_="$(id -u)"
RT="/run/user/$UID_"
[[ -d "$RT" && ! -L "$RT" ]] || { echo "webkit-real: sem $RT" >&2; exit 1; }
mapfile -t WAYLANDS < <(find "$RT" -maxdepth 1 -type s -regex '.*/wayland-[0-9]+' -printf '%f\n')
[[ ${#WAYLANDS[@]} -eq 1 ]] || { echo "webkit-real: precisa de exatamente um wayland-<n> em $RT (achou ${#WAYLANDS[@]})" >&2; exit 1; }
[[ -S "$RT/bus" && ! -L "$RT/bus" ]] || { echo "webkit-real: sem $RT/bus" >&2; exit 1; }

# Iscas fora do .dev: nada pode existir antes (não sobrescreve nada de ninguém).
MPS=/tmp/nvidia-mps
SHM_CUDA=/dev/shm/cuda_injection_path_shm
[[ ! -e "$MPS" && ! -L "$MPS" ]] || { echo "webkit-real: $MPS já existe; não mexo" >&2; exit 1; }
[[ ! -e "$SHM_CUDA" && ! -L "$SHM_CUDA" ]] || { echo "webkit-real: $SHM_CUDA já existe; não mexo" >&2; exit 1; }

BASE="$RAIZ/.dev/webkit-real"
mkdir -p -m 0700 "$BASE"
chmod 0700 "$BASE"

# --- Valores de D57: os fixos saem do lancador.rs (uma fonte só) -------------------
LANCADOR="$RAIZ/crates/ward-plataforma/src/lancador.rs"
valor_fixo() { # nome da variável em VARIAVEIS_FIXAS
    python3 -I - "$LANCADOR" "$1" "$(uname -m)" <<'PY'
import re, sys
s = open(sys.argv[1]).read()
consts = {}
# Constante com `#[cfg(target_arch = …)]`: só a da arquitetura desta máquina.
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
[[ "$GST" == "/usr/lib/$(uname -m)-linux-gnu/gstreamer-1.0" && -d "$GST" ]] \
    || { echo "webkit-real: GST_PLUGIN_SYSTEM_PATH inesperado ou ausente: $GST" >&2; exit 1; }

# --- Harness: cópia do app em .dev, sem a partida -----------------------------------
H="$BASE/src-tauri"
rm -rf "$H"
mkdir -p "$H"
tar -C "$RAIZ/app/src-tauri" --exclude=./target -cf - . | tar -C "$H" -xf -
python3 -I - "$H" "$RAIZ" <<'PY'
import sys
h, raiz = sys.argv[1], sys.argv[2]
p = h + "/Cargo.toml"
s = open(p).read()
assert s.count('path = "../nucleo"') == 2, "Cargo.toml do app mudou"
s = s.replace('path = "../nucleo"', 'path = "%s/app/nucleo"' % raiz)
s = s.replace("lto = true", "lto = false").replace("codegen-units = 1", "codegen-units = 16")
open(p, "w").write(s)
p = h + "/tauri.conf.json"
s = open(p).read()
assert '"frontendDist": "../dist"' in s, "tauri.conf.json do app mudou"
open(p, "w").write(s.replace('"frontendDist": "../dist"', '"frontendDist": "%s/app/dist"' % raiz))
p = h + "/src/main.rs"
s = open(p).read()
ini, fim = "    // Antes do GTK (D47, D57).", "    let app = com_comandos("
assert s.count(ini) == 1 and s.count(fim) == 1, "main.rs do app mudou"
a, b = s.index(ini), s.index(fim)
s = s[:a] + "    // webkit-real.sh: sem a partida (só em .dev).\n    let servico = Arc::new(montar_servico(Err(ErroApp::ConexaoPerdida)));\n\n" + s[b:]
s = s.replace("use ward_app_nucleo::partida;\n", "")
open(p, "w").write(s)
PY
echo "==> compilando o harness em .dev/webkit-real (release, sem LTO)"
"$CARGO" build --quiet --release --locked --manifest-path "$H/Cargo.toml"
BIN="$H/target/release/ward-app"

# --- Iscas ---------------------------------------------------------------------------
SAIDA="$BASE/saida"
rm -rf "$SAIDA"
mkdir -m 0700 "$SAIDA"
CASA="$SAIDA/casa"
MARCA="$SAIDA/marcas"
cat > "$SAIDA/marca.c" <<EOF
#include <fcntl.h>
#include <unistd.h>
__attribute__((constructor)) static void marca(void) {
    int fd = open("$MARCA", O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (fd >= 0) { write(fd, ROT "\\n", sizeof(ROT "\\n") - 1); close(fd); }
}
EOF
isca() { # rótulo destino
    mkdir -p "$(dirname "$2")"
    gcc -shared -fPIC -DROT="\"$1\"" -o "$2" "$SAIDA/marca.c"
}
isca gst "$CASA/.local/share/gstreamer-1.0/plugins/libgstmarca.so"
isca frei0r "$CASA/.frei0r-1/lib/marca.so"
isca ladspa "$CASA/.ladspa/marca.so"
isca libvisual "$CASA/.libvisual/actor/libactor_marca.so"
isca lv2 "$CASA/.lv2/marca.lv2/marca.so"
printf '@prefix lv2: <http://lv2plug.in/ns/lv2core#> .\n<urn:ward:marca> a lv2:Plugin ; lv2:binary <marca.so> .\n' \
    > "$CASA/.lv2/marca.lv2/manifest.ttl"
isca cuda "$SAIDA/cuda-marca.so"

LPID=""
limpar() {
    [[ -n "$LPID" ]] && kill "$LPID" 2> /dev/null || true
    rm -f "$MPS/control"
    rmdir "$MPS" 2> /dev/null || true
    rm -f "$SHM_CUDA"
}
trap limpar EXIT

(umask 077 && printf '%s' "$SAIDA/cuda-marca.so" > "$SHM_CUDA")
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
' "$SAIDA/listener.log" &
LPID=$!
for _ in $(seq 50); do grep -qs pronto "$SAIDA/listener.log" && break; sleep 0.1; done
grep -qs pronto "$SAIDA/listener.log" || { echo "webkit-real: listener não subiu" >&2; exit 1; }

# --- Ambiente de D57 -----------------------------------------------------------------
LOCALE="${LANG:-C.UTF-8}"
[[ "$LOCALE" =~ ^[A-Za-z0-9_.@-]{1,64}$ ]] || LOCALE=C.UTF-8
VARS=(
    "XDG_RUNTIME_DIR=$RT"
    "WAYLAND_DISPLAY=${WAYLANDS[0]}"
    "DBUS_SESSION_BUS_ADDRESS=unix:path=$RT/bus"
    "XDG_SESSION_TYPE=$(valor_fixo XDG_SESSION_TYPE)"
    "GDK_BACKEND=$(valor_fixo GDK_BACKEND)"
    "HOME=$CASA"
    "PATH=$(valor_fixo PATH)"
    "LANG=$LOCALE"
    "GST_PLUGIN_SYSTEM_PATH=$GST"
    "NO_AT_BRIDGE=$(valor_fixo NO_AT_BRIDGE)"
    "WEBKIT_A11Y_BUS_ADDRESS=$(valor_fixo WEBKIT_A11Y_BUS_ADDRESS)"
)
[[ $SEM_MPS -eq 1 ]] || VARS+=("CUDA_MPS_PIPE_DIRECTORY=$(valor_fixo CUDA_MPS_PIPE_DIRECTORY)")

echo "==> abrindo o app (WebKitGTK real) sob strace -f por 20 s"
set +e
(cd / && timeout -s TERM 20 strace -f -qq -s 300 -e trace=%file,%network,%ipc -o "$SAIDA/strace.log" \
    env -i "${VARS[@]}" "$BIN" > "$SAIDA/app.out" 2>&1)
set -e
sleep 1
kill "$LPID" 2> /dev/null || true
wait "$LPID" 2> /dev/null || true
LPID=""

# --- Conferência ---------------------------------------------------------------------
falhou=0
pendente=0
ok() { echo "ok - $1"; }
nao() { echo "FALHOU - $1"; falhou=1; }

grep -q 'execve("[^"]*WebKitWebProcess' "$SAIDA/strace.log" && ok "o WebKitWebProcess subiu" \
    || nao "o WebKitWebProcess não subiu (o app abriu a janela?)"
# O `env -i` recebe as variáveis no argv; o `execve` do app mostra quantas recebeu.
ARGV_ENV="$(grep -m1 -E '^[0-9]+ execve\("/usr/bin/env"' "$SAIDA/strace.log" || true)"
ENV_LINHA="$(grep -m1 -E '^[0-9]+ execve\("[^"]*/ward-app"' "$SAIDA/strace.log" || true)"
for v in "${VARS[@]}"; do
    grep -qF "\"$v\"" <<< "$ARGV_ENV" || nao "a variável ${v%%=*} não foi passada ao app"
done
n_env="$(grep -o '/\* [0-9]* vars \*/' <<< "$ENV_LINHA" | tr -dc 0-9)"
[[ "$n_env" == "${#VARS[@]}" ]] && ok "o ambiente do app tem só as ${#VARS[@]} variáveis de D57" \
    || nao "o ambiente do app tem $n_env variáveis, esperado ${#VARS[@]}"

# Sem a varredura do GStreamer as iscas não provam nada: o registro tem de ter sido
# montado a partir da pasta do sistema.
grep -q "\"$GST\", O_RDONLY|O_NONBLOCK|O_CLOEXEC|O_DIRECTORY) = [0-9]" "$SAIDA/strace.log" \
    && ok "o GStreamer varreu a pasta de plugins do sistema" \
    || nao "o GStreamer não varreu $GST (o teste não provaria nada)"
if grep -q '/tmp/nvidia-mps' "$SAIDA/strace.log"; then
    nao "algum processo do app fez open/connect em /tmp/nvidia-mps"
else
    ok "nenhum processo do app tocou /tmp/nvidia-mps"
fi
if grep -q conexao "$SAIDA/listener.log"; then
    nao "o listener do atacante em /tmp/nvidia-mps/control recebeu conexão"
else
    ok "o listener do atacante em /tmp/nvidia-mps/control não recebeu conexão"
fi
if grep -qx gst "$MARCA" 2> /dev/null; then
    nao "o plugin-marca de ~/.local/share/gstreamer-1.0/plugins carregou"
else
    ok "o plugin-marca de ~/.local/share/gstreamer-1.0/plugins não carregou"
fi
for rot in frei0r ladspa libvisual lv2 cuda; do
    if grep -qx "$rot" "$MARCA" 2> /dev/null; then
        echo "PENDENTE - código da isca '$rot' carregou num processo do app (canal no planner, handoff 29b)"
        pendente=1
    else
        echo "ok - a isca '$rot' não carregou"
    fi
done

echo "==> open/connect/bind/mkdir em pastas graváveis fora do root (para o handoff)"
python3 -I - "$SAIDA/strace.log" "$CASA" "$RT" "${WAYLANDS[0]}" "$RAIZ" <<'PY'
import collections, re, sys
log, casa, rt, wl, raiz = sys.argv[1:6]
r = re.compile(r'^\d+ (open|openat|openat2|creat|connect|bind|mkdir|mkdirat|unlink|unlinkat|rename\w*|symlink\w*|link\w*)\((.*?)(?:\) = |\s+<unfinished)(.*)$')
cats = collections.OrderedDict()
for linha in open(log, errors="replace"):
    m = r.match(linha)
    if not m:
        continue
    sc, args, ret = m.groups()
    ok = "erro " + ret.split()[1] if ret.startswith("-1") else "ok"
    for p in re.findall(r'sun_path=@"([^"]*)"', args):
        cats.setdefault("socket abstrato", collections.Counter())[(sc, "@" + re.sub(r"\d+", "N", p.rstrip("\\0")), ok)] += 1
    for p in re.findall(r'"(/[^"]*)"', args):
        if p.startswith(casa):
            c, p = "HOME (falso)", "~" + p[len(casa):]
        elif p.startswith(raiz):
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

if [[ $falhou -ne 0 ]]; then
    echo "webkit-real: V5.32 falhou (strace em $SAIDA/strace.log)" >&2
    exit 1
fi
if [[ $pendente -ne 0 ]]; then
    echo "webkit-real: V5.32 ok, mas há canal pendente no planner (handoff 29b)" >&2
    exit 2
fi
echo "webkit-real: V5.32 ok"
