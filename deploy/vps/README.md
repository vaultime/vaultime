# Vaultime VPS Deploy Pack

This folder contains the minimum files needed to bootstrap the invite-only
self-hosted Vaultime cloud stack on your VPS.

## Files

- `Caddyfile` - reverse proxy for `api.codfishcloud.de`
- `api.env.example` - environment variables for the API service
- `vaultime-api.service` - example systemd unit for the API
- `self-hosted-postgres-schema.sql` - PostgreSQL schema for accounts, invites,
  devices, refresh tokens, and backup metadata
- `bootstrap-admin-account.py` - creates or promotes the first admin account
- `finalize-admin-setup.sh` - repairs DB ownership if needed, bootstraps admin,
  rebuilds the API, and restarts the service
- `redeploy-api.sh` - rebuilds the API, installs the binary, restarts the
  systemd service, and runs health checks
- `generate-cloud-invite.py` - generates and inserts invite codes directly

## Expected stack

- Caddy
- custom `vaultime-api` service on `127.0.0.1:9005`
- PostgreSQL on the VPS
- encrypted backup files stored under `/srv/vaultime/backups`
- source for the API lives in `apps/api` in this repo

## Suggested install paths

```text
/srv/vaultime/
  api/
    current/
  backups/
  tmp/
/etc/vaultime/
  api.env
/etc/caddy/
  Caddyfile
```

## Notes

- Build the API from `apps/api` and deploy the resulting `vaultime-api` binary
  to `/srv/vaultime/api/current/`.
- Backups should be encrypted on the client before upload.
- PostgreSQL should stay bound to localhost, not exposed publicly.
- Run `bootstrap-admin-account.py` once to promote
  `admin@vaultime.com` or another chosen account to the admin role.
- If your first schema import was done as `postgres`, run
  `finalize-admin-setup.sh` instead so table ownership is corrected before the
  admin bootstrap continues.
- Run `generate-cloud-invite.py` on the VPS when you need a shareable invite
  before the desktop admin UI exists.
- Run `redeploy-api.sh` after API changes so you do not need to repeat the
  manual build/install/restart steps.
