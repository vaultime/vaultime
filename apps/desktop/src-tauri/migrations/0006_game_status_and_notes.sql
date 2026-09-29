-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Where a game stands for the player, kept as a history of changes so the
-- journal can tell when a game was finished, and short notes on sessions.
-- Both only annotate: they never change tracked time or session events.

CREATE TABLE IF NOT EXISTS game_status_changes (
    id TEXT PRIMARY KEY,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('backlog', 'playing', 'finished', 'dropped', 'none')),
    changed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS game_status_changes_game_idx
    ON game_status_changes(game_id, changed_at);

CREATE TABLE IF NOT EXISTS session_notes (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    note TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
