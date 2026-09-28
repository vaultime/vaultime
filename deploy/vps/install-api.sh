#!/usr/bin/env bash
# Installs or updates the Vaultime API on Ubuntu 24.04. Run as root with the
# release binary and the systemd units (vaultime-api.service and the
# vaultime-db-backup service and timer) next to this script. Running it again
# updates the binary and keeps the database and secrets. Caddy has to
# be installed already. The script writes the site to its own file and
# imports it from the Caddyfile.
#
# Optional files next to the script: site.tar.gz replaces the website, and
# release-upload.pub is the key the release workflow uploads installers with.
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

# Keep the running version, so a release that does not come up is rolled back.
binary=/srv/vaultime/api/current/vaultime-api
unit=/etc/systemd/system/vaultime-api.service
[ -f "$binary" ] && cp -f "$binary" "$binary.previous"
[ -f "$unit" ] && cp -f "$unit" "$unit.previous"
install -m 0755 "$here/vaultime-api" "$binary.new"
mv -f "$binary.new" "$binary"
install -m 0644 "$here/vaultime-api.service" "$unit"

# Nightly database dumps, readable by root and postgres only.
install -d -o postgres -g postgres -m 0700 /var/backups/vaultime
install -m 0644 "$here/vaultime-db-backup.service" "$here/vaultime-db-backup.timer" /etc/systemd/system/

# Admin tools for creating the first admin account and invites.
install -d -o root -g root -m 0700 /srv/vaultime/tools
for tool in bootstrap-admin-account.py generate-cloud-invite.py; do
  if [ -f "$here/$tool" ]; then install -m 0700 "$here/$tool" /srv/vaultime/tools/; fi
done
systemctl daemon-reload
systemctl enable --now vaultime-db-backup.timer >/dev/null
systemctl enable vaultime-api >/dev/null
systemctl restart vaultime-api

# The website, replaced as a whole.
site_root=/srv/vaultime-site
if [ -f "$here/site.tar.gz" ]; then
  rm -rf "$site_root.new" "$site_root.old"
  install -d -m 0755 "$site_root.new"
  tar -xzf "$here/site.tar.gz" -C "$site_root.new" --no-same-owner --no-same-permissions
  chmod -R u=rwX,go=rX "$site_root.new"
  [ -d "$site_root" ] && mv "$site_root" "$site_root.old"
  mv "$site_root.new" "$site_root"
  rm -rf "$site_root.old"
fi
install -d -m 0755 "$site_root"

# Installers and the updater manifest. The release workflow signs in as
# vaultime-release, and its key may only run rrsync inside this folder. Links
# it uploads are made harmless, so Caddy cannot be pointed outside the folder.
downloads=/srv/vaultime-downloads
release_home=/var/lib/vaultime-release
id vaultime-release >/dev/null 2>&1 \
  || useradd --system --create-home --home-dir "$release_home" --shell /bin/sh vaultime-release
# No password can match "*", and unlike a locked account it still takes keys.
usermod -p '*' vaultime-release
install -d -o vaultime-release -g vaultime-release -m 0755 "$downloads"
if [ -f "$here/release-upload.pub" ]; then
  key=$(tr -d '\r' <"$here/release-upload.pub" | head -n 1)
  case $key in
    ssh-ed25519\ *) ;;
    *) echo "release-upload.pub is not an ed25519 public key" >&2; exit 1 ;;
  esac
  install -d -o vaultime-release -g vaultime-release -m 0700 "$release_home/.ssh"
  printf 'command="/usr/bin/rrsync -munge %s",restrict %s\n' "$downloads" "$key" >"$release_home/.ssh/authorized_keys"
  chown vaultime-release:vaultime-release "$release_home/.ssh/authorized_keys"
  chmod 0600 "$release_home/.ssh/authorized_keys"
fi

healthy() {
  for _ in $(seq 1 20); do
    curl -fsS http://127.0.0.1:9005/healthz >/dev/null 2>&1 && return 0
    sleep 1
  done
  return 1
}
if ! healthy; then
  journalctl -u vaultime-api -n 30 --no-pager
  if [ -f "$binary.previous" ]; then
    mv -f "$binary.previous" "$binary"
    [ -f "$unit.previous" ] && mv -f "$unit.previous" "$unit"
    systemctl daemon-reload
    systemctl restart vaultime-api
    healthy && echo "The new release did not come up, the previous one runs again" >&2
  fi
  exit 1
fi
echo "API is up on 127.0.0.1:9005"

# The Caddy site lives in its own file that the Caddyfile imports. Other sites
# on the box stay untouched, and a config that does not validate is rolled back.
caddyfile=/etc/caddy/Caddyfile
caddy_site=/etc/caddy/vaultime.caddy
cp "$caddyfile" "$caddyfile.before-vaultime"
if [ -f "$caddy_site" ]; then cp "$caddy_site" "$caddy_site.before-vaultime"; fi
cat >"$caddy_site" <<EOF
$domain {
	encode zstd gzip

	handle /v1/* {
		reverse_proxy 127.0.0.1:9005
	}
	handle /healthz {
		reverse_proxy 127.0.0.1:9005
	}

	# Installers change behind the same latest links, so browsers and the
	# updater always ask again.
	handle_path /downloads/* {
		root * $downloads
		header Cache-Control no-cache
		file_server
	}

	handle {
		root * $site_root
		header {
			X-Content-Type-Options nosniff
			Referrer-Policy no-referrer
			Content-Security-Policy "default-src 'self'; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'"
		}
		file_server
	}
}
EOF
chmod 0644 "$caddy_site"
if ! grep -qxF "import $caddy_site" "$caddyfile"; then
  # Earlier installs wrote the site into the Caddyfile itself.
  {
    awk -v start="$domain {" '$0 == start { skip = 1; next } skip { if ($0 == "}") skip = 0; next } { print }' \
      "$caddyfile.before-vaultime"
    printf '\nimport %s\n' "$caddy_site"
  } | cat -s >"$caddyfile"
fi

restore_caddy() {
  mv -f "$caddyfile.before-vaultime" "$caddyfile"
  if [ -f "$caddy_site.before-vaultime" ]; then
    mv -f "$caddy_site.before-vaultime" "$caddy_site"
  else
    rm -f "$caddy_site"
  fi
}
if ! caddy validate --config "$caddyfile" --adapter caddyfile >/dev/null 2>&1; then
  restore_caddy
  echo "Caddy config did not validate, restored the previous one" >&2
  exit 1
fi
if cmp -s "$caddyfile" "$caddyfile.before-vaultime" \
  && cmp -s "$caddy_site" "$caddy_site.before-vaultime"; then
  echo "Caddy config unchanged"
else
  systemctl reload caddy
  echo "Caddy now serves $domain"
fi
rm -f "$caddyfile.before-vaultime" "$caddy_site.before-vaultime"
