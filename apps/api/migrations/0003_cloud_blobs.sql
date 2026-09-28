-- Artwork is stored once per account as encrypted blobs, and backups refer
-- to the blobs they need. Blob ids are keyed hashes the client computes, so
-- the server cannot tell what an image shows.
CREATE TABLE IF NOT EXISTS cloud_blobs (
    account_id UUID NOT NULL REFERENCES cloud_accounts(id) ON DELETE CASCADE,
    id TEXT NOT NULL,
    storage_key TEXT NOT NULL UNIQUE,
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, id)
);

-- A blob cannot be deleted while a backup, even an unfinished one, refers to it.
CREATE TABLE IF NOT EXISTS cloud_backup_blobs (
    backup_id UUID NOT NULL REFERENCES cloud_backups(id) ON DELETE CASCADE,
    account_id UUID NOT NULL,
    blob_id TEXT NOT NULL,
    PRIMARY KEY (backup_id, blob_id),
    FOREIGN KEY (account_id, blob_id) REFERENCES cloud_blobs(account_id, id)
);

CREATE INDEX IF NOT EXISTS cloud_backup_blobs_account_blob_idx
    ON cloud_backup_blobs(account_id, blob_id);
