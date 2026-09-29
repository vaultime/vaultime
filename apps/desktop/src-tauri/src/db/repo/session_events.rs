// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read access to the append-only session event log.

use rusqlite::Row;

use crate::db::connection::Database;
use crate::db::models::SessionEvent;
use crate::db::repo::map_db;
use crate::error::Result;

pub(crate) fn row_to_session_event(row: &Row) -> rusqlite::Result<SessionEvent> {
    Ok(SessionEvent {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        sequence: row.get("sequence")?,
        event_type: row.get("event_type")?,
        event_time_wall: row.get("event_time_wall")?,
        event_time_monotonic: row.get("event_time_monotonic")?,
        payload_json: row.get("payload_json")?,
        hash_prev: row.get("hash_prev")?,
        hash_self: row.get("hash_self")?,
        signature: row.get("signature")?,
    })
}

/// Returns all events for a game, newest first.
pub fn list_events_for_game(db: &Database, game_id: &str) -> Result<Vec<SessionEvent>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT se.*
                 FROM session_events se
                 INNER JOIN sessions s ON s.id = se.session_id
                 WHERE s.game_id = ?1
                 ORDER BY se.event_time_wall DESC, se.sequence DESC",
            )
            .map_err(map_db)?;

        let rows = stmt
            .query_map([game_id], row_to_session_event)
            .map_err(map_db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}
