// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the player adds to their history: the status of a game over time and
//! short notes on sessions. Neither touches tracked time.

use rusqlite::{OptionalExtension, Row, params};

use crate::constants::SESSION_NOTE_MAX_CHARS;
use crate::db::connection::Database;
use crate::db::models::{GameStatusChange, SessionNote};
use crate::db::repo::map_db;
use crate::error::{Result, VaultimeError};
use crate::integrity;

/// Statuses a game can have. `none` clears it.
pub const GAME_STATUSES: &[&str] = &["backlog", "playing", "finished", "dropped", "none"];

fn row_to_change(row: &Row) -> rusqlite::Result<GameStatusChange> {
    Ok(GameStatusChange {
        id: row.get("id")?,
        game_id: row.get("game_id")?,
        status: row.get("status")?,
        changed_at: row.get("changed_at")?,
    })
}

fn row_to_note(row: &Row) -> rusqlite::Result<SessionNote> {
    Ok(SessionNote {
        session_id: row.get("session_id")?,
        note: row.get("note")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Every status change, oldest first.
pub fn list_status_changes(db: &Database) -> Result<Vec<GameStatusChange>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM game_status_changes ORDER BY changed_at, rowid")
            .map_err(map_db)?;
        let rows = stmt.query_map([], row_to_change).map_err(map_db)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Records a new status for a game. Setting the status it already has records
/// nothing and returns `None`.
pub fn set_game_status(
    db: &Database,
    game_id: &str,
    status: &str,
) -> Result<Option<GameStatusChange>> {
    if !GAME_STATUSES.contains(&status) {
        return Err(VaultimeError::Invalid(format!(
            "unknown game status {status}"
        )));
    }
    db.with_conn(|conn| {
        let current: Option<String> = conn
            .query_row(
                "SELECT status FROM game_status_changes WHERE game_id = ?1
                 ORDER BY changed_at DESC, rowid DESC LIMIT 1",
                [game_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_db)?;
        if current.as_deref().unwrap_or("none") == status {
            return Ok(None);
        }
        let change = GameStatusChange {
            id: uuid::Uuid::new_v4().to_string(),
            game_id: game_id.to_owned(),
            status: status.to_owned(),
            changed_at: integrity::now_timestamp(),
        };
        conn.execute(
            "INSERT INTO game_status_changes (id, game_id, status, changed_at) VALUES (?1, ?2, ?3, ?4)",
            params![change.id, change.game_id, change.status, change.changed_at],
        )
        .map_err(map_db)?;
        Ok(Some(change))
    })
}

pub fn list_session_notes(db: &Database) -> Result<Vec<SessionNote>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM session_notes ORDER BY session_id")
            .map_err(map_db)?;
        let rows = stmt.query_map([], row_to_note).map_err(map_db)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Sets the note of a session. An empty note removes it.
pub fn set_session_note(
    db: &Database,
    session_id: &str,
    note: &str,
) -> Result<Option<SessionNote>> {
    let note = note.trim();
    if note.chars().count() > SESSION_NOTE_MAX_CHARS {
        return Err(VaultimeError::Invalid(format!(
            "a note has at most {SESSION_NOTE_MAX_CHARS} characters"
        )));
    }
    db.with_conn(|conn| {
        if note.is_empty() {
            conn.execute(
                "DELETE FROM session_notes WHERE session_id = ?1",
                [session_id],
            )
            .map_err(map_db)?;
            return Ok(None);
        }
        let saved = SessionNote {
            session_id: session_id.to_owned(),
            note: note.to_owned(),
            updated_at: integrity::now_timestamp(),
        };
        conn.execute(
            "INSERT INTO session_notes (session_id, note, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET note = ?2, updated_at = ?3",
            params![saved.session_id, saved.note, saved.updated_at],
        )
        .map_err(map_db)?;
        Ok(Some(saved))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::{devices, games, sessions};

    fn game(db: &Database) -> String {
        games::create_game(
            db,
            &CreateGame {
                title: "Hades II".into(),
                executable_path: Some("C:/Games/Hades2.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn keeps_a_history_of_statuses_without_repeats() {
        let db = Database::open_in_memory().unwrap();
        let id = game(&db);
        assert!(
            set_game_status(&db, &id, "none").unwrap().is_none(),
            "no status is the start"
        );
        assert!(set_game_status(&db, &id, "playing").unwrap().is_some());
        assert!(set_game_status(&db, &id, "playing").unwrap().is_none());
        assert!(set_game_status(&db, &id, "finished").unwrap().is_some());
        assert!(set_game_status(&db, &id, "won").is_err());
        let statuses: Vec<String> = list_status_changes(&db)
            .unwrap()
            .into_iter()
            .map(|change| change.status)
            .collect();
        assert_eq!(statuses, ["playing", "finished"]);
    }

    #[test]
    fn notes_are_set_replaced_and_cleared() {
        let db = Database::open_in_memory().unwrap();
        let id = game(&db);
        devices::ensure_device(&db, "device", "windows", "0.1.0").unwrap();
        let session = sessions::create_session(&db, &id, "device").unwrap();

        set_session_note(&db, &session.id, "  Beat Malenia  ").unwrap();
        set_session_note(&db, &session.id, "Beat Malenia at last").unwrap();
        let notes = list_session_notes(&db).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].note, "Beat Malenia at last");

        let long = "x".repeat(SESSION_NOTE_MAX_CHARS + 1);
        assert!(set_session_note(&db, &session.id, &long).is_err());
        set_session_note(&db, &session.id, "   ").unwrap();
        assert!(list_session_notes(&db).unwrap().is_empty());
    }
}
