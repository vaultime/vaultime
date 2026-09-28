#!/usr/bin/env bash
# Runs the cloud backup round trip of the desktop core against the real API.
# PostgreSQL and the API run in Docker, the desktop test runs on this machine.
#
#   bash scripts/cloud-e2e.sh
#
# Needs Docker, Python and the Rust toolchain. Everything it starts is removed
# again when it ends.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
repo_mount="$(cd "$repo" && pwd -W 2>/dev/null || pwd)"
network=vaultime-e2e
db=vaultime-e2e-db
api=vaultime-e2e-api
port=19005
url="http://127.0.0.1:$port"
python=$(command -v python3 || command -v python)

cleanup() {
  docker rm -f "$api" "$db" >/dev/null 2>&1 || true
  docker network rm "$network" >/dev/null 2>&1 || true
}
trap cleanup EXIT
cleanup

echo "== building the API"
mkdir -p "$repo/dist-api"
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

echo "== starting PostgreSQL and the API"
docker network create "$network" >/dev/null
docker run -d --name "$db" --network "$network" \
  -e POSTGRES_USER=vaultime -e POSTGRES_PASSWORD=e2e -e POSTGRES_DB=vaultime \
  postgres:16 >/dev/null
# Over TCP, so the temporary server of the first start does not count.
until docker exec "$db" pg_isready -h 127.0.0.1 -U vaultime -q; do sleep 1; done

secret() { "$python" -c "import secrets; print(secrets.token_hex(32))"; }
# The test needs two kept backups, no pause between backups and cleanup of
# unused artwork at once.
MSYS_NO_PATHCONV=1 docker run -d --name "$api" --network "$network" \
  -p "127.0.0.1:$port:9005" \
  --mount "type=bind,source=$repo_mount/dist-api,target=/app,readonly" \
  -e VAULTIME_API_BIND=0.0.0.0:9005 \
  -e VAULTIME_PUBLIC_BASE_URL="$url" \
  -e VAULTIME_DATABASE_URL="postgres://vaultime:e2e@$db:5432/vaultime" \
  -e VAULTIME_BACKUP_ROOT=/tmp/backups \
  -e VAULTIME_ACCESS_TOKEN_SECRET="$(secret)" \
  -e VAULTIME_REFRESH_TOKEN_PEPPER="$(secret)" \
  -e VAULTIME_MAX_COMPLETE_BACKUPS_PER_ACCOUNT=2 \
  -e VAULTIME_MIN_BACKUP_INTERVAL_SECONDS=0 \
  -e VAULTIME_STALE_PENDING_BACKUP_SECONDS=0 \
  vaultime-linux-check /app/vaultime-api >/dev/null
for _ in $(seq 1 30); do
  curl -fsS "$url/healthz" >/dev/null 2>&1 && break
  sleep 1
done
curl -fsS "$url/healthz" >/dev/null || { docker logs "$api" | tail -30; exit 1; }

echo "== creating a test account"
code=$("$python" - "$repo/deploy/vps/generate-cloud-invite.py" "$db" <<'EOF'
import importlib.util
import subprocess
import sys

spec = importlib.util.spec_from_file_location("invite", sys.argv[1])
invite = importlib.util.module_from_spec(spec)
spec.loader.exec_module(invite)
run = subprocess.run


def psql_in_container(command, *args, **kwargs):
    if command[0] == "psql":
        command = ["docker", "exec", sys.argv[2], "psql", "-U", "vaultime", "-d", "vaultime"] + command[2:]
    return run(command, *args, **kwargs)


invite.subprocess.run = psql_in_container
created = invite.generate_invite("VTLINV", 1, None, "e2e")
invite.insert_invite("", created)
print(created["code"])
EOF
)
token=$(curl -fsS -X POST "$url/v1/auth/signup" -H "content-type: application/json" \
  -d "{\"email\":\"e2e@example.com\",\"password\":\"e2e-password-1234\",\"invite_code\":\"$code\"}" \
  | "$python" -c "import json, sys; print(json.load(sys.stdin)['access_token'])")

echo "== running the desktop round trip"
cd "$repo/apps/desktop/src-tauri"
if ! VAULTIME_E2E_API="$url" VAULTIME_E2E_TOKEN="$token" \
  cargo test --lib artwork_is_uploaded_once_and_restores -- --ignored --nocapture; then
  echo "== API log"
  docker logs "$api" 2>&1 | tail -40
  exit 1
fi
echo "== beta application form"
apply() { curl -s -o /dev/null -w '%{redirect_url}' -X POST "$url/v1/beta/apply" "$@"; }
[[ $(apply -d email=tester@example.com -d platform=linux -d consent=yes -d note=e2e) == */applied.html ]] \
  || { echo "a valid application was not taken"; exit 1; }
[[ $(apply -d email=tester2@example.com -d platform=linux) == */apply-failed.html ]] \
  || { echo "an application without consent was taken"; exit 1; }
stored=$(docker exec "$db" psql -U vaultime -d vaultime -tAc "SELECT COUNT(*) FROM beta_applications")
[ "$stored" = 1 ] || { echo "expected one stored application, found $stored"; exit 1; }
[ "$(curl -s -o /dev/null -w '%{http_code}' -H "authorization: Bearer $token" "$url/v1/admin/beta-applications")" = 403 ] \
  || { echo "a normal account could list applications"; exit 1; }

# Some failures, like a failed artwork cleanup, are only logged.
if docker logs "$api" 2>&1 | grep "ERROR"; then
  echo "The API logged errors"
  exit 1
fi
echo "Cloud round trip passed"
