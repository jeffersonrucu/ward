#!/bin/bash -p
# `make test-chrome-policy` (Phase 51a; V9.5, V9.6; critical finding of the M9 audit):
# the Chrome policy that `install.sh` generates (the same as the `.deb`'s, which uses the
# same staging) turns off the native messaging manifests of the user's profile
# (`NativeMessagingUserLevelHosts: false`), with the real Chrome.
#
# Chrome reads /etc/opt/chrome; here it runs in a bwrap where /etc/opt is a tmpfs and
# /etc/opt/chrome is a folder of this test in `.dev/chrome-policy.*` (nothing of the
# system changes, no sudo). Profile, HOME and the user's manifests stay in it too; the
# real Chrome profile and `~/.config/google-chrome/` are never touched.
#
# Four rounds (`dev/tests/chrome_policy.py`, from the audit's scripts), plus 2b (Phase
# 44e: `chrome://policy` read at run time, with and without the policy):
#   1. precedence without a policy (control): the profile's host is called (the attack
#      works; proves the test sees the attack);
#   2. precedence with the generated `ward.json`: the profile's host is NOT called and
#      the system's is;
#   3. the whole audit attack without a policy (control, outside the bwrap: the host has
#      no /etc/opt/chrome; with one, the round is skipped): the profile's host does an
#      exec of the test `ward-nm` and takes passwords from the test `wardd`
#      (extension variable set to `chrome`); the daemon and Chrome logs without a
#      pattern of `.dev/variations.txt`;
#   4. the same attack with the generated `ward.json`, in the bwrap: the attacker's host
#      does not even run, the system's runs and nothing comes out.
#
# Phase 44c (D9): the `login` entries of the test vault are `http://localhost`, and
# a fill over `http` now waits for the origin confirmation in the window. In the
# control of round 3 the test window (the test approver binary) confirms everything, like
# a user who confirms without looking: the attack still reproduces (RA10). The policy
# also turns off Chrome's password manager and the leak check (verified in `ward.json`).
#
# Usage: chrome-policy.sh <wardd test-mode> <ward-nm test-mode> <ward> <wardd> <ward-nm> <ward-app>
#        <test approver>
#        (the 3rd to the 6th without `test-mode`, for `install.sh` to generate the policy).
# Needs bwrap, openssl, `make dev-vault`, `make dev-variations` and Chrome in
# /opt/google/chrome; out of `make check`.

export PATH=/usr/bin:/bin
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
unset MAKEFLAGS MAKELEVEL MFLAGS
cd "$ROOT"
# Phase 65a: refuses before anything when the Chrome of this suite would see the installed
# Ward (policy in /etc/opt/chrome/policies or the br.ward.nm host manifest): a test
# extension (or the forced one) would reach /usr/local/bin/ward-nm and the real vault.
/usr/bin/python3 -I -B "$ROOT/dev/tests/chrome_guard.py" check || exit 3

usage="uso: chrome-policy.sh <wardd-teste> <ward-nm-teste> <ward> <wardd> <ward-nm> <ward-app> <wardd-test-approver>"
[ $# -eq 7 ] || { echo "$usage" >&2; exit 2; }
abs() { case "$1" in /*) printf '%s' "$1" ;; *) printf '%s/%s' "$ROOT" "$1" ;; esac; }
WARDD_T="$(abs "$1")" NM_T="$(abs "$2")" APPROVER="$(abs "$7")"
B_WARD="$(abs "$3")" B_WARDD="$(abs "$4")" B_NM="$(abs "$5")" B_APP="$(abs "$6")"
for b in "$WARDD_T" "$NM_T" "$B_WARD" "$B_WARDD" "$B_NM" "$B_APP" "$APPROVER"; do
    [ -x "$b" ] || { echo "falta $b" >&2; exit 1; }
done
CHROME=/opt/google/chrome/chrome
[ -x "$CHROME" ] || { echo "falta o Chrome em $CHROME" >&2; exit 1; }
for p in bwrap openssl; do command -v "$p" >/dev/null || { echo "falta o programa: $p" >&2; exit 1; }; done
VAULT="$ROOT/.dev/test-vault.ward"
PATTERN_LIST="$ROOT/.dev/variations.txt"
[ -f "$VAULT" ] && [ ! -L "$VAULT" ] || { echo "falta o cofre de teste: rode 'make dev-vault'" >&2; exit 1; }
[ -s "$PATTERN_LIST" ] && [ ! -L "$PATTERN_LIST" ] || { echo "falta a lista: rode 'make dev-variations'" >&2; exit 1; }
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
case "$RUNTIME" in /*) ;; *) echo "XDG_RUNTIME_DIR não é absoluto" >&2; exit 1 ;; esac

umask 077
mkdir -p -m 0700 "$ROOT/.dev"
WORK_DIR="$(mktemp -d "$ROOT/.dev/chrome-policy.XXXXXX")"
trap 'rm -rf -- "$WORK_DIR"' EXIT
mkdir -m 0700 "$WORK_DIR/etc-chrome"

failures=0
ok() { echo "ok: $*"; }
failed() { echo "FALHOU: $*"; failures=$((failures + 1)); }

# The real policy: `install.sh` with DESTDIR (staging, like `make deb`).
CRX="$WORK_DIR/ward.crx"
ID="$(/usr/bin/python3 -I "$ROOT/dev/tests/fake_crx.py" "$CRX")"
/usr/bin/make -s -C "$ROOT" install DESTDIR="$WORK_DIR/root" EXTENSION_ID="$ID" EXTENSION_CRX="$CRX" \
    BIN_WARD="$B_WARD" BIN_WARDD="$B_WARDD" BIN_NM="$B_NM" BIN_APP="$B_APP" WARD_UID= >/dev/null
POLICY="$WORK_DIR/root/etc/opt/chrome/policies/managed/ward.json"
r="$(/usr/bin/python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get("NativeMessagingUserLevelHosts"))' "$POLICY")"
[ "$r" = False ] && ok "a política gerada pelo install.sh tem NativeMessagingUserLevelHosts false" ||
    failed "a política gerada não desliga os manifestos do usuário ($r)"
# Phase 44c: Chrome's password manager and the leak check turned off.
for name in PasswordManagerEnabled PasswordLeakDetectionEnabled; do
    r="$(/usr/bin/python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1])).get(sys.argv[2]))' "$POLICY" "$name")"
    [ "$r" = False ] && ok "a política gerada pelo install.sh tem $name false" ||
        failed "a política gerada não desliga $name ($r)"
done
# Key of the test extension (only for the id; discarded at the end).
openssl genrsa -out "$WORK_DIR/k.pem" 2048 2>/dev/null

run_round() {
    env -i HOME="$WORK_DIR" PATH=/usr/bin:/bin LANG=C.UTF-8 XDG_RUNTIME_DIR="$RUNTIME" \
        bwrap --dev-bind / / --tmpfs /etc/opt --bind "$WORK_DIR/etc-chrome" /etc/opt/chrome \
        --die-with-parent -- /usr/bin/python3 -I -B "$ROOT/dev/tests/chrome_policy.py" "$ROOT" "$WORK_DIR" "$@"
}
field() { /usr/bin/python3 -I -c 'import json,sys; print(json.loads(sys.argv[1]).get(sys.argv[2]))' "$1" "$2"; }

echo "==> 1. precedência sem política (controle)"
r="$(run_round precedencia -)" || failed "rodada 1 saiu ≠ 0"
echo "    $r"
[ "$(field "$r" usuario)" = True ] && ok "sem a política, o manifesto plantado no perfil vence (o ataque da auditoria)" ||
    failed "controle: o host do perfil não foi chamado; o teste não enxerga o ataque"

echo "==> 2. precedência com a política do Ward"
r="$(run_round precedencia "$POLICY")" || failed "rodada 2 saiu ≠ 0"
echo "    $r"
[ "$(field "$r" usuario)" = False ] && ok "com a política, o manifesto plantado no perfil não é chamado" ||
    failed "com a política, o host do perfil foi chamado"
[ "$(field "$r" sistema)" = True ] && ok "com a política, o manifesto do sistema é chamado" ||
    failed "com a política, o host do sistema não foi chamado"

# Phase 44e: what Chrome really applies, read from `chrome://policy` at run time (the
# generated JSON only says what was written). Without the policy (control), the page
# shows "Not set." and the reading sees the difference.
echo "==> 2b. chrome://policy em execução"
r="$(run_round pagina -)" || failed "rodada 2b (controle) saiu ≠ 0"
echo "    sem política: $r"
r2="$(run_round pagina "$POLICY")" || failed "rodada 2b saiu ≠ 0"
echo "    com a política: $r2"
for name in NativeMessagingUserLevelHosts PasswordManagerEnabled PasswordLeakDetectionEnabled; do
    without="$(/usr/bin/python3 -I -c 'import json,sys; print(json.loads(sys.argv[1])[sys.argv[2]]["valor"])' "$r" "$name")"
    with="$(/usr/bin/python3 -I -c 'import json,sys; d=json.loads(sys.argv[1])[sys.argv[2]]; print(d["valor"], d["ok"])' "$r2" "$name")"
    [ "$without" = "Not set." ] && ok "controle: sem a política, chrome://policy mostra $name sem valor" ||
        failed "controle: sem a política, chrome://policy mostra $name = $without"
    [ "$with" = "false True" ] && ok "chrome://policy mostra $name = false, obrigatória, da máquina, OK" ||
        failed "chrome://policy não aplica $name ($with)"
done

echo "==> 3. ataque da auditoria sem política (controle, fora do bwrap)"
r="$(env -i HOME="$WORK_DIR" PATH=/usr/bin:/bin LANG=C.UTF-8 XDG_RUNTIME_DIR="$RUNTIME" \
    /usr/bin/python3 -I -B "$ROOT/dev/tests/chrome_policy.py" "$ROOT" "$WORK_DIR" roubo - "$WARDD_T" "$NM_T" "$APPROVER")" ||
    failed "rodada 3 saiu ≠ 0"
echo "    $r"
if [ "$(field "$r" pulado)" != None ]; then
    echo "pulado: rodada 3 ($(field "$r" pulado))"
else
    [ "$(field "$r" atacante_rodou)" = True ] && [ "$(field "$r" com_segredo)" != 0 ] &&
        ok "sem a política, o host do atacante faz exec do ward-nm e tira senhas (controle)" ||
        failed "controle: o ataque da auditoria não reproduziu"
    for f in "$WORK_DIR/roubo-sem-politica/wardd.log" "$WORK_DIR/roubo-sem-politica/chrome.log" \
        "$WORK_DIR/roubo-sem-politica/logs/wardd-test-approver.log"; do
        if [ -f "$f" ] && grep -qaF -f "$PATTERN_LIST" -- "$f"; then
            failed "padrão de .dev/variations.txt em ${f#"$WORK_DIR"/}"
        else
            ok "${f#"$WORK_DIR"/} sem padrão de .dev/variations.txt"
        fi
    done
fi

echo "==> 4. ataque da auditoria com a política do Ward"
r="$(run_round roubo "$POLICY" "$WARDD_T" "$NM_T")" || failed "rodada 4 saiu ≠ 0"
echo "    $r"
[ "$(field "$r" atacante_rodou)" = False ] && ok "com a política, o host do atacante (no perfil) nem roda" ||
    failed "com a política, o host do atacante rodou"
[ "$(field "$r" sistema_rodou)" = True ] && ok "com a política, o Chrome abre o host do sistema" ||
    failed "com a política, o host do sistema não rodou"
[ "$(field "$r" com_segredo)" = 0 ] && ok "com a política, nenhuma senha sai" || failed "com a política, saiu senha"

if [ "$failures" -ne 0 ]; then
    echo "test-chrome-policy: $failures falha(s)"
    exit 1
fi
echo "test-chrome-policy: tudo ok"
