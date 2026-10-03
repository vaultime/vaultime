-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Where each session's time fell, a quarter hour at a time, so stats can ask
-- for any day, week, month or year without reading every session. Worked
-- out from the session events and rebuilt whenever they change, so it is a
-- cache, never part of the history. Every UTC offset is a whole number of
-- quarter hours, so a quarter hour never spans a local midnight.

CREATE TABLE play_slices (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    -- Start of the quarter hour, milliseconds since 1970 in UTC.
    slice_start INTEGER NOT NULL,
    runtime_ms INTEGER NOT NULL,
    active_ms INTEGER NOT NULL,
    idle_ms INTEGER NOT NULL,
    PRIMARY KEY (session_id, slice_start)
) WITHOUT ROWID;

CREATE INDEX idx_play_slices_start ON play_slices (slice_start);
