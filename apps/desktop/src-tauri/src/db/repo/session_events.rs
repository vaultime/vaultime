// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Session event repository — read access for the append-only event log.

use rusqlite::Row;

use crate::db::connection::Database;
use crate::db::models::SessionEvent;
use crate::error::{Result, VaultimeError};

fn row_to_session_event(row: &Row) -> rusqlite::Result<SessionEvent> {
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

fn map_db(error: rusqlite::Error) -> VaultimeError {
    VaultimeError::Database(format!("{error}"))
}

/// Returns events that have not been synced to the cloud yet, oldest first.
/// Limited to `batch_size` to keep upload payloads manageable.
pub fn list_unsynced_events(db: &Database, batch_size: u32) -> Result<Vec<SessionEvent>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, sequence, event_type, event_time_wall,
                        event_time_monotonic, payload_json, hash_prev, hash_self, signature
                 FROM session_events
                 WHERE synced_at IS NULL
                 ORDER BY event_time_wall ASC, sequence ASC
                 LIMIT ?1",
            )
            .map_err(map_db)?;

        let rows = stmt
            .query_map([batch_size], row_to_session_event)
            .map_err(map_db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Marks the given event IDs as synced with the current timestamp.
pub fn mark_events_synced(db: &Database, event_ids: &[String]) -> Result<()> {
    if event_ids.is_empty() {
        return Ok(());
    }
    db.with_conn(|conn| {
        let placeholders: Vec<String> = (1..=event_ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "UPDATE session_events SET synced_at = datetime('now') WHERE id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            event_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        conn.execute(&sql, params.as_slice()).map_err(map_db)?;
        Ok(())
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
