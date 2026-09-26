#!/usr/bin/env bash
# Installs or updates the Vaultime API on Ubuntu 24.04. Run as root with the
# release binary and vaultime-api.service next to this script. Running it
# again updates the binary and keeps the database and secrets. Caddy has to
# be installed already. The script adds a site to its config.
#
#   install-api.sh <domain>     the public API host, for example api.example.com
set -euo pipefail

domain=${1:?usage: install-api.sh <domain>}
here=$(cd "$(dirname "$0")" && pwd)

[ "$(id -u)" -eq 0 ] || { echo "run as root" >&2; exit 1; }
[ -f "$here/vaultime-api" ] || { echo "vaultime-api binary missing next to this script" >&2; exit 1; }

export DEBIAN_FRONTEND=noninteractive
if ! command -v psql >/dev/null; then
  apt-get update -qq
  apt-get install -y -qq postgresql >/dev/null
fi
systemctl enable --now postgresql >/dev/null

id vaultime >/dev/null 2>&1 \
  || useradd --system --home-dir /srv/vaultime --shell /usr/sbin/nologin vaultime
install -d -o vaultime -g vaultime -m 0750 /srv/vaultime /srv/vaultime/backups /srv/vaultime/tmp
install -d -o root -g root -m 0755 /srv/vaultime/api /srv/vaultime/api/current
install -d -o root -g vaultime -m 0750 /etc/vaultime

pg() { runuser -u postgres -- psql -qtA -v ON_ERROR_STOP=1 "$@"; }

# First install: database, role and secrets. Secrets never leave the server.
env_file=/etc/vaultime/api.env
if [ ! -f "$env_file" ]; then
  db_password=$(openssl rand -hex 24)
  if [ "$(pg -c "SELECT 1 FROM pg_roles WHERE rolname = 'vaultime'")" = "1" ]; then
    pg -c "ALTER ROLE vaultime LOGIN PASSWORD '$db_password'"
  else
    pg -c "CREATE ROLE vaultime LOGIN PASSWORD '$db_password'"
  fi
  if [ "$(pg -c "SELECT 1 FROM pg_database WHERE datname = 'vaultime'")" != "1" ]; then
    runuser -u postgres -- createdb -O vaultime vaultime
  fi
  pg -d vaultime -c "CREATE EXTENSION IF NOT EXISTS pgcrypto; CREATE EXTENSION IF NOT EXISTS citext;"

  umask 027
  cat >"$env_file" <<EOF
VAULTIME_API_BIND=127.0.0.1:9005
VAULTIME_PUBLIC_BASE_URL=https://$domain
VAULTIME_DATABASE_URL=postgres://vaultime:$db_password@127.0.0.1:5432/vaultime
VAULTIME_BACKUP_ROOT=/srv/vaultime/backups
VAULTIME_ACCESS_TOKEN_SECRET=$(openssl rand -hex 32)
VAULTIME_REFRESH_TOKEN_PEPPER=$(openssl rand -hex 32)
VAULTIME_MAX_BACKUP_BYTES=536870912
VAULTIME_MAX_PENDING_BACKUPS_PER_ACCOUNT=1
VAULTIME_MAX_COMPLETE_BACKUPS_PER_ACCOUNT=30
VAULTIME_MIN_BACKUP_INTERVAL_SECONDS=900
VAULTIME_STALE_PENDING_BACKUP_SECONDS=3600
EOF
  chown root:vaultime "$env_file"
  chmod 0640 "$env_file"
fi

install -m 0755 "$here/vaultime-api" /srv/vaultime/api/current/vaultime-api.new
mv -f /srv/vaultime/api/current/vaultime-api.new /srv/vaultime/api/current/vaultime-api
install -m 0644 "$here/vaultime-api.service" /etc/systemd/system/vaultime-api.service

# Admin tools for creating the first admin account and invites.
install -d -o root -g root -m 0700 /srv/vaultime/tools
for tool in bootstrap-admin-account.py generate-cloud-invite.py; do
  if [ -f "$here/$tool" ]; then install -m 0700 "$here/$tool" /srv/vaultime/tools/; fi
done
systemctl daemon-reload
systemctl enable vaultime-api >/dev/null
systemctl restart vaultime-api

for _ in $(seq 1 20); do
  curl -fsS http://127.0.0.1:9005/healthz >/dev/null 2>&1 && break
  sleep 1
done
curl -fsS http://127.0.0.1:9005/healthz >/dev/null \
  || { journalctl -u vaultime-api -n 30 --no-pager; exit 1; }
echo "API is up on 127.0.0.1:9005"

# Add the site to Caddy once. Other sites on the box stay untouched, and a
# config that does not validate is rolled back.
caddyfile=/etc/caddy/Caddyfile
if ! grep -q "^$domain {" "$caddyfile"; then
  cp "$caddyfile" "$caddyfile.before-vaultime"
  printf '\n%s {\n\tencode zstd gzip\n\treverse_proxy 127.0.0.1:9005\n}\n' "$domain" >>"$caddyfile"
  if caddy validate --config "$caddyfile" --adapter caddyfile >/dev/null 2>&1; then
    systemctl reload caddy
    echo "Caddy now serves $domain"
  else
    mv -f "$caddyfile.before-vaultime" "$caddyfile"
    echo "Caddy config did not validate, restored the previous one" >&2
    exit 1
  fi
fi
