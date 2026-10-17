// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read access to the append-only session event log.

use rusqlite::Row;

use crate::db::connection::Database;
use crate::db::models::SessionEvent;
use crate::db::repo::map_db;
use crate::error::Result;

/// The columns `row_to_session_event` reads, in its order, from a table
/// named `se`. Read by position, since a chain check reads every event.
pub(crate) const SESSION_EVENT_COLUMNS: &str = "se.id, se.session_id, se.sequence, se.event_type,
    se.event_time_wall, se.event_time_monotonic, se.payload_json, se.hash_prev, se.hash_self,
    se.signature";

pub(crate) fn row_to_session_event(row: &Row) -> rusqlite::Result<SessionEvent> {
    Ok(SessionEvent {
        id: row.get(0)?,
        session_id: row.get(1)?,
        sequence: row.get(2)?,
        event_type: row.get(3)?,
        event_time_wall: row.get(4)?,
        event_time_monotonic: row.get(5)?,
        payload_json: row.get(6)?,
        hash_prev: row.get(7)?,
        hash_self: row.get(8)?,
        signature: row.get(9)?,
    })
}

/// Events of a game's sessions, also of those of the games linked to it,
/// newest first, those `filter` keeps.
fn events_for_game(db: &Database, game_id: &str, filter: &str) -> Result<Vec<SessionEvent>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare_cached(&format!(
                "SELECT {SESSION_EVENT_COLUMNS}
                 FROM session_events se
                 INNER JOIN sessions s ON s.id = se.session_id
                 WHERE (s.game_id = ?1
                    OR s.game_id IN (SELECT game_id FROM game_links WHERE linked_game_id = ?1))
                   AND {filter}
                 ORDER BY se.event_time_wall DESC, se.sequence DESC"
            ))
            .map_err(map_db)?;

        let rows = stmt
            .query_map([game_id], row_to_session_event)
            .map_err(map_db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Every event of a game's sessions, also of those of the games linked to
/// it, newest first.
#[cfg(test)]
pub fn list_events_for_game(db: &Database, game_id: &str) -> Result<Vec<SessionEvent>> {
    events_for_game(db, game_id, "1")
}

/// The events of a game's sessions that say something, also of those of the
/// games linked to it, newest first. Checkpoints, which only place time and
/// are most of the events, stay out.
pub fn list_notable_events_for_game(db: &Database, game_id: &str) -> Result<Vec<SessionEvent>> {
    events_for_game(db, game_id, "se.event_type != 'heartbeat'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playtime::testing::{Mark, database, game, recorded};

    #[test]
    fn notable_events_leave_the_checkpoints_out() {
        let db = database();
        let one = game(&db, "One");
        recorded(
            &db,
            &one,
            "device",
            "2026-10-02T18:00:00Z",
            &[
                Mark::Check(1, 1, 1),
                Mark::Check(2, 2, 2),
                Mark::Check(3, 3, 3),
            ],
        );
        let types = |events: Vec<SessionEvent>| -> Vec<String> {
            events.into_iter().map(|event| event.event_type).collect()
        };
        assert_eq!(
            types(list_events_for_game(&db, &one).unwrap()),
            ["ended", "heartbeat", "heartbeat", "started"]
        );
        assert_eq!(
            types(list_notable_events_for_game(&db, &one).unwrap()),
            ["ended", "started"]
        );
    }
}
