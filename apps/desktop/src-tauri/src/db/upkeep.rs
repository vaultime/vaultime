// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keeps the database file small and its query plans current as the history
//! grows over the years.

use log::info;
use rusqlite::Connection;

use crate::constants::{VACUUM_MIN_FREE_BYTES, VACUUM_MIN_FREE_PERCENT};
use crate::db::connection::Database;
use crate::db::repo::map_db;
use crate::error::Result;

/// Brings the statistics the query planner works from up to date, which
/// SQLite only does for tables that changed enough since the last time.
pub fn optimize(db: &Database) -> Result<()> {
    db.with_conn(|conn| conn.execute_batch("PRAGMA optimize;").map_err(map_db))
}

/// Gives the space of deleted rows back to the disk once it is a large share
/// of the file, as after rebuilding the play slices or removing a game with
/// a long history. Runs at start, before tracking, as it rewrites the file.
/// Returns the bytes given back.
pub fn reclaim_space(db: &Database) -> Result<i64> {
    db.with_conn(|conn| {
        let free = free_bytes(conn)?;
        let total = pragma(conn, "page_count")? * pragma(conn, "page_size")?;
        if free < VACUUM_MIN_FREE_BYTES || free * 100 < total * VACUUM_MIN_FREE_PERCENT {
            return Ok(0);
        }
        conn.execute_batch("VACUUM;").map_err(map_db)?;
        let reclaimed = total - pragma(conn, "page_count")? * pragma(conn, "page_size")?;
        info!("gave {reclaimed} bytes of the database back to the disk");
        Ok(reclaimed)
    })
}

fn free_bytes(conn: &Connection) -> Result<i64> {
    Ok(pragma(conn, "freelist_count")? * pragma(conn, "page_size")?)
}

fn pragma(conn: &Connection, name: &str) -> Result<i64> {
    conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .map_err(map_db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_is_given_back_only_when_much_of_the_file_is_unused() {
        let dir = std::env::temp_dir().join(format!("vaultime-upkeep-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("vaultime.db")).unwrap();
        let rows = VACUUM_MIN_FREE_BYTES / 1_000 * 2;
        db.with_conn(|conn| {
            conn.execute_batch(&format!(
                "CREATE TABLE filler (value TEXT);
                 WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {rows})
                 INSERT INTO filler SELECT printf('%.1000d', i) FROM n;"
            ))
            .map_err(map_db)
        })
        .unwrap();
        assert_eq!(reclaim_space(&db).unwrap(), 0, "nothing deleted yet");

        db.with_conn(|conn| conn.execute_batch("DELETE FROM filler;").map_err(map_db))
            .unwrap();
        assert!(reclaim_space(&db).unwrap() >= VACUUM_MIN_FREE_BYTES);
        assert_eq!(db.with_conn(free_bytes).unwrap(), 0);
        optimize(&db).unwrap();
        drop(db);
        std::fs::remove_dir_all(&dir).ok();
    }
}
