-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Programs discovery has shown the player, so the look for new games at
-- start offers each one only once. It belongs to this PC: restores and
-- merges leave it alone.

CREATE TABLE offered_programs (
    -- The path as `path_key` compares it, lowercase on Windows.
    path_key TEXT PRIMARY KEY,
    offered_at TEXT NOT NULL DEFAULT (datetime('now'))
);
