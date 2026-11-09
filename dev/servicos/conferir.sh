#!/bin/bash -p
# Confere os serviços de teste (V1.4). Precisa de `make dev-servicos` e do compose de
# pé. Uso: dev/servicos/conferir.sh   (ou `make dev-servicos-conferir`)
set -euo pipefail

# PATH fixo antes de qualquer comando (Fase 12b): nada do PATH do usuário roda aqui.
export PATH=/usr/bin:/bin
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/servicos/comum.sh
. "$RAIZ/dev/servicos/comum.sh"
exigir docker sshpass ssh curl

falhas=0
ok() { printf 'ok    %s\n' "$1"; }
falhou() { printf 'FALHA %s\n' "$1" >&2; falhas=$((falhas + 1)); }

esperar_wordpress || falhou "wpcli"

# Antes de qualquer outro cliente: prova, sob strace e com o home coberto por tmpfs, que
# nenhum cliente de teste abre caminho do home do usuário real (Fase 12a).
"$RAIZ/dev/testes/home-real.sh" || { echo "clientes de teste tocam o home real; abortando" >&2; exit 1; }

# postgres (55432) entra com app
if [ "$(psql_app 'select 1' 2>/dev/null)" = "1" ]; then ok "postgres 55432 entra com app"; else falhou "postgres 55432 com app"; fi

# ssh-senha (2222) aceita deploy por senha
if [ "$(ssh_senha "$SEG/ssh-deploy" 2222 deploy whoami 2>/dev/null)" = "deploy" ]; then
    ok "ssh-senha 2222 aceita deploy por senha"
else
    falhou "ssh-senha 2222 deploy por senha"
fi

# sftpuser entra por sftp
if printf 'ls\n' | ssh_senha "$SEG/ssh-sftpuser" 2222 sftpuser -s sftp >/dev/null 2>&1; then
    ok "ssh-senha 2222 aceita sftpuser por sftp"
else
    falhou "ssh-senha 2222 sftpuser por sftp"
fi

# ssh-chave (2223) recusa senha e aceita a chave
if ssh_senha "$SEG/ssh-deploy" 2223 deploy true >/dev/null 2>&1; then
    falhou "ssh-chave 2223 ACEITOU senha"
else
    ok "ssh-chave 2223 recusa senha"
fi
if [ "$(ssh_chave 2223 deploy whoami 2>/dev/null)" = "deploy" ]; then ok "ssh-chave 2223 aceita a chave"; else falhou "ssh-chave 2223 com a chave"; fi

# ssh-salto (2224) aceita salto
if [ "$(ssh_senha "$SEG/ssh-salto" 2224 salto whoami 2>/dev/null)" = "salto" ]; then ok "ssh-salto 2224 aceita salto"; else falhou "ssh-salto 2224 salto"; fi

# ssh-interno: sem porta no host; responde a partir do salto
if [ -z "$(docker port "$("${COMPOSE[@]}" ps -q ssh-interno)")" ]; then
    ok "ssh-interno sem porta publicada no host"
else
    falhou "ssh-interno tem porta publicada"
fi
if [ "$(ssh_interno hostname 2>/dev/null)" = "ssh-interno" ]; then ok "ssh-interno responde pelo ssh-salto"; else falhou "ssh-interno pelo salto"; fi

# WordPress (8081) aceita wp_admin (e wp_editor)
for par in "wp_admin:wp-admin" "wp_editor:wp-editor"; do
    usuario="${par%%:*}" arq="$SEG/${par#*:}"
    cab="$(wp_login "$usuario" "$arq" 2>/dev/null || true)"
    if grep -qi '^location: http://localhost:8081/wp-admin' <<<"$cab" && grep -qi 'set-cookie: wordpress_logged_in' <<<"$cab"; then
        ok "wordpress 8081 aceita $usuario"
    else
        falhou "wordpress 8081 $usuario"
    fi
done

# phish (8082), totp (8083) e redir (8084) respondem
[ "$(cliente curl -q -s -o /dev/null -w '%{http_code}' http://localhost:8082/wp-login.php)" = 200 ] && ok "phish 8082 responde" || falhou "phish 8082"
[ "$(cliente curl -q -s -o /dev/null -w '%{http_code}' http://localhost:8083/login)" = 200 ] && ok "totp 8083 responde" || falhou "totp 8083"
loc="$(cliente curl -q -s -o /dev/null -w '%{http_code} %{redirect_url}' http://localhost:8084/login)"
[ "$loc" = "302 http://localhost:8082/wp-login.php" ] && ok "redir 8084 redireciona para 8082" || falhou "redir 8084 ($loc)"

if [ "$falhas" -ne 0 ]; then
    printf '\n%d conferência(s) falharam.\n' "$falhas" >&2
    exit 1
fi
printf '\nServiços de teste ok (V1.4).\n'
