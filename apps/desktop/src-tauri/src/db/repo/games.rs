// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Game repository — CRUD operations for the `games` table.

use rusqlite::{Row, params};

use crate::db::connection::Database;
use crate::db::models::{CreateGame, Game, UpdateGame};
use crate::error::{Result, VaultimeError};

fn row_to_game(row: &Row) -> rusqlite::Result<Game> {
    Ok(Game {
        id: row.get("id")?,
        title: row.get("title")?,
        executable_path: row.get("executable_path")?,
        install_folder: row.get("install_folder")?,
        launcher_source: row.get("launcher_source")?,
        metadata_json: row.get("metadata_json")?,
        is_hidden: row.get("is_hidden")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_db(e: rusqlite::Error) -> VaultimeError {
    VaultimeError::Database(format!("{e}"))
}

/// Inserts a new game and returns it.
pub fn create_game(db: &Database, input: &CreateGame) -> Result<Game> {
    let id = uuid::Uuid::new_v4().to_string();

    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO games (id, title, executable_path, install_folder, launcher_source)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                input.title,
                input.executable_path,
                input.install_folder,
                input.launcher_source,
            ],
        )
        .map_err(map_db)?;

        conn.query_row("SELECT * FROM games WHERE id = ?1", [&id], row_to_game)
            .map_err(map_db)
    })
}

/// Returns all non-hidden games, ordered by title.
pub fn list_games(db: &Database) -> Result<Vec<Game>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM games WHERE is_hidden = 0 ORDER BY title COLLATE NOCASE")
            .map_err(map_db)?;

        let rows = stmt.query_map([], row_to_game).map_err(map_db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Returns all games including hidden, ordered by title.
pub fn list_all_games(db: &Database) -> Result<Vec<Game>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM games ORDER BY title COLLATE NOCASE")
            .map_err(map_db)?;

        let rows = stmt.query_map([], row_to_game).map_err(map_db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Returns a single game by ID.
pub fn get_game(db: &Database, id: &str) -> Result<Game> {
    db.with_conn(|conn| {
        conn.query_row("SELECT * FROM games WHERE id = ?1", [id], row_to_game)
            .map_err(map_db)
    })
}

/// Updates a game in place. Only provided fields are changed.
pub fn update_game(db: &Database, id: &str, input: &UpdateGame) -> Result<Game> {
    db.with_conn(|conn| {
        let current = conn
            .query_row("SELECT * FROM games WHERE id = ?1", [id], row_to_game)
            .map_err(map_db)?;

        let title = input.title.as_deref().unwrap_or(&current.title);
        let exe = input
            .executable_path
            .as_deref()
            .or(current.executable_path.as_deref());
        let folder = input
            .install_folder
            .as_deref()
            .or(current.install_folder.as_deref());
        let launcher = input
            .launcher_source
            .as_deref()
            .or(current.launcher_source.as_deref());
        let hidden = input.is_hidden.unwrap_or(current.is_hidden);

        conn.execute(
            "UPDATE games
             SET title = ?1, executable_path = ?2, install_folder = ?3,
                 launcher_source = ?4, is_hidden = ?5, updated_at = datetime('now')
             WHERE id = ?6",
            params![title, exe, folder, launcher, hidden, id],
        )
        .map_err(map_db)?;

        conn.query_row("SELECT * FROM games WHERE id = ?1", [id], row_to_game)
            .map_err(map_db)
    })
}

/// Deletes a game by ID. Returns `true` if a row was removed.
pub fn delete_game(db: &Database, id: &str) -> Result<bool> {
    db.with_conn(|conn| {
        let count = conn
            .execute("DELETE FROM games WHERE id = ?1", [id])
            .map_err(map_db)?;
        Ok(count > 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::Database;

    fn test_db() -> Database {
        Database::open_in_memory().expect("in-memory db")
    }

    #[test]
    fn create_and_get() {
        let db = test_db();
        let input = CreateGame {
            title: "Test Game".into(),
            executable_path: Some("/usr/bin/game".into()),
            install_folder: None,
            launcher_source: None,
        };
        let game = create_game(&db, &input).unwrap();
        assert_eq!(game.title, "Test Game");

        let fetched = get_game(&db, &game.id).unwrap();
        assert_eq!(fetched.id, game.id);
    }

    #[test]
    fn list_excludes_hidden() {
        let db = test_db();
        let g1 = create_game(
            &db,
            &CreateGame {
                title: "Visible".into(),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();

        let g2 = create_game(
            &db,
            &CreateGame {
                title: "Hidden".into(),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();

        update_game(
            &db,
            &g2.id,
            &UpdateGame {
                title: None,
                executable_path: None,
                install_folder: None,
                launcher_source: None,
                is_hidden: Some(true),
            },
        )
        .unwrap();

        let visible = list_games(&db).unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, g1.id);

        let all = list_all_games(&db).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn update_partial() {
        let db = test_db();
        let game = create_game(
            &db,
            &CreateGame {
                title: "Original".into(),
                executable_path: Some("/bin/orig".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();

        let updated = update_game(
            &db,
            &game.id,
            &UpdateGame {
                title: Some("Renamed".into()),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
                is_hidden: None,
            },
        )
        .unwrap();

        assert_eq!(updated.title, "Renamed");
        assert_eq!(updated.executable_path.as_deref(), Some("/bin/orig"));
    }

    #[test]
    fn delete_removes_game() {
        let db = test_db();
        let game = create_game(
            &db,
            &CreateGame {
                title: "Doomed".into(),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();

        assert!(delete_game(&db, &game.id).unwrap());
        assert!(!delete_game(&db, &game.id).unwrap());
    }
}
