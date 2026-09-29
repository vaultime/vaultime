-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Playtime a game had before Vaultime tracked it, read once from a launcher.
-- It stays apart from sessions, which remain the only tracked history. Both
-- the launcher's total and the runtime Vaultime had already tracked at the
-- import are kept, so the earlier playtime can always be worked out again.

CREATE TABLE IF NOT EXISTS earlier_playtime (
    game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    launcher_minutes INTEGER NOT NULL CHECK (launcher_minutes >= 0),
    tracked_before_ms INTEGER NOT NULL CHECK (tracked_before_ms >= 0),
    last_played_at TEXT,
    imported_at TEXT NOT NULL
);
