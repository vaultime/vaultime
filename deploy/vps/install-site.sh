#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: AGPL-3.0-or-later

# Installs or updates the Vaultime website and its downloads on Ubuntu 24.04.
# Run as root. Caddy has to be installed already. The script writes the site
# to its own file and imports it from the Caddyfile.
#
# Optional files next to the script: site.tar.gz replaces the website, and
# release-upload.pub is the key the release workflow uploads installers with.
#
#   install-site.sh <domain>     the public host, for example vaultime.example.com

set -euo pipefail

domain=${1:?usage: install-site.sh <domain>}
here=$(cd "$(dirname "$0")" && pwd)
# How long browsers remember to use https only, one year in seconds.
hsts_max_age_secs=31536000

[ "$(id -u)" -eq 0 ] || { echo "run as root" >&2; exit 1; }

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

# The Caddy site lives in its own file that the Caddyfile imports. Other sites
# on the box stay untouched, and a config that does not validate is rolled back.
caddyfile=/etc/caddy/Caddyfile
caddy_site=/etc/caddy/vaultime.caddy
cp "$caddyfile" "$caddyfile.before-vaultime"
if [ -f "$caddy_site" ]; then cp "$caddy_site" "$caddy_site.before-vaultime"; fi
cat >"$caddy_site" <<EOF
$domain {
	encode zstd gzip
	header {
		Strict-Transport-Security "max-age=$hsts_max_age_secs"
		X-Content-Type-Options nosniff
		Referrer-Policy no-referrer
		Content-Security-Policy "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'"
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
		file_server
	}
	# A missing file gets the site's 404 page, a missing download the page
	# that says it is not available.
	handle_errors {
		@missing_download expression \`{err.status_code} == 404 && {http.request.orig_uri.path}.startsWith("/downloads/")\`
		@missing expression \`{err.status_code} == 404\`
		handle @missing_download {
			root * $site_root
			rewrite * /download-soon.html
			file_server
		}
		handle @missing {
			root * $site_root
			rewrite * /404.html
			file_server
		}
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

# The hosted cloud API of earlier installs gets no requests from Caddy any
# more. Its data stays until the operator removes it.
if [ -f /etc/systemd/system/vaultime-api.service ]; then
  echo "The old vaultime-api service is still installed but no longer reachable"
fi
