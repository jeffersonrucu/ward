#!/bin/bash -p
# V2.29, V2.52, V2.57, V3.29 e ataques ao `make dev-vault` / `make dev-variations` (Fases
# 19, 19d, 19e e 22a).
# Cada ataque tem de FALHAR. Uso: dev/tests/dev-vault.sh  (ou `make test-dev-vault`).
#
# Nunca imprime segredo: as buscas usam `grep -q`/`-c` e as mensagens só dizem o quê.
# Não precisa dos serviços de pé (só do `make dev-services`, que os alvos já chamam).
# Os links e arquivos de ataque dentro de `.dev/` têm nome `teste-ataque-*` e saem no fim.

set -euo pipefail

export PATH=/usr/bin:/bin
RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$RAIZ/dev/env.sh"
# cargo do rustup, no fim do PATH (como o Makefile).
export PATH="/usr/bin:/bin:$CASA_PASSWD/.cargo/bin"
CARGO_BIN="$CASA_PASSWD/.cargo/bin/cargo"
MAKE=(/usr/bin/make -s --no-print-directory -C "$RAIZ")

COFRE="$RAIZ/.dev/test-vault.ward"
LISTA="$RAIZ/.dev/variations.txt"

# Uma rodada por vez (scrutiny do M3): o teste recria o cofre e a lista e usa nomes
# fixos em `.dev/` (`teste-ataque-*`); duas rodadas juntas apagavam os links uma da
# outra. A trava é um `flock` num arquivo de `.dev/` (0600, ignorado); a 2ª rodada
# espera a 1ª (até 15 min) em vez de pisar nela. A trava sai quando o processo acaba.
mkdir -p -m 0700 -- "$RAIZ/.dev"
TRAVA="$RAIZ/.dev/.test-dev-vault.lock"
[ ! -L "$TRAVA" ] || { echo "$TRAVA é link; recusado" >&2; exit 1; }
exec 9>>"$TRAVA"
chmod 0600 -- "$TRAVA"
if ! flock -n 9; then
    echo "test-dev-vault: outra rodada em andamento; esperando a trava" >&2
    flock -w 900 9 || { echo "test-dev-vault: trava ocupada há 15 min; desisti" >&2; exit 1; }
fi

TMP="$(mktemp -d)"
chmod 0700 "$TMP"
limpar() {
    rm -rf "$TMP"
    rm -f "$RAIZ"/.dev/test-attack-*
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
    if "${MAKE[@]}" dev-variations >"$TMP/var1.log" 2>&1; then ok "make dev-variations"; else
        falhou "make dev-variations (saída em $TMP/var1.log)"; cat "$TMP/var1.log" >&2; return
    fi
    sem_segredo "make dev-variations" "$TMP/var1.log"
    local h1 h2
    h1="$(sha256sum <"$LISTA")"
    "${MAKE[@]}" dev-variations >/dev/null 2>&1
    h2="$(sha256sum <"$LISTA")"
    [ "$h1" = "$h2" ] && ok "dev-variations é determinístico" || falhou "dev-variations muda a cada rodada"
    [ "$(stat -c %a "$LISTA")" = 600 ] && ok "variations.txt 0600" || falhou "variations.txt não é 0600"
    if grep -qx '' "$LISTA" || awk 'length($0) < 6 { r = 1 } END { exit !r }' "$LISTA"; then
        falhou "variations.txt tem linha vazia ou padrão com menos de 6 bytes"
    else
        ok "variations.txt sem linha vazia nem padrão curto"
    fi

    # Literais: das fixtures (valores e arquivos citados), da senha mestra e do PIN.
    python3 -I -B - "$RAIZ/dev/fixtures/entries.json" >"$TMP/literais.txt" <<'PY'
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

    # Variações de D11 calculadas aqui com coreutils (independente do variations.py).
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
    local chave="$RAIZ/.dev/services/keys/acme-deploy_ed25519"
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

# --- V2.52: D36 e D37 no oráculo (Fase 19d) ---------------------------------------------
variacoes_d36_d37() {
    # Testes do variations.py com valores falsos montados no próprio teste.
    if python3 -I -B "$RAIZ/dev/tests/variations_test.py" >"$TMP/var-teste.log" 2>&1; then
        ok "dev/tests/variations_test.py (D36, D37, D11)"
    else
        falhou "dev/tests/variations_test.py (saída em $TMP/var-teste.log)"; cat "$TMP/var-teste.log" >&2
    fi
    sem_segredo "variations_test.py" "$TMP/var-teste.log"

    # Na lista real: as formas de D37 de cada literal das fixtures, calculadas aqui com
    # a biblioteca padrão (só as que diferem da literal), e as linhas/valores de D36 do
    # anexo. Nada é impresso: o script só grava os padrões esperados num arquivo do TMP.
    python3 -I -B - "$RAIZ/dev/fixtures/entries.json" "$RAIZ/dev/fixtures/attachments" >"$TMP/d37.txt" <<'PY'
import json, pathlib, re, sys
d = json.load(open(sys.argv[1]))
vals = {d["senha_mestra"], d["pin"]}
for e in d["entradas"]:
    for k, v in e.get("segredos", {}).items():
        if not k.endswith("_arquivo"):
            vals.add(v)
    for x in e.get("extras", []):
        if x.get("segredo"):
            vals.add(x["valor"])
for p in sorted(pathlib.Path(sys.argv[2]).rglob("*")):
    if p.is_file() and not p.is_symlink():
        for linha in p.read_text(encoding="utf-8").splitlines():
            linha = linha.strip()
            if len(linha) >= 6:
                vals.add(linha)
saida = set()
bs = chr(92)
for v in vals:
    go = json.dumps(v, ensure_ascii=False)[1:-1]
    for a, b in (("&", "0026"), ("<", "003c"), (">", "003e"), (chr(0x2028), "2028"), (chr(0x2029), "2029")):
        go = go.replace(a, bs + "u" + b)
    formas = {go, json.dumps(v)[1:-1]}
    if "\n" not in v:
        formas.add("".join(bs + c if c in "$`\"" + bs else c for c in v))
    saida |= {f for f in formas if f != v}
print("\n".join(sorted(saida)))
PY
    local faltou=0 v n=0
    while IFS= read -r v; do
        [ -n "$v" ] || continue
        n=$((n + 1))
        grep -qxF -- "$v" "$LISTA" || { faltou=1; falhou "forma escapada (D37) de segredo das fixtures fora da lista"; }
    done <"$TMP/d37.txt"
    if [ "$n" = 0 ]; then
        ok "D37: nenhum segredo das fixtures tem caractere a escapar (coberto pelo variations_test.py)"
    elif [ "$faltou" = 0 ]; then
        ok "formas escapadas de D37 das fixtures na lista ($n)"
    fi
    # Linha do anexo e valor de CHAVE=valor do anexo (D36 vale também para eles).
    if grep -qxF -- 'falso-anexo-01' "$LISTA"; then
        ok "valor de linha do anexo na lista"
    else
        falhou "valor de linha do anexo fora da lista"
    fi
}

# --- V2.29: roda duas vezes, 22 entradas da tabela, abre com a senha de teste -----------
cofre_duas_vezes() {
    local i
    for i in 1 2; do
        if "${MAKE[@]}" dev-vault >"$TMP/cofre$i.log" 2>&1; then ok "make dev-vault ($i.ª vez)"; else
            falhou "make dev-vault ($i.ª vez)"; cat "$TMP/cofre$i.log" >&2; return
        fi
        sem_segredo "make dev-vault" "$TMP/cofre$i.log"
    done
    [ -f "$COFRE" ] && [ ! -L "$COFRE" ] && ok ".dev/test-vault.ward existe" || falhou "cofre não existe"
    [ "$(stat -c %a "$COFRE")" = 600 ] && ok "cofre 0600" || falhou "cofre não é 0600"
    [ "$(stat -c %a "$RAIZ/.dev")" = 700 ] && ok ".dev 0700" || falhou ".dev não é 0700"
    (cd "$RAIZ" && git check-ignore -q .dev/test-vault.ward && git check-ignore -q .dev/variations.txt) &&
        ok "cofre e lista ignorados pelo git" || falhou "cofre ou lista não ignorados pelo git"
    if compgen -G "$RAIZ/.dev/.test-vault.ward*" >/dev/null; then
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
    if "${MAKE[@]}" dev-vault-verify >"$TMP/conferir.log" 2>&1; then ok "make dev-vault-verify"; else
        falhou "make dev-vault-verify"; cat "$TMP/conferir.log" >&2; return
    fi
    sem_segredo "dev-vault-verify" "$TMP/conferir.log"
    grep -P '^  \S+\t\S+\t\S+$' "$TMP/conferir.log" | sed 's/^  //' | sort >"$TMP/lidas.txt"
    if [ "$(wc -l <"$TMP/tabela.txt")" = 22 ] && cmp -s "$TMP/tabela.txt" "$TMP/lidas.txt"; then
        ok "o cofre reaberto tem as 22 entradas da tabela, com tipo e nível (inclui acme-totp-dev e delta-api-simbolos)"
    else
        falhou "entradas do cofre diferem da tabela do contrato"
        diff "$TMP/tabela.txt" "$TMP/lidas.txt" >&2 || true
    fi
    grep -qxP 'acme-totp-dev\tlogin\tlivre' "$TMP/lidas.txt" && ok "acme-totp-dev (D28) no cofre" || falhou "acme-totp-dev fora do cofre"
    grep -qxP 'delta-api-simbolos\tapi\tlivre' "$TMP/lidas.txt" && ok "delta-api-simbolos (D59) no cofre" || falhou "delta-api-simbolos fora do cofre"

    # A lista de nomes/níveis não casa com a lista de segredos (sem falso positivo).
    if grep -qF -f "$LISTA" "$TMP/lidas.txt"; then falhou "nome de entrada casa com a lista de segredos"; else
        ok "nomes, tipos e níveis não casam com a lista de segredos"
    fi

    # Nada em claro no arquivo: nenhum segredo nem variação.
    if grep -qaF -f "$LISTA" "$COFRE"; then falhou "segredo ou variação em claro no cofre"; else
        ok "nenhum segredo nem variação em claro no cofre"
    fi

    # Corpo v4 (Fase 19b, D30; Fase 19c, D39; Fase 22d, D65 d): a conferência abriu a seção de
    # configuração, o cofre recém-criado está na geração 1 e o PIN de teste conferiu no
    # hash Argon2id; o PIN e o texto PHC não aparecem em claro.
    if grep -qxF 'dev-cofre: configuração: geração 1, hash do PIN Argon2id (m=65536 KiB, t=3, p=1), trava em 240 min, backup externo desligado' "$TMP/conferir.log"; then
        ok "configuração do corpo v4: geração 1, hash Argon2id do PIN de teste, trava 240, backup desligado"
    else
        falhou "conferência sem a configuração do corpo v4"
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

# --- V2.57: teste cruzado oráculo → máscara (Fase 19e, D54) ------------------------------
# Cada padrão do `variations.py`, sozinho numa saída, passa pela máscara da entrada de
# onde ele veio e tem de sair `••••`. Os padrões vão do Python para o `dev-cofre` por um
# pipe (nenhum arquivo); o `dev-cofre` só imprime contagens e números de linha.
cruzado() {
    if python3 -I -B - "$RAIZ" <<'PY' | dev_cofre --cruzado >"$TMP/cruzado.log" 2>&1; then
import importlib.util, json, pathlib, sys
raiz = pathlib.Path(sys.argv[1])
spec = importlib.util.spec_from_file_location("variacoes", raiz / "dev" / "variations.py")
v = importlib.util.module_from_spec(spec)
spec.loader.exec_module(v)
dados = json.loads((raiz / "dev" / "fixtures" / "entries.json").read_text(encoding="utf-8"))
saida = sys.stdout.buffer
for e in dados["entradas"]:
    for tipo, padroes in (("inteiro", v.entry_patterns(e)), ("url", v.database_url(e))):
        for p in sorted(padroes):
            saida.write(e["nome"].encode() + b"\t" + tipo.encode() + b"\t" + p.hex().encode() + b"\n")
PY
        ok "V2.57: todo padrão do oráculo sai •••• pela máscara da entrada ($(grep -o 'os [0-9]* padrões de [0-9]* entradas' "$TMP/cruzado.log"))"
    else
        falhou "V2.57: padrão do oráculo que a máscara não cobre (saída em $TMP/cruzado.log)"
        cat "$TMP/cruzado.log" >&2
    fi
    sem_segredo "teste cruzado" "$TMP/cruzado.log"
}

# --- V3.29: as formas de texto de delta-api-simbolos (D54, D58, D59) na lista -----------
# Calculadas aqui pelo bash real (`${X@Q}`, `declare -p`, `printf %q`) e pela biblioteca
# padrão do Python (JSON), sem o `variations.py`; o teste cruzado confere depois que cada
# uma sai `••••` pela máscara da entrada. Nada é impresso: os padrões vão para o TMP.
simbolos_d59() {
    python3 -I -B - "$RAIZ/dev/fixtures/entries.json" >"$TMP/simbolos.env" <<'PY'
import json, shlex, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
e = next(x for x in d["entradas"] if x["nome"] == "delta-api-simbolos")
assert e["tipo"] == "api" and e["nivel"] == "livre" and e["campos"]["variavel"] == "SIMBOLOS_TOKEN"
extras = [x for x in e.get("extras", []) if x.get("segredo") is True]
assert [x["nome"] for x in extras] == ["SEGREDO_ASCII"]
print("TOKEN=" + shlex.quote(e["segredos"]["token"]))
print("ASCII=" + shlex.quote(extras[0]["valor"]))
PY
    python3 -I -B - "$TMP/simbolos.env" >"$TMP/simbolos-json.txt" <<'PY'
import json, shlex, sys, base64
from urllib.parse import quote
v = {}
for linha in open(sys.argv[1], encoding="utf-8"):
    k, _, r = linha.rstrip("\n").partition("=")
    v[k] = shlex.split(r)[0]
bs = chr(92)
html = {"&": bs + "u0026", "<": bs + "u003c", ">": bs + "u003e"}
saida = []
for x in (v["TOKEN"], v["ASCII"]):
    saida.append(x)
    saida.append(base64.b64encode(x.encode()).decode())
    saida.append(quote(x, safe=""))
    saida.append("".join(html.get(c, c) for c in json.dumps(x, ensure_ascii=False)[1:-1]))
t = v["TOKEN"]
saida.append(json.dumps(t, ensure_ascii=True)[1:-1].replace("/", bs + "/"))
go_ascii = "".join(html.get(c, c) for c in json.dumps(t, ensure_ascii=True)[1:-1])
# A forma de D58 citada no V3.29, montada em pedaços (o texto não fica inteiro aqui).
assert go_ascii == "falso-Ab3$k" + bs + "u0026" + bs + "u003c9" + bs + "u003eZq!'/" + bs + "u00e7-01"
saida.append(go_ascii)
print("\n".join(saida))
PY
    # As formas do shell, pelo bash real, em locale UTF-8 (no C o bash troca o valor fora
    # do ASCII por `$'…'`, que fica em RA3).
    (
        set +u
        export LC_ALL=C.UTF-8
        # shellcheck disable=SC1091
        . "$TMP/simbolos.env"
        q="${TOKEN@Q}"; printf '%s\n' "${q:1:${#q}-2}"
        q="${ASCII@Q}"; printf '%s\n' "${q:1:${#q}-2}"
        printf '%q\n' "$ASCII"
        d="$(declare -p TOKEN)"; d="${d#*=\"}"; printf '%s\n' "${d%\"}"
    ) >"$TMP/simbolos-shell.txt"
    local faltou=0 v n=0
    while IFS= read -r v; do
        [ -n "$v" ] || continue
        n=$((n + 1))
        grep -qxF -- "$v" "$LISTA" || { faltou=1; falhou "V3.29: forma de delta-api-simbolos fora da lista (padrão nº $n)"; }
    done < <(cat "$TMP/simbolos-json.txt" "$TMP/simbolos-shell.txt")
    [ "$n" -ge 14 ] || falhou "V3.29: poucas formas de delta-api-simbolos ($n)"
    [ "$faltou" = 0 ] && ok "V3.29: $n formas de delta-api-simbolos na lista (literal, base64, percent, @Q, aspas duplas, %q, Go, PHP, Go + ensure_ascii)"
    rm -f "$TMP/simbolos.env"
}

# --- V2.29 (S15): saída fora de .dev/ recusada, nos dois alvos -------------------------
saida_fora() { # alvo variável extensão
    local alvo="$1" var="$2" ext="$3" fora="$TMP/fora-$1"
    mkdir -p "$fora"
    printf 'original\n' >"$fora/vitima$ext"
    ln -s "$fora" "$RAIZ/.dev/test-attack-dir"
    ln -s "$fora/vitima$ext" "$RAIZ/.dev/test-attack-arquivo$ext"
    ln -s "$fora/nao-existe$ext" "$RAIZ/.dev/test-attack-quebrado$ext"
    local pedido pedidos=(
        "$fora/roubado$ext"
        "$TMP/roubado$ext"
        ".dev/../fora-teste-ataque$ext"
        "fora-teste-ataque$ext"
        "../fora-teste-ataque$ext"
        ".dev/test-attack-dir/roubado$ext"
        ".dev/test-attack-arquivo$ext"
        ".dev/test-attack-quebrado$ext"
        ".dev/test-attack.outro"
        ".dev/test-attack\"; touch '$TMP/injetado'; echo \"$ext"
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
    rm -f "$RAIZ"/.dev/test-attack-*
}

# Saída dentro de .dev/ com outro nome funciona (e sai no fim).
saida_dentro() {
    if "${MAKE[@]}" dev-variations VARIATIONS=.dev/test-attack-lista.txt >/dev/null 2>&1 &&
        cmp -s "$RAIZ/.dev/test-attack-lista.txt" "$LISTA"; then
        ok "VARIATIONS=.dev/<outro>.txt aceito"
    else
        falhou "VARIATIONS dentro de .dev/ recusado"
    fi
}

variacoes
variacoes_d36_d37
cofre_duas_vezes
simbolos_d59
cruzado
saida_fora dev-vault VAULT .ward
saida_fora dev-variations VARIATIONS .txt
saida_dentro

if [ "$falhas" -gt 0 ]; then
    echo "test-dev-vault: $falhas falha(s)" >&2
    exit 1
fi
echo "test-dev-vault: tudo barrado/conferido"
