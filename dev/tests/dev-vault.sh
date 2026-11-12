#!/bin/bash -p
# V2.29, V2.52, V2.57, V3.29 and attacks on `make dev-vault` / `make dev-variations` (Phases
# 19, 19d, 19e and 22a).
# Every attack has to FAIL. Usage: dev/tests/dev-vault.sh  (or `make test-dev-vault`).
#
# Never prints a secret: the searches use `grep -q`/`-c` and the messages only say what.
# Does not need the services up (only `make dev-services`, which the targets already call).
# The attack links and files inside `.dev/` are named `test-attack-*` and go away at the end.

set -euo pipefail

export PATH=/usr/bin:/bin
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
# rustup's cargo, at the end of PATH (like the Makefile).
export PATH="/usr/bin:/bin:$PASSWD_HOME/.cargo/bin"
CARGO_BIN="$PASSWD_HOME/.cargo/bin/cargo"
MAKE=(/usr/bin/make -s --no-print-directory -C "$ROOT")

VAULT="$ROOT/.dev/test-vault.ward"
PATTERN_LIST="$ROOT/.dev/variations.txt"

# One run at a time (M3 scrutiny): the test recreates the vault and the list and uses
# fixed names in `.dev/` (`test-attack-*`); two runs at once deleted each other's links.
# The lock is a `flock` on a file in `.dev/` (0600, ignored); the 2nd run waits for the
# 1st (up to 15 min) instead of stepping on it. The lock goes away when the process ends.
mkdir -p -m 0700 -- "$ROOT/.dev"
LOCK_FILE="$ROOT/.dev/.test-dev-vault.lock"
[ ! -L "$LOCK_FILE" ] || { echo "$LOCK_FILE é link; recusado" >&2; exit 1; }
exec 9>>"$LOCK_FILE"
chmod 0600 -- "$LOCK_FILE"
if ! flock -n 9; then
    echo "test-dev-vault: outra rodada em andamento; esperando a trava" >&2
    flock -w 900 9 || { echo "test-dev-vault: trava ocupada há 15 min; desisti" >&2; exit 1; }
fi

TMP="$(mktemp -d)"
chmod 0700 "$TMP"
cleanup() {
    rm -rf "$TMP"
    rm -f "$ROOT"/.dev/test-attack-*
    rm -f "$ROOT/outside-test-attack.ward" "$ROOT/outside-test-attack.txt"
}
trap cleanup EXIT

failures=0
ok() { printf 'ok    %s\n' "$1"; }
failed() { printf 'FALHA %s\n' "$1" >&2; failures=$((failures + 1)); }

# Output of a command without a secret (the list may not exist yet on the 1st call).
no_secret() { # description file
    if [ -s "$PATTERN_LIST" ] && grep -qaF -f "$PATTERN_LIST" "$2"; then
        failed "$1: segredo na saída"
    fi
}

dev_vault() { "$CARGO_BIN" run --quiet --locked -p ward-core --example dev-vault -- "$@"; }

# --- Variations list first (the other tests use it) -------------------------------------
variations() {
    if "${MAKE[@]}" dev-variations >"$TMP/var1.log" 2>&1; then ok "make dev-variations"; else
        failed "make dev-variations (saída em $TMP/var1.log)"; cat "$TMP/var1.log" >&2; return
    fi
    no_secret "make dev-variations" "$TMP/var1.log"
    local h1 h2
    h1="$(sha256sum <"$PATTERN_LIST")"
    "${MAKE[@]}" dev-variations >/dev/null 2>&1
    h2="$(sha256sum <"$PATTERN_LIST")"
    [ "$h1" = "$h2" ] && ok "dev-variations é determinístico" || failed "dev-variations muda a cada rodada"
    [ "$(stat -c %a "$PATTERN_LIST")" = 600 ] && ok "variations.txt 0600" || failed "variations.txt não é 0600"
    if grep -qx '' "$PATTERN_LIST" || awk 'length($0) < 6 { r = 1 } END { exit !r }' "$PATTERN_LIST"; then
        failed "variations.txt tem linha vazia ou padrão com menos de 6 bytes"
    else
        ok "variations.txt sem linha vazia nem padrão curto"
    fi

    # Literals: from the fixtures (values and the files they name), the master password and the PIN.
    python3 -I -B - "$ROOT/dev/fixtures/entries.json" >"$TMP/literals.txt" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
vals = {d["master_password"], d["pin"]}
for e in d["entries"]:
    for k, v in e.get("secrets", {}).items():
        if not k.endswith("_file"):
            vals.add(v)
    for x in e.get("extras", []):
        if x.get("secret"):
            vals.add(x["value"])
print("\n".join(sorted(vals)))
PY
    local missing=0 v
    while IFS= read -r v; do
        grep -qxF -- "$v" "$PATTERN_LIST" || { missing=1; failed "literal de segredo das fixtures fora da lista"; }
    done <"$TMP/literals.txt"
    [ "$(wc -l <"$TMP/literals.txt")" -ge 18 ] || failed "poucos literais nas fixtures"
    [ "$missing" = 0 ] && ok "todo literal das fixtures está na lista"

    # D11 variations computed here with coreutils (independent of variations.py).
    local expected=(
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
    missing=0
    for v in "${expected[@]}"; do
        grep -qxF -- "$v" "$PATTERN_LIST" || { missing=1; failed "variação esperada fora da lista"; }
    done
    [ "$missing" = 0 ] && ok "variações de D11 (URL, base64, Basic, DATABASE_URL, anexo) na lista"

    # The private key generated in .dev/ is in (base64 of the whole file and each line).
    local key="$ROOT/.dev/services/keys/acme-deploy_ed25519"
    if grep -qxF -- "$(base64 -w0 <"$key")" "$PATTERN_LIST" &&
        [ "$(grep -cvxF -f "$PATTERN_LIST" <(awk 'length($0) >= 6' "$key"))" = 0 ]; then
        ok "chave privada de .dev/ na lista (inteira em base64 e linha a linha)"
    else
        failed "chave privada de .dev/ incompleta na lista"
    fi

    # Secret inside a larger base64 (x-access-token:<token>) and in a form URL.
    if printf %s 'x-access-token:falso_token_github_01' | base64 -w0 | grep -qF -f "$PATTERN_LIST" &&
        printf %s 'pass=falso-SMTP-01&x=1' | python3 -I -B -c 'import sys,urllib.parse;print(urllib.parse.quote_plus(sys.stdin.read()))' | grep -qF -f "$PATTERN_LIST"; then
        ok "segredo dentro de base64 maior e de formulário é achado"
    else
        failed "segredo dentro de base64 maior ou de formulário escapou da lista"
    fi
}

# --- V2.52: D36 and D37 in the oracle (Phase 19d) ---------------------------------------
variations_d36_d37() {
    # Tests of variations.py with fake values built in the test itself.
    if python3 -I -B "$ROOT/dev/tests/variations_test.py" >"$TMP/var-test.log" 2>&1; then
        ok "dev/tests/variations_test.py (D36, D37, D11)"
    else
        failed "dev/tests/variations_test.py (saída em $TMP/var-test.log)"; cat "$TMP/var-test.log" >&2
    fi
    no_secret "variations_test.py" "$TMP/var-test.log"

    # In the real list: the D37 forms of each literal of the fixtures, computed here with
    # the standard library (only those that differ from the literal), and the D36
    # lines/values of the attachment. Nothing is printed: the script only writes the
    # expected patterns to a file in TMP.
    python3 -I -B - "$ROOT/dev/fixtures/entries.json" "$ROOT/dev/fixtures/attachments" >"$TMP/d37.txt" <<'PY'
import json, pathlib, re, sys
d = json.load(open(sys.argv[1]))
vals = {d["master_password"], d["pin"]}
for e in d["entries"]:
    for k, v in e.get("secrets", {}).items():
        if not k.endswith("_file"):
            vals.add(v)
    for x in e.get("extras", []):
        if x.get("secret"):
            vals.add(x["value"])
for p in sorted(pathlib.Path(sys.argv[2]).rglob("*")):
    if p.is_file() and not p.is_symlink():
        for line in p.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if len(line) >= 6:
                vals.add(line)
out = set()
bs = chr(92)
for v in vals:
    go = json.dumps(v, ensure_ascii=False)[1:-1]
    for a, b in (("&", "0026"), ("<", "003c"), (">", "003e"), (chr(0x2028), "2028"), (chr(0x2029), "2029")):
        go = go.replace(a, bs + "u" + b)
    forms = {go, json.dumps(v)[1:-1]}
    if "\n" not in v:
        forms.add("".join(bs + c if c in "$`\"" + bs else c for c in v))
    out |= {f for f in forms if f != v}
print("\n".join(sorted(out)))
PY
    local missing=0 v n=0
    while IFS= read -r v; do
        [ -n "$v" ] || continue
        n=$((n + 1))
        grep -qxF -- "$v" "$PATTERN_LIST" || { missing=1; failed "forma escapada (D37) de segredo das fixtures fora da lista"; }
    done <"$TMP/d37.txt"
    if [ "$n" = 0 ]; then
        ok "D37: nenhum segredo das fixtures tem caractere a escapar (coberto pelo variations_test.py)"
    elif [ "$missing" = 0 ]; then
        ok "formas escapadas de D37 das fixtures na lista ($n)"
    fi
    # Attachment line and the value of the attachment's KEY=value (D36 holds for them too).
    if grep -qxF -- 'falso-anexo-01' "$PATTERN_LIST"; then
        ok "valor de linha do anexo na lista"
    else
        failed "valor de linha do anexo fora da lista"
    fi
}

# --- V2.29: runs twice, 22 entries of the table, opens with the test password ----------
vault_twice() {
    local i
    for i in 1 2; do
        if "${MAKE[@]}" dev-vault >"$TMP/vault$i.log" 2>&1; then ok "make dev-vault ($i.ª vez)"; else
            failed "make dev-vault ($i.ª vez)"; cat "$TMP/vault$i.log" >&2; return
        fi
        no_secret "make dev-vault" "$TMP/vault$i.log"
    done
    [ -f "$VAULT" ] && [ ! -L "$VAULT" ] && ok ".dev/test-vault.ward existe" || failed "cofre não existe"
    [ "$(stat -c %a "$VAULT")" = 600 ] && ok "cofre 0600" || failed "cofre não é 0600"
    [ "$(stat -c %a "$ROOT/.dev")" = 700 ] && ok ".dev 0700" || failed ".dev não é 0700"
    (cd "$ROOT" && git check-ignore -q .dev/test-vault.ward && git check-ignore -q .dev/variations.txt) &&
        ok "cofre e lista ignorados pelo git" || failed "cofre ou lista não ignorados pelo git"
    if compgen -G "$ROOT/.dev/.test-vault.ward*" >/dev/null; then
        failed "temporário da gravação sobrou em .dev/"
    else
        ok "nenhum temporário sobrou"
    fi
    if [ "$(head -c 8 "$VAULT")" = WARDCOFR ]; then ok "arquivo com a magia do formato"; else failed "arquivo sem a magia"; fi

    # The contract's test vault table (D15; a source independent of the fixtures): name,
    # type and level of each row, against what the reopened vault lists.
    python3 -I -B - "$ROOT/docs/mission/validation-contract.md" >"$TMP/table.txt" <<'PY'
import re, sys
text = open(sys.argv[1], encoding="utf-8").read()
section = text.split("## Cofre de teste (D15)", 1)[1].split("\n## ", 1)[0]
# The table has the body v5 names (m2:DT4); the database row also names the engine.
kinds = {"login": "login", "database (postgres)": "database", "server": "server", "api": "api",
         "aws": "aws", "smtp": "smtp", "ftp-sftp": "ftp-sftp", "secure-note": "secure-note",
         "generic": "generic"}
levels = {"free": "free", "approval": "approval", "blocked": "blocked"}
for line in section.splitlines():
    m = re.match(r"^\| `([^`]+)` \| ([^|]+) \| [^|]+ \| [^|]+ \| ([^|]+) \|", line)
    if m:
        print(f"{m.group(1)}\t{kinds[m.group(2).strip()]}\t{levels[m.group(3).strip()]}")
PY
    sort -o "$TMP/table.txt" "$TMP/table.txt"
    if "${MAKE[@]}" dev-vault-verify >"$TMP/verify.log" 2>&1; then ok "make dev-vault-verify"; else
        failed "make dev-vault-verify"; cat "$TMP/verify.log" >&2; return
    fi
    no_secret "dev-vault-verify" "$TMP/verify.log"
    grep -P '^  \S+\t\S+\t\S+$' "$TMP/verify.log" | sed 's/^  //' | sort >"$TMP/listed.txt"
    if [ "$(wc -l <"$TMP/table.txt")" = 22 ] && cmp -s "$TMP/table.txt" "$TMP/listed.txt"; then
        ok "o cofre reaberto tem as 22 entradas da tabela, com tipo e nível (inclui acme-totp-dev e delta-api-simbolos)"
    else
        failed "entradas do cofre diferem da tabela do contrato"
        diff "$TMP/table.txt" "$TMP/listed.txt" >&2 || true
    fi
    grep -qxP 'acme-totp-dev\tlogin\tfree' "$TMP/listed.txt" && ok "acme-totp-dev (D28) no cofre" || failed "acme-totp-dev fora do cofre"
    grep -qxP 'delta-api-simbolos\tapi\tfree' "$TMP/listed.txt" && ok "delta-api-simbolos (D59) no cofre" || failed "delta-api-simbolos fora do cofre"

    # The list of names/levels does not match the list of secrets (no false positive).
    if grep -qF -f "$PATTERN_LIST" "$TMP/listed.txt"; then failed "nome de entrada casa com a lista de segredos"; else
        ok "nomes, tipos e níveis não casam com a lista de segredos"
    fi

    # Nothing in the clear in the file: no secret nor variation.
    if grep -qaF -f "$PATTERN_LIST" "$VAULT"; then failed "segredo ou variação em claro no cofre"; else
        ok "nenhum segredo nem variação em claro no cofre"
    fi

    # Body v4 (Phase 19b, D30; Phase 19c, D39; Phase 22d, D65 d): the verification opened
    # the settings section, the newly created vault is at generation 1 and the test PIN
    # matched the Argon2id hash; the PIN and the PHC text do not show up in the clear.
    if grep -qxF 'dev-vault: configuração: geração 1, hash do PIN Argon2id (m=65536 KiB, t=3, p=1), trava em 240 min, backup externo desligado' "$TMP/verify.log"; then
        ok "configuração do corpo v4: geração 1, hash Argon2id do PIN de teste, trava 240, backup desligado"
    else
        failed "conferência sem a configuração do corpo v4"
    fi
    if grep -qaF -e '135790' -e '$argon2id$' "$VAULT"; then failed "PIN ou hash PHC em claro no cofre"; else
        ok "PIN de teste e hash PHC fora do arquivo em claro"
    fi

    # Password through stdin: the right one opens; the wrong one does not, with no secret in the output.
    printf '%s\n' 'falso-mestra-ward-01' >"$TMP/right-password"
    head -c 32 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' >"$TMP/wrong-password"
    chmod 0600 "$TMP/right-password" "$TMP/wrong-password"
    if dev_vault --verify --password-stdin <"$TMP/right-password" >"$TMP/right.log" 2>&1; then
        ok "abre com a senha de teste (pelo stdin)"
    else
        failed "não abre com a senha de teste"
    fi
    if dev_vault --verify --password-stdin <"$TMP/wrong-password" >"$TMP/wrong.log" 2>&1; then
        failed "abriu com senha errada"
    else
        grep -q 'não abriu' "$TMP/wrong.log" && ok "senha errada recusada" || failed "senha errada: erro inesperado"
    fi
    no_secret "senha errada" "$TMP/wrong.log"
    # A password in argv does not exist.
    if dev_vault --verify --password falso-mestra-ward-01 >/dev/null 2>&1; then
        failed "aceitou senha em argv"
    else
        ok "senha em argv recusada"
    fi
}

# --- V2.57: oracle → mask cross test (Phase 19e, D54) ------------------------------------
# Each pattern of `variations.py`, alone in an output, goes through the mask of the entry
# it came from and has to come out as `••••`. The patterns go from Python to the vault
# example through a pipe (no file); the example only prints counts and line numbers.
cross_check() {
    if python3 -I -B - "$ROOT" <<'PY' | dev_vault --cross >"$TMP/cross.log" 2>&1; then
import importlib.util, json, pathlib, sys
root = pathlib.Path(sys.argv[1])
spec = importlib.util.spec_from_file_location("variacoes", root / "dev" / "variations.py")
v = importlib.util.module_from_spec(spec)
spec.loader.exec_module(v)
data = json.loads((root / "dev" / "fixtures" / "entries.json").read_text(encoding="utf-8"))
out = sys.stdout.buffer
for e in data["entries"]:
    for kind, patterns in (("inteiro", v.entry_patterns(e)), ("url", v.database_url(e))):
        for p in sorted(patterns):
            out.write(e["name"].encode() + b"\t" + kind.encode() + b"\t" + p.hex().encode() + b"\n")
PY
        ok "V2.57: todo padrão do oráculo sai •••• pela máscara da entrada ($(grep -o 'os [0-9]* padrões de [0-9]* entradas' "$TMP/cross.log"))"
    else
        failed "V2.57: padrão do oráculo que a máscara não cobre (saída em $TMP/cross.log)"
        cat "$TMP/cross.log" >&2
    fi
    no_secret "teste cruzado" "$TMP/cross.log"
}

# --- V3.29: the text forms of the D59 symbols entry (D54, D58) in the list ---------------
# Computed here by the real bash (`${X@Q}`, `declare -p`, `printf %q`) and by the Python
# standard library (JSON), without `variations.py`; the cross test then checks that each
# one comes out as `••••` through the entry's mask. Nothing is printed: the patterns go to TMP.
symbols_d59() {
    python3 -I -B - "$ROOT/dev/fixtures/entries.json" >"$TMP/symbols.env" <<'PY'
import json, shlex, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
e = next(x for x in d["entries"] if x["name"] == "delta-api-simbolos")
assert e["type"] == "api" and e["level"] == "free" and e["fields"]["variable"] == "SIMBOLOS_TOKEN"
extras = [x for x in e.get("extras", []) if x.get("secret") is True]
assert [x["name"] for x in extras] == ["SEGREDO_ASCII"]
print("TOKEN=" + shlex.quote(e["secrets"]["token"]))
print("ASCII=" + shlex.quote(extras[0]["value"]))
PY
    python3 -I -B - "$TMP/symbols.env" >"$TMP/symbols-json.txt" <<'PY'
import json, shlex, sys, base64
from urllib.parse import quote
v = {}
for line in open(sys.argv[1], encoding="utf-8"):
    k, _, r = line.rstrip("\n").partition("=")
    v[k] = shlex.split(r)[0]
bs = chr(92)
html = {"&": bs + "u0026", "<": bs + "u003c", ">": bs + "u003e"}
out = []
for x in (v["TOKEN"], v["ASCII"]):
    out.append(x)
    out.append(base64.b64encode(x.encode()).decode())
    out.append(quote(x, safe=""))
    out.append("".join(html.get(c, c) for c in json.dumps(x, ensure_ascii=False)[1:-1]))
t = v["TOKEN"]
out.append(json.dumps(t, ensure_ascii=True)[1:-1].replace("/", bs + "/"))
go_ascii = "".join(html.get(c, c) for c in json.dumps(t, ensure_ascii=True)[1:-1])
# The D58 form cited in V3.29, built in pieces (the text is never whole here).
assert go_ascii == "falso-Ab3$k" + bs + "u0026" + bs + "u003c9" + bs + "u003eZq!'/" + bs + "u00e7-01"
out.append(go_ascii)
print("\n".join(out))
PY
    # The shell forms, by the real bash, in a UTF-8 locale (in C, bash turns the value
    # outside ASCII into `$'…'`, which stays in RA3).
    (
        set +u
        export LC_ALL=C.UTF-8
        # shellcheck disable=SC1091
        . "$TMP/symbols.env"
        q="${TOKEN@Q}"; printf '%s\n' "${q:1:${#q}-2}"
        q="${ASCII@Q}"; printf '%s\n' "${q:1:${#q}-2}"
        printf '%q\n' "$ASCII"
        d="$(declare -p TOKEN)"; d="${d#*=\"}"; printf '%s\n' "${d%\"}"
    ) >"$TMP/symbols-shell.txt"
    local missing=0 v n=0
    while IFS= read -r v; do
        [ -n "$v" ] || continue
        n=$((n + 1))
        grep -qxF -- "$v" "$PATTERN_LIST" || { missing=1; failed "V3.29: forma de delta-api-simbolos fora da lista (padrão nº $n)"; }
    done < <(cat "$TMP/symbols-json.txt" "$TMP/symbols-shell.txt")
    [ "$n" -ge 14 ] || failed "V3.29: poucas formas de delta-api-simbolos ($n)"
    [ "$missing" = 0 ] && ok "V3.29: $n formas de delta-api-simbolos na lista (literal, base64, percent, @Q, aspas duplas, %q, Go, PHP, Go + ensure_ascii)"
    rm -f "$TMP/symbols.env"
}

# --- V2.29 (S15): output outside .dev/ refused, on both targets --------------------------
output_outside() { # target variable extension
    local target="$1" var="$2" ext="$3" outside="$TMP/outside-$1"
    mkdir -p "$outside"
    printf 'original\n' >"$outside/victim$ext"
    ln -s "$outside" "$ROOT/.dev/test-attack-dir"
    ln -s "$outside/victim$ext" "$ROOT/.dev/test-attack-file$ext"
    ln -s "$outside/does-not-exist$ext" "$ROOT/.dev/test-attack-broken$ext"
    local request requests=(
        "$outside/stolen$ext"
        "$TMP/stolen$ext"
        ".dev/../outside-test-attack$ext"
        "outside-test-attack$ext"
        "../outside-test-attack$ext"
        ".dev/test-attack-dir/stolen$ext"
        ".dev/test-attack-file$ext"
        ".dev/test-attack-broken$ext"
        ".dev/test-attack.other"
        ".dev/test-attack\"; touch '$TMP/injected'; echo \"$ext"
        "\$(touch $TMP/injected)$ext"
    )
    for request in "${requests[@]}"; do
        if "${MAKE[@]}" "$target" "$var=$request" >"$TMP/outside.log" 2>&1; then
            failed "$target aceitou saída fora de .dev/"
        elif ! grep -q 'recusado' "$TMP/outside.log"; then
            failed "$target falhou sem a recusa do caminho"
            cat "$TMP/outside.log" >&2
        fi
        no_secret "$target recusado" "$TMP/outside.log"
    done
    local clean=1
    [ -e "$outside/stolen$ext" ] && clean=0
    [ -e "$TMP/stolen$ext" ] && clean=0
    [ -e "$outside/does-not-exist$ext" ] && clean=0
    [ -e "$ROOT/outside-test-attack$ext" ] && clean=0
    [ -e "$TMP/injected" ] && clean=0
    [ "$(cat "$outside/victim$ext")" = original ] || clean=0
    [ -n "$(find "$outside" -mindepth 1 -newer "$outside/victim$ext" -print -quit)" ] && clean=0
    if [ "$clean" = 1 ]; then
        ok "$target recusa ${#requests[@]} saídas fora de .dev/ (absoluta, '..', links, injeção) e nada é criado fora"
    else
        failed "$target criou ou mudou arquivo fora de .dev/"
    fi
    rm -f "$ROOT"/.dev/test-attack-*
}

# Output inside .dev/ with another name works (and goes away at the end).
output_inside() {
    if "${MAKE[@]}" dev-variations VARIATIONS=.dev/test-attack-list.txt >/dev/null 2>&1 &&
        cmp -s "$ROOT/.dev/test-attack-list.txt" "$PATTERN_LIST"; then
        ok "VARIATIONS=.dev/<outro>.txt aceito"
    else
        failed "VARIATIONS dentro de .dev/ recusado"
    fi
}

variations
variations_d36_d37
vault_twice
symbols_d59
cross_check
output_outside dev-vault VAULT .ward
output_outside dev-variations VARIATIONS .txt
output_inside

if [ "$failures" -gt 0 ]; then
    echo "test-dev-vault: $failures falha(s)" >&2
    exit 1
fi
echo "test-dev-vault: tudo barrado/conferido"
