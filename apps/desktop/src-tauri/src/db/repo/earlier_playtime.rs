// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Playtime from before Vaultime, imported from a launcher.

use rusqlite::{Row, params};

use crate::db::connection::Database;
use crate::db::models::EarlierPlaytime;
use crate::db::repo::map_db;
use crate::error::Result;

const MS_PER_MINUTE: i64 = 60_000;

fn row_to_earlier(row: &Row) -> rusqlite::Result<EarlierPlaytime> {
    let launcher_minutes: i64 = row.get("launcher_minutes")?;
    let tracked_before_ms: i64 = row.get("tracked_before_ms")?;
    Ok(EarlierPlaytime {
        game_id: row.get("game_id")?,
        source: row.get("source")?,
        launcher_minutes,
        tracked_before_ms,
        earlier_ms: earlier_ms(launcher_minutes, tracked_before_ms),
        last_played_at: row.get("last_played_at")?,
        imported_at: row.get("imported_at")?,
    })
}

/// The launcher's total without the part Vaultime had tracked already.
pub fn earlier_ms(launcher_minutes: i64, tracked_before_ms: i64) -> i64 {
    (launcher_minutes.saturating_mul(MS_PER_MINUTE) - tracked_before_ms).max(0)
}

pub fn list_earlier_playtime(db: &Database) -> Result<Vec<EarlierPlaytime>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM earlier_playtime ORDER BY game_id")
            .map_err(map_db)?;
        let rows = stmt.query_map([], row_to_earlier).map_err(map_db)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Replaces the imported playtime of the given games, in one transaction.
pub fn replace_earlier_playtime(db: &Database, entries: &[EarlierPlaytime]) -> Result<()> {
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction().map_err(map_db)?;
        for entry in entries {
            tx.execute(
                "INSERT INTO earlier_playtime
                     (game_id, source, launcher_minutes, tracked_before_ms, last_played_at, imported_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(game_id) DO UPDATE SET
                     source = ?2, launcher_minutes = ?3, tracked_before_ms = ?4,
                     last_played_at = ?5, imported_at = ?6",
                params![
                    entry.game_id,
                    entry.source,
                    entry.launcher_minutes,
                    entry.tracked_before_ms,
                    entry.last_played_at,
                    entry.imported_at,
                ],
            )
            .map_err(map_db)?;
        }
        tx.commit().map_err(map_db)
    })
}

/// Removes all imported playtime from one source.
pub fn clear_earlier_playtime(db: &Database, source: &str) -> Result<usize> {
    db.with_conn(|conn| {
        conn.execute("DELETE FROM earlier_playtime WHERE source = ?1", [source])
            .map_err(map_db)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::games;

    #[test]
    fn stores_replaces_and_clears() {
        let db = Database::open_in_memory().unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Counter-Strike 2".into(),
                executable_path: Some("C:/Games/cs2.exe".into()),
                install_folder: None,
                launcher_source: Some("steam".into()),
            },
        )
        .unwrap();
        let entry = |minutes: i64| EarlierPlaytime {
            game_id: game.id.clone(),
            source: "steam".into(),
            launcher_minutes: minutes,
            tracked_before_ms: 30 * MS_PER_MINUTE,
            earlier_ms: 0,
            last_played_at: None,
            imported_at: "2026-09-30T10:00:00Z".into(),
        };

        replace_earlier_playtime(&db, &[entry(100)]).unwrap();
        replace_earlier_playtime(&db, &[entry(120)]).unwrap();
        let stored = list_earlier_playtime(&db).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].earlier_ms, 90 * MS_PER_MINUTE);

        assert_eq!(clear_earlier_playtime(&db, "steam").unwrap(), 1);
        assert!(list_earlier_playtime(&db).unwrap().is_empty());
    }

    #[test]
    fn tracked_time_never_makes_it_negative() {
        assert_eq!(earlier_ms(10, 20 * MS_PER_MINUTE), 0);
        assert_eq!(earlier_ms(10, 0), 10 * MS_PER_MINUTE);
    }
}
