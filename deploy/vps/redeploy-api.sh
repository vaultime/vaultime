#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "Run this as root."
  exit 1
fi

REPO_ROOT="${REPO_ROOT:-/root/vaultime}"
API_DIR="${API_DIR:-$REPO_ROOT/apps/api}"
API_BINARY="${API_BINARY:-/srv/vaultime/api/current/vaultime-api}"
SERVICE_NAME="${SERVICE_NAME:-vaultime-api}"
LOCAL_HEALTH_URL="${LOCAL_HEALTH_URL:-http://127.0.0.1:9005/healthz}"
PUBLIC_HEALTH_URL="${PUBLIC_HEALTH_URL:-https://api.codfishcloud.de/healthz}"

if [[ ! -d "$API_DIR" ]]; then
  echo "API source directory not found: $API_DIR"
  exit 1
fi

if [[ ! -f "$API_DIR/Cargo.toml" ]]; then
  echo "Missing Cargo.toml in $API_DIR"
  exit 1
fi

if [[ ! -f /root/.cargo/env ]]; then
  echo "Rust environment file not found at /root/.cargo/env"
  exit 1
fi

source /root/.cargo/env

echo "Building Vaultime API from $API_DIR"
cd "$API_DIR"
cargo build --release --locked

echo "Installing binary to $API_BINARY"
install -m 0755 target/release/vaultime-api "$API_BINARY"
chown vaultime:vaultime "$API_BINARY"

echo "Restarting $SERVICE_NAME"
systemctl restart "$SERVICE_NAME"
systemctl --no-pager --full status "$SERVICE_NAME"

echo
echo "Local health check:"
curl --fail --silent --show-error "$LOCAL_HEALTH_URL"
echo
echo

echo "Public health check:"
curl --fail --silent --show-error "$PUBLIC_HEALTH_URL"
echo
echo

echo "Vaultime API redeploy complete."
