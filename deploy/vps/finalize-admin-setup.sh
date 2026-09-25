#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "Run this as root."
  exit 1
fi

REPO_ROOT="/root/vaultime"
API_DIR="$REPO_ROOT/apps/api"
API_BINARY="/srv/vaultime/api/current/vaultime-api"

python3 "$REPO_ROOT/deploy/bootstrap-admin-account.py" "$@"

source /root/.cargo/env
cd "$API_DIR"
cargo build --release --locked

install -m 0755 target/release/vaultime-api "$API_BINARY"
chown vaultime:vaultime "$API_BINARY"
systemctl restart vaultime-api

echo
echo "Admin setup complete."
echo "Verify:"
echo "  curl https://api.codfishcloud.de/healthz"
echo "  systemctl status vaultime-api --no-pager"
