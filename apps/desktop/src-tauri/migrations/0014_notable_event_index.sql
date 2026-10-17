-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- A game's page lists the events of its sessions without the checkpoints,
-- which are most of them. This index holds only the others, so the list no
-- longer reads every checkpoint.

CREATE INDEX idx_session_events_notable ON session_events(session_id, sequence)
    WHERE event_type != 'heartbeat';
