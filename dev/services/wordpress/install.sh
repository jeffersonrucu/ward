#!/bin/sh
# Installs the test WordPress (8081) and syncs wp_admin and wp_editor with the
# fixtures (D15 passwords read from /run/ward/). Passwords go in through stdin with
# `--prompt`, never in argv. Runs in the `wpcli` service and exits.
set -eu
cd /var/www/html

echo "wpcli: esperando o WordPress e o banco"
i=0
# Connection checked by PHP's mysqli; the password is read from the file, it does not go in argv.
db_ready() {
    php -r '$s = trim(file_get_contents("/run/ward/mariadb-wp"));
        $m = @mysqli_connect("mariadb", "wp", $s, "wordpress"); exit($m ? 0 : 1);' 2>/dev/null
}
until [ -f wp-config.php ] && db_ready; do
    i=$((i + 1))
    [ "$i" -le 90 ] || { echo "wpcli: WordPress/banco não ficou pronto" >&2; exit 1; }
    sleep 2
done

if ! wp core is-installed 2>/dev/null; then
    wp core install --url=http://localhost:8081 --title="Acme (teste)" \
        --admin_user=wp_admin --admin_email=wp_admin@teste.invalid --skip-email \
        --prompt=admin_password </run/ward/wp-admin >/dev/null
fi
wp user update wp_admin --skip-email --prompt=user_pass </run/ward/wp-admin >/dev/null
if ! wp user get wp_editor --field=ID >/dev/null 2>&1; then
    wp user create wp_editor wp_editor@teste.invalid --role=editor --porcelain \
        --prompt=user_pass </run/ward/wp-editor >/dev/null
else
    wp user update wp_editor --skip-email --prompt=user_pass </run/ward/wp-editor >/dev/null
fi
echo "wpcli: WordPress de teste pronto em http://localhost:8081 (wp_admin, wp_editor)"
