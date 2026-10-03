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
];

/// Names of all migrations this build knows about.
pub fn known_migrations() -> impl Iterator<Item = &'static str> {
    MIGRATIONS.iter().map(|(name, _)| *name)
}

/// Applies every migration that has not run yet.
pub(crate) fn run_migrations(db: &Database) -> Result<()> {
    db.with_conn(apply_pending)
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
