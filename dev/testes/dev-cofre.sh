#!/bin/bash -p
# V2.29 e ataques ao `make dev-cofre` / `make dev-variacoes` (Fase 19). Cada ataque tem
# de FALHAR. Uso: dev/testes/dev-cofre.sh  (ou `make teste-dev-cofre`).
#
# Nunca imprime segredo: as buscas usam `grep -q`/`-c` e as mensagens só dizem o quê.
# Não precisa dos serviços de pé (só do `make dev-servicos`, que os alvos já chamam).
# Os links e arquivos de ataque dentro de `.dev/` têm nome `teste-ataque-*` e saem no fim.

set -euo pipefail

export PATH=/usr/bin:/bin
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/ambiente.sh
. "$RAIZ/dev/ambiente.sh"
# cargo do rustup, no fim do PATH (como o Makefile).
export PATH="/usr/bin:/bin:$CASA_PASSWD/.cargo/bin"
CARGO_BIN="$CASA_PASSWD/.cargo/bin/cargo"
MAKE=(/usr/bin/make -s --no-print-directory -C "$RAIZ")

COFRE="$RAIZ/.dev/cofre-teste.ward"
LISTA="$RAIZ/.dev/variacoes.txt"
TMP="$(mktemp -d)"
chmod 0700 "$TMP"
limpar() {
    rm -rf "$TMP"
    rm -f "$RAIZ"/.dev/teste-ataque-*
    rm -f "$RAIZ/fora-teste-ataque.ward" "$RAIZ/fora-teste-ataque.txt"
}
trap limpar EXIT

falhas=0
ok() { printf 'ok    %s\n' "$1"; }
falhou() { printf 'FALHA %s\n' "$1" >&2; falhas=$((falhas + 1)); }

# Saída de um comando sem segredo (a lista ainda pode não existir na 1ª chamada).
sem_segredo() { # descrição arquivo
    if [ -s "$LISTA" ] && grep -qaF -f "$LISTA" "$2"; then
        falhou "$1: segredo na saída"
    fi
}

dev_cofre() { "$CARGO_BIN" run --quiet --locked -p ward-core --example dev-cofre -- "$@"; }

# --- Lista de variações primeiro (os outros testes usam) -------------------------------
variacoes() {
    if "${MAKE[@]}" dev-variacoes >"$TMP/var1.log" 2>&1; then ok "make dev-variacoes"; else
        falhou "make dev-variacoes (saída em $TMP/var1.log)"; cat "$TMP/var1.log" >&2; return
    fi
    sem_segredo "make dev-variacoes" "$TMP/var1.log"
    local h1 h2
    h1="$(sha256sum <"$LISTA")"
    "${MAKE[@]}" dev-variacoes >/dev/null 2>&1
    h2="$(sha256sum <"$LISTA")"
    [ "$h1" = "$h2" ] && ok "dev-variacoes é determinístico" || falhou "dev-variacoes muda a cada rodada"
    [ "$(stat -c %a "$LISTA")" = 600 ] && ok "variacoes.txt 0600" || falhou "variacoes.txt não é 0600"
    if grep -qx '' "$LISTA" || awk 'length($0) < 6 { r = 1 } END { exit !r }' "$LISTA"; then
        falhou "variacoes.txt tem linha vazia ou padrão com menos de 6 bytes"
    else
        ok "variacoes.txt sem linha vazia nem padrão curto"
    fi

    # Literais: das fixtures (valores e arquivos citados), da senha mestra e do PIN.
    python3 -I -B - "$RAIZ/dev/fixtures/entradas.json" >"$TMP/literais.txt" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
vals = {d["senha_mestra"], d["pin"]}
for e in d["entradas"]:
    for k, v in e.get("segredos", {}).items():
        if not k.endswith("_arquivo"):
            vals.add(v)
    for x in e.get("extras", []):
        if x.get("segredo"):
            vals.add(x["valor"])
print("\n".join(sorted(vals)))
PY
    local faltou=0 v
    while IFS= read -r v; do
        grep -qxF -- "$v" "$LISTA" || { faltou=1; falhou "literal de segredo das fixtures fora da lista"; }
    done <"$TMP/literais.txt"
    [ "$(wc -l <"$TMP/literais.txt")" -ge 18 ] || falhou "poucos literais nas fixtures"
    [ "$faltou" = 0 ] && ok "todo literal das fixtures está na lista"

    # Variações de D11 calculadas aqui com coreutils (independente do variacoes.py).
    local esperadas=(
        'falso-PG%3As3nha%2F01%40%23'
        'falso-PG%3as3nha%2f01%40%23'
        'postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432/app_teste'
        "$(printf %s 'falso-WP-senha-02' | base64 -w0)"
        "$(printf %s 'falso-WP-senha-02' | base64 -w0 | tr -d =)"
        "$(printf %s 'falso_token_github_01' | base64 -w0 | tr '+/' '-_')"
        "$(printf %s 'wp_editor:falso-WP-senha-02' | base64 -w0)"
        "$(printf %s 'deploy:falso-SSH-senha-01' | base64 -w0)"
        "$(printf %s 'falso-mestra-ward-01' | base64 -w0)"
        'FALSO_ENV=falso-anexo-01'
    )
    faltou=0
    for v in "${esperadas[@]}"; do
        grep -qxF -- "$v" "$LISTA" || { faltou=1; falhou "variação esperada fora da lista"; }
    done
    [ "$faltou" = 0 ] && ok "variações de D11 (URL, base64, Basic, DATABASE_URL, anexo) na lista"

    # A chave privada gerada em .dev/ entra (base64 do arquivo inteiro e cada linha).
    local chave="$RAIZ/.dev/servicos/chaves/acme-deploy_ed25519"
    if grep -qxF -- "$(base64 -w0 <"$chave")" "$LISTA" &&
        [ "$(grep -cvxF -f "$LISTA" <(awk 'length($0) >= 6' "$chave"))" = 0 ]; then
        ok "chave privada de .dev/ na lista (inteira em base64 e linha a linha)"
    else
        falhou "chave privada de .dev/ incompleta na lista"
    fi

    # Segredo dentro de um base64 maior (x-access-token:<token>) e em URL de formulário.
    if printf %s 'x-access-token:falso_token_github_01' | base64 -w0 | grep -qF -f "$LISTA" &&
        printf %s 'pass=falso-SMTP-01&x=1' | python3 -I -B -c 'import sys,urllib.parse;print(urllib.parse.quote_plus(sys.stdin.read()))' | grep -qF -f "$LISTA"; then
        ok "segredo dentro de base64 maior e de formulário é achado"
    else
        falhou "segredo dentro de base64 maior ou de formulário escapou da lista"
    fi
}

# --- V2.29: roda duas vezes, 21 entradas da tabela, abre com a senha de teste -----------
cofre_duas_vezes() {
    local i
    for i in 1 2; do
        if "${MAKE[@]}" dev-cofre >"$TMP/cofre$i.log" 2>&1; then ok "make dev-cofre ($i.ª vez)"; else
            falhou "make dev-cofre ($i.ª vez)"; cat "$TMP/cofre$i.log" >&2; return
        fi
        sem_segredo "make dev-cofre" "$TMP/cofre$i.log"
    done
    [ -f "$COFRE" ] && [ ! -L "$COFRE" ] && ok ".dev/cofre-teste.ward existe" || falhou "cofre não existe"
    [ "$(stat -c %a "$COFRE")" = 600 ] && ok "cofre 0600" || falhou "cofre não é 0600"
    [ "$(stat -c %a "$RAIZ/.dev")" = 700 ] && ok ".dev 0700" || falhou ".dev não é 0700"
    (cd "$RAIZ" && git check-ignore -q .dev/cofre-teste.ward && git check-ignore -q .dev/variacoes.txt) &&
        ok "cofre e lista ignorados pelo git" || falhou "cofre ou lista não ignorados pelo git"
    if compgen -G "$RAIZ/.dev/.cofre-teste.ward*" >/dev/null; then
        falhou "temporário da gravação sobrou em .dev/"
    else
        ok "nenhum temporário sobrou"
    fi
    if [ "$(head -c 8 "$COFRE")" = WARDCOFR ]; then ok "arquivo com a magia do formato"; else falhou "arquivo sem a magia"; fi

    # Tabela "Cofre de teste" do contrato (fonte independente das fixtures): nome, tipo e
    # nível de cada linha, contra o que o cofre reaberto lista.
    python3 -I -B - "$RAIZ/docs/mission/validation-contract.md" >"$TMP/tabela.txt" <<'PY'
import re, sys
texto = open(sys.argv[1], encoding="utf-8").read()
secao = texto.split("## Cofre de teste (D15)", 1)[1].split("\n## ", 1)[0]
tipos = {"login": "login", "banco (postgres)": "banco", "servidor": "servidor", "api": "api",
         "aws": "aws", "smtp": "smtp", "ftp/sftp": "ftp-sftp", "nota segura": "nota-segura",
         "genérico": "generico"}
niveis = {"livre": "livre", "com aprovação": "com-aprovacao", "bloqueado": "bloqueado"}
for linha in secao.splitlines():
    m = re.match(r"^\| `([^`]+)` \| ([^|]+) \| [^|]+ \| [^|]+ \| ([^|]+) \|", linha)
    if m:
        print(f"{m.group(1)}\t{tipos[m.group(2).strip()]}\t{niveis[m.group(3).strip()]}")
PY
    sort -o "$TMP/tabela.txt" "$TMP/tabela.txt"
    if "${MAKE[@]}" dev-cofre-conferir >"$TMP/conferir.log" 2>&1; then ok "make dev-cofre-conferir"; else
        falhou "make dev-cofre-conferir"; cat "$TMP/conferir.log" >&2; return
    fi
    sem_segredo "dev-cofre-conferir" "$TMP/conferir.log"
    grep -P '^  \S+\t\S+\t\S+$' "$TMP/conferir.log" | sed 's/^  //' | sort >"$TMP/lidas.txt"
    if [ "$(wc -l <"$TMP/tabela.txt")" = 21 ] && cmp -s "$TMP/tabela.txt" "$TMP/lidas.txt"; then
        ok "o cofre reaberto tem as 21 entradas da tabela, com tipo e nível (inclui acme-totp-dev)"
    else
        falhou "entradas do cofre diferem da tabela do contrato"
        diff "$TMP/tabela.txt" "$TMP/lidas.txt" >&2 || true
    fi
    grep -qxP 'acme-totp-dev\tlogin\tlivre' "$TMP/lidas.txt" && ok "acme-totp-dev (D28) no cofre" || falhou "acme-totp-dev fora do cofre"

    # A lista de nomes/níveis não casa com a lista de segredos (sem falso positivo).
    if grep -qF -f "$LISTA" "$TMP/lidas.txt"; then falhou "nome de entrada casa com a lista de segredos"; else
        ok "nomes, tipos e níveis não casam com a lista de segredos"
    fi

    # Nada em claro no arquivo: nenhum segredo nem variação.
    if grep -qaF -f "$LISTA" "$COFRE"; then falhou "segredo ou variação em claro no cofre"; else
        ok "nenhum segredo nem variação em claro no cofre"
    fi

    # Corpo v3 (Fase 19b, D30; Fase 19c, D39): a conferência abriu a seção de
    # configuração, o cofre recém-criado está na geração 1 e o PIN de teste conferiu no
    # hash Argon2id; o PIN e o texto PHC não aparecem em claro.
    if grep -qxF 'dev-cofre: configuração: geração 1, hash do PIN Argon2id (m=65536 KiB, t=3, p=1), trava em 240 min, backup externo desligado' "$TMP/conferir.log"; then
        ok "configuração do corpo v3: geração 1, hash Argon2id do PIN de teste, trava 240, backup desligado"
    else
        falhou "conferência sem a configuração do corpo v3"
    fi
    if grep -qaF -e '135790' -e '$argon2id$' "$COFRE"; then falhou "PIN ou hash PHC em claro no cofre"; else
        ok "PIN de teste e hash PHC fora do arquivo em claro"
    fi

    # Senha pelo stdin: a certa abre; a errada não, sem segredo na saída.
    printf '%s\n' 'falso-mestra-ward-01' >"$TMP/senha-certa"
    head -c 32 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' >"$TMP/senha-errada"
    chmod 0600 "$TMP/senha-certa" "$TMP/senha-errada"
    if dev_cofre --conferir --senha-stdin <"$TMP/senha-certa" >"$TMP/certa.log" 2>&1; then
        ok "abre com a senha de teste (pelo stdin)"
    else
        falhou "não abre com a senha de teste"
    fi
    if dev_cofre --conferir --senha-stdin <"$TMP/senha-errada" >"$TMP/errada.log" 2>&1; then
        falhou "abriu com senha errada"
    else
        grep -q 'não abriu' "$TMP/errada.log" && ok "senha errada recusada" || falhou "senha errada: erro inesperado"
    fi
    sem_segredo "senha errada" "$TMP/errada.log"
    # Senha em argv não existe.
    if dev_cofre --conferir --senha falso-mestra-ward-01 >/dev/null 2>&1; then
        falhou "aceitou senha em argv"
    else
        ok "senha em argv recusada"
    fi
}

# --- V2.29 (S15): saída fora de .dev/ recusada, nos dois alvos -------------------------
saida_fora() { # alvo variável extensão
    local alvo="$1" var="$2" ext="$3" fora="$TMP/fora-$1"
    mkdir -p "$fora"
    printf 'original\n' >"$fora/vitima$ext"
    ln -s "$fora" "$RAIZ/.dev/teste-ataque-dir"
    ln -s "$fora/vitima$ext" "$RAIZ/.dev/teste-ataque-arquivo$ext"
    ln -s "$fora/nao-existe$ext" "$RAIZ/.dev/teste-ataque-quebrado$ext"
    local pedido pedidos=(
        "$fora/roubado$ext"
        "$TMP/roubado$ext"
        ".dev/../fora-teste-ataque$ext"
        "fora-teste-ataque$ext"
        "../fora-teste-ataque$ext"
        ".dev/teste-ataque-dir/roubado$ext"
        ".dev/teste-ataque-arquivo$ext"
        ".dev/teste-ataque-quebrado$ext"
        ".dev/teste-ataque.outro"
        ".dev/teste-ataque\"; touch '$TMP/injetado'; echo \"$ext"
        "\$(touch $TMP/injetado)$ext"
    )
    for pedido in "${pedidos[@]}"; do
        if "${MAKE[@]}" "$alvo" "$var=$pedido" >"$TMP/fora.log" 2>&1; then
            falhou "$alvo aceitou saída fora de .dev/"
        elif ! grep -q 'recusado' "$TMP/fora.log"; then
            falhou "$alvo falhou sem a recusa do caminho"
            cat "$TMP/fora.log" >&2
        fi
        sem_segredo "$alvo recusado" "$TMP/fora.log"
    done
    local nada=1
    [ -e "$fora/roubado$ext" ] && nada=0
    [ -e "$TMP/roubado$ext" ] && nada=0
    [ -e "$fora/nao-existe$ext" ] && nada=0
    [ -e "$RAIZ/fora-teste-ataque$ext" ] && nada=0
    [ -e "$TMP/injetado" ] && nada=0
    [ "$(cat "$fora/vitima$ext")" = original ] || nada=0
    [ -n "$(find "$fora" -mindepth 1 -newer "$fora/vitima$ext" -print -quit)" ] && nada=0
    if [ "$nada" = 1 ]; then
        ok "$alvo recusa ${#pedidos[@]} saídas fora de .dev/ (absoluta, '..', links, injeção) e nada é criado fora"
    else
        falhou "$alvo criou ou mudou arquivo fora de .dev/"
    fi
    rm -f "$RAIZ"/.dev/teste-ataque-*
}

# Saída dentro de .dev/ com outro nome funciona (e sai no fim).
saida_dentro() {
    if "${MAKE[@]}" dev-variacoes VARIACOES=.dev/teste-ataque-lista.txt >/dev/null 2>&1 &&
        cmp -s "$RAIZ/.dev/teste-ataque-lista.txt" "$LISTA"; then
        ok "VARIACOES=.dev/<outro>.txt aceito"
    else
        falhou "VARIACOES dentro de .dev/ recusado"
    fi
}

variacoes
cofre_duas_vezes
saida_fora dev-cofre COFRE .ward
saida_fora dev-variacoes VARIACOES .txt
saida_dentro

if [ "$falhas" -gt 0 ]; then
    echo "teste-dev-cofre: $falhas falha(s)" >&2
    exit 1
fi
echo "teste-dev-cofre: tudo barrado/conferido"
