#!/usr/bin/env bash
# Builds the API for Ubuntu 24.04 in Docker and installs it on the server
# together with the website.
#
#   VAULTIME_DEPLOY_HOST=root@server scripts/deploy-api.sh [domain]
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
repo_mount="$(cd "$repo" && pwd -W 2>/dev/null || pwd)"
stage="$repo/dist-api"
mkdir -p "$stage"

docker build -q -t vaultime-linux-check "$repo/scripts/linux-check" >/dev/null
MSYS_NO_PATHCONV=1 docker run --rm \
  --mount "type=bind,source=$repo_mount,target=/src,readonly" \
  --mount "type=bind,source=$repo_mount/dist-api,target=/out" \
  -e CARGO_TARGET_DIR=/target \
  -v vaultime-api-target:/target \
  -v vaultime-linux-registry:/opt/cargo/registry \
  -v vaultime-linux-toolchains:/opt/rustup \
  vaultime-linux-check bash -c '
    cd /src/apps/api
    rustup toolchain install >/dev/null
    cargo build --release --locked -q
    cp /target/release/vaultime-api /out/
  '

payload=(install-api.sh vaultime-api.service vaultime-db-backup.service vaultime-db-backup.timer
  bootstrap-admin-account.py generate-cloud-invite.py)
for file in "${payload[@]}"; do cp "$repo/deploy/vps/$file" "$stage/"; done

python=$(command -v python3 || command -v python)
"$python" "$repo/scripts/build-site.py" >/dev/null
tar -czf "$stage/site.tar.gz" -C "$repo/dist-site" .
payload+=(site.tar.gz)
release_key=${VAULTIME_RELEASE_KEY:-$HOME/.ssh/vaultime-release-upload.pub}
if [ -f "$release_key" ]; then
  cp "$release_key" "$stage/release-upload.pub"
  payload+=(release-upload.pub)
fi

"$ssh_cmd" "$host" 'rm -rf /tmp/vaultime-deploy && mkdir -p /tmp/vaultime-deploy'
# Windows OpenSSH wants Windows paths.
local_path() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
files=("$(local_path "$stage/vaultime-api")")
for file in "${payload[@]}"; do files+=("$(local_path "$stage/$file")"); done
"$scp_cmd" -q "${files[@]}" "$host:/tmp/vaultime-deploy/"
"$ssh_cmd" "$host" "bash /tmp/vaultime-deploy/install-api.sh '$domain' && rm -rf /tmp/vaultime-deploy"
