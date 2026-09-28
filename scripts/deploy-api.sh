#!/usr/bin/env bash
# Builds the API for Ubuntu 24.04 in Docker and installs it on the server.
#
#   VAULTIME_DEPLOY_HOST=root@server scripts/deploy-api.sh [domain]
#
# domain defaults to vaultime.codfishcloud.de. On Windows, set SSH and SCP to the
# Windows OpenSSH binaries when the key lives in the Windows ssh-agent.
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

tools=(install-api.sh vaultime-api.service vaultime-db-backup.service vaultime-db-backup.timer
  bootstrap-admin-account.py generate-cloud-invite.py)
for file in "${tools[@]}"; do cp "$repo/deploy/vps/$file" "$stage/"; done

"$ssh_cmd" "$host" 'rm -rf /tmp/vaultime-deploy && mkdir -p /tmp/vaultime-deploy'
# Windows OpenSSH wants Windows paths.
local_path() { cygpath -w "$1" 2>/dev/null || echo "$1"; }
files=("$(local_path "$stage/vaultime-api")")
for file in "${tools[@]}"; do files+=("$(local_path "$stage/$file")"); done
"$scp_cmd" -q "${files[@]}" "$host:/tmp/vaultime-deploy/"
"$ssh_cmd" "$host" "bash /tmp/vaultime-deploy/install-api.sh '$domain' && rm -rf /tmp/vaultime-deploy"
