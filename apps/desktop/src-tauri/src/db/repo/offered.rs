// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Programs discovery has shown the player on this PC.

use std::collections::HashSet;

use crate::db::connection::Database;
use crate::db::repo::map_db;
use crate::error::Result;
use crate::platform::process::path_key;

/// The path keys of every program shown so far.
pub fn keys(db: &Database) -> Result<HashSet<String>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare_cached("SELECT path_key FROM offered_programs")
            .map_err(map_db)?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })
}

/// Remembers that these programs were shown. Returns how many are new.
pub fn remember(db: &Database, paths: &[String]) -> Result<usize> {
    db.with_transaction(|conn| {
        let mut stmt = conn
            .prepare_cached("INSERT OR IGNORE INTO offered_programs (path_key) VALUES (?1)")
            .map_err(map_db)?;
        let mut added = 0;
        for path in paths.iter().map(|path| path.trim()) {
            if !path.is_empty() {
                added += stmt.execute([path_key(path)]).map_err(map_db)?;
            }
        }
        Ok(added)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_each_program_once() {
        let db = Database::open_in_memory().unwrap();
        assert!(keys(&db).unwrap().is_empty());

        let first = remember(&db, &["/games/a/a.exe".into(), " ".into()]).unwrap();
        let again = remember(&db, &["/games/a/a.exe".into(), "/games/b/b.exe".into()]).unwrap();

        assert_eq!((first, again), (1, 1));
        assert_eq!(
            keys(&db).unwrap(),
            HashSet::from([path_key("/games/a/a.exe"), path_key("/games/b/b.exe")])
        );
    }
}
