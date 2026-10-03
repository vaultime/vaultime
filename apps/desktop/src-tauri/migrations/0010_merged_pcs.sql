-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Sessions of other PCs, merged from their backups. They keep the game and
-- the PC they were recorded with, as both are part of their hashed history.
-- A game that came along this way is never tracked here, and the player can
-- link it to a game of this PC so the two count as one.

-- When sessions of this PC were last merged here. Sessions of a merged PC
-- can only be corrected on that PC.
ALTER TABLE devices ADD COLUMN merged_at TEXT;

-- The PC a game came from with merged sessions. Empty for this PC's games.
ALTER TABLE games ADD COLUMN origin_device_id TEXT;

CREATE TABLE game_links (
    -- A game from another PC.
    game_id        TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    -- The game of this PC it counts as.
    linked_game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    linked_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_game_links_linked_game_id ON game_links(linked_game_id);
