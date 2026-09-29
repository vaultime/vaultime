-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Track cloud acknowledgement timestamps and sync status details.

ALTER TABLE session_events ADD COLUMN server_ack_at TEXT;
ALTER TABLE sessions ADD COLUMN cloud_verified_at TEXT;

INSERT OR IGNORE INTO settings (key, value) VALUES
    ('cloud_last_sync_at', ''),
    ('cloud_last_backup_at', ''),
    ('cloud_last_backup_id', '');
