-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- The live bar, the tray and the journal look for the sessions still running
-- or ended after a moment, and without an index on the end each look read
-- every session. The few running sessions of a PC get an index of their own.

CREATE INDEX idx_sessions_ended_at ON sessions(ended_at_wall);
CREATE INDEX idx_sessions_running ON sessions(device_id) WHERE ended_at_wall IS NULL;
