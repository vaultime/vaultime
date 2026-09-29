// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Queries for the `sessions` table. Every write also appends a session event.

use log::warn;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::json;

use crate::db::connection::Database;
use crate::db::models::Session;
use crate::db::repo::map_db;
use crate::error::{Result, VaultimeError};
use crate::integrity;

pub(crate) fn row_to_session(row: &Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get("id")?,
        game_id: row.get("game_id")?,
        device_id: row.get("device_id")?,
        started_at_wall: row.get("started_at_wall")?,
        ended_at_wall: row.get("ended_at_wall")?,
        elapsed_monotonic_ms: row.get("elapsed_monotonic_ms")?,
        active_ms: row.get("active_ms")?,
        idle_ms: row.get("idle_ms")?,
        runtime_ms: row.get("runtime_ms")?,
        integrity_status: row.get("integrity_status")?,
        closed_cleanly: row.get("closed_cleanly")?,
    })
}

pub(crate) fn attach_validated_status(conn: &Connection, mut session: Session) -> Result<Session> {
    if let Some(reason) = integrity::validate_session_history(conn, &session)? {
        warn!("session {} failed validation: {reason}", session.id);
        session.integrity_status = integrity::STATUS_SUSPICIOUS.into();
    }

    Ok(session)
}

/// Creates a new open session for a game on a device.
pub fn create_session(db: &Database, game_id: &str, device_id: &str) -> Result<Session> {
    let id = uuid::Uuid::new_v4().to_string();
    let started_at_wall = integrity::now_timestamp();

    db.with_transaction(|conn| {
        conn.execute(
            "INSERT INTO sessions
                (id, game_id, device_id, started_at_wall,
                 elapsed_monotonic_ms, active_ms, idle_ms, runtime_ms,
                 integrity_status, closed_cleanly)
             VALUES (?1, ?2, ?3, ?4, 0, 0, 0, 0, ?5, 0)",
            params![
                id,
                game_id,
                device_id,
                started_at_wall,
                integrity::STATUS_LOCAL
            ],
        )
        .map_err(map_db)?;

        integrity::append_session_event(
            conn,
            &id,
            "started",
            &started_at_wall,
            Some(0),
            &json!({
                "game_id": game_id,
                "device_id": device_id,
                "integrity_status": integrity::STATUS_LOCAL,
            })
            .to_string(),
        )?;

        conn.query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [&id],
            row_to_session,
        )
        .map_err(map_db)
    })
}

/// Closes an open session, recording its final timing data.
pub fn end_session(
    db: &Database,
    session_id: &str,
    runtime_ms: i64,
    active_ms: i64,
    idle_ms: i64,
    integrity_status: &str,
) -> Result<Session> {
    let ended_at_wall = integrity::now_timestamp();

    db.with_transaction(|conn| {
        let updated = conn
            .execute(
                "UPDATE sessions
                 SET ended_at_wall = ?1,
                     elapsed_monotonic_ms = ?2,
                     active_ms = ?3,
                     idle_ms = ?4,
                     runtime_ms = ?2,
                     integrity_status = ?5,
                     closed_cleanly = 1
                 WHERE id = ?6 AND ended_at_wall IS NULL",
                params![
                    ended_at_wall,
                    runtime_ms,
                    active_ms,
                    idle_ms,
                    integrity_status,
                    session_id
                ],
            )
            .map_err(map_db)?;

        if updated == 0 {
            return Err(VaultimeError::Tracking(
                "session not found or already closed".into(),
            ));
        }

        integrity::append_session_event(
            conn,
            session_id,
            "ended",
            &ended_at_wall,
            Some(runtime_ms),
            &json!({
                "runtime_ms": runtime_ms,
                "active_ms": active_ms,
                "idle_ms": idle_ms,
                "integrity_status": integrity_status,
                "closed_cleanly": true,
            })
            .to_string(),
        )?;

        conn.query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [session_id],
            row_to_session,
        )
        .map_err(map_db)
    })
}

/// Persists the latest timing counters for an open session.
#[expect(clippy::too_many_arguments)]
pub fn update_session_timing(
    db: &Database,
    session_id: &str,
    runtime_ms: i64,
    active_ms: i64,
    idle_ms: i64,
    wall_elapsed_ms: i64,
    drift_ms: i64,
    integrity_status: &str,
) -> Result<()> {
    let event_time_wall = integrity::now_timestamp();

    db.with_transaction(|conn| {
        let updated = conn
            .execute(
                "UPDATE sessions
                 SET elapsed_monotonic_ms = ?1,
                     active_ms = ?2,
                     idle_ms = ?3,
                     runtime_ms = ?1,
                     integrity_status = ?4
                 WHERE id = ?5 AND ended_at_wall IS NULL",
                params![runtime_ms, active_ms, idle_ms, integrity_status, session_id],
            )
            .map_err(map_db)?;

        if updated == 0 {
            return Err(VaultimeError::Tracking(
                "session not found or already closed".into(),
            ));
        }

        integrity::append_session_event(
            conn,
            session_id,
            "heartbeat",
            &event_time_wall,
            Some(runtime_ms),
            &json!({
                "runtime_ms": runtime_ms,
                "active_ms": active_ms,
                "idle_ms": idle_ms,
                "wall_elapsed_ms": wall_elapsed_ms,
                "drift_ms": drift_ms,
                "integrity_status": integrity_status,
            })
            .to_string(),
        )?;

        Ok(())
    })
}

/// Marks a session as suspicious and records the reason in the event log.
pub fn flag_session_suspicious(
    db: &Database,
    session_id: &str,
    runtime_ms: i64,
    wall_elapsed_ms: i64,
    drift_ms: i64,
    reason: &str,
) -> Result<()> {
    let event_time_wall = integrity::now_timestamp();

    db.with_transaction(|conn| {
        let updated = conn
            .execute(
                "UPDATE sessions
                 SET integrity_status = ?1
                 WHERE id = ?2 AND integrity_status != ?1",
                params![integrity::STATUS_SUSPICIOUS, session_id],
            )
            .map_err(map_db)?;

        if updated == 0 {
            return Ok(());
        }

        integrity::append_session_event(
            conn,
            session_id,
            "integrity_flagged",
            &event_time_wall,
            Some(runtime_ms),
            &json!({
                "reason": reason,
                "wall_elapsed_ms": wall_elapsed_ms,
                "drift_ms": drift_ms,
                "integrity_status": integrity::STATUS_SUSPICIOUS,
            })
            .to_string(),
        )
    })
}

/// Logs a pause between ticks, such as system sleep, that was left out of the
/// session time. `event_time_wall` is when tracking resumed.
pub fn record_tracking_gap(
    db: &Database,
    session_id: &str,
    runtime_ms: i64,
    wall_gap_ms: i64,
    monotonic_gap_ms: i64,
) -> Result<()> {
    let event_time_wall = integrity::now_timestamp();

    db.with_conn(|conn| {
        integrity::append_session_event(
            conn,
            session_id,
            "tracking_gap",
            &event_time_wall,
            Some(runtime_ms),
            &json!({
                "wall_gap_ms": wall_gap_ms,
                "monotonic_gap_ms": monotonic_gap_ms,
            })
            .to_string(),
        )
    })
}

/// Closes a session nothing tracks any more, for example after a crash. It
/// ends where its record ends, the last moment the tracker saw it, not now.
/// A session that was flagged stays Suspicious.
pub fn recover_session(db: &Database, session_id: &str, reason: &str) -> Result<Session> {
    let recovered_at = integrity::now_timestamp();

    db.with_transaction(|conn| {
        let session = attach_validated_status(
            conn,
            conn.query_row(
                "SELECT * FROM sessions WHERE id = ?1",
                [session_id],
                row_to_session,
            )
            .map_err(map_db)?,
        )?;
        let status = if session.integrity_status == integrity::STATUS_SUSPICIOUS {
            integrity::STATUS_SUSPICIOUS
        } else {
            integrity::STATUS_RECOVERED
        };
        let ended_at_wall: String = conn
            .query_row(
                "SELECT event_time_wall FROM session_events
                 WHERE session_id = ?1 ORDER BY sequence DESC LIMIT 1",
                [session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db)?
            .unwrap_or_else(|| session.started_at_wall.clone());

        let updated = conn
            .execute(
                "UPDATE sessions
                 SET ended_at_wall = ?1,
                     integrity_status = ?2,
                     closed_cleanly = 0
                 WHERE id = ?3 AND ended_at_wall IS NULL",
                params![ended_at_wall, status, session_id],
            )
            .map_err(map_db)?;

        if updated == 0 {
            return Err(VaultimeError::Tracking(
                "session not found or already closed".into(),
            ));
        }

        integrity::append_session_event(
            conn,
            session_id,
            "recovered",
            &recovered_at,
            Some(session.runtime_ms),
            &json!({
                "reason": reason,
                "ended_at_wall": ended_at_wall,
                "runtime_ms": session.runtime_ms,
                "active_ms": session.active_ms,
                "idle_ms": session.idle_ms,
                "integrity_status": status,
                "closed_cleanly": false,
            })
            .to_string(),
        )?;

        conn.query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [session_id],
            row_to_session,
        )
        .map_err(map_db)
    })
}

/// Returns all sessions that have not been closed yet.
pub fn get_active_sessions(db: &Database) -> Result<Vec<Session>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM sessions
                 WHERE ended_at_wall IS NULL
                 ORDER BY started_at_wall DESC",
            )
            .map_err(map_db)?;

        let rows = stmt.query_map([], row_to_session).map_err(map_db)?;
        let sessions = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)?;
        sessions
            .into_iter()
            .map(|session| attach_validated_status(conn, session))
            .collect()
    })
}

/// When a session ran and for how long, with its game's title.
#[derive(Debug, Clone)]
pub struct SessionSpan {
    pub game_title: String,
    pub started_at_wall: String,
    pub ended_at_wall: Option<String>,
    pub runtime_ms: i64,
}

/// Sessions that started at `since` or later and every running session,
/// oldest first. It skips the integrity check, so it stays cheap to call often.
pub fn spans_since(db: &Database, since: &str) -> Result<Vec<SessionSpan>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT games.title, sessions.started_at_wall, sessions.ended_at_wall,
                        sessions.runtime_ms
                 FROM sessions JOIN games ON games.id = sessions.game_id
                 WHERE sessions.started_at_wall >= ?1 OR sessions.ended_at_wall IS NULL
                 ORDER BY sessions.started_at_wall",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([since], |row| {
                Ok(SessionSpan {
                    game_title: row.get(0)?,
                    started_at_wall: row.get(1)?,
                    ended_at_wall: row.get(2)?,
                    runtime_ms: row.get(3)?,
                })
            })
            .map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })
}

/// Runtime per game id that `launcher` counted as well. A tracked session
/// counts at the length the tracker recorded, before any correction, since
/// the launcher saw the game run that long. A session added by hand counts
/// only when it was played through that launcher.
pub fn launcher_runtime_by_game(
    db: &Database,
    launcher: &str,
) -> Result<std::collections::HashMap<String, i64>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT sessions.game_id, SUM(CASE
                     WHEN manual.id IS NOT NULL THEN sessions.runtime_ms
                     ELSE COALESCE(
                         (SELECT json_extract(first.payload_json, '$.previous.runtime_ms')
                          FROM session_events AS first
                          WHERE first.session_id = sessions.id
                            AND first.event_type = 'corrected'
                          ORDER BY first.sequence LIMIT 1),
                         sessions.runtime_ms)
                 END)
                 FROM sessions
                 LEFT JOIN session_events AS manual
                     ON manual.session_id = sessions.id AND manual.event_type = 'added_manually'
                 WHERE manual.id IS NULL
                    OR json_extract(manual.payload_json, '$.launcher') = ?1
                 GROUP BY sessions.game_id",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([launcher], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })
}

/// Newest first.
#[cfg(test)]
pub fn list_sessions_for_game(db: &Database, game_id: &str) -> Result<Vec<Session>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM sessions
                 WHERE game_id = ?1
                 ORDER BY started_at_wall DESC",
            )
            .map_err(map_db)?;

        let rows = stmt.query_map([game_id], row_to_session).map_err(map_db)?;
        let sessions = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)?;
        sessions
            .into_iter()
            .map(|session| attach_validated_status(conn, session))
            .collect()
    })
}

/// Newest first.
pub fn list_all_sessions(db: &Database) -> Result<Vec<Session>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM sessions ORDER BY started_at_wall DESC")
            .map_err(map_db)?;

        let rows = stmt.query_map([], row_to_session).map_err(map_db)?;
        let sessions = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)?;
        sessions
            .into_iter()
            .map(|mut session| {
                let (reason, checked_now) =
                    integrity::validate_session_history_cached(conn, &session)?;
                if let Some(reason) = reason {
                    if checked_now {
                        warn!("session {} failed validation: {reason}", session.id);
                    }
                    session.integrity_status = integrity::STATUS_SUSPICIOUS.into();
                }
                Ok(session)
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::Database;
    use crate::db::models::CreateGame;
    use crate::db::repo::{devices, games};

    const DEV_ID: &str = "test-device";

    fn get_session(db: &Database, id: &str) -> Result<Session> {
        db.with_conn(|conn| {
            let session = conn
                .query_row("SELECT * FROM sessions WHERE id = ?1", [id], row_to_session)
                .map_err(map_db)?;
            attach_validated_status(conn, session)
        })
    }

    fn test_db() -> Database {
        let db = Database::open_in_memory().expect("in-memory db");
        devices::ensure_device(&db, DEV_ID, "linux", "0.1.0").unwrap();
        db
    }

    fn seed_game(db: &Database) -> String {
        let game = games::create_game(
            db,
            &CreateGame {
                title: "Test Game".into(),
                executable_path: Some("/usr/bin/test-game".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        game.id
    }

    #[test]
    fn create_and_get() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();

        assert_eq!(session.game_id, game_id);
        assert_eq!(session.device_id, DEV_ID);
        assert!(session.ended_at_wall.is_none());
        assert!(!session.closed_cleanly);

        let fetched = get_session(&db, &session.id).unwrap();
        assert_eq!(fetched.id, session.id);
    }

    #[test]
    fn end_session_closes_cleanly() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();

        let ended = end_session(
            &db,
            &session.id,
            60_000,
            45_000,
            15_000,
            integrity::STATUS_LOCAL,
        )
        .unwrap();
        assert!(ended.ended_at_wall.is_some());
        assert!(ended.closed_cleanly);
        assert_eq!(ended.runtime_ms, 60_000);
        assert_eq!(ended.active_ms, 45_000);
        assert_eq!(ended.idle_ms, 15_000);
    }

    #[test]
    fn end_already_closed_fails() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();
        end_session(&db, &session.id, 1000, 1000, 0, integrity::STATUS_LOCAL).unwrap();

        let result = end_session(&db, &session.id, 2000, 1500, 500, integrity::STATUS_LOCAL);
        assert!(result.is_err());
    }

    #[test]
    fn active_sessions_excludes_closed() {
        let db = test_db();
        let game_id = seed_game(&db);

        let s1 = create_session(&db, &game_id, DEV_ID).unwrap();
        let _s2 = create_session(&db, &game_id, DEV_ID).unwrap();
        end_session(&db, &s1.id, 5000, 4000, 1000, integrity::STATUS_LOCAL).unwrap();

        let active = get_active_sessions(&db).unwrap();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn update_session_timing_persists_progress() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();

        update_session_timing(
            &db,
            &session.id,
            90_000,
            60_000,
            30_000,
            91_000,
            1_000,
            integrity::STATUS_LOCAL,
        )
        .unwrap();

        let updated = get_session(&db, &session.id).unwrap();
        assert_eq!(updated.runtime_ms, 90_000);
        assert_eq!(updated.active_ms, 60_000);
        assert_eq!(updated.idle_ms, 30_000);
        assert!(updated.ended_at_wall.is_none());
    }

    #[test]
    fn suspicious_flag_updates_status() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();

        flag_session_suspicious(
            &db,
            &session.id,
            120_000,
            190_000,
            70_000,
            "wall_clock_drift_exceeded",
        )
        .unwrap();

        let updated = get_session(&db, &session.id).unwrap();
        assert_eq!(updated.integrity_status, integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn recover_session_marks_recovered() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();

        let recovered = recover_session(&db, &session.id, "startup_orphan_cleanup").unwrap();
        assert_eq!(recovered.integrity_status, integrity::STATUS_RECOVERED);
        assert!(!recovered.closed_cleanly);
        assert!(recovered.ended_at_wall.is_some());
    }

    #[test]
    fn tampered_event_payload_marks_session_suspicious() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();
        end_session(
            &db,
            &session.id,
            60_000,
            45_000,
            15_000,
            integrity::STATUS_LOCAL,
        )
        .unwrap();

        db.with_conn(|conn| {
            conn.execute(
                "UPDATE session_events
                 SET payload_json = ?1
                 WHERE session_id = ?2 AND event_type = 'ended'",
                params![
                    r#"{"runtime_ms":61000,"active_ms":45000,"idle_ms":15000,"integrity_status":"local","closed_cleanly":true}"#,
                    session.id
                ],
            )
            .unwrap();
            Ok(())
        })
        .unwrap();

        let tampered = get_session(&db, &session.id).unwrap();
        assert_eq!(tampered.integrity_status, integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn tampered_hash_chain_marks_session_suspicious() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();
        end_session(
            &db,
            &session.id,
            60_000,
            45_000,
            15_000,
            integrity::STATUS_LOCAL,
        )
        .unwrap();

        db.with_conn(|conn| {
            conn.execute(
                "UPDATE session_events
                 SET hash_self = 'tampered'
                 WHERE session_id = ?1 AND sequence = 1",
                [&session.id],
            )
            .unwrap();
            Ok(())
        })
        .unwrap();

        let tampered = get_session(&db, &session.id).unwrap();
        assert_eq!(tampered.integrity_status, integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn tampered_session_row_marks_session_suspicious() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();
        end_session(
            &db,
            &session.id,
            60_000,
            45_000,
            15_000,
            integrity::STATUS_LOCAL,
        )
        .unwrap();

        db.with_conn(|conn| {
            conn.execute(
                "UPDATE sessions
                 SET runtime_ms = 61_000
                 WHERE id = ?1",
                [&session.id],
            )
            .unwrap();
            Ok(())
        })
        .unwrap();

        let tampered = get_session(&db, &session.id).unwrap();
        assert_eq!(tampered.integrity_status, integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn list_for_game() {
        let db = test_db();
        let g1 = seed_game(&db);
        let g2 = games::create_game(
            &db,
            &CreateGame {
                title: "Other Game".into(),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap()
        .id;

        create_session(&db, &g1, DEV_ID).unwrap();
        create_session(&db, &g1, DEV_ID).unwrap();
        create_session(&db, &g2, DEV_ID).unwrap();

        let g1_sessions = list_sessions_for_game(&db, &g1).unwrap();
        assert_eq!(g1_sessions.len(), 2);

        let all = list_all_sessions(&db).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn recovery_ends_where_the_record_ends() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();
        update_session_timing(
            &db,
            &session.id,
            5_000,
            5_000,
            0,
            5_000,
            0,
            integrity::STATUS_LOCAL,
        )
        .unwrap();
        let last_heartbeat: String = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT event_time_wall FROM session_events
                     WHERE session_id = ?1 ORDER BY sequence DESC LIMIT 1",
                    [&session.id],
                    |row| row.get(0),
                )
                .map_err(map_db)
            })
            .unwrap();

        let recovered = recover_session(&db, &session.id, "startup_orphan_cleanup").unwrap();
        assert_eq!(
            recovered.ended_at_wall.as_deref(),
            Some(last_heartbeat.as_str())
        );
        let checked = get_session(&db, &session.id).unwrap();
        assert_eq!(checked.integrity_status, integrity::STATUS_RECOVERED);
    }

    #[test]
    fn a_dropped_flag_and_a_false_manual_label_are_caught() {
        let db = test_db();
        let game_id = seed_game(&db);

        let flagged = create_session(&db, &game_id, DEV_ID).unwrap();
        flag_session_suspicious(&db, &flagged.id, 0, 0, 30_000, "wall_clock_moved_backwards")
            .unwrap();
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE sessions SET integrity_status = 'local' WHERE id = ?1",
                [&flagged.id],
            )
            .map_err(map_db)?;
            Ok(())
        })
        .unwrap();
        // A heartbeat that claims the flag away, written through the chain.
        update_session_timing(&db, &flagged.id, 0, 0, 0, 0, 0, integrity::STATUS_LOCAL).unwrap();
        assert_eq!(
            get_session(&db, &flagged.id).unwrap().integrity_status,
            integrity::STATUS_SUSPICIOUS
        );

        let tracked = create_session(&db, &game_id, DEV_ID).unwrap();
        end_session(&db, &tracked.id, 0, 0, 0, integrity::STATUS_MANUAL).unwrap();
        assert_eq!(
            get_session(&db, &tracked.id).unwrap().integrity_status,
            integrity::STATUS_SUSPICIOUS
        );
    }

    #[test]
    fn listing_notices_a_changed_row_between_calls() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();
        end_session(&db, &session.id, 60_000, 60_000, 0, integrity::STATUS_LOCAL).unwrap();
        assert_eq!(
            list_all_sessions(&db).unwrap()[0].integrity_status,
            integrity::STATUS_LOCAL
        );

        db.with_conn(|conn| {
            conn.execute(
                "UPDATE sessions SET runtime_ms = 999999 WHERE id = ?1",
                [&session.id],
            )
            .map_err(map_db)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            list_all_sessions(&db).unwrap()[0].integrity_status,
            integrity::STATUS_SUSPICIOUS
        );
    }
}
