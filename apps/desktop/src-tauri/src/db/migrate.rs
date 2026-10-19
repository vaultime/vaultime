// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Forward-only migration runner.
//!
//! Migrations are embedded from `migrations/` and applied in order, each in its
//! own transaction. Applied names are recorded in `_migrations`.

use log::info;
use rusqlite::Connection;

use crate::error::{Result, VaultimeError};

use super::connection::Database;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial_schema",
        include_str!("../../migrations/0001_initial_schema.sql"),
    ),
    (
        "0002_cloud_sync",
        include_str!("../../migrations/0002_cloud_sync.sql"),
    ),
    (
        "0003_cloud_backup_state",
        include_str!("../../migrations/0003_cloud_backup_state.sql"),
    ),
    (
        "0004_drop_cloud_sync",
        include_str!("../../migrations/0004_drop_cloud_sync.sql"),
    ),
    (
        "0005_earlier_playtime",
        include_str!("../../migrations/0005_earlier_playtime.sql"),
    ),
    (
        "0006_game_status_and_notes",
        include_str!("../../migrations/0006_game_status_and_notes.sql"),
    ),
    (
        "0007_play_slices",
        include_str!("../../migrations/0007_play_slices.sql"),
    ),
    (
        "0008_ignored_programs",
        include_str!("../../migrations/0008_ignored_programs.sql"),
    ),
    (
        "0009_pc_ledgers",
        include_str!("../../migrations/0009_pc_ledgers.sql"),
    ),
    (
        "0010_merged_pcs",
        include_str!("../../migrations/0010_merged_pcs.sql"),
    ),
    (
        "0011_offered_programs",
        include_str!("../../migrations/0011_offered_programs.sql"),
    ),
    (
        "0012_session_event_indexes",
        include_str!("../../migrations/0012_session_event_indexes.sql"),
    ),
    (
        "0013_play_spans",
        include_str!("../../migrations/0013_play_spans.sql"),
    ),
    (
        "0014_notable_event_index",
        include_str!("../../migrations/0014_notable_event_index.sql"),
    ),
];

/// Names of all migrations this build knows about.
pub fn known_migrations() -> impl Iterator<Item = &'static str> {
    MIGRATIONS.iter().map(|(name, _)| *name)
}

/// Applies every migration that has not run yet.
pub(crate) fn run_migrations(db: &Database) -> Result<()> {
    db.with_conn(apply_pending)
}

/// The first migration of the database this build does not know, which a
/// newer Vaultime applied. None for a database without migrations.
pub fn newer_migration(conn: &Connection) -> Result<Option<String>> {
    let has_table: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_migrations')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| VaultimeError::Database(format!("failed to read migrations: {error}")))?;
    if !has_table {
        return Ok(None);
    }
    let mut statement = conn
        .prepare("SELECT name FROM _migrations ORDER BY name")
        .map_err(|error| VaultimeError::Database(format!("failed to read migrations: {error}")))?;
    let applied = statement
        .query_map([], |row| row.get::<_, String>(0))
        .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
        .map_err(|error| VaultimeError::Database(format!("failed to read migrations: {error}")))?;
    let known: Vec<&str> = known_migrations().collect();
    Ok(applied
        .into_iter()
        .find(|name| !known.contains(&name.as_str())))
}

fn apply_pending(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .map_err(|error| {
        VaultimeError::Database(format!("failed to create _migrations table: {error}"))
    })?;

    // A newer Vaultime may rely on what it added, so an older one must not
    // write to its database.
    if let Some(newer) = newer_migration(conn)? {
        return Err(VaultimeError::Database(format!(
            "this database was written by a newer version of Vaultime (migration {newer}). Install that version again or a later one."
        )));
    }

    for &(name, sql) in MIGRATIONS {
        let applied: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = ?1)",
                [name],
                |row| row.get(0),
            )
            .map_err(|error| {
                VaultimeError::Database(format!("failed to check migration {name}: {error}"))
            })?;

        if applied {
            continue;
        }

        info!("applying migration: {name}");
        let escaped_name = name.replace('\'', "''");
        conn.execute_batch(&format!(
            "BEGIN;\n{sql}\nINSERT INTO _migrations (name) VALUES ('{escaped_name}');\nCOMMIT;"
        ))
        .map_err(|error| {
            let _ = conn.execute_batch("ROLLBACK;");
            VaultimeError::Database(format!("migration {name} failed: {error}"))
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_database_a_newer_version_migrated_is_left_alone() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(newer_migration(&conn).unwrap(), None);
        apply_pending(&conn).unwrap();
        assert_eq!(newer_migration(&conn).unwrap(), None);

        conn.execute(
            "INSERT INTO _migrations (name) VALUES ('9999_from_a_newer_version')",
            [],
        )
        .unwrap();
        assert_eq!(
            newer_migration(&conn).unwrap().as_deref(),
            Some("9999_from_a_newer_version")
        );
        let refused = apply_pending(&conn).err().unwrap().to_string();
        assert!(refused.contains("newer version of Vaultime"), "{refused}");
        assert!(refused.contains("9999_from_a_newer_version"), "{refused}");
    }
}
