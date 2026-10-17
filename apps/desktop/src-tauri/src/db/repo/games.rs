// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Queries for the `games` table.

use std::collections::HashMap;

use rusqlite::{Row, params};

use crate::db::connection::Database;
use crate::db::models::{CreateGame, Game, GameMetadata, UpdateGame};
use crate::db::repo::map_db;
use crate::error::{Result, VaultimeError};
use crate::integrity;

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
        origin_device_id: row.get("origin_device_id")?,
    })
}

/// The games of this PC, the ones the tracker watches and discovery knows.
/// Games that came with sessions of another PC are left out.
pub fn list_local_games(db: &Database) -> Result<Vec<Game>> {
    let this = integrity::ledger::this_device().ok();
    Ok(list_all_games(db)?
        .into_iter()
        .filter(|game| game.origin_device_id.is_none() || game.origin_device_id == this)
        .collect())
}

/// The games the library shows: every game that is not linked to another
/// one. A linked game counts as the game it is linked to.
pub fn list_shown_games(db: &Database) -> Result<Vec<Game>> {
    let linked = links(db)?;
    Ok(list_all_games(db)?
        .into_iter()
        .filter(|game| !linked.contains_key(&game.id))
        .collect())
}

/// Each linked game and the game it counts as. Links never chain, so one
/// lookup gives the game shown.
pub fn links(db: &Database) -> Result<HashMap<String, String>> {
    db.with_conn(links_in)
}

pub(crate) fn links_in(conn: &rusqlite::Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn
        .prepare_cached("SELECT game_id, linked_game_id FROM game_links")
        .map_err(map_db)?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(map_db)?;
    rows.collect::<rusqlite::Result<_>>().map_err(map_db)
}

/// The game a game is shown as: the one it is linked to, or itself.
pub(crate) fn shown_id(conn: &rusqlite::Connection, game_id: &str) -> Result<String> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .prepare_cached("SELECT linked_game_id FROM game_links WHERE game_id = ?1")
        .and_then(|mut stmt| stmt.query_row([game_id], |row| row.get(0)).optional())
        .map_err(map_db)?
        .unwrap_or_else(|| game_id.to_owned()))
}

/// Every game that counts as the same game as `game_id`, the shown one first.
pub(crate) fn linked_set(conn: &rusqlite::Connection, game_id: &str) -> Result<Vec<String>> {
    with_linked(conn, &shown_id(conn, game_id)?)
}

/// A game and the games linked to it.
pub(crate) fn with_linked(conn: &rusqlite::Connection, game_id: &str) -> Result<Vec<String>> {
    let mut ids = vec![game_id.to_owned()];
    let mut stmt = conn
        .prepare_cached("SELECT game_id FROM game_links WHERE linked_game_id = ?1")
        .map_err(map_db)?;
    let rows = stmt
        .query_map([game_id], |row| row.get::<_, String>(0))
        .map_err(map_db)?;
    for row in rows {
        ids.push(row.map_err(map_db)?);
    }
    Ok(ids)
}

/// A game linked to another game: a game of another PC, or another install
/// of a game on this PC, like a copy from a second launcher.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GameLink {
    pub game_id: String,
    pub title: String,
    pub origin_device_id: Option<String>,
    pub linked_game_id: String,
    pub launcher_source: Option<String>,
    pub executable_path: Option<String>,
    pub install_folder: Option<String>,
}

/// Every link, with what the linked game has of its own.
pub fn list_game_links(db: &Database) -> Result<Vec<GameLink>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT g.id, g.title, g.origin_device_id, l.linked_game_id,
                        g.launcher_source, g.executable_path, g.install_folder
                 FROM game_links l JOIN games g ON g.id = l.game_id
                 ORDER BY g.title COLLATE NOCASE, g.launcher_source",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(GameLink {
                    game_id: row.get(0)?,
                    title: row.get(1)?,
                    origin_device_id: row.get(2)?,
                    linked_game_id: row.get(3)?,
                    launcher_source: row.get(4)?,
                    executable_path: row.get(5)?,
                    install_folder: row.get(6)?,
                })
            })
            .map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })
}

/// Makes a game count as a game of this PC, or as itself again with `None`.
/// The game may be another install of the same game here, or a game of
/// another PC. Both keep their own sessions, only what is shown changes.
pub fn link_game(db: &Database, game_id: &str, linked_game_id: Option<&str>) -> Result<()> {
    db.with_transaction(|conn| link_in(conn, game_id, linked_game_id))
}

/// `link_game` inside the caller's transaction. Links never chain: a game
/// that counts as another one cannot be linked to, and games that counted as
/// `game_id` move along to the game it now counts as.
pub(crate) fn link_in(
    conn: &rusqlite::Connection,
    game_id: &str,
    linked_game_id: Option<&str>,
) -> Result<()> {
    use rusqlite::OptionalExtension;
    let this = integrity::ledger::this_device().ok();
    let game = |id: &str| -> Result<Option<(String, Option<String>)>> {
        conn.query_row(
            "SELECT title, origin_device_id FROM games WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_db)
    };
    if game(game_id)?.is_none() {
        return Err(VaultimeError::Invalid(
            "This game is not in the library.".into(),
        ));
    }
    conn.execute("DELETE FROM game_links WHERE game_id = ?1", [game_id])
        .map_err(map_db)?;
    let Some(linked) = linked_game_id else {
        return Ok(());
    };
    if linked == game_id {
        return Err(VaultimeError::Invalid(
            "A game cannot count as itself.".into(),
        ));
    }
    let Some((title, origin)) = game(linked)? else {
        return Err(VaultimeError::Invalid(
            "That game is not in the library.".into(),
        ));
    };
    if origin.is_some() && origin != this {
        return Err(VaultimeError::Invalid(
            "A game can only count as a game of this PC.".into(),
        ));
    }
    if shown_id(conn, linked)? != linked {
        return Err(VaultimeError::Invalid(format!(
            "{title} counts as another game already. Pick that game instead."
        )));
    }
    conn.execute(
        "UPDATE game_links SET linked_game_id = ?2 WHERE linked_game_id = ?1",
        params![game_id, linked],
    )
    .map_err(map_db)?;
    conn.execute(
        "INSERT INTO game_links (game_id, linked_game_id) VALUES (?1, ?2)",
        params![game_id, linked],
    )
    .map_err(map_db)?;
    Ok(())
}

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

/// Hidden games included, ordered by title.
pub fn list_all_games(db: &Database) -> Result<Vec<Game>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM games ORDER BY title COLLATE NOCASE")
            .map_err(map_db)?;

        let rows = stmt.query_map([], row_to_game).map_err(map_db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

pub fn get_game(db: &Database, id: &str) -> Result<Game> {
    db.with_conn(|conn| {
        conn.query_row("SELECT * FROM games WHERE id = ?1", [id], row_to_game)
            .map_err(map_db)
    })
}

/// Only the fields set in `input` change.
pub fn update_game(db: &Database, id: &str, input: &UpdateGame) -> Result<Game> {
    db.with_conn(|conn| {
        let current = conn
            .query_row("SELECT * FROM games WHERE id = ?1", [id], row_to_game)
            .map_err(map_db)?;

        let title = input.title.as_deref().unwrap_or(&current.title);
        // An empty path clears the field, a missing one keeps it.
        let path_or_keep = |given: Option<&str>, kept: Option<&str>| match given {
            Some(path) if path.trim().is_empty() => None,
            Some(path) => Some(path.to_owned()),
            None => kept.map(str::to_owned),
        };
        let exe = path_or_keep(
            input.executable_path.as_deref(),
            current.executable_path.as_deref(),
        );
        let folder = path_or_keep(
            input.install_folder.as_deref(),
            current.install_folder.as_deref(),
        );
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

/// Returns `false` when there was no such game. Games linked to it go too,
/// other installs and games of other PCs, as the library showed their
/// sessions as its own. The ledger notes each session as removed by the
/// player.
pub fn delete_game(db: &Database, id: &str) -> Result<bool> {
    db.with_transaction(|conn| {
        let ids = with_linked(conn, id)?;
        let mut deleted = false;
        for game_id in &ids {
            let sessions: Vec<String> = {
                let mut stmt = conn
                    .prepare("SELECT id FROM sessions WHERE game_id = ?1 ORDER BY started_at_wall")
                    .map_err(map_db)?;
                let rows = stmt
                    .query_map([game_id], |row| row.get(0))
                    .map_err(map_db)?;
                rows.collect::<rusqlite::Result<_>>().map_err(map_db)?
            };
            for session_id in &sessions {
                integrity::ledger::record_removed(conn, session_id, "game_deleted")?;
            }
            deleted |= conn
                .execute("DELETE FROM games WHERE id = ?1", [game_id])
                .map_err(map_db)?
                > 0;
        }
        Ok(deleted)
    })
}

/// Makes a game count only while no other game runs, or always again. The
/// rest of its metadata stays as it is.
pub fn set_steps_aside(db: &Database, id: &str, steps_aside: bool) -> Result<Game> {
    change_metadata(db, id, |metadata| metadata.steps_aside = steps_aside)
}

/// Stores the launcher's own id for a game. The rest of its metadata stays.
pub fn set_launcher_id(db: &Database, id: &str, launcher_id: &str) -> Result<Game> {
    change_metadata(db, id, |metadata| {
        metadata.launcher_id = Some(launcher_id.to_owned());
    })
}

/// The launcher's own id for a game, when it was imported from a launcher.
pub fn launcher_id(game: &Game) -> Option<String> {
    serde_json::from_str::<GameMetadata>(&game.metadata_json)
        .ok()?
        .launcher_id
}

fn change_metadata(
    db: &Database,
    id: &str,
    change: impl FnOnce(&mut GameMetadata),
) -> Result<Game> {
    db.with_transaction(|conn| {
        let game = conn
            .query_row("SELECT * FROM games WHERE id = ?1", [id], row_to_game)
            .map_err(map_db)?;
        let mut metadata: GameMetadata =
            serde_json::from_str(&game.metadata_json).unwrap_or_default();
        change(&mut metadata);
        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|error| VaultimeError::Database(format!("invalid metadata json: {error}")))?;
        conn.execute(
            "UPDATE games SET metadata_json = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![metadata_json, id],
        )
        .map_err(map_db)?;
        conn.query_row("SELECT * FROM games WHERE id = ?1", [id], row_to_game)
            .map_err(map_db)
    })
}

pub fn set_metadata(db: &Database, id: &str, metadata: &GameMetadata) -> Result<Game> {
    let metadata_json = serde_json::to_string(metadata)
        .map_err(|error| VaultimeError::Database(format!("invalid metadata json: {error}")))?;

    db.with_conn(|conn| {
        conn.execute(
            "UPDATE games
             SET metadata_json = ?1, updated_at = datetime('now')
             WHERE id = ?2",
            params![metadata_json, id],
        )
        .map_err(map_db)?;

        conn.query_row("SELECT * FROM games WHERE id = ?1", [id], row_to_game)
            .map_err(map_db)
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
    fn hidden_games_stay_listed_with_their_flag() {
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

        let all = list_all_games(&db).unwrap();
        assert_eq!(all.len(), 2);
        let hidden: Vec<&str> = all
            .iter()
            .filter(|game| game.is_hidden)
            .map(|game| game.id.as_str())
            .collect();
        assert_eq!(hidden, [g2.id.as_str()]);
        assert!(all.iter().any(|game| game.id == g1.id && !game.is_hidden));
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

    fn titled(db: &Database, title: &str, launcher: &str) -> String {
        create_game(
            db,
            &CreateGame {
                title: title.into(),
                executable_path: Some(format!("/{launcher}/{title}.exe")),
                install_folder: None,
                launcher_source: Some(launcher.into()),
            },
        )
        .unwrap()
        .id
    }

    fn shown(db: &Database) -> Vec<String> {
        list_shown_games(db)
            .unwrap()
            .into_iter()
            .map(|game| game.id)
            .collect()
    }

    #[test]
    fn two_installs_count_as_one_game_until_unlinked() {
        let db = test_db();
        let steam = titled(&db, "Cyberpunk 2077", "steam");
        let gog = titled(&db, "Cyberpunk 2077", "gog");

        link_game(&db, &gog, Some(&steam)).unwrap();
        assert_eq!(shown(&db), std::slice::from_ref(&steam));
        assert_eq!(links(&db).unwrap()[&gog], steam);
        // Both stay games of this PC, so the tracker keeps watching both.
        assert_eq!(list_local_games(&db).unwrap().len(), 2);
        let set = db.with_conn(|conn| linked_set(conn, &gog)).unwrap();
        assert_eq!(set, [steam.clone(), gog.clone()]);

        link_game(&db, &gog, None).unwrap();
        assert_eq!(shown(&db).len(), 2);
        assert!(links(&db).unwrap().is_empty());
    }

    #[test]
    fn links_never_chain_and_move_along() {
        let db = test_db();
        let steam = titled(&db, "Cyberpunk 2077", "steam");
        let gog = titled(&db, "Cyberpunk 2077", "gog");
        let xbox = titled(&db, "Cyberpunk 2077", "xbox");
        let laptop = titled(&db, "Cyberpunk 2077", "steam");
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE games SET origin_device_id = 'laptop' WHERE id = ?1",
                [&laptop],
            )
            .map_err(map_db)
        })
        .unwrap();

        assert!(link_game(&db, &steam, Some(&steam)).is_err());
        link_game(&db, &gog, Some(&steam)).unwrap();
        link_game(&db, &laptop, Some(&steam)).unwrap();
        // A game that counts as another one is no target.
        let refused = link_game(&db, &xbox, Some(&gog)).unwrap_err();
        assert!(
            refused
                .to_string()
                .contains("counts as another game already")
        );
        // Nor is a game of another PC.
        assert!(link_game(&db, &xbox, Some(&laptop)).is_err());

        // The target counts as another game now, and what counted as it
        // follows, so the map stays flat.
        link_game(&db, &steam, Some(&xbox)).unwrap();
        let map = links(&db).unwrap();
        assert_eq!(map.len(), 3);
        assert!(map.values().all(|target| *target == xbox));
        assert_eq!(shown(&db), std::slice::from_ref(&xbox));
    }
}
