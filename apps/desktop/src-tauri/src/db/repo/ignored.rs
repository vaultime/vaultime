// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Programs the player said are no game.

use std::collections::HashSet;

use rusqlite::params;
use serde::Serialize;

use crate::db::connection::Database;
use crate::db::repo::map_db;
use crate::error::{Result, VaultimeError};
use crate::platform::process::path_key;

/// A program discovery never offers and the tracker never counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IgnoredProgram {
    pub path_key: String,
    pub path: String,
    pub title: String,
    pub ignored_at: String,
}

/// Every ignored program, newest first.
pub fn list(db: &Database) -> Result<Vec<IgnoredProgram>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT path_key, path, title, ignored_at FROM ignored_programs ORDER BY ignored_at DESC, title")
            .map_err(map_db)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(IgnoredProgram {
                    path_key: row.get(0)?,
                    path: row.get(1)?,
                    title: row.get(2)?,
                    ignored_at: row.get(3)?,
                })
            })
            .map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })
}

/// The path keys of every ignored program.
pub fn keys(db: &Database) -> Result<HashSet<String>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare_cached("SELECT path_key FROM ignored_programs")
            .map_err(map_db)?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })
}

/// The paths of all ignored programs, for the tracker to match like games.
pub fn paths(db: &Database) -> Result<Vec<String>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare_cached("SELECT path FROM ignored_programs")
            .map_err(map_db)?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })
}

/// Ignores a program from now on. Ignoring it again keeps the first entry.
pub fn ignore(db: &Database, path: &str, title: &str) -> Result<()> {
    let path = path.trim();
    if path.is_empty() {
        return Err(VaultimeError::Invalid("no program to ignore".into()));
    }
    let title = title.trim();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO ignored_programs (path_key, path, title) VALUES (?1, ?2, ?3)",
            params![
                path_key(path),
                path,
                if title.is_empty() { path } else { title }
            ],
        )
        .map_err(map_db)?;
        Ok(())
    })
}

/// Lets discovery offer a program again and the tracker count it.
pub fn allow(db: &Database, key: &str) -> Result<bool> {
    db.with_conn(|conn| {
        let removed = conn
            .execute("DELETE FROM ignored_programs WHERE path_key = ?1", [key])
            .map_err(map_db)?;
        Ok(removed > 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_lists_and_allows_programs() {
        let db = Database::open_in_memory().unwrap();
        ignore(&db, r"C:\Tools\Benchmark.exe", "Benchmark").unwrap();
        ignore(&db, r"C:\Tools\Benchmark.exe", "Again").unwrap();
        assert!(ignore(&db, "  ", "Nothing").is_err());

        let listed = list(&db).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "Benchmark");
        assert!(
            keys(&db)
                .unwrap()
                .contains(&path_key(r"C:\Tools\Benchmark.exe"))
        );

        assert!(allow(&db, &listed[0].path_key).unwrap());
        assert!(keys(&db).unwrap().is_empty());
    }
}
