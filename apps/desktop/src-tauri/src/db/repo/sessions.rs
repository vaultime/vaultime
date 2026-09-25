// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Session repository — CRUD operations for the `sessions` table.

use rusqlite::{Row, params};

use crate::db::connection::Database;
use crate::db::models::Session;
use crate::error::{Result, VaultimeError};

fn row_to_session(row: &Row) -> rusqlite::Result<Session> {
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

fn map_db(e: rusqlite::Error) -> VaultimeError {
    VaultimeError::Database(format!("{e}"))
}

/// Creates a new open session for a game on a device.
pub fn create_session(db: &Database, game_id: &str, device_id: &str) -> Result<Session> {
    let id = uuid::Uuid::new_v4().to_string();

    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO sessions
                (id, game_id, device_id, started_at_wall,
                 elapsed_monotonic_ms, active_ms, idle_ms, runtime_ms,
                 integrity_status, closed_cleanly)
             VALUES (?1, ?2, ?3, datetime('now'), 0, 0, 0, 0, 'local', 0)",
            params![id, game_id, device_id],
        )
        .map_err(map_db)?;

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
) -> Result<Session> {
    db.with_conn(|conn| {
        let updated = conn
            .execute(
                "UPDATE sessions
                 SET ended_at_wall = datetime('now'),
                     elapsed_monotonic_ms = ?1,
                     active_ms = ?2,
                     idle_ms = ?3,
                     runtime_ms = ?1,
                     closed_cleanly = 1
                 WHERE id = ?4 AND ended_at_wall IS NULL",
                params![runtime_ms, active_ms, idle_ms, session_id],
            )
            .map_err(map_db)?;

        if updated == 0 {
            return Err(VaultimeError::Tracking(
                "session not found or already closed".into(),
            ));
        }

        conn.query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [session_id],
            row_to_session,
        )
        .map_err(map_db)
    })
}

/// Persists the latest timing counters for an open session.
pub fn update_session_timing(
    db: &Database,
    session_id: &str,
    runtime_ms: i64,
    active_ms: i64,
    idle_ms: i64,
) -> Result<()> {
    db.with_conn(|conn| {
        let updated = conn
            .execute(
                "UPDATE sessions
                 SET elapsed_monotonic_ms = ?1,
                     active_ms = ?2,
                     idle_ms = ?3,
                     runtime_ms = ?1
                 WHERE id = ?4 AND ended_at_wall IS NULL",
                params![runtime_ms, active_ms, idle_ms, session_id],
            )
            .map_err(map_db)?;

        if updated == 0 {
            return Err(VaultimeError::Tracking(
                "session not found or already closed".into(),
            ));
        }

        Ok(())
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
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Returns all sessions for a specific game, newest first.
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

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Returns all sessions, newest first.
pub fn list_all_sessions(db: &Database) -> Result<Vec<Session>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM sessions ORDER BY started_at_wall DESC")
            .map_err(map_db)?;

        let rows = stmt.query_map([], row_to_session).map_err(map_db)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Returns a single session by ID.
pub fn get_session(db: &Database, id: &str) -> Result<Session> {
    db.with_conn(|conn| {
        conn.query_row("SELECT * FROM sessions WHERE id = ?1", [id], row_to_session)
            .map_err(map_db)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::Database;
    use crate::db::models::CreateGame;
    use crate::db::repo::{devices, games};

    const DEV_ID: &str = "test-device";

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

        let ended = end_session(&db, &session.id, 60_000, 45_000, 15_000).unwrap();
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
        end_session(&db, &session.id, 1000, 1000, 0).unwrap();

        let result = end_session(&db, &session.id, 2000, 1500, 500);
        assert!(result.is_err());
    }

    #[test]
    fn active_sessions_excludes_closed() {
        let db = test_db();
        let game_id = seed_game(&db);

        let s1 = create_session(&db, &game_id, DEV_ID).unwrap();
        let _s2 = create_session(&db, &game_id, DEV_ID).unwrap();
        end_session(&db, &s1.id, 5000, 4000, 1000).unwrap();

        let active = get_active_sessions(&db).unwrap();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn update_session_timing_persists_progress() {
        let db = test_db();
        let game_id = seed_game(&db);
        let session = create_session(&db, &game_id, DEV_ID).unwrap();

        update_session_timing(&db, &session.id, 90_000, 60_000, 30_000).unwrap();

        let updated = get_session(&db, &session.id).unwrap();
        assert_eq!(updated.runtime_ms, 90_000);
        assert_eq!(updated.active_ms, 60_000);
        assert_eq!(updated.idle_ms, 30_000);
        assert!(updated.ended_at_wall.is_none());
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
}
