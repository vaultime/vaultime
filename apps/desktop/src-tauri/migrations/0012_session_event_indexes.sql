-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Sessions added by hand and corrections are rare among the events, so each
-- gets an index of its own, and finding them no longer reads every
-- checkpoint. The plain index on session ids goes, as the unique index on
-- session and sequence serves every lookup it served.

DROP INDEX idx_session_events_session_id;

CREATE INDEX idx_session_events_added ON session_events(session_id)
    WHERE event_type = 'added_manually';

CREATE INDEX idx_session_events_corrected ON session_events(session_id, sequence)
    WHERE event_type = 'corrected';
