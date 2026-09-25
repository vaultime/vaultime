-- SPDX-FileCopyrightText: 2026 Vaultime Contributors
-- SPDX-License-Identifier: MIT
--
-- Initial Vaultime schema: games, assets, sessions, events, devices, settings.

CREATE TABLE devices (
    id          TEXT PRIMARY KEY,
    platform    TEXT NOT NULL,
    app_version TEXT NOT NULL,
    key_id      TEXT,
    registered_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE games (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    executable_path TEXT,
    install_folder  TEXT,
    launcher_source TEXT,
    metadata_json   TEXT NOT NULL DEFAULT '{}',
    is_hidden       INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE game_assets (
    id          TEXT PRIMARY KEY,
    game_id     TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    asset_type  TEXT NOT NULL,   -- cover, banner, icon, screenshot
    source      TEXT NOT NULL,   -- local_import, extracted_icon, user_picked
    file_path   TEXT NOT NULL,
    cache_path  TEXT,
    hash        TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_game_assets_game_id ON game_assets(game_id);

CREATE TABLE sessions (
    id                  TEXT PRIMARY KEY,
    game_id             TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    device_id           TEXT NOT NULL REFERENCES devices(id),
    started_at_wall     TEXT NOT NULL,
    ended_at_wall       TEXT,
    elapsed_monotonic_ms INTEGER NOT NULL DEFAULT 0,
    active_ms           INTEGER NOT NULL DEFAULT 0,
    idle_ms             INTEGER NOT NULL DEFAULT 0,
    runtime_ms          INTEGER NOT NULL DEFAULT 0,
    integrity_status    TEXT NOT NULL DEFAULT 'local',
    closed_cleanly      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_sessions_game_id ON sessions(game_id);
CREATE INDEX idx_sessions_device_id ON sessions(device_id);
CREATE INDEX idx_sessions_started_at ON sessions(started_at_wall);

CREATE TABLE session_events (
    id                   TEXT PRIMARY KEY,
    session_id           TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    sequence             INTEGER NOT NULL,
    event_type           TEXT NOT NULL,
    event_time_wall      TEXT NOT NULL,
    event_time_monotonic INTEGER,
    payload_json         TEXT NOT NULL DEFAULT '{}',
    hash_prev            TEXT,
    hash_self            TEXT,
    signature            TEXT,
    UNIQUE(session_id, sequence)
);

CREATE INDEX idx_session_events_session_id ON session_events(session_id);

CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE backup_snapshots (
    id                TEXT PRIMARY KEY,
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    source_device_id  TEXT REFERENCES devices(id),
    checksum          TEXT NOT NULL,
    remote_path       TEXT,
    restore_point_label TEXT
);

-- Seed default settings
INSERT INTO settings (key, value) VALUES
    ('idle_threshold_seconds', '300'),
    ('treat_background_as_active', 'false'),
    ('theme', 'dark'),
    ('preferred_image_mode', 'cover');
