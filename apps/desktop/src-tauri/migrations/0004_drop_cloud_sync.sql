-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Remove the columns and settings of the retired event sync.

ALTER TABLE session_events DROP COLUMN synced_at;
ALTER TABLE session_events DROP COLUMN server_ack_at;
ALTER TABLE sessions DROP COLUMN cloud_verified;
ALTER TABLE sessions DROP COLUMN cloud_verified_at;

DELETE FROM settings WHERE key IN (
    'cloud_user_id',
    'cloud_last_sync_at',
    'cloud_last_backup_at',
    'cloud_last_backup_id'
);
