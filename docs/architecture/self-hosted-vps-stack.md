# Self-Hosted VPS Stack

This is the recommended minimum system for running invite-only Vaultime cloud
accounts and remote backups on your own VPS.

## Recommended topology

- `api.codfishcloud.de`
  Reverse proxy entrypoint with TLS termination.
- `vaultime-api`
  Your custom HTTP API for account auth, invite claims, device registration,
  backup metadata, upload authorization, and restore authorization.
- `PostgreSQL`
  The only required database. Store account, invite, device, refresh-token, and
  backup metadata here.
- Filesystem-backed blob storage
  Store encrypted backup archives on disk first. Do not store large backup
  payloads in PostgreSQL.

## What to run on the VPS

### 1. Reverse proxy

Use Caddy in front of the API.

Why:
- automatic HTTPS
- simple reverse-proxy config
- easy systemd deployment

Minimal shape:

```caddyfile
api.codfishcloud.de {
    encode zstd gzip
    reverse_proxy 127.0.0.1:9005
}
```

### 2. API service

Use the Rust + Axum service in `apps/api`.

The API should own:
- invite claim
- email/password signup and login
- access-token issuance
- refresh-token rotation
- device registration
- backup listing
- upload authorization
- restore authorization

Do not put invite validation in the desktop app. The server must decide whether
an account is allowed to use remote backup.

### 3. PostgreSQL

Use PostgreSQL for:
- accounts
- password hashes
- account roles
- refresh tokens
- invite records
- device records
- backup manifests
- restore audit rows if you want them later

Use the starter schema in
[`self-hosted-postgres-schema.sql`](self-hosted-postgres-schema.sql).

### 4. Blob storage

Start with filesystem-backed storage on the same VPS:

- root path: `/srv/vaultime/backups`
- per-account prefixes:
  `/srv/vaultime/backups/<account-id>/<backup-id>.tar.zst.enc`

This is enough for an invite-only beta. Add MinIO later only if you need:
- S3-compatible clients
- bucket lifecycle policies
- object versioning
- multi-node expansion

## Directory layout

Suggested filesystem layout:

```text
/srv/vaultime/
  api/
    current/
  backups/
  tmp/
/etc/vaultime/
  api.env
/var/log/vaultime/
```

Suggested environment file:

```env
VAULTIME_API_BIND=127.0.0.1:9005
VAULTIME_PUBLIC_BASE_URL=https://api.codfishcloud.de
VAULTIME_DATABASE_URL=postgres://vaultime:...@127.0.0.1:5432/vaultime
VAULTIME_BACKUP_ROOT=/srv/vaultime/backups
VAULTIME_ACCESS_TOKEN_SECRET=replace-me
VAULTIME_REFRESH_TOKEN_PEPPER=replace-me
```

## Account model

### Authentication

Use email + password with:
- Argon2id password hashing
- short-lived access tokens
- rotated refresh tokens stored as hashes in PostgreSQL
- a simple `role` field such as `user` or `admin`

Recommended token shape:
- access token: 10-15 minutes
- refresh token: 30-60 days, rotated on each refresh

### Invite flow

1. Generate invite material with `scripts/generate-invite-key.mjs`.
2. Insert `lookup_key`, `salt`, `code_hash`, `max_redemptions`, and
   `expires_at` into PostgreSQL.
3. During signup, the client sends email, password, and raw invite code.
4. The server:
   - extracts the lookup key
   - finds the invite row
   - recomputes the hash
   - checks expiry and revocation
   - records the redemption
   - creates the account with remote-backup access

### Device model

Register a device after login and keep:
- account id
- client device id from the desktop app
- device name
- platform
- app version
- optional public key later if you add signed sync events again

## Backup model

### What the desktop app should upload

The client should upload:
- a compressed archive of the local backup export
- a manifest or metadata JSON
- checksum and size metadata

### Encryption model

Encrypt backup archives on the client before upload.

Server stores:
- encrypted file
- file size
- checksum
- created timestamp
- source client device id

This keeps the VPS from holding readable backup contents at rest.

### Upload flow

Recommended minimal flow:

1. Client creates a local backup archive.
2. Client encrypts the archive.
3. Client requests an upload slot from the API.
4. API creates a backup row in PostgreSQL and returns a storage key.
5. Client uploads the encrypted payload to the API.
6. API streams the payload to disk and finalizes the row.

### Restore flow

1. Client requests available backups.
2. Client asks for restore preview metadata.
3. API returns metadata only.
4. Client confirms restore.
5. API streams the encrypted archive back.
6. Client decrypts and restores locally.

## Minimal API surface

Recommended first endpoints:

- `POST /v1/auth/signup`
- `POST /v1/auth/login`
- `POST /v1/auth/refresh`
- `POST /v1/auth/logout`
- `POST /v1/admin/invites`
- `POST /v1/devices/register`
- `GET /v1/backups`
- `POST /v1/backups`
- `GET /v1/backups/:id`
- `PUT /v1/backups/:id/content`
- `GET /v1/backups/:id/download`

Keep sync-event verification out of v1 if backup is the main goal. Re-add it
later only after account auth and backup restore are reliable.

## Operations checklist

### Firewall

Open only:
- `22/tcp`
- `80/tcp`
- `443/tcp`

Keep PostgreSQL bound to localhost.

### systemd

Run the API as a dedicated service:

```ini
[Unit]
Description=Vaultime API
After=network-online.target postgresql.service
Wants=network-online.target

[Service]
User=vaultime
Group=vaultime
WorkingDirectory=/srv/vaultime/api/current
EnvironmentFile=/etc/vaultime/api.env
ExecStart=/srv/vaultime/api/current/vaultime-api
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### Backups of the backup service

Your cloud backup VPS must itself be backed up. At minimum:
- daily PostgreSQL dump
- filesystem snapshot or `restic` backup of `/srv/vaultime/backups`
- off-VPS copy to a second location

### Monitoring

Track:
- free disk space
- PostgreSQL health
- API error rate
- backup upload failures
- TLS certificate renewals

## Suggested rollout order

1. Provision PostgreSQL and Caddy.
2. Deploy the API with signup, login, refresh, and invite claim.
3. Implement device registration.
4. Add encrypted backup upload and listing.
5. Add restore preview and download.
6. Add quotas, retention rules, and audit logging.
7. Only then consider server-backed verification for trust labels.
