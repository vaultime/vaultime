#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Builds the website and installs it on the server, next to the folder the
# release workflow uploads installers to.
#
#   VAULTIME_DEPLOY_HOST=root@server scripts/deploy-site.sh [domain]
#
# domain defaults to vaultime.codfishcloud.de. On Windows, set SSH and SCP to the
# Windows OpenSSH binaries when the key lives in the Windows ssh-agent. When
# ~/.ssh/vaultime-release-upload.pub exists (or VAULTIME_RELEASE_KEY names
# another public key), the release workflow may upload installers with it.
set -euo pipefail

host=${VAULTIME_DEPLOY_HOST:?set VAULTIME_DEPLOY_HOST, for example root@your-server}
domain=${1:-vaultime.codfishcloud.de}
ssh_cmd=${SSH:-ssh}
scp_cmd=${SCP:-scp}

repo="$(cd "$(dirname "$0")/.." && pwd)"
stage="$repo/dist-deploy"
rm -rf "$stage"
mkdir -p "$stage"

payload=(install-site.sh)
cp "$repo/deploy/vps/install-site.sh" "$stage/"

python=$(command -v python3 || command -v python)
"$python" "$repo/scripts/build-site.py" >/dev/null
tar -czf "$stage/site.tar.gz" -C "$repo/dist-site" .
payload+=(site.tar.gz)
release_key=${VAULTIME_RELEASE_KEY:-$HOME/.ssh/vaultime-release-upload.pub}
if [ -f "$release_key" ]; then
  cp "$release_key" "$stage/release-upload.pub"
  payload+=(release-upload.pub)
fi

# A fresh folder only root can enter, so nobody else on the box can swap
# what root runs next.
remote_dir=$("$ssh_cmd" "$host" 'mktemp -d /tmp/vaultime-deploy.XXXXXXXX')
# Windows OpenSSH wants Windows paths.
local_path() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
files=()
for file in "${payload[@]}"; do files+=("$(local_path "$stage/$file")"); done
"$scp_cmd" -q "${files[@]}" "$host:$remote_dir/"
"$ssh_cmd" "$host" "bash '$remote_dir/install-site.sh' '$domain'; status=\$?; rm -rf '$remote_dir'; exit \$status"
