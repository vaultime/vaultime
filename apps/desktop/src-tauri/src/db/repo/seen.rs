// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Programs from the game folders that ran in front without being a game of
//! the library.

use rusqlite::params;
use serde::Serialize;

use crate::db::connection::Database;
use crate::db::repo::map_db;
use crate::error::Result;
use crate::platform::process::path_key;

/// A program Discover offers because the player ran it from a game folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeenProgram {
    pub path: String,
    pub title: String,
    pub front_seconds: i64,
    pub last_seen_at: String,
}

/// Adds `seconds` in front to the program at `path`, noted first if new.
pub fn add_front_time(db: &Database, path: &str, title: &str, seconds: i64) -> Result<()> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO seen_programs (path_key, path, title, front_seconds) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path_key) DO UPDATE SET
                 path = excluded.path,
                 front_seconds = front_seconds + excluded.front_seconds,
                 last_seen_at = datetime('now')",
            params![path_key(path), path, title, seconds],
        )
        .map(|_| ())
        .map_err(map_db)
    })
}

/// The programs in front for at least `min_seconds`, the last seen first,
/// without those ignored since.
pub fn list(db: &Database, min_seconds: i64) -> Result<Vec<SeenProgram>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT path, title, front_seconds, last_seen_at FROM seen_programs
                 WHERE front_seconds >= ?1
                   AND path_key NOT IN (SELECT path_key FROM ignored_programs)
                 ORDER BY last_seen_at DESC, title",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([min_seconds], |row| {
                Ok(SeenProgram {
                    path: row.get(0)?,
                    title: row.get(1)?,
                    front_seconds: row.get(2)?,
                    last_seen_at: row.get(3)?,
                })
            })
            .map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })
}

/// Forgets a program, once it is a game of the library or ignored.
pub fn forget(db: &Database, path: &str) -> Result<()> {
    db.with_conn(|conn| {
        conn.execute(
            "DELETE FROM seen_programs WHERE path_key = ?1",
            [path_key(path)],
        )
        .map(|_| ())
        .map_err(map_db)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::ignored;
    use crate::playtime::testing::database;

    const GAME: &str = "C:/Games/Tunic/Tunic.exe";

    #[test]
    fn adds_up_the_time_in_front_over_runs() {
        let db = database();
        add_front_time(&db, GAME, "Tunic", 300).unwrap();
        assert!(
            list(&db, 600).unwrap().is_empty(),
            "five minutes are not enough"
        );
        add_front_time(&db, GAME, "Tunic", 300).unwrap();
        let seen = list(&db, 600).unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(
            (seen[0].title.as_str(), seen[0].front_seconds),
            ("Tunic", 600)
        );
    }

    #[test]
    fn an_ignored_or_forgotten_program_is_not_offered() {
        let db = database();
        add_front_time(&db, GAME, "Tunic", 900).unwrap();
        ignored::ignore(&db, GAME, "Tunic").unwrap();
        assert!(list(&db, 600).unwrap().is_empty());
        ignored::allow(&db, &path_key(GAME)).unwrap();
        assert_eq!(list(&db, 600).unwrap().len(), 1);
        forget(&db, GAME).unwrap();
        assert!(list(&db, 600).unwrap().is_empty());
    }
}
