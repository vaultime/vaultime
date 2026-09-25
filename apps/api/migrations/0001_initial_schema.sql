CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS citext;

CREATE TABLE IF NOT EXISTS cloud_invites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lookup_key TEXT NOT NULL UNIQUE,
    salt TEXT NOT NULL,
    code_hash TEXT NOT NULL,
    max_redemptions INTEGER NOT NULL DEFAULT 1 CHECK (max_redemptions > 0),
    redeemed_count INTEGER NOT NULL DEFAULT 0 CHECK (redeemed_count >= 0),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS cloud_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email CITEXT NOT NULL UNIQUE,
    role TEXT NOT NULL DEFAULT 'user',
    access_state TEXT NOT NULL DEFAULT 'active',
    invited_by_invite_id UUID REFERENCES cloud_invites(id),
    access_granted_at TIMESTAMPTZ,
    access_revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMPTZ
);

ALTER TABLE cloud_accounts
    ADD COLUMN IF NOT EXISTS role TEXT NOT NULL DEFAULT 'user';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'cloud_accounts_role_check'
    ) THEN
        ALTER TABLE cloud_accounts
            ADD CONSTRAINT cloud_accounts_role_check
            CHECK (role IN ('user', 'admin'));
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS cloud_account_passwords (
    account_id UUID PRIMARY KEY REFERENCES cloud_accounts(id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL,
    password_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS cloud_refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES cloud_accounts(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    user_agent TEXT,
    last_used_ip INET,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS cloud_invite_redemptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invite_id UUID NOT NULL REFERENCES cloud_invites(id) ON DELETE CASCADE,
    account_id UUID NOT NULL REFERENCES cloud_accounts(id) ON DELETE CASCADE,
    redeemed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source_ip INET
);

CREATE TABLE IF NOT EXISTS cloud_devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES cloud_accounts(id) ON DELETE CASCADE,
    client_device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,
    app_version TEXT NOT NULL,
    device_public_key TEXT,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS cloud_backups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES cloud_accounts(id) ON DELETE CASCADE,
    device_id UUID REFERENCES cloud_devices(id) ON DELETE SET NULL,
    label TEXT,
    storage_key TEXT NOT NULL UNIQUE,
    checksum TEXT NOT NULL,
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    backup_created_at TIMESTAMPTZ NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status TEXT NOT NULL DEFAULT 'complete',
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb
);

ALTER TABLE cloud_backups
    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'complete';

ALTER TABLE cloud_backups
    ADD COLUMN IF NOT EXISTS metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE cloud_backups
SET metadata_json = '{}'::jsonb
WHERE metadata_json IS NULL;

ALTER TABLE cloud_backups
    ALTER COLUMN metadata_json SET DEFAULT '{}'::jsonb;

ALTER TABLE cloud_backups
    ALTER COLUMN metadata_json SET NOT NULL;

CREATE INDEX IF NOT EXISTS cloud_refresh_tokens_account_id_idx
    ON cloud_refresh_tokens(account_id);

CREATE INDEX IF NOT EXISTS cloud_invite_redemptions_invite_id_idx
    ON cloud_invite_redemptions(invite_id);

CREATE UNIQUE INDEX IF NOT EXISTS cloud_devices_account_id_client_device_id_idx
    ON cloud_devices(account_id, client_device_id);

CREATE INDEX IF NOT EXISTS cloud_devices_account_id_idx
    ON cloud_devices(account_id);

CREATE INDEX IF NOT EXISTS cloud_backups_account_id_uploaded_at_idx
    ON cloud_backups(account_id, uploaded_at DESC);
