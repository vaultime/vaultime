-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: AGPL-3.0-or-later

-- The public keys each PC's ledger signs with, as the app registers them
-- with its uploads. Rows are only added, never changed, so a key a backup
-- newly claims for a PC stands out against the keys that PC registered when
-- another PC merges it.
CREATE TABLE IF NOT EXISTS cloud_ledger_keys (
    account_id UUID NOT NULL REFERENCES cloud_accounts(id) ON DELETE CASCADE,
    ledger_device_id TEXT NOT NULL,
    public_key TEXT NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, ledger_device_id, public_key)
);
