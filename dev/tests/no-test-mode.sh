#!/bin/bash -p
# `make test-no-test-mode` (Fase 23a; D63, D7; V3.37). A exceção do app de teste
# (papel app sem o egid, `WARD_DEV_APP`; papel extensão sem o egid, `WARD_DEV_EXTENSAO`,
# Fase 41) não vaza para o release:
#
# 1. nenhum membro do workspace liga `wardd/test-mode` nem `ward-platform/test-mode`
#    por dependência normal ou de build (`cargo tree -e normal,build`, sem
#    `--all-features`); a conferência acusa o caso forçado com `--features`, para não
#    passar por engano;
# 2. os testes de unidade do `wardd` e do `ward-platform` **sem** a feature (o par
#    sem egid aberto pelo lançador fica agente);
# 3. `strings` no `wardd` de release e na biblioteca `ward-platform` de release não
#    acha `WARD_DEV_APP` (nem `WARD_DEV_DIR`, nem o app de teste).
#
# Compila em target/no-test-mode (o `make check` usa --all-features em target/).

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$RAIZ/dev/env.sh"
cd "$RAIZ"

CARGO="${CARGO:?o make passa o CARGO}"
ALVO="$RAIZ/target/no-test-mode"
TMP="$(mktemp -d)"
trap 'rm -rf -- "$TMP"' EXIT
falhou=0
# Linhas `pacote vX (caminho)|features` dos crates do Ward que têm `test-mode` (ou o
# nome antigo `modo-teste`, m2:DT12: um rename desfeito também acusa).
com_modo_teste() {
    grep -E '^(wardd|ward-platform) v[^|]*\|.*(test-mode|modo-teste)' "$1" || true
}

echo "==> 1/3 cargo tree: ninguém liga test-mode em dependência normal"
"$CARGO" tree --workspace --locked -e normal,build -f '{p}|{f}' --prefix none > "$TMP/arvore"
for pacote in ward wardd ward-nm ward-app-core ward-test-bridge; do
    "$CARGO" tree -p "$pacote" --locked -e normal,build -f '{p}|{f}' --prefix none >> "$TMP/arvore"
done
achado="$(com_modo_teste "$TMP/arvore")"
if [ -n "$achado" ]; then
    echo "sem-modo-teste: test-mode ligado por dependência normal:" >&2
    echo "$achado" >&2
    falhou=1
fi
# Autoteste: com a feature forçada, a conferência tem de acusar.
"$CARGO" tree -p wardd --features test-mode --locked -e normal,build -f '{p}|{f}' --prefix none > "$TMP/forcado"
if [ -z "$(com_modo_teste "$TMP/forcado")" ]; then
    echo "sem-modo-teste: a conferência do cargo tree não acusou o caso forçado" >&2
    falhou=1
fi

echo "==> 2/3 testes de unidade sem a feature"
"$CARGO" test --locked --target-dir "$ALVO" -p wardd -p ward-platform --lib

echo "==> 3/3 strings no release"
"$CARGO" build --release --locked --target-dir "$ALVO" -p wardd --bin wardd
# Fase 40: o host da extensão do release também não pode ter a forma de teste.
"$CARGO" build --release --locked --target-dir "$ALVO" -p ward-nm --bin ward-nm
"$CARGO" build --release --locked --target-dir "$ALVO" -p ward-platform --lib \
    --message-format=json > "$TMP/build.json"
RLIB="$(grep -o '"filenames":\["[^"]*/libward_platform[^"]*\.rlib"' "$TMP/build.json" \
    | sed -e 's/.*\["//' -e 's/"$//' | tail -n 1 || true)"
[ -f "$RLIB" ] || { echo "sem-modo-teste: rlib do ward-platform não achada" >&2; exit 1; }
# Na rlib, a metadata guarda os nomes dos itens tirados pelo `cfg` (para o diagnóstico
# "item configurado fora"), sem código: lá só se procuram os textos.
conferir() {
    local binario="$1"
    shift
    # `strings` vai para um arquivo antes do `grep -q` (SIGPIPE com `pipefail`).
    strings -a -n 4 -- "$binario" > "$TMP/textos"
    for marca in "$@"; do
        if grep -qF -- "$marca" "$TMP/textos"; then
            echo "sem-modo-teste: ${binario#"$RAIZ"/} contém \"$marca\"" >&2
            falhou=1
        fi
    done
}
# Fase 41: a exceção da extensão de teste (`WARD_DEV_EXTENSAO`) também não.
TEXTOS=(WARD_DEV_APP WARD_DEV_DIR WARD_DEV_SOCKET_DIR WARD_DEV_EXTENSAO wardd-app-teste wardd-aprovador-teste ward-ponte-teste)
conferir "$RLIB" "${TEXTOS[@]}"
conferir "$ALVO/release/ward-nm" "${TEXTOS[@]}"
conferir "$ALVO/release/wardd" "${TEXTOS[@]}" classify_test_app classify_test_extension_host \
    TestBinary launch_for_test
# O binário conferido é mesmo o daemon (tem o socket do sistema).
grep -qF -- '/run/ward' "$TMP/textos" || {
    echo "sem-modo-teste: '/run/ward' não achado; o binário conferido é mesmo o wardd?" >&2
    falhou=1
}

[ $falhou -eq 0 ] && echo "sem-modo-teste: release sem WARD_DEV_APP nem a exceção do app de teste"
exit $falhou
