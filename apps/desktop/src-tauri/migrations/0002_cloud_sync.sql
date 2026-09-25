-- SPDX-FileCopyrightText: 2026 Vaultime Contributors
-- SPDX-License-Identifier: MIT
--
-- Add cloud sync tracking columns.

-- Track which session events have been uploaded.
ALTER TABLE session_events ADD COLUMN synced_at TEXT;

-- Track which sessions have been acknowledged by the server.
ALTER TABLE sessions ADD COLUMN cloud_verified INTEGER NOT NULL DEFAULT 0;

-- Store the cloud user ID locally for session attribution.
INSERT OR IGNORE INTO settings (key, value) VALUES ('cloud_user_id', '');
