#!/bin/bash -p
# fase62: m2:V6.6 (CLI part), m2:V2.5, m2:V4.20 and m2:V6.7 (without the bridge) by hand
# with the test CLI against the test daemon. Usage: v66.sh <repo root>
export PATH=/usr/bin:/bin
R="$1"; cd "$R" || exit 2
W="$R/target/debug/ward"
step() { printf '\n## %s\n' "$*"; }
rc() { echo "exit=$?"; }

step "V6.6 ward run acme-pg-dev -- pg_select.py 'select 1' (sem psql no host; cliente pg_select.py)"
"$W" run acme-pg-dev -- /usr/bin/python3 -I "$R/dev/tests/pg_select.py" 'select 1' </dev/null; rc
step "V6.6 ward run acme-pg-tunel -- pg_select.py 'select 1'"
"$W" run acme-pg-tunel -- /usr/bin/python3 -I "$R/dev/tests/pg_select.py" 'select 1' </dev/null; rc
for e in acme-ssh-senha-stage acme-ssh-chave-prod acme-ssh-interno; do
    step "V6.6 ward ssh $e -- true"
    timeout 90 "$W" ssh "$e" -- true </dev/null; rc
done
step "V6.6 ward ssh acme-ssh-interno -- hostname (salto)"
timeout 90 "$W" ssh acme-ssh-interno -- hostname </dev/null; rc

step "V2.5 máscara: PGPASSWORD inteiro, em dois write, URL-encoded, base64 e DATABASE_URL"
"$W" run acme-pg-dev -- /bin/sh -c 'printf "%s\n" "$PGPASSWORD"; printf "%s" "${PGPASSWORD%%s3nha*}"; printf "%s\n" "s3nha${PGPASSWORD#*s3nha}"; printf "%s\n" "$DATABASE_URL"; printf "%s" "$PGPASSWORD" | base64 -w0; echo; python3 -I -c "import os,urllib.parse;print(urllib.parse.quote(os.environ[\"PGPASSWORD\"],safe=\"\"))"' </dev/null; rc

step "V4.20 anexo 0600 em <rt>tmp/ e apagado no fim (gama-generico, acme-ssh-chave-prod)"
"$W" run gama-generico -- /bin/sh -c 'stat -c "%a %U" "$WARD_FILE_APP_ENV"; echo "$WARD_FILE_APP_ENV" > '"$R"'/.dev/fase62-anexo-1' </dev/null; rc
f=$(cat "$R/.dev/fase62-anexo-1"); case "$f" in "$XDG_RUNTIME_DIR"/ward/tmp/*) echo "caminho em <rt>/ward/tmp/: sim";; *) echo "caminho em <rt>/ward/tmp/: NÃO ($f)";; esac
[ -e "$f" ] && echo "anexo ainda existe: SIM (falha)" || echo "anexo apagado depois do fim: sim"
"$W" run acme-ssh-chave-prod -- /bin/sh -c 'stat -c "%a %U" "$WARD_FILE_KEY"; echo "$WARD_FILE_KEY" > '"$R"'/.dev/fase62-anexo-2; exit 7' </dev/null; rc
f=$(cat "$R/.dev/fase62-anexo-2"); [ -e "$f" ] && echo "chave ainda existe depois de exit 7: SIM (falha)" || echo "chave apagada depois de exit 7: sim"
"$W" run gama-generico -- /bin/sh -c 'echo "$WARD_FILE_APP_ENV" > '"$R"'/.dev/fase62-anexo-3; kill -KILL $$' </dev/null; rc
f=$(cat "$R/.dev/fase62-anexo-3"); [ -e "$f" ] && echo "anexo ainda existe depois de SIGKILL: SIM (falha)" || echo "anexo apagado depois de SIGKILL: sim"
rm -f "$R"/.dev/fase62-anexo-[123]

step "V6.7 ward list sem gama-bloqueada"
"$W" list | grep -c gama-bloqueada; echo "(0 = não aparece)"
"$W" list --json | grep -c gama-bloqueada; echo "(0 = não aparece no --json)"
step "V6.7 ward run gama-bloqueada -- true"
"$W" run gama-bloqueada -- true </dev/null; rc
step "V6.7 ward run inexistente-ward -- true (comparação)"
"$W" run inexistente-ward -- true </dev/null; rc
step "V6.7 ward run acme-pg-dev -- true (livre: não abre aprovação)"
"$W" run acme-pg-dev -- true </dev/null; rc
step "V6.7 ward run acme-pg-aprovacao -- true (sem ponte: aprovação indisponível)"
timeout 30 "$W" run acme-pg-aprovacao -- true </dev/null; rc
