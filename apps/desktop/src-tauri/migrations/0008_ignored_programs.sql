-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Programs the player said are no game. Discovery never offers them again
-- and the tracker never counts them, also inside a game's folder. Hiding a
-- game is something else: a hidden game is still tracked.

CREATE TABLE ignored_programs (
    -- The path as `path_key` compares it, lowercase on Windows.
    path_key TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    ignored_at TEXT NOT NULL DEFAULT (datetime('now'))
);
