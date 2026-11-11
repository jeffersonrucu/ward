#!/bin/bash -p
# Phase 64 (mission 2, Battery D): migration end to end and mixed versions, against test
# daemons of this script only (copies in .dev/f64.*, own sockets by WARD_DEV_SOCKET_DIR).
# Never the real vault, the installed daemon or the `make dev-daemon` one. Only fake values
# (D15): the password and the PIN go to 0600 files read by f64.py, never to argv or output.
#
# Usage: f64-run.sh <section...>   sections: migrate v412 v45 v416 v417 kill
export PATH=/usr/bin:/bin
set -uo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../../../../../.." && pwd -P)"
# shellcheck source=dev/env.sh
. "$ROOT/dev/env.sh"
cd "$ROOT"
umask 077

HERE="$ROOT/docs/mission/evidence/M6/fase-64/scripts"
W020="$ROOT/target/debug/wardd"; C020="$ROOT/target/debug/ward"
W014="$ROOT/.dev/baseline/bin-test-mode/wardd"; C014="$ROOT/.dev/baseline/bin-test-mode/ward"
LEG="$ROOT/dev/fixtures/legacy"
RT="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
for f in "$W020" "$C020" "$W014" "$C014"; do [ -x "$f" ] || { echo "falta $f" >&2; exit 1; }; done
(cd "$ROOT/.dev/baseline" && sha256sum --quiet -c SHA256SUMS) || { echo "linha de base não confere" >&2; exit 1; }
(cd "$LEG" && sha256sum --quiet -c SHA256SUMS) || { echo "fixtures legadas não conferem" >&2; exit 1; }

WORK="$(mktemp -d "$ROOT/.dev/f64.XXXXXX")"
mkdir -p "$WORK/app" "$WORK/logs"
install -m 0700 "$HERE/f64.py" "$WORK/app/f64app"
F64=(/usr/bin/python3 -I -B "$HERE/f64.py")
PIDS=()
SOCKS=()
cleanup() {
    for p in "${PIDS[@]}"; do
        if kill -0 "$p" 2>/dev/null; then kill -TERM "$p" 2>/dev/null; sleep 0.5; kill -KILL "$p" 2>/dev/null; fi
        wait "$p" 2>/dev/null
    done
    for s in "${SOCKS[@]}"; do rm -rf -- "$s"; done
    if [ -z "${KEEP:-}" ]; then rm -rf -- "$WORK"; else echo "WORK mantido: $WORK"; fi
}
trap cleanup EXIT

fails=0
ok() { echo "ok    $*"; }
bad() { echo "FALHA $*"; fails=$((fails + 1)); }
check() { local cond="$1"; shift; if eval "$cond"; then ok "$*"; else bad "$*"; fi; }

secrets() { # <dir>: password and PIN of D15 (fake), 0600, from entries.json
    /usr/bin/python3 -I -B -c '
import json, os, sys
e = json.load(open(sys.argv[1]))
for n, v in (("f64-pw", e["master_password"]), ("f64-pin", e["pin"])):
    fd = os.open(os.path.join(sys.argv[2], n), os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.write(fd, v.encode()); os.close(fd)
' "$ROOT/dev/fixtures/entries.json" "$1"
}

# new_dir <name> <proto 1|2> : WARD_DEV_DIR with the 0.1.4 vault, state and backups
new_dir() {
    local d="$WORK/$1" proto="$2"
    mkdir -m 0700 "$d" "$d/backups" "$d/logs"
    if [ "$proto" = 2 ]; then
        install -m 0600 "$LEG/vault-0.1.4.ward" "$d/test-vault.ward"
        install -m 0600 "$LEG/state-0.1.4" "$d/test-vault.state"
    else
        install -m 0600 "$LEG/vault-0.1.4.ward" "$d/cofre-teste.ward"
        install -m 0600 "$LEG/state-0.1.4" "$d/cofre-teste.estado"
    fi
    install -m 0600 "$LEG"/backup-0.1.4/*.ward "$d/backups/"
    echo "$proto" > "$d/f64-proto"
    secrets "$d"
    echo "$d"
}

# start <name> <wardd> <dir> [app] : prints "pid socket"
LAST_PID=""; LAST_SOCK=""
start() {
    local name="$1" bin="$2" dir="$3" app="${4:-}" sd
    sd="$RT/ward-f64-$name-$$-${WORK##*.}"
    SOCKS+=("$sd")
    env -i HOME="$dir" PATH=/usr/bin:/bin XDG_RUNTIME_DIR="$RT" LANG=C.UTF-8 \
        WARD_DEV_DIR="$dir" WARD_DEV_SOCKET_DIR="$sd" RUST_LOG=trace ${app:+WARD_DEV_APP="$app"} \
        "$bin" </dev/null >>"$WORK/logs/fase64-wardd-$name.log" 2>&1 &
    LAST_PID=$!
    PIDS+=("$LAST_PID")
    LAST_SOCK="$sd/wardd.sock"
    for _ in $(seq 1 200); do
        [ -S "$LAST_SOCK" ] && return 0
        kill -0 "$LAST_PID" 2>/dev/null || { echo "o wardd $name não subiu"; tail -n 5 "$WORK/logs/fase64-wardd-$name.log"; return 1; }
        sleep 0.05
    done
    return 1
}
stop() { kill -TERM "$1" 2>/dev/null; for _ in $(seq 1 50); do kill -0 "$1" 2>/dev/null || break; sleep 0.1; done; kill -KILL "$1" 2>/dev/null; wait "$1" 2>/dev/null; }

agent() { "${F64[@]}" agent "$@"; }
# run_app <proto> <sock> <dir> <step> : opens the app, waits for the output
run_app() {
    local proto="$1" sock="$2" dir="$3" step="$4" out
    out="$dir/f64-out-${step%% *}.json"
    rm -f -- "$out"
    echo "$step" > "$dir/f64-step"
    agent "$proto" "$sock" open_app
    for _ in $(seq 1 1200); do [ -f "$out" ] && break; sleep 0.1; done
    [ -f "$out" ] || { echo "o app não escreveu $out"; tail -n 5 "$dir/logs/"* 2>/dev/null; return 1; }
    sleep 0.3
}
ident() { echo "$(stat -c '%i %y %s' -- "$1") $(sha256sum -- "$1" | cut -d' ' -f1)"; }
magic() { head -c 8 -- "$1"; }
jget() { /usr/bin/python3 -I -B -c 'import json,sys; d=json.load(open(sys.argv[1]));
for k in sys.argv[2].split("."): d = d[int(k)] if isinstance(d, list) else d.get(k)
print(json.dumps(d, ensure_ascii=False) if not isinstance(d, str) else d)' "$@"; }
V4SHA="$(sha256sum -- "$LEG/vault-0.1.4.ward" | cut -d' ' -f1)"

# ---------------------------------------------------------------------------------------
sec_migrate() {
    echo "==> m2:V4.3/V4.11/V4.5(0.2.0)/V4.6/V4.7/V4.10: wardd 0.2.0 sobre a cópia da 0.1.4"
    local d; d="$(new_dir d020 2)"
    local st0; st0="$(sha256sum -- "$d/test-vault.state" | cut -d' ' -f1)"
    start 020 "$W020" "$d" "$WORK/app/f64app" || { bad "wardd 0.2.0 subiu"; return; }
    local pid=$LAST_PID sock=$LAST_SOCK
    check '[ "$(sha256sum -- "$d/test-vault.state" | cut -d" " -f1)" = "$st0" ]' "V4.11 subir com o estado v1 não regrava o estado"
    r="$(agent 2 "$sock" status)"; echo "      status: $r"
    check '[[ "$r" == "status - locked=True role=agent" ]]' "travado antes do primeiro destrave"
    run_app 2 "$sock" "$d" dump || { bad "app dump"; return; }
    cp -- "$d/f64-out-dump.json" "$WORK/dump020.json"
    echo "      app: papel=$(jget "$d/f64-out-dump.json" role) destrave=$(jget "$d/f64-out-dump.json" unlock)"
    check '[ "$(jget "$d/f64-out-dump.json" role)" = app ]' "o processo aberto pelo daemon tem o papel app"
    check '[ "$(jget "$d/f64-out-dump.json" unlock)" = "[\"ok\", \"-\"]" ]' "V4.3 primeiro destrave da v4 responde ok"
    check '[ "$(magic "$d/test-vault.ward")" = WARDCOFR ]' "V4.3 o cofre migrado começa com WARDCOFR"
    check '[ "$(sha256sum -- "$d/test-vault.ward" | cut -d" " -f1)" != "$V4SHA" ]' "V4.3 o arquivo do cofre foi regravado (v5)"
    check '[ "$(stat -c %a -- "$d/test-vault.ward")" = 600 ]' "cofre migrado 0600"
    echo "      estado: $(paste -sd' ' "$d/test-vault.state") modo $(stat -c %a "$d/test-vault.state")"
    check '[ "$(head -n1 "$d/test-vault.state")" = "ward-state 2" ]' "V4.11 o estado virou ward-state 2"
    check 'grep -qx "seen 4" "$d/test-vault.state" && grep -qx "burned 4" "$d/test-vault.state" && grep -qx "agent_failures 0" "$d/test-vault.state"' "V4.11 estado com geração g+1=4 e falhas do agente iguais (0)"
    check '[ "$(stat -c %a -- "$d/test-vault.state")" = 600 ]' "V4.11 estado 0600"
    ls -la "$d" "$d/backups" | sed "s|$ROOT|<repo>|g"
    check '[ -z "$(find "$d" -maxdepth 2 -name "*tmp*" -o -maxdepth 2 -name ".*.ward*")" ]' "nenhum temporário sobrou na pasta"
    if grep -qaF -f "$ROOT/.dev/variations.txt" -- "$d/test-vault.ward" "$d/test-vault.state" "$d"/backups/*; then bad "S2: padrão de variations.txt nos arquivos"; else ok "S2: cofre, estado e backups sem padrão de variations.txt"; fi
    if grep -qaE 'database|secure-note|banco|servidor|nota-segura|com-aprovacao|approval' -- "$d/test-vault.ward" "$d"/backups/*; then bad "S2: nome de tipo/nível em claro"; else ok "S2: nenhum nome de tipo/nível em claro"; fi
    check '[ "$(ls "$d/backups" | wc -l)" -ge 3 ]' "V4.3 a migração gravou uma versão nova no backup local ($(ls "$d/backups" | wc -l) versões)"
    cp -p -- "$d/test-vault.ward" "$WORK/v5-migrated.ward"
    cp -p -- "$d/test-vault.state" "$WORK/v5-migrated.state"

    echo "==> m2:V4.6: trava e destrava 3 vezes, e um daemon novo, sem regravar"
    local a b sa sb
    a="$(ident "$d/test-vault.ward")"; sa="$(ident "$d/test-vault.state")"
    for i in 1 2 3; do
        r1="$(agent 2 "$sock" lock)"; r2="$(agent 2 "$sock" unlock "$d/f64-pw")"; echo "      rodada $i: $r1 / $r2"
    done
    stop "$pid"; start 020b "$W020" "$d" "$WORK/app/f64app" || { bad "reinício"; return; }
    pid=$LAST_PID; sock=$LAST_SOCK
    r="$(agent 2 "$sock" unlock "$d/f64-pw")"; echo "      daemon novo: $r"
    b="$(ident "$d/test-vault.ward")"; sb="$(ident "$d/test-vault.state")"
    check '[ "$a" = "$b" ]' "V4.6 cofre com o mesmo inode, mtime, tamanho e sha256"
    check '[ "$sa" = "$sb" ]' "V4.6 estado igual (geração igual)"

    echo "==> m2:V4.7: cópia v4 de volta"
    agent 2 "$sock" lock >/dev/null
    cp -- "$LEG/vault-0.1.4.ward" "$d/.swap" && mv -f -- "$d/.swap" "$d/test-vault.ward"
    r="$(agent 2 "$sock" unlock "$d/f64-pw")"; echo "      com o daemon rodando: $r"
    check '[[ "$r" == "error vault_changed"* || "$r" == "error rollback"* ]]' "V4.7 trocar com o daemon rodando é recusado (contrato: vault_changed)"
    [[ "$r" == "error rollback"* ]] && echo "      nota: o daemon rodando respondeu rollback, não vault_changed"
    check '[ "$(sha256sum -- "$d/test-vault.ward" | cut -d" " -f1)" = "$V4SHA" ]' "V4.7 a cópia v4 não foi regravada (daemon rodando)"
    stop "$pid"
    echo "      com o daemon rodando e destravado, troca e depois uma gravação (troca de PIN):"
    cp -p -- "$WORK/v5-migrated.ward" "$d/test-vault.ward"
    start 020v "$W020" "$d" "$WORK/app/f64app" || { bad "reinício"; return; }
    pid=$LAST_PID; sock=$LAST_SOCK
    r="$(agent 2 "$sock" unlock "$d/f64-pw")"; echo "      destravar o v5: $r"
    cp -- "$LEG/vault-0.1.4.ward" "$d/.swap" && mv -f -- "$d/.swap" "$d/test-vault.ward"
    run_app 2 "$sock" "$d" pinonly || bad "app pinonly"
    r="$(jget "$d/f64-out-pinonly.json" pin_right)"; echo "      gravação depois da troca: $r"
    check '[ "$r" = "[\"error\", \"vault_changed\"]" ]' "V4.7 trocar com o daemon rodando (destravado) dá vault_changed na gravação"
    check '[ "$(sha256sum -- "$d/test-vault.ward" | cut -d" " -f1)" = "$V4SHA" ]' "V4.7 a cópia v4 não foi sobrescrita"
    check '[ "$(ident "$d/test-vault.state")" = "$sb" ]' "V4.7 o estado não mudou"
    stop "$pid"
    start 020c "$W020" "$d" "$WORK/app/f64app" || { bad "reinício"; return; }
    pid=$LAST_PID; sock=$LAST_SOCK
    r="$(agent 2 "$sock" unlock "$d/f64-pw")"; echo "      daemon parado e subido de novo: $r"
    check '[[ "$r" == "error rollback"* ]]' "V4.7 com o daemon parado, destravar a cópia v4 dá rollback"
    check '[ "$(sha256sum -- "$d/test-vault.ward" | cut -d" " -f1)" = "$V4SHA" ]' "V4.7 a cópia v4 continua intacta"
    check '[ "$(ident "$d/test-vault.state")" = "$sb" ]' "V4.7 o estado não mudou"
    stop "$pid"
    cp -p -- "$WORK/v5-migrated.ward" "$d/test-vault.ward"

    echo "==> m2:V4.10: restaurar a versão v4 mais velha (geração 2) e a rotação"
    start 020d "$W020" "$d" "$WORK/app/f64app" || { bad "reinício"; return; }
    pid=$LAST_PID; sock=$LAST_SOCK
    r="$(agent 2 "$sock" status)"; echo "      status: $r"
    run_app 2 "$sock" "$d" "restore ward-20261110-213053.ward" || { bad "app restore"; return; }
    echo "      restaurar: $(jget "$d/f64-out-restore.json" restore); versões: $(jget "$d/f64-out-restore.json" backups)"
    check '[ "$(jget "$d/f64-out-restore.json" restore)" = "[\"ok\", \"-\"]" ]' "V4.10 a versão v4 restaurada abre (ok)"
    check '[ "$(magic "$d/test-vault.ward")" = WARDCOFR ] && ! cmp -s "$d/test-vault.ward" "$LEG/backup-0.1.4/ward-20261110-213053.ward"' "V4.10 o cofre restaurado foi gravado de novo (não é a cópia v4)"
    echo "      estado: $(paste -sd' ' "$d/test-vault.state")"
    check 'grep -qx "seen 5" "$d/test-vault.state"' "V4.10 a restauração tem geração nova (5)"
    run_app 2 "$sock" "$d" dump || { bad "app dump depois da restauração"; return; }
    cp -- "$d/f64-out-dump.json" "$WORK/dump020-restored.json"
    echo "      restaurado (geração 2 da 0.1.4) × migrado (geração 3):"
    "${F64[@]}" compare "$WORK/dump020.json" "$WORK/dump020-restored.json" | sed 's/^/      /'
    # Junk with a version name, a version of another vault and a renamed copy.
    head -c 5278 /dev/urandom > "$d/backups/ward-20261111-000001.ward"
    cp -- "$ROOT/.dev/test-vault.ward" "$d/backups/ward-20261111-000002.ward"
    first="$(ls "$d/backups" | grep -v -e '^ward-20261110-' -e '^ward-20261111-00000' | head -n1)"
    echo "      cópia renomeada de $first"
    cp -- "$d/backups/$first" "$d/backups/ward-20261111-000003.ward"
    chmod 600 "$d"/backups/*
    run_app 2 "$sock" "$d" backups || { bad "app backups"; return; }
    echo "      versões com lixo: $(jget "$d/f64-out-backups.json" backups)"
    /usr/bin/python3 -I -B -c '
import json, sys
v = {n: (ok, o) for n, ok, o in json.load(open(sys.argv[1]))["backups"]}
v4 = [n for n in v if n.startswith("ward-20261110-")]
junk = ["ward-20261111-000001.ward", "ward-20261111-000002.ward", "ward-20261111-000003.ward"]
print("ok    V4.10 as duas versões v4 da 0.1.4 conferidas" if all(v[n][0] for n in v4) and len(v4) == 2 else "FALHA V4.10 versões v4 não conferidas: %s" % v4)
v5 = [n for n in v if n not in v4 and n not in junk]
print("ok    V4.10 versões v5 conferidas (%d)" % len(v5) if v5 and all(v[n][0] for n in v5) else "FALHA V4.10 versões v5: %s" % v5)
print("ok    V4.10 lixo e outro cofre não conferidos" if all(n in v and not v[n][0] for n in junk[:2]) else "FALHA V4.10 lixo conferido: %s" % {n: v.get(n) for n in junk})
print("      nota: cópia renomeada listada como %s (conferida, fora_de_ordem); o lugar na rotação é conferido abaixo" % (v.get(junk[2]),))
' "$d/f64-out-backups.json" | tee -a "$WORK/py.out"
    grep -q '^FALHA' "$WORK/py.out" && fails=$((fails + 1))
    rm -f -- "$WORK/py.out"
    echo "      rotação: 35 gravações (tempo de trava) com o lixo, o outro cofre e a cópia renomeada na pasta"
    run_app 2 "$sock" "$d" "rotate 35" || { bad "app rotate"; return; }
    echo "      gravações: $(jget "$d/f64-out-rotate.json" writes | tr -d ' ' | head -c 300)"
    local left; left="$(ls "$d/backups")"
    echo "      sobraram $(echo "$left" | wc -l) arquivos: v4=$(echo "$left" | grep -c '^ward-20261110-') lixo=$(echo "$left" | grep -c -e 000001 -e 000002) cópia=$(echo "$left" | grep -c 000003) outros=$(echo "$left" | grep -v -e '^ward-20261110-' -e '^ward-20261111-00000' | wc -l)"
    check '! echo "$left" | grep -q "^ward-20261110-"' "V4.10 as versões v4 saíram quando as 30 gerações mais altas são v5"
    check '! echo "$left" | grep -q 000003' "V4.10 a cópia renomeada não ocupou lugar (saiu)"
    check 'echo "$left" | grep -q 000001 && echo "$left" | grep -q 000002' "V4.10 o lixo e a versão de outro cofre ficam (só o usuário apaga)"
    check '[ "$(echo "$left" | grep -v -e "^ward-20261111-00000" | wc -l)" = 30 ]' "V4.10 exatamente 30 versões reais"
    rm -f -- "$d"/backups/ward-20261111-00000*.ward

    echo "==> m2:V4.5: PIN (wrong, depois o certo) no 0.2.0"
    run_app 2 "$sock" "$d" pin || { bad "app pin"; return; }
    echo "      pin errado: $(jget "$d/f64-out-pin.json" pin_wrong); pin certo: $(jget "$d/f64-out-pin.json" pin_right)"
    check '[ "$(jget "$d/f64-out-pin.json" pin_wrong)" = "[\"error\", \"wrong_pin\"]" ]' "V4.5 PIN errado recusado (wrong_pin)"
    check '[ "$(jget "$d/f64-out-pin.json" pin_right)" = "[\"ok\", \"-\"]" ]' "V4.5 o PIN de teste confere no cofre migrado"
    stop "$pid"
    if grep -qaF -f "$ROOT/.dev/variations.txt" -- "$WORK"/logs/fase64-wardd-020*.log; then bad "S13: log do wardd 0.2.0 com padrão"; else ok "S13: logs do wardd 0.2.0 (trace) sem padrão"; fi
}

# ---------------------------------------------------------------------------------------
sec_v45() {
    echo "==> m2:V4.5: o mesmo pelo wardd 0.1.4 em vault-0.1.4.ward, e a comparação"
    local d; d="$(new_dir d014 1)"
    start 014 "$W014" "$d" "$WORK/app/f64app" || { bad "wardd 0.1.4 subiu"; return; }
    local pid=$LAST_PID sock=$LAST_SOCK
    r="$(agent 1 "$sock" status)"; echo "      status 0.1.4: $r"
    run_app 1 "$sock" "$d" dump || { bad "app dump 0.1.4"; stop "$pid"; return; }
    cp -- "$d/f64-out-dump.json" "$WORK/dump014.json"
    echo "      app 0.1.4: papel=$(jget "$d/f64-out-dump.json" role) destrave=$(jget "$d/f64-out-dump.json" unlock)"
    run_app 1 "$sock" "$d" pin || { bad "app pin 0.1.4"; }
    echo "      0.1.4 pin errado: $(jget "$d/f64-out-pin.json" pin_wrong); certo: $(jget "$d/f64-out-pin.json" pin_right)"
    stop "$pid"
    if [ -f "$WORK/dump020.json" ]; then
        "${F64[@]}" compare "$WORK/dump014.json" "$WORK/dump020.json" | sed 's/^/      /'
        if "${F64[@]}" compare "$WORK/dump014.json" "$WORK/dump020.json" >/dev/null; then ok "V4.5 0.1.4 × 0.2.0 migrado: entradas e configuração iguais"; else bad "V4.5 diferenças entre 0.1.4 e 0.2.0"; fi
        /usr/bin/python3 -I -B -c '
import json, sys
a, b = json.load(open(sys.argv[1])), json.load(open(sys.argv[2]))
print("      entradas: 0.1.4=%d 0.2.0=%d; totp conferido pela semente em %d entradas; config 0.1.4=%s 0.2.0=%s" % (
    len(a["entries"]), len(b["entries"]), sum(1 for e in b["entries"].values() if e.get("totp", {}).get("matches_seed")), a["config"], b["config"]))
' "$WORK/dump014.json" "$WORK/dump020.json"
    fi
    install -m 0600 "$WORK/dump014.json" "$WORK/logs/fase64-dump-0.1.4.json"
    [ -f "$WORK/dump020.json" ] && install -m 0600 "$WORK/dump020.json" "$WORK/logs/fase64-dump-0.2.0.json"
}

# ---------------------------------------------------------------------------------------
sec_v412() {
    echo "==> m2:V4.12: o wardd 0.1.4 diante do cofre já migrado (v5)"
    [ -f "$WORK/v5-migrated.ward" ] || { bad "falta o cofre migrado (rode migrate antes)"; return; }
    local d="$WORK/d014v5"; mkdir -m 0700 "$d" "$d/backups" "$d/logs"
    install -m 0600 "$WORK/v5-migrated.ward" "$d/cofre-teste.ward"
    printf 'ward-estado 1\nvista 4\nqueimada 4\nfalhas_agente 0\n' > "$d/cofre-teste.estado"; chmod 600 "$d/cofre-teste.estado"
    echo 1 > "$d/f64-proto"; secrets "$d"
    local before list0; before="$(ident "$d/cofre-teste.ward")"; list0="$(ls -A "$d" "$d/backups")"
    start 014v5 "$W014" "$d" || { bad "wardd 0.1.4 subiu"; return; }
    local pid=$LAST_PID sock=$LAST_SOCK
    r="$(agent 1 "$sock" unlock "$d/f64-pw")"; echo "      destravar no 0.1.4: $r"
    check '[[ "$r" == erro* ]]' "V4.12 o 0.1.4 recusa o cofre v5"
    [[ "$r" == *"vers"* ]] || echo "      nota: a mensagem não cita versão (contrato: \"versão do corpo desconhecida\")"
    r="$(agent 1 "$sock" status)"; echo "      status 0.1.4: $r"
    stop "$pid"
    check '[ "$(ident "$d/cofre-teste.ward")" = "$before" ]' "V4.12 o 0.1.4 não gravou o cofre (inode, mtime, sha256)"
    check '[ "$(ls -A "$d" "$d/backups" | grep -v -e "^f64-" -e logs)" = "$(echo "$list0" | grep -v -e "^f64-" -e logs)" ]' "V4.12 nenhum arquivo novo na pasta"
    grep -a -i -E "vers|corpo|inválid" "$WORK/logs/fase64-wardd-014v5.log" | tail -n 3 | sed 's/^/      log: /'
    echo "      e com o estado v2 da 0.2.0 no lugar do v1:"
    cp -p -- "$WORK/v5-migrated.state" "$d/cofre-teste.estado"
    start 014v5b "$W014" "$d" || { echo "      o 0.1.4 não sobe com o estado v2: $(tail -n 2 "$WORK/logs/fase64-wardd-014v5b.log" | paste -sd' ')"; }
    if [ -S "$LAST_SOCK" ] && kill -0 "$LAST_PID" 2>/dev/null; then
        r="$(agent 1 "$LAST_SOCK" unlock "$d/f64-pw")"; echo "      destravar: $r"
        stop "$LAST_PID"
    fi
    check '[ "$(ident "$d/cofre-teste.ward")" = "$before" ]' "V4.12 o cofre continua intacto"
    echo "      o mesmo arquivo continua abrindo no 0.2.0:"
    local e="$WORK/d020v5"; mkdir -m 0700 "$e" "$e/backups" "$e/logs"
    install -m 0600 "$d/cofre-teste.ward" "$e/test-vault.ward"
    install -m 0600 "$WORK/v5-migrated.state" "$e/test-vault.state"
    echo 2 > "$e/f64-proto"; secrets "$e"
    start 020v5 "$W020" "$e" || { bad "wardd 0.2.0 subiu"; return; }
    r="$(agent 2 "$LAST_SOCK" unlock "$e/f64-pw")"; echo "      destravar no 0.2.0: $r"
    check '[[ "$r" == "ok -"* ]]' "V4.12 o arquivo continua abrindo na 0.2.0"
    stop "$LAST_PID"
}

# ---------------------------------------------------------------------------------------
sec_v416() {
    echo "==> m2:V4.16 (parte 0.1.4): CLI 0.2.0 diante do wardd 0.1.4"
    local d; d="$(new_dir d014cli 1)"
    start 014cli "$W014" "$d" || { bad "wardd 0.1.4 subiu"; return; }
    local pid=$LAST_PID sock=$LAST_SOCK
    local before; before="$(wc -l < "$WORK/logs/fase64-wardd-014cli.log")"
    r="$(WARD_DEV_SOCKET_DIR="${sock%/wardd.sock}" HOME=/nao-existe LANG=C.UTF-8 "${F64[@]}" pty "$WORK/logs/fase64-v416-unlock.screen" 20 "$d/f64-pw" -- "$C020" unlock)"
    echo "      ward 0.2.0 unlock: $r"; sed 's/^/      tela: /' "$WORK/logs/fase64-v416-unlock.screen"
    check '[[ "$r" != "exit=0 "* ]]' "V4.16 a CLI 0.2.0 sai ≠ 0"
    check '[[ "$r" == *"prompt=0" ]]' "V4.16 o prompt da senha não apareceu"
    check 'grep -q "incompatível" "$WORK/logs/fase64-v416-unlock.screen"' "V4.16 mensagem pt-BR de versão incompatível"
    for sub in "list" "lock"; do
        r="$(WARD_DEV_SOCKET_DIR="${sock%/wardd.sock}" HOME=/nao-existe LANG=C.UTF-8 "${F64[@]}" pty "$WORK/logs/fase64-v416-$sub.screen" 20 - -- "$C020" $sub)"
        echo "      ward 0.2.0 $sub: $r — $(tr -d '\r' < "$WORK/logs/fase64-v416-$sub.screen" | paste -sd' ')"
    done
    r="$(agent 1 "$sock" status)"; echo "      status 0.1.4 depois: $r"
    check '[[ "$r" == "estado - locked=True"* ]]' "V4.16 o wardd 0.1.4 continua travado"
    echo "      o que o 0.1.4 registrou (sem valores):"
    tail -n +"$((before + 1))" "$WORK/logs/fase64-wardd-014cli.log" | grep -a -o -E 'operacao=[a-z_]+|op(eração)?[ =:][a-z_]+|destrav[a-z]*|versao[a-z_]*|pedido[^,]{0,40}' | sort | uniq -c | sed 's/^/        /'
    if tail -n +"$((before + 1))" "$WORK/logs/fase64-wardd-014cli.log" | grep -qa -i 'destravar'; then bad "V4.16 o 0.1.4 recebeu um destravar"; else ok "V4.16 o 0.1.4 não registrou nenhum destravar"; fi
    stop "$pid"
}

# ---------------------------------------------------------------------------------------
sec_v417() {
    echo "==> m2:V4.17: CLI 0.1.4 diante do wardd 0.2.0"
    local d="$WORK/d020cli"; mkdir -m 0700 "$d" "$d/backups" "$d/logs"
    install -m 0600 "$ROOT/.dev/test-vault.ward" "$d/test-vault.ward"
    printf 'ward-state 2\nseen 0\nburned 0\nagent_failures 0\n' > "$d/test-vault.state"
    echo 2 > "$d/f64-proto"; secrets "$d"
    start 020cli "$W020" "$d" || { bad "wardd 0.2.0 subiu"; return; }
    local pid=$LAST_PID sock=$LAST_SOCK sd="${LAST_SOCK%/wardd.sock}"
    local before; before="$(wc -l < "$WORK/logs/fase64-wardd-020cli.log")"
    local mark="$WORK/v417-executou"
    r="$(WARD_DEV_SOCKET_DIR="$sd" HOME=/nao-existe LANG=C.UTF-8 "${F64[@]}" pty "$WORK/logs/fase64-v417-lista.screen" 20 - -- "$C014" lista)"
    echo "      ward 0.1.4 lista: $r — $(tr -d '\r' < "$WORK/logs/fase64-v417-lista.screen" | paste -sd' ')"
    check '[[ "$r" != "exit=0 "* ]]' "V4.17 lista sai ≠ 0"
    r="$(WARD_DEV_SOCKET_DIR="$sd" HOME=/nao-existe LANG=C.UTF-8 "${F64[@]}" pty "$WORK/logs/fase64-v417-executar.screen" 20 - -- "$C014" executar acme-pg-dev -- /usr/bin/touch "$mark")"
    echo "      ward 0.1.4 executar (não existe na CLI 0.1.4): $r — $(tr -d '\r' < "$WORK/logs/fase64-v417-executar.screen" | paste -sd' ')"
    r="$(WARD_DEV_SOCKET_DIR="$sd" HOME=/nao-existe LANG=C.UTF-8 "${F64[@]}" pty "$WORK/logs/fase64-v417-run.screen" 20 - -- "$C014" run acme-pg-dev -- /usr/bin/touch "$mark")"
    echo "      ward 0.1.4 run (operação executar): $r — $(tr -d '\r' < "$WORK/logs/fase64-v417-run.screen" | paste -sd' ')"
    check '[[ "$r" != "exit=0 "* ]]' "V4.17 run (executar) sai ≠ 0"
    check '[ ! -e "$mark" ]' "V4.17 nenhum comando executou"
    r="$(WARD_DEV_SOCKET_DIR="$sd" HOME=/nao-existe LANG=C.UTF-8 "${F64[@]}" pty "$WORK/logs/fase64-v417-destravar.screen" 30 "$d/f64-pw" -- "$C014" destravar)"
    echo "      ward 0.1.4 destravar: $r — $(tr -d '\r' < "$WORK/logs/fase64-v417-destravar.screen" | paste -sd' ')"
    check '[[ "$r" != "exit=0 "* ]]' "V4.17 destravar sai ≠ 0"
    r="$(agent 2 "$sock" status)"; echo "      status 0.2.0 depois: $r"
    check '[[ "$r" == "status - locked=True"* ]]' "V4.17 o daemon 0.2.0 continua travado"
    echo "      o que o 0.2.0 registrou (sem valores):"
    tail -n +"$((before + 1))" "$WORK/logs/fase64-wardd-020cli.log" | grep -a -o -E 'unknown_version|vers[a-zãõ ]+desconhecida|unlock[a-z_ ]{0,20}|destrav[a-z]*' | sort | uniq -c | sed 's/^/        /'
    stop "$pid"
}

# ---------------------------------------------------------------------------------------
sec_kill() {
    local n="${KILL_RUNS:-60}"
    echo "==> m2:V4.3 de ponta a ponta: SIGKILL no wardd 0.2.0 em instantes variados do primeiro destrave ($n rodadas)"
    local d; d="$(new_dir dkill 2)"
    local counts_v4=0 counts_v5=0 counts_other=0 i delay
    for i in $(seq 1 "$n"); do
        install -m 0600 "$LEG/vault-0.1.4.ward" "$d/test-vault.ward"
        install -m 0600 "$LEG/state-0.1.4" "$d/test-vault.state"
        rm -rf -- "$d/backups"; mkdir -m 0700 "$d/backups"; install -m 0600 "$LEG"/backup-0.1.4/*.ward "$d/backups/"
        start "k$i" "$W020" "$d" || { bad "rodada $i: subiu"; continue; }
        local pid=$LAST_PID sock=$LAST_SOCK
        # The first unlock (Argon2id + migration) ends well under 0.5 s on this host (a
        # first run with 0.5-4.4 s only saw v5): spread the kill over KILL_FROM..KILL_TO.
        delay="$(LC_ALL=C awk -v i="$i" -v n="$n" -v a="${KILL_FROM:-0.02}" -v b="${KILL_TO:-0.62}" 'BEGIN{printf "%.3f", a + (b - a) * (i - 1) / (n - 1)}')"
        agent 2 "$sock" unlock "$d/f64-pw" >/dev/null 2>&1 &
        local cpid=$!
        LC_ALL=C sleep "$delay"; kill -KILL "$pid" 2>/dev/null; wait "$pid" 2>/dev/null; wait "$cpid" 2>/dev/null
        if [ "$(sha256sum -- "$d/test-vault.ward" | cut -d' ' -f1)" = "$V4SHA" ]; then
            counts_v4=$((counts_v4 + 1)); kind=v4
        elif [ "$(magic "$d/test-vault.ward")" = WARDCOFR ]; then
            counts_v5=$((counts_v5 + 1)); kind=v5
        else
            counts_other=$((counts_other + 1)); kind=outro
        fi
        # The next opening works (and migrates if it is still v4).
        start "k${i}b" "$W020" "$d" || { bad "rodada $i: não subiu de novo"; continue; }
        r="$(agent 2 "$LAST_SOCK" unlock "$d/f64-pw")"
        stop "$LAST_PID"
        tmpfiles="$(find "$d" -maxdepth 2 -name '*tmp*' | wc -l)"
        echo "      rodada $i: kill em ${delay}s → $kind; próximo destrave: $r; depois: $(magic "$d/test-vault.ward") $(head -n1 "$d/test-vault.state"); temporários sobrando: $tmpfiles"
        [[ "$r" == "ok -"* ]] || bad "rodada $i: o próximo destrave não funcionou ($r)"
        [ "$(magic "$d/test-vault.ward")" = WARDCOFR ] && ! cmp -s "$d/test-vault.ward" "$LEG/vault-0.1.4.ward" || bad "rodada $i: depois do destrave o cofre não é v5"
        rm -f -- "$WORK"/logs/fase64-wardd-k"$i".log.keep
    done
    echo "      resultado: v4 intacta=$counts_v4 v5=$counts_v5 outro=$counts_other"
    check '[ "$counts_other" = 0 ]' "V4.3 nunca outra coisa além da v4 original ou da v5"
    if cat "$WORK"/logs/fase64-wardd-k*.log | grep -qaF -f "$ROOT/.dev/variations.txt"; then bad "S13: logs das rodadas com padrão"; else ok "S13: logs das rodadas sem padrão"; fi
}

for s in "$@"; do "sec_$s"; done
mkdir -p "$ROOT/.dev/f64-logs"
cp -- "$WORK"/logs/* "$ROOT/.dev/f64-logs/" 2>/dev/null
echo "fase64: $fails falha(s)"
[ "$fails" -eq 0 ]
