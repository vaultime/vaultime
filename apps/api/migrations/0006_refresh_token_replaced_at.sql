-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: AGPL-3.0-or-later

-- When a refresh replaced the token. Only a replaced token that comes back
-- ends its family. One that signing out or a password change revoked is
-- only refused.
ALTER TABLE cloud_refresh_tokens ADD COLUMN replaced_at TIMESTAMPTZ;
