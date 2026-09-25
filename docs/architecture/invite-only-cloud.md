# Invite-Only Cloud Backup

## Goal

Replace public billing with manual access control for remote backup. The cloud
backup service should be private, invite-only, and hosted on `codfishcloud.de`.

## Non-negotiables

- Invite validation must happen on the server, not in the desktop app.
- The desktop app must stay fully usable offline without an account.
- Remote backup access should be revocable without changing local tracking.
- PostgreSQL should hold invite metadata, account access state, and backup
  manifests.
- Large encrypted backup blobs should stay outside PostgreSQL.

## Recommended stack

- API service: custom Rust or TypeScript service on your VPS
- Database: PostgreSQL
- Blob storage: filesystem or MinIO on the same VPS
- TLS / routing: Caddy or Traefik
- Backup encryption: client-side before upload

## Invite model

Each invite should be a one-time or limited-use capability token. Store only a
derived hash server-side, not the raw code.

Recommended flow:

1. Generate an invite code locally with `scripts/generate-invite-key.mjs`.
2. Store `lookup_key`, `salt`, `code_hash`, `max_redemptions`, and `expires_at`
   in PostgreSQL.
3. When a user enters the invite in the cloud signup flow, the server:
   - finds the invite row by `lookup_key`
   - recomputes the hash with the stored salt
   - checks expiry, revocation, and remaining redemptions
   - marks the invite as claimed or increments the redemption count
   - grants the account access to cloud backup
4. Admin accounts should be able to generate fresh invite codes server-side
   without storing raw codes in PostgreSQL.
5. Every backup endpoint checks account access server-side before allowing sync
   or upload.

## Suggested schema

```sql
CREATE TABLE cloud_invites (
    id UUID PRIMARY KEY,
    lookup_key TEXT NOT NULL UNIQUE,
    salt TEXT NOT NULL,
    code_hash TEXT NOT NULL,
    max_redemptions INTEGER NOT NULL DEFAULT 1,
    redeemed_count INTEGER NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE cloud_accounts (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    role TEXT NOT NULL DEFAULT 'user',
    access_state TEXT NOT NULL DEFAULT 'pending',
    invited_by_invite_id UUID REFERENCES cloud_invites(id),
    access_granted_at TIMESTAMPTZ,
    access_revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Why this is better than client-side invite checks

If the desktop app contains the invite secret or validates it locally, the
secret can be extracted from the binary and reused. That is not access control.
It is only UI friction. Real invite-only access has to be enforced by the
server that owns the backup API.

## Near-term rollout

1. Keep local backups as the default recovery path.
2. Build invite generation and PostgreSQL storage first.
3. Add a minimal account + invite-claim API on `codfishcloud.de`.
4. Add an admin-only invite generation endpoint for trusted operators.
5. Add encrypted backup upload and restore.
6. Add optional server-backed trust labels later, after backup reliability is
   proven.

For the concrete VPS layout, service list, and starter schema, see:

- [`self-hosted-vps-stack.md`](self-hosted-vps-stack.md)
- [`self-hosted-postgres-schema.sql`](self-hosted-postgres-schema.sql)
