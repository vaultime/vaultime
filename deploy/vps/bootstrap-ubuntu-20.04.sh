#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

if [[ "${EUID}" -ne 0 ]]; then
  echo "Run this as root."
  exit 1
fi

REPO_ROOT="/root/vaultime"
API_DIR="$REPO_ROOT/apps/api"
DEPLOY_DIR="$REPO_ROOT/deploy/vps"

SERVICE_USER="vaultime"
SERVICE_GROUP="vaultime"

DB_NAME="vaultime"
DB_USER="vaultime"

PUBLIC_API_URL="https://api.codfishcloud.de"
API_BIND="127.0.0.1:9005"
BACKUP_ROOT="/srv/vaultime/backups"
API_ROOT="/srv/vaultime/api/current"

ENV_FILE="/etc/vaultime/api.env"
SECRETS_FILE="/root/vaultime-bootstrap-secrets.txt"

for file in \
  "$API_DIR/Cargo.toml" \
  "$API_DIR/Cargo.lock" \
  "$DEPLOY_DIR/self-hosted-postgres-schema.sql" \
  "$DEPLOY_DIR/vaultime-api.service"
do
  if [[ ! -f "$file" ]]; then
    echo "Missing required file: $file"
    exit 1
  fi
done

apt-get update
apt-get install -y \
  ca-certificates \
  curl \
  git \
  build-essential \
  pkg-config \
  openssl \
  gnupg \
  debian-keyring \
  debian-archive-keyring \
  apt-transport-https \
  postgresql \
  postgresql-contrib

if ! command -v caddy >/dev/null 2>&1; then
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
    | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
    > /etc/apt/sources.list.d/caddy-stable.list
  chmod o+r /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  chmod o+r /etc/apt/sources.list.d/caddy-stable.list
  apt-get update
  apt-get install -y caddy
fi

if [[ ! -x /root/.cargo/bin/cargo ]]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

source /root/.cargo/env
export PATH="/root/.cargo/bin:$PATH"

if ! id -u "$SERVICE_USER" >/dev/null 2>&1; then
  adduser --system --group --home /srv/vaultime --no-create-home "$SERVICE_USER"
fi

mkdir -p "$API_ROOT" "$BACKUP_ROOT" /srv/vaultime/tmp /etc/vaultime
chown -R "$SERVICE_USER:$SERVICE_GROUP" /srv/vaultime
chmod 700 "$BACKUP_ROOT"

# Avoid inherited cwd warnings when switching to the postgres user.
cd /tmp

systemctl enable --now postgresql

if [[ -f "$ENV_FILE" ]]; then
  DB_PASSWORD="$(sed -n "s#^VAULTIME_DATABASE_URL=postgres://$DB_USER:\\([^@]*\\)@127\\.0\\.0\\.1:5432/$DB_NAME\$#\\1#p" "$ENV_FILE")"
  ACCESS_TOKEN_SECRET="$(sed -n 's/^VAULTIME_ACCESS_TOKEN_SECRET=//p' "$ENV_FILE")"
  REFRESH_TOKEN_PEPPER="$(sed -n 's/^VAULTIME_REFRESH_TOKEN_PEPPER=//p' "$ENV_FILE")"
else
  DB_PASSWORD="$(openssl rand -hex 24)"
  ACCESS_TOKEN_SECRET="$(openssl rand -hex 32)"
  REFRESH_TOKEN_PEPPER="$(openssl rand -hex 32)"
fi

if [[ -z "${DB_PASSWORD:-}" || -z "${ACCESS_TOKEN_SECRET:-}" || -z "${REFRESH_TOKEN_PEPPER:-}" ]]; then
  echo "Failed to determine secrets."
  exit 1
fi

runuser -u postgres -- psql <<SQL
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '${DB_USER}') THEN
    CREATE ROLE ${DB_USER} LOGIN PASSWORD '${DB_PASSWORD}';
  ELSE
    ALTER ROLE ${DB_USER} WITH LOGIN PASSWORD '${DB_PASSWORD}';
  END IF;
END
\$\$;
SQL

if ! runuser -u postgres -- psql -tAc "SELECT 1 FROM pg_database WHERE datname='${DB_NAME}'" | grep -q 1; then
  runuser -u postgres -- psql -c "CREATE DATABASE ${DB_NAME} OWNER ${DB_USER};"
fi

runuser -u postgres -- psql -c "ALTER DATABASE ${DB_NAME} OWNER TO ${DB_USER};"

if [[ -z "$(runuser -u postgres -- psql -d "$DB_NAME" -tAc "SELECT to_regclass('public.cloud_accounts')")" ]]; then
  psql "postgres://${DB_USER}:${DB_PASSWORD}@127.0.0.1:5432/${DB_NAME}" \
    -v ON_ERROR_STOP=1 \
    < "$DEPLOY_DIR/self-hosted-postgres-schema.sql"
fi

cat > "$ENV_FILE" <<ENV
VAULTIME_API_BIND=$API_BIND
VAULTIME_PUBLIC_BASE_URL=$PUBLIC_API_URL
VAULTIME_DATABASE_URL=postgres://$DB_USER:$DB_PASSWORD@127.0.0.1:5432/$DB_NAME
VAULTIME_BACKUP_ROOT=$BACKUP_ROOT
VAULTIME_ACCESS_TOKEN_SECRET=$ACCESS_TOKEN_SECRET
VAULTIME_REFRESH_TOKEN_PEPPER=$REFRESH_TOKEN_PEPPER
ENV

chmod 600 "$ENV_FILE"

cat > "$SECRETS_FILE" <<SECRETS
DB_NAME=$DB_NAME
DB_USER=$DB_USER
DB_PASSWORD=$DB_PASSWORD
VAULTIME_ACCESS_TOKEN_SECRET=$ACCESS_TOKEN_SECRET
VAULTIME_REFRESH_TOKEN_PEPPER=$REFRESH_TOKEN_PEPPER
SECRETS

chmod 600 "$SECRETS_FILE"

cd "$API_DIR"
cargo build --release --locked

install -m 0755 target/release/vaultime-api "$API_ROOT/vaultime-api"
chown "$SERVICE_USER:$SERVICE_GROUP" "$API_ROOT/vaultime-api"

install -m 0644 "$DEPLOY_DIR/vaultime-api.service" /etc/systemd/system/vaultime-api.service
systemctl daemon-reload
systemctl enable --now vaultime-api

touch /etc/caddy/Caddyfile
cp -a /etc/caddy/Caddyfile "/etc/caddy/Caddyfile.bak.$(date +%Y%m%d%H%M%S)"

if ! grep -q 'api.codfishcloud.de' /etc/caddy/Caddyfile; then
  cat >> /etc/caddy/Caddyfile <<'CADDY'

api.codfishcloud.de {
    encode zstd gzip
    reverse_proxy 127.0.0.1:9005
}
CADDY
fi

caddy validate --config /etc/caddy/Caddyfile
systemctl enable --now caddy
systemctl reload caddy

sleep 2

echo "Local health:"
curl --fail --silent --show-error http://127.0.0.1:9005/healthz
echo
echo

echo "Public health:"
if curl --fail --silent --show-error https://api.codfishcloud.de/healthz; then
  echo
else
  echo
  echo "Public health check failed. Check:"
  echo "journalctl -u caddy -n 100 --no-pager"
fi

echo
echo "Done."
echo "Secrets saved to: $SECRETS_FILE"
echo "API service status:"
systemctl --no-pager --full status vaultime-api || true
