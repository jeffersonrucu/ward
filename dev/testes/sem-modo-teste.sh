#!/bin/bash -p
# `make teste-sem-modo-teste` (Fase 23a; D63, D7; V3.37). A exceção do app de teste
# (papel app sem o egid, `WARD_DEV_APP`; papel extensão sem o egid, `WARD_DEV_EXTENSAO`,
# Fase 41) não vaza para o release:
#
# 1. nenhum membro do workspace liga `wardd/modo-teste` nem `ward-plataforma/modo-teste`
#    por dependência normal ou de build (`cargo tree -e normal,build`, sem
#    `--all-features`); a conferência acusa o caso forçado com `--features`, para não
#    passar por engano;
# 2. os testes de unidade do `wardd` e do `ward-plataforma` **sem** a feature (o par
#    sem egid aberto pelo lançador fica agente);
# 3. `strings` no `wardd` de release e na biblioteca `ward-plataforma` de release não
#    acha `WARD_DEV_APP` (nem `WARD_DEV_DIR`, nem o app de teste).
#
# Compila em target/sem-modo-teste (o `make check` usa --all-features em target/).

export PATH=/usr/bin:/bin
set -euo pipefail
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/ambiente.sh
. "$RAIZ/dev/ambiente.sh"
cd "$RAIZ"

CARGO="${CARGO:?o make passa o CARGO}"
ALVO="$RAIZ/target/sem-modo-teste"
TMP="$(mktemp -d)"
trap 'rm -rf -- "$TMP"' EXIT
falhou=0

# Linhas `pacote vX (caminho)|features` dos crates do Ward que têm `modo-teste`.
com_modo_teste() {
    grep -E '^(wardd|ward-plataforma) v[^|]*\|.*modo-teste' "$1" || true
}

echo "==> 1/3 cargo tree: ninguém liga modo-teste em dependência normal"
"$CARGO" tree --workspace --locked -e normal,build -f '{p}|{f}' --prefix none > "$TMP/arvore"
for pacote in ward wardd ward-nm ward-app-nucleo ward-ponte-teste; do
    "$CARGO" tree -p "$pacote" --locked -e normal,build -f '{p}|{f}' --prefix none >> "$TMP/arvore"
done
achado="$(com_modo_teste "$TMP/arvore")"
if [ -n "$achado" ]; then
    echo "sem-modo-teste: modo-teste ligado por dependência normal:" >&2
    echo "$achado" >&2
    falhou=1
fi
# Autoteste: com a feature forçada, a conferência tem de acusar.
"$CARGO" tree -p wardd --features modo-teste --locked -e normal,build -f '{p}|{f}' --prefix none > "$TMP/forcado"
if [ -z "$(com_modo_teste "$TMP/forcado")" ]; then
    echo "sem-modo-teste: a conferência do cargo tree não acusou o caso forçado" >&2
    falhou=1
fi

echo "==> 2/3 testes de unidade sem a feature"
"$CARGO" test --locked --target-dir "$ALVO" -p wardd -p ward-plataforma --lib

echo "==> 3/3 strings no release"
"$CARGO" build --release --locked --target-dir "$ALVO" -p wardd --bin wardd
# Fase 40: o host da extensão do release também não pode ter a forma de teste.
"$CARGO" build --release --locked --target-dir "$ALVO" -p ward-nm --bin ward-nm
"$CARGO" build --release --locked --target-dir "$ALVO" -p ward-plataforma --lib \
    --message-format=json > "$TMP/build.json"
RLIB="$(grep -o '"filenames":\["[^"]*/libward_plataforma[^"]*\.rlib"' "$TMP/build.json" \
    | sed -e 's/.*\["//' -e 's/"$//' | tail -n 1 || true)"
[ -f "$RLIB" ] || { echo "sem-modo-teste: rlib do ward-plataforma não achada" >&2; exit 1; }
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
TEXTOS=(WARD_DEV_APP WARD_DEV_DIR WARD_DEV_SOCKET_DIR WARD_DEV_EXTENSAO wardd-app-teste ward-ponte-teste)
conferir "$RLIB" "${TEXTOS[@]}"
conferir "$ALVO/release/ward-nm" "${TEXTOS[@]}"
conferir "$ALVO/release/wardd" "${TEXTOS[@]}" classificar_app_de_teste classificar_host_extensao_de_teste \
    BinarioDeTeste lancar_para_teste
# O binário conferido é mesmo o daemon (tem o socket do sistema).
grep -qF -- '/run/ward' "$TMP/textos" || {
    echo "sem-modo-teste: '/run/ward' não achado; o binário conferido é mesmo o wardd?" >&2
    falhou=1
}

[ $falhou -eq 0 ] && echo "sem-modo-teste: release sem WARD_DEV_APP nem a exceção do app de teste"
exit $falhou
