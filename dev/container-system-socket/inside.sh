#!/bin/bash -p
# Inside part of `make test-system-socket` (m2:Phase 54a; S1, S5, D46). Runs as root
# **only inside** the disposable `ubuntu:24.04` container without network; from the host
# come the worktree (at its own path, for the test binaries), the read-only mount under
# /opt (the list of test binaries, the release `ward-nm` and the fake server) and
# the output folder /opt/out.
#
# 1. Plants the fake system socket: `/run/ward/1000/` of root 0755, `wardd.sock` 0600 of
#    uid 1000, listened by the fake (`dev/container-platform/fake.c`) as root and
#    non-dumpable: what the system systemd gives the installed `wardd` (D46), so the
#    release client accepts it. The fake writes one line per connection and per read.
# 2. Runs each test binary as uid 1000 (the image's `ubuntu`), with a clean environment
#    and a time limit; the exit code only goes to the report.
# 3. The record has to be empty. Then the control: the release `ward-nm` with a `fill`
#    has to connect to the fake (the plant is reachable and the record catches it).

export PATH=/usr/sbin:/usr/bin:/bin
set -euo pipefail

[ "$(id -u)" = 0 ] || { echo "inside.sh roda como root, só no contêiner" >&2; exit 1; }
[ -f /.dockerenv ] || { echo "inside.sh só roda dentro do contêiner" >&2; exit 1; }

OUT=/opt/out
RECORD=/run/ward-fake-record
PER_BINARY="${PER_BINARY:-600}"
SOCK=/run/ward/1000/wardd.sock
ORIGIN="chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/"
failures=0
ok() { echo "ok: $*"; }
failed() { echo "FALHOU: $*"; failures=$((failures + 1)); }
as_user() {
    setpriv --reuid=1000 --regid=1000 --clear-groups -- \
        env -i PATH=/usr/bin:/bin HOME=/home/ubuntu LANG=C.UTF-8 \
        XDG_RUNTIME_DIR=/run/user/1000 "$@"
}
connections() { grep -c '^conexao' "$RECORD" || true; }

[ -e /run/ward ] && { echo "/run/ward já existe no contêiner" >&2; exit 1; }
install -d -m 0755 -o 0 -g 0 /run/ward /run/ward/1000
install -d -m 0700 -o 1000 -g 1000 /run/user/1000
install -d -m 0755 -o 1000 -g 1000 /home/ubuntu
: > "$RECORD"
chmod 0600 "$RECORD"

/opt/teste/fake "$SOCK" "$RECORD" 86400 > /tmp/fake.out 2>&1 &
FAKE=$!
for _ in $(seq 1 100); do
    grep -q '^pronto' /tmp/fake.out 2>/dev/null && break
    sleep 0.1
done
grep -q '^pronto dumpable=0' /tmp/fake.out || { echo "o falso não subiu:"; cat /tmp/fake.out; exit 1; }
chown 1000:1000 "$SOCK"
chmod 0600 "$SOCK"
ok "socket do sistema falso em $SOCK (pasta do root, ouvido pelo uid 0, não-dumpable)"

total=0
passed=0
not_passed=0
n=0
while IFS=$'\t' read -r dir exe; do
    n=$((n + 1))
    total=$((total + 1))
    log="$OUT/logs/$n-${exe##*/}.log"
    code=0
    (cd "$dir" && as_user CARGO_MANIFEST_DIR="$dir" \
        timeout --kill-after=10 "$PER_BINARY" "$exe" --test-threads=2 -q) \
        > "$log" 2>&1 < /dev/null || code=$?
    if [ "$code" -eq 0 ]; then passed=$((passed + 1)); else not_passed=$((not_passed + 1)); fi
    echo "saída $code: ${exe#"$dir"/} (conexões até aqui: $(connections))"
done < /opt/teste/binaries
echo "binários: $total; passaram: $passed; não passaram no contêiner: $not_passed"

# Late connections (a test that left a child behind) still count.
sleep 3
c="$(connections)"
if [ "$c" -eq 0 ]; then
    ok "nenhum teste conectou no socket do sistema falso"
else
    failed "$c conexão(ões) no socket do sistema falso durante os testes"
fi

# Control: the release host, with a `fill`, goes to the system socket.
fill_frame() {
    # 4 bytes of native size + the JSON (the host runs here, little-endian x86_64).
    local body='{"v":2,"op":"fill","data":{"entry":"acme-wp-dev","origin":"http://localhost:8081"}}'
    local len=${#body}
    printf "\\x$(printf %02x $((len & 255)))\\x$(printf %02x $(((len >> 8) & 255)))\\x00\\x00%s" "$body"
}
before="$c"
fill_frame | as_user timeout 30 /opt/teste/ward-nm "$ORIGIN" > /dev/null 2>&1 || true
sleep 3
after="$(connections)"
if [ "$after" -gt "$before" ] && grep -q '^bytes' "$RECORD"; then
    ok "controle: o ward-nm de release conecta no socket falso e o registro pega"
else
    failed "controle: o ward-nm de release não chegou ao socket falso (o teste não provaria nada)"
fi

kill "$FAKE" 2>/dev/null || true
cp -- "$RECORD" "$OUT/record"
chown -R 1000:1000 "$OUT"
chmod -R u+rwX,go+rX "$OUT"
exit $((failures > 0))
