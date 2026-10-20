#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Runs the cloud backup round trip of the desktop core against the real API.
# PostgreSQL and the API run in Docker, the desktop test runs on this machine.
#
#   bash scripts/cloud-e2e.sh
#
# Needs Docker, Python and the Rust toolchain. Everything it starts is removed
# again when it ends.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
repo_mount="$(cd "$repo" && { pwd -W 2>/dev/null || pwd; })"
network=vaultime-e2e
db=vaultime-e2e-db
api=vaultime-e2e-api
port=19005
# A second API with little storage, to see old backups make room.
small_api=vaultime-e2e-api-small
small_port=19006
# Its storage and largest backup, in bytes.
small_account_bytes=3145728
small_backup_bytes=4194304
# Its grace for a retried refresh, short so the test can wait it out.
small_grace_secs=2
# The port the API listens on inside its container, as in install-api.sh.
api_port=9005
# Random bytes in each test secret, as install-api.sh makes them.
secret_bytes=32
# How long the API gets to answer its health check, in seconds.
health_wait_secs=30
# Log lines of the API shown when something fails.
log_tail_lines=40
url="http://127.0.0.1:$port"
python=$(command -v python3 || command -v python)

cleanup() {
  docker rm -f "$api" "$small_api" "$db" >/dev/null 2>&1 || true
  docker network rm "$network" >/dev/null 2>&1 || true
}
trap cleanup EXIT
cleanup

echo "== building the API"
bash "$repo/scripts/build-api.sh"

echo "== starting PostgreSQL and the API"
docker network create "$network" >/dev/null
docker run -d --name "$db" --network "$network" \
  -e POSTGRES_USER=vaultime -e POSTGRES_PASSWORD=e2e -e POSTGRES_DB=vaultime \
  postgres:16 >/dev/null
# Over TCP, so the temporary server of the first start does not count.
until docker exec "$db" pg_isready -h 127.0.0.1 -U vaultime -q; do sleep 1; done

secret() { "$python" -c "import secrets; print(secrets.token_hex($secret_bytes))"; }

# Starts an API container on a host port and waits for its health check.
# Further arguments go to docker run. Every API takes backups without a pause
# between them and cleans up unused artwork at once.
start_api() {
  local name=$1 host_port=$2
  shift 2
  MSYS_NO_PATHCONV=1 docker run -d --name "$name" --network "$network" \
    -p "127.0.0.1:$host_port:$api_port" \
    --mount "type=bind,source=$repo_mount/dist-api,target=/app,readonly" \
    -e VAULTIME_API_BIND="0.0.0.0:$api_port" \
    -e VAULTIME_PUBLIC_BASE_URL="http://127.0.0.1:$host_port" \
    -e VAULTIME_DATABASE_URL="postgres://vaultime:e2e@$db:5432/vaultime" \
    -e VAULTIME_BACKUP_ROOT=/tmp/backups \
    -e VAULTIME_ACCESS_TOKEN_SECRET="$(secret)" \
    -e VAULTIME_REFRESH_TOKEN_PEPPER="$(secret)" \
    -e VAULTIME_MIN_BACKUP_INTERVAL_SECONDS=0 \
    -e VAULTIME_STALE_PENDING_BACKUP_SECONDS=0 \
    "$@" vaultime-linux-check /app/vaultime-api >/dev/null
  for _ in $(seq 1 "$health_wait_secs"); do
    curl -fsS "http://127.0.0.1:$host_port/healthz" >/dev/null 2>&1 && return 0
    sleep 1
  done
  docker logs "$name" 2>&1 | tail -"$log_tail_lines"
  echo "$name did not come up" >&2
  return 1
}

# Prints the code of a new single use invite, stored with the server's own
# invite tool.
make_invite() {
  "$python" - "$repo/deploy/vps/generate-cloud-invite.py" "$db" <<'EOF'
import importlib.util
import subprocess
import sys

spec = importlib.util.spec_from_file_location("invite", sys.argv[1])
invite = importlib.util.module_from_spec(spec)
spec.loader.exec_module(invite)
run = subprocess.run


def psql_in_container(command, *args, **kwargs):
    if command[0] == "psql":
        command = ["docker", "exec", "-i", sys.argv[2], "psql", "-U", "vaultime", "-d", "vaultime"] + command[2:]
    return run(command, *args, **kwargs)


invite.subprocess.run = psql_in_container
created = invite.generate_invite(invite.INVITE_PREFIX, 1, None, "e2e")
invite.insert_invite("", created)
print(created["code"])
EOF
}

# The round trip needs two kept backups. A reused refresh token ends its
# session at once here, the grace for a retry has its own check further down.
start_api "$api" "$port" -e VAULTIME_MAX_COMPLETE_BACKUPS_PER_ACCOUNT=2 \
  -e VAULTIME_REFRESH_REUSE_GRACE_SECONDS=0

echo "== creating a test account"
code=$(make_invite)
session=$(curl -fsS -X POST "$url/v1/auth/signup" -H "content-type: application/json" \
  -d "{\"email\":\"e2e@example.com\",\"password\":\"e2e-password-1234\",\"invite_code\":\"$code\"}")
field() { "$python" -c "import json, sys; print(json.load(sys.stdin)['$1'])"; }
token=$(field access_token <<<"$session")

echo "== refresh token reuse"
refresh() { curl -s -o /dev/null -w '%{http_code}' -X POST "$url/v1/auth/refresh" \
  -H "content-type: application/json" -d "{\"refresh_token\":\"$1\"}"; }
first=$(field refresh_token <<<"$session")
second=$(curl -fsS -X POST "$url/v1/auth/refresh" -H "content-type: application/json" \
  -d "{\"refresh_token\":\"$first\"}" | field refresh_token)
[ "$(refresh "$first")" = 401 ] || { echo "a used refresh token was taken again"; exit 1; }
[ "$(refresh "$second")" = 401 ] || { echo "reusing a token did not end its session"; exit 1; }
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$url/v1/auth/login" -H "content-type: application/json" \
  -d '{"email":"nobody@example.com","password":"wrong-password-1"}')" = 401 ] \
  || { echo "an unknown account did not get 401"; exit 1; }

echo "== running the desktop round trip"
# The Tauri build script needs the frontend folder to exist.
mkdir -p "$repo/apps/desktop/dist"
cd "$repo/apps/desktop/src-tauri"
if ! VAULTIME_E2E_API="$url" VAULTIME_E2E_TOKEN="$token" \
  cargo test --lib artwork_is_uploaded_once_and_restores -- --ignored --nocapture; then
  echo "== API log"
  docker logs "$api" 2>&1 | tail -"$log_tail_lines"
  exit 1
fi
echo "== two PCs merge through the cloud"
if ! VAULTIME_E2E_API="$url" VAULTIME_E2E_TOKEN="$token" \
  cargo test --lib two_pcs_merge_through_the_cloud -- --ignored --nocapture; then
  echo "== API log"
  docker logs "$api" 2>&1 | tail -"$log_tail_lines"
  exit 1
fi
register_keys() { curl -s -o /dev/null -w '%{http_code}' -X POST "$url/v1/ledger-keys" \
  -H "authorization: Bearer $token" -H "content-type: application/json" -d "$1"; }
[ "$(register_keys '{"ledger_device_id":"pc","public_keys":["not-a-key"]}')" = 400 ] \
  || { echo "a malformed ledger key was taken"; exit 1; }
[ "$(register_keys '{"ledger_device_id":"","public_keys":[]}')" = 400 ] \
  || { echo "ledger keys without a PC were taken"; exit 1; }
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

echo "== admin account"
"$python" - "$repo/deploy/vps/bootstrap-admin-account.py" "$db" "$url" <<'EOF'
import importlib.util
import subprocess
import sys

tool, container, url = sys.argv[1:]
spec = importlib.util.spec_from_file_location("bootstrap", tool)
bootstrap = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bootstrap)
run = subprocess.run


def psql_in_container(command, *args, **kwargs):
    if command[0] == "psql":
        command = ["docker", "exec", "-i", container, "psql", "-U", "vaultime", "-d", "vaultime"] + command[2:]
    return run(command, *args, **kwargs)


subprocess.run = psql_in_container
sys.argv = ["bootstrap", "--email", "admin@example.com", "--password", "e2e-admin-password",
            "--api-base-url", url, "--db-url", "postgres://vaultime@db/vaultime"]
sys.exit(bootstrap.main())
EOF
admin_token=$(curl -fsS -X POST "$url/v1/auth/login" -H "content-type: application/json" \
  -d '{"email":"admin@example.com","password":"e2e-admin-password"}' | field access_token)
[ "$(curl -s -o /dev/null -w '%{http_code}' -H "authorization: Bearer $admin_token" "$url/v1/admin/beta-applications")" = 200 ] \
  || { echo "the admin could not list applications"; exit 1; }
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "authorization: Bearer $admin_token" \
  -H "content-type: application/json" -d '{"note":"e2e"}' "$url/v1/admin/invites")" = 201 ] \
  || { echo "the admin could not create an invite"; exit 1; }

echo "== old backups make room"
small_url="http://127.0.0.1:$small_port"
start_api "$small_api" "$small_port" \
  -e VAULTIME_MAX_ACCOUNT_BYTES="$small_account_bytes" \
  -e VAULTIME_MAX_BACKUP_BYTES="$small_backup_bytes" \
  -e VAULTIME_MAX_COMPLETE_BACKUPS_PER_ACCOUNT=3 \
  -e VAULTIME_REFRESH_REUSE_GRACE_SECONDS="$small_grace_secs"
code=$(make_invite)
small_token=$(curl -fsS -X POST "$small_url/v1/auth/signup" -H "content-type: application/json" \
  -d "{\"email\":\"rotation@example.com\",\"password\":\"e2e-password-1234\",\"invite_code\":\"$code\"}" \
  | field access_token)
"$python" - "$small_url" "$small_token" "$small_account_bytes" <<'EOF'
import hashlib
import json
import os
import sys
import urllib.error
import urllib.request

url, token, account_bytes = sys.argv[1], sys.argv[2], int(sys.argv[3])
MIB = 1024 * 1024


def call(method, path, body=None, content_type="application/json"):
    request = urllib.request.Request(url + path, data=body, method=method)
    request.add_header("authorization", f"Bearer {token}")
    if body is not None:
        request.add_header("content-type", content_type)
    try:
        with urllib.request.urlopen(request) as response:
            return response.status, json.loads(response.read() or b"null")
    except urllib.error.HTTPError as error:
        return error.code, json.loads(error.read() or b"null")


def upload(size):
    content = os.urandom(size)
    checksum = hashlib.sha256(content).hexdigest()
    status, backup = call("POST", "/v1/backups", json.dumps(
        {"checksum": checksum, "backup_created_at": "2026-10-03T12:00:00Z"}).encode())
    assert status == 201, (status, backup)
    status, _ = call("PUT", f"/v1/backups/{backup['id']}/content", content, "application/octet-stream")
    return backup["id"], status


def stored():
    status, backups = call("GET", "/v1/backups")
    assert status == 200, status
    return [backup["id"] for backup in backups if backup["status"] == "complete"]


first, _ = upload(MIB + MIB // 4)
second, _ = upload(MIB + MIB // 4)
third, status = upload(MIB + MIB // 4)
assert status == 200, f"the third backup was turned away: {status}"
kept = stored()
assert first not in kept and second in kept and third in kept, f"the oldest backup did not make room: {kept}"
status, storage = call("GET", "/v1/storage")
assert storage["backup_bytes"] + storage["artwork_bytes"] <= account_bytes, storage

# At the count limit, so a backup turned away must not cost one by count
# either.
fourth, status = upload(MIB // 4)
assert status == 200, f"a small backup was turned away: {status}"
kept = stored()
assert len(kept) == 3, kept

# A backup that does not fit even alone goes, and the older ones stay.
_, status = upload(3 * MIB + MIB // 2)
assert status == 413, f"a backup larger than the storage was kept: {status}"
assert stored() == kept, f"older backups went for a backup that did not fit: {stored()}"
print("old backups made room")
EOF

echo "== a refresh retried right after it renewed"
session=$(curl -fsS -X POST "$small_url/v1/auth/login" -H "content-type: application/json" \
  -d '{"email":"rotation@example.com","password":"e2e-password-1234"}')
renew() { curl -s -w '\n%{http_code}' -X POST "$small_url/v1/auth/refresh" \
  -H "content-type: application/json" -d "{\"refresh_token\":\"$1\"}"; }
first=$(field refresh_token <<<"$session")
lost=$(renew "$first" | head -1 | field refresh_token)
retry=$(renew "$first")
[ "$(tail -1 <<<"$retry")" = 200 ] || { echo "a quick retry of a refresh was refused"; exit 1; }
again=$(head -1 <<<"$retry" | field refresh_token)
next=$(renew "$again")
[ "$(tail -1 <<<"$next")" = 200 ] || { echo "the retried session was ended"; exit 1; }
next=$(head -1 <<<"$next" | field refresh_token)
# The unused token of the first refresh coming back later is a copy in
# someone else's hands, so the whole session ends.
sleep $((small_grace_secs + 1))
[ "$(renew "$lost" | tail -1)" = 401 ] || { echo "the unused token of the first refresh still renews"; exit 1; }
[ "$(renew "$next" | tail -1)" = 401 ] || { echo "a copied token did not end its session"; exit 1; }

echo "== listing and removing devices"
device() { curl -fsS -X POST "$small_url/v1/devices/register" -H "authorization: Bearer $small_token" \
  -H "content-type: application/json" \
  -d "{\"client_device_id\":\"$1\",\"device_name\":\"PC\",\"platform\":\"windows\",\"app_version\":\"0.6.0\"}" | field id; }
keep=$(device "kept-pc")
gone=$(device "removed-pc")
count() { curl -fsS -H "authorization: Bearer $small_token" "$small_url/v1/devices" \
  | "$python" -c "import json, sys; print(len(json.load(sys.stdin)))"; }
[ "$(count)" = 2 ] || { echo "expected two devices"; exit 1; }
remove() { curl -s -o /dev/null -w '%{http_code}' -X DELETE -H "authorization: Bearer $small_token" "$small_url/v1/devices/$1"; }
[ "$(remove "$gone")" = 204 ] || { echo "a device was not removed"; exit 1; }
[ "$(remove "$gone")" = 404 ] || { echo "a removed device was removed again"; exit 1; }
[ "$(count)" = 1 ] || { echo "expected one device after the removal"; exit 1; }
[ "$keep" != "$gone" ] || { echo "two devices got one id"; exit 1; }

# Some failures, like a failed artwork cleanup, are only logged. The reused
# refresh token above is expected to log a warning, not an error.
if docker logs "$api" 2>&1 | grep "ERROR" || docker logs "$small_api" 2>&1 | grep "ERROR"; then
  echo "The API logged errors"
  exit 1
fi
echo "Cloud round trip passed"
