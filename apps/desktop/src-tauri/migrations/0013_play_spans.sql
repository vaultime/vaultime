-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- The stretches of each session's time on the clock, each all active or all
-- idle, that the play slices are spread from. Totals that count clock time
-- once, for games side by side, and the parts of a session on each day come
-- from them. A cache like the slices, rebuilt with them.

CREATE TABLE play_spans (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    -- Milliseconds since 1970 in UTC. A stretch without length holds time
    -- whose clock went backwards.
    span_start INTEGER NOT NULL,
    span_end INTEGER NOT NULL,
    runtime_ms INTEGER NOT NULL,
    active_ms INTEGER NOT NULL,
    idle_ms INTEGER NOT NULL
);

CREATE INDEX idx_play_spans_session ON play_spans (session_id);
CREATE INDEX idx_play_spans_end ON play_spans (span_end, span_start);
