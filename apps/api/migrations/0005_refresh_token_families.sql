-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: AGPL-3.0-or-later

-- Refresh tokens that replace each other share a family. A token that was
-- already replaced and comes back means someone else holds a copy, so the
-- whole family is revoked.
ALTER TABLE cloud_refresh_tokens ADD COLUMN family_id UUID;
UPDATE cloud_refresh_tokens SET family_id = id WHERE family_id IS NULL;
ALTER TABLE cloud_refresh_tokens ALTER COLUMN family_id SET NOT NULL;
CREATE INDEX cloud_refresh_tokens_family_id_idx ON cloud_refresh_tokens(family_id);
