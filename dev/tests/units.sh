#!/bin/bash -p
# `make test-units` (Fase 24; V3.21; D46, D57, D62).
#
# 1. Sintaxe: `systemd-analyze verify` das units de `install/systemd/` numa cópia em
#    `.dev/` (instância `wardd@1000`), sem tocar no systemd do host (nada é instalado
#    nem recarregado; o verify só lê os arquivos passados).
# 2. Contêiner: uma imagem `ubuntu:24.04` (mesma digest das Fases 21b/22) com o
#    systemd (`dev/container-units/Dockerfile`; rede só no build), rodando sem rede,
#    com root só dentro dele. O `inside.sh` instala as units e o `wardd` (sem
#    `test-mode`), e confere que a primeira conexão ao socket sobe o daemon como o
#    uid 1000, com `LimitCORE=0`, `LimitMEMLOCK=infinity` e sem `no_new_privs`; com o
#    cofre de teste destravado, `VmLck` cobre o `VmRSS` (mlockall, D62), e o mesmo
#    binário com `RLIMIT_MEMLOCK` de 8 MiB sai ≠ 0 com mensagem (V3.34).
# 3. V4.38 (Fase 26, D63): com os grupos `ward-app`/`ward-nm` e cópias setgid do
#    cliente mínimo, `ward run <livre> -- ward-app …` (a CLI de release) não dá papel
#    app ao filho do daemon: o par tem o grupo e o pai certos, mas não é o pid que o
#    lançador abriu; o journal tem "papel app recusado".
#
# O systemd precisa escrever no /sys/fs/cgroup do contêiner (a subárvore dele, com
# `--cgroupns=private`), que o Docker monta somente leitura: o contêiner ganha
# CAP_SYS_ADMIN e sai do perfil do AppArmor só para o `start.sh` remontar. Continua sem
# rede, sem `--privileged`, sem montar nada do host além de /opt/teste (ro) e é apagado
# no fim. O `--ulimit memlock=-1:-1` deixa o systemd de dentro aplicar o
# `LimitMEMLOCK=infinity` da unit (o limite do Docker é 8 MiB).
#
# Fica fora do `make check` (exige Docker). Uso: dev/tests/units.sh <wardd> <cliente> <ward>

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/services/common.sh
. "$RAIZ/dev/services/common.sh"

WARDD="${1:?uso: units.sh <wardd> <wardd-test-client> <ward>}"
CLIENTE="${2:?uso: units.sh <wardd> <wardd-test-client> <ward>}"
WARD="${3:?uso: units.sh <wardd> <wardd-test-client> <ward>}"
for b in "$WARDD" "$CLIENTE" "$WARD"; do
    [ -x "$b" ] || { echo "binário não encontrado: $b" >&2; exit 1; }
done
for p in docker systemd-analyze; do
    command -v "$p" >/dev/null 2>&1 || { echo "falta o programa: $p" >&2; exit 1; }
done

mkdir -p -m 0700 "$RAIZ/.dev"
TMP="$(mktemp -d "$RAIZ/.dev/container-units.XXXXXX")"
NOME="ward-units-$$"
trap 'docker rm -f "$NOME" >/dev/null 2>&1 || true; rm -rf -- "$TMP"' EXIT
chmod 0755 "$TMP"
install -m 0644 "$RAIZ/install/systemd/wardd@.socket" "$RAIZ/install/systemd/wardd@.service" "$TMP/"

echo "==> systemd-analyze verify"
# O verify reclama do ExecStart que não existe no host; o resto tem de sair limpo.
saida="$(systemd-analyze verify --man=no "$TMP/wardd@1000.socket" "$TMP/wardd@1000.service" 2>&1 || true)"
resto="$(printf '%s\n' "$saida" | grep -v -e '/usr/local/bin/wardd' -e '^$' || true)"
if [ -n "$resto" ]; then
    printf '%s\n' "$saida"
    echo "FALHOU: systemd-analyze verify"
    exit 1
fi
echo "ok: systemd-analyze verify"
# Controle: o mesmo verify acusa uma diretiva inválida (senão o "ok" acima não vale nada).
mkdir "$TMP/controle"
install -m 0644 "$TMP/wardd@.socket" "$TMP/controle/"
sed 's/^LimitCORE=0$/LimitCORE=nao-e-numero\nDiretivaInventada=1/' "$TMP/wardd@.service" >"$TMP/controle/wardd@.service"
saida="$(systemd-analyze verify --man=no "$TMP/controle/wardd@1000.service" 2>&1 || true)"
if printf '%s\n' "$saida" | grep -q -e 'DiretivaInventada' -e 'LimitCORE'; then
    echo "ok: o verify acusa diretiva inválida (controle)"
else
    printf '%s\n' "$saida"
    echo "FALHOU: o verify não acusou a diretiva inválida do controle"
    exit 1
fi

install -m 0755 "$WARDD" "$TMP/wardd"
install -m 0755 "$CLIENTE" "$TMP/cliente"
install -m 0755 "$WARD" "$TMP/ward"
# V3.34 (D62): o cofre de teste (só valores falsos, de `make dev-vault`) para o `wardd`
# do contêiner destravar e mostrar o `VmLck`.
COFRE_TESTE="$RAIZ/.dev/test-vault.ward"
[ -f "$COFRE_TESTE" ] || { echo "falta $COFRE_TESTE; rode 'make dev-vault' antes" >&2; exit 1; }
install -m 0600 "$COFRE_TESTE" "$TMP/cofre.ward"

echo "==> imagem com systemd"
IMAGEM="ward-teste/units-systemd:1"
docker build --quiet -t "$IMAGEM" "$RAIZ/dev/container-units" >/dev/null

echo "==> contêiner"
docker run -d --name "$NOME" --network none --pull never \
    --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cgroupns=private --cap-add SYS_ADMIN --security-opt apparmor=unconfined \
    --ulimit memlock=-1:-1 \
    -v "$TMP:/opt/teste:ro" \
    -v "$RAIZ/dev/container-units/inside.sh:/opt/inside.sh:ro" \
    "$IMAGEM" >/dev/null
docker exec "$NOME" /bin/bash -p /opt/inside.sh
