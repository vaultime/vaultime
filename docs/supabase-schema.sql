-- SPDX-FileCopyrightText: 2026 Vaultime Contributors
-- SPDX-License-Identifier: MIT
--
-- Supabase cloud schema for Vaultime.
-- Run this in the Supabase SQL editor to set up the required tables.
-- Row Level Security (RLS) policies ensure users can only access their own data.

-- ---------------------------------------------------------------------------
-- cloud_devices — registered devices per user
-- ---------------------------------------------------------------------------

CREATE TABLE cloud_devices (
    id          TEXT PRIMARY KEY,
    user_id     UUID NOT NULL DEFAULT auth.uid() REFERENCES auth.users(id) ON DELETE CASCADE,
    device_name TEXT NOT NULL,
    platform    TEXT NOT NULL,
    app_version TEXT NOT NULL,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_sync_at  TIMESTAMPTZ
);

ALTER TABLE cloud_devices ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can manage their own devices"
    ON cloud_devices
    FOR ALL
    USING (user_id = auth.uid())
    WITH CHECK (user_id = auth.uid());

-- ---------------------------------------------------------------------------
-- cloud_sync_events — session events uploaded from devices (Milestone 11)
-- ---------------------------------------------------------------------------

CREATE TABLE cloud_sync_events (
    id              TEXT PRIMARY KEY,
    user_id         UUID NOT NULL DEFAULT auth.uid() REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id       TEXT NOT NULL REFERENCES cloud_devices(id),
    session_id      TEXT NOT NULL,
    sequence        INTEGER NOT NULL,
    event_type      TEXT NOT NULL,
    event_time_wall TIMESTAMPTZ NOT NULL,
    payload_json    JSONB NOT NULL DEFAULT '{}',
    hash_self       TEXT,
    server_received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(session_id, sequence)
);

ALTER TABLE cloud_sync_events ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can manage their own sync events"
    ON cloud_sync_events
    FOR ALL
    USING (user_id = auth.uid())
    WITH CHECK (user_id = auth.uid());

CREATE INDEX idx_sync_events_session ON cloud_sync_events(session_id, sequence);
CREATE INDEX idx_sync_events_device ON cloud_sync_events(device_id);

-- ---------------------------------------------------------------------------
-- cloud_backups — encrypted backup metadata (Milestone 11)
-- ---------------------------------------------------------------------------

CREATE TABLE cloud_backups (
    id              TEXT PRIMARY KEY,
    user_id         UUID NOT NULL DEFAULT auth.uid() REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id       TEXT NOT NULL REFERENCES cloud_devices(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    checksum        TEXT NOT NULL,
    storage_path    TEXT NOT NULL,
    size_bytes      BIGINT,
    label           TEXT
);

ALTER TABLE cloud_backups ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can manage their own backups"
    ON cloud_backups
    FOR ALL
    USING (user_id = auth.uid())
    WITH CHECK (user_id = auth.uid());
