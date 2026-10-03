-- SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Each PC keeps a ledger: a hash chain of the moments that shape its
-- sessions, signed with a key that stays on the PC. An entry pins a
-- session's event chain at one event, notes a session the player removed or
-- notes that a backup replaced the history. Entries never change, and a
-- removed session keeps its entries. PCs also get a name.

CREATE TABLE ledger_entries (
    -- The recording PC's public key in hex, which names its ledger.
    key_id       TEXT NOT NULL,
    sequence     INTEGER NOT NULL,
    entry_type   TEXT NOT NULL,
    -- No reference, so a removed session keeps its entries.
    session_id   TEXT,
    recorded_at  TEXT NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}',
    hash_prev    TEXT,
    hash_self    TEXT NOT NULL,
    signature    TEXT NOT NULL,
    PRIMARY KEY (key_id, sequence)
);

CREATE INDEX idx_ledger_entries_session_id ON ledger_entries(session_id);

ALTER TABLE devices ADD COLUMN name TEXT;
