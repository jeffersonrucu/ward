#!/bin/bash -p
# Fase 65 (m2, Bateria E): capturas das telas do app pela ponte de teste, para comparar
# com docs/mission/arquivo/m1/evidence/final/. Mesmo arranjo do dev/tests/app-e2e.sh:
# um wardd de teste só deste roteiro (socket próprio em WARD_DEV_SOCKET_DIR), a ponte
# aberta pelo daemon (D63), Chrome headless pelo Playwright do Python. O cofre de teste
# volta ao que era no fim, e .dev/backups (criado pelas gravações) é apagado.
#
# Precisa de .dev/f65-password e .dev/f65-pin (0600) com a senha e o PIN de teste (D15).
# Não rodou na Fase 65: a chamada foi negada pela permissão do ambiente do agente.
# Uso: PW_PYTHONPATH=<site-packages> f65-ui.sh <raiz do repositório> <pasta de saída>
export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="${1:?raiz}"
OUT="${2:?saída}"
. "$ROOT/dev/env.sh"
PW="${PW_PYTHONPATH:?PW_PYTHONPATH}"
DEV="$ROOT/.dev"
UID_="$(id -u)"
SOCKDIR="/run/user/$UID_/ward-f65-ui"
HERE="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"

umask 077
BACKUP="$DEV/f65-ui-backup"
rm -rf "$BACKUP"
mkdir -m 0700 "$BACKUP"
cp -p "$DEV/test-vault.ward" "$BACKUP/"
[[ -f "$DEV/test-vault.state" ]] && cp -p "$DEV/test-vault.state" "$BACKUP/"
[[ ! -e "$DEV/backups" ]] || { echo "f65-ui: .dev/backups já existe; apague antes" >&2; exit 1; }

cleanup() {
    WARD_DEV_SOCKET_DIR="$SOCKDIR" "$ROOT/dev/daemon.sh" stop > /dev/null 2>&1 || true
    cp -p "$BACKUP/test-vault.ward" "$DEV/test-vault.ward"
    if [[ -f "$BACKUP/test-vault.state" ]]; then
        cp -p "$BACKUP/test-vault.state" "$DEV/test-vault.state"
    else
        rm -f "$DEV/test-vault.state"
    fi
    rm -rf "$BACKUP" "$DEV/backups" "$DEV/f65-mark"
}
trap cleanup EXIT

echo "==> daemon de teste em $SOCKDIR com a ponte (D63)"
WARD_DEV_SOCKET_DIR="$SOCKDIR" WARD_DEV_APP="$ROOT/target/debug/ward-test-bridge" "$ROOT/dev/daemon.sh" up
URL="$(WARD_DEV_SOCKET_DIR="$SOCKDIR" "$ROOT/dev/bridge.sh")"

echo "==> capturas (Chrome headless)"
set +e
printf '%s\n' "$URL" | /usr/bin/python3 -I -B "$HERE/f65_ui.py" "$ROOT" "$SOCKDIR" "$PW" "$OUT"
qa=$?
set -e
unset URL
WARD_DEV_SOCKET_DIR="$SOCKDIR" "$ROOT/dev/daemon.sh" stop > /dev/null 2>&1 || true
echo "==> varredura de logs (V2.8/V5.27): .dev/logs"
/usr/bin/python3 -I -B "$ROOT/dev/tests/log-scan.py" "$DEV/logs"
exit "$qa"
