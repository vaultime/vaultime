-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Programs from the game folders that ran in front without being a game of
-- the library, which Discover offers to add. Nothing outside those folders
-- is ever noted. It belongs to this PC: restores and merges leave it alone.

CREATE TABLE seen_programs (
    -- The path as `path_key` compares it, lowercase on Windows.
    path_key TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    -- Seconds it had the foreground, over every run of Vaultime.
    front_seconds INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen_at TEXT NOT NULL DEFAULT (datetime('now'))
);
