#!/bin/bash -p
# Verifies the test services (V1.4). Needs `make dev-services` and compose
# up. Usage: dev/services/verify.sh   (or `make dev-services-verify`)
set -euo pipefail

# Fixed PATH before any command (Phase 12b): nothing from the user's PATH runs here.
export PATH=/usr/bin:/bin
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/services/common.sh
. "$ROOT/dev/services/common.sh"
require docker sshpass ssh curl

failures=0
ok() { printf 'ok    %s\n' "$1"; }
failed() { printf 'FALHA %s\n' "$1" >&2; failures=$((failures + 1)); }

wait_wordpress || failed "wpcli"

# Before any other client: proves, under strace and with the home covered by tmpfs, that
# no test client opens a path of the real user's home (Phase 12a).
"$ROOT/dev/tests/real-home.sh" || { echo "clientes de teste tocam o home real; abortando" >&2; exit 1; }

# postgres (55432) lets app in
if [ "$(psql_app 'select 1' 2>/dev/null)" = "1" ]; then ok "postgres 55432 entra com app"; else failed "postgres 55432 com app"; fi

# 2222 accepts deploy by password
if [ "$(ssh_password "$SECRETS_DIR/ssh-deploy" 2222 deploy whoami 2>/dev/null)" = "deploy" ]; then
    ok "ssh-senha 2222 aceita deploy por senha"
else
    failed "ssh-senha 2222 deploy por senha"
fi

# sftpuser gets in by sftp
if printf 'ls\n' | ssh_password "$SECRETS_DIR/ssh-sftpuser" 2222 sftpuser -s sftp >/dev/null 2>&1; then
    ok "ssh-senha 2222 aceita sftpuser por sftp"
else
    failed "ssh-senha 2222 sftpuser por sftp"
fi

# 2223 refuses the password and accepts the key
if ssh_password "$SECRETS_DIR/ssh-deploy" 2223 deploy true >/dev/null 2>&1; then
    failed "ssh-chave 2223 ACEITOU senha"
else
    ok "ssh-chave 2223 recusa senha"
fi
if [ "$(ssh_key 2223 deploy whoami 2>/dev/null)" = "deploy" ]; then ok "ssh-chave 2223 aceita a chave"; else failed "ssh-chave 2223 com a chave"; fi

# 2224 accepts the jump user
if [ "$(ssh_password "$SECRETS_DIR/ssh-salto" 2224 salto whoami 2>/dev/null)" = "salto" ]; then ok "ssh-salto 2224 aceita salto"; else failed "ssh-salto 2224 salto"; fi

# Internal host: no port on the host; answers from the jump
if [ -z "$(docker port "$("${COMPOSE[@]}" ps -q ssh-interno)")" ]; then
    ok "ssh-interno sem porta publicada no host"
else
    failed "ssh-interno tem porta publicada"
fi
if [ "$(ssh_internal hostname 2>/dev/null)" = "ssh-interno" ]; then ok "ssh-interno responde pelo ssh-salto"; else failed "ssh-interno pelo salto"; fi

# WordPress (8081) accepts wp_admin (and wp_editor)
for pair in "wp_admin:wp-admin" "wp_editor:wp-editor"; do
    user="${pair%%:*}" file="$SECRETS_DIR/${pair#*:}"
    headers="$(wp_login "$user" "$file" 2>/dev/null || true)"
    if grep -qi '^location: http://localhost:8081/wp-admin' <<<"$headers" && grep -qi 'set-cookie: wordpress_logged_in' <<<"$headers"; then
        ok "wordpress 8081 aceita $user"
    else
        failed "wordpress 8081 $user"
    fi
done

# phish (8082), totp (8083) and redir (8084) answer
[ "$(client curl -q -s -o /dev/null -w '%{http_code}' http://localhost:8082/wp-login.php)" = 200 ] && ok "phish 8082 responde" || failed "phish 8082"
[ "$(client curl -q -s -o /dev/null -w '%{http_code}' http://localhost:8083/login)" = 200 ] && ok "totp 8083 responde" || failed "totp 8083"
loc="$(client curl -q -s -o /dev/null -w '%{http_code} %{redirect_url}' http://localhost:8084/login)"
[ "$loc" = "302 http://localhost:8082/wp-login.php" ] && ok "redir 8084 redireciona para 8082" || failed "redir 8084 ($loc)"

if [ "$failures" -ne 0 ]; then
    printf '\n%d conferência(s) falharam.\n' "$failures" >&2
    exit 1
fi
printf '\nServiços de teste ok (V1.4).\n'
