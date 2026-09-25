// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Simple forward-only migration runner.
//!
//! Migrations are embedded at compile time from the `migrations/` directory.
//! Each migration is a `.sql` file named `NNNN_description.sql` and runs in
//! lexicographic order. Applied migrations are tracked in `_migrations`.

use log::info;

use crate::error::{Result, VaultimeError};

use super::connection::Database;

/// Embedded migration files, sorted by name at compile time.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial_schema",
        include_str!("../../migrations/0001_initial_schema.sql"),
    ),
    (
        "0002_cloud_sync",
        include_str!("../../migrations/0002_cloud_sync.sql"),
    ),
];

/// Creates the migration tracking table if it doesn't exist, then applies
/// any migrations that haven't been run yet.
pub(crate) fn run_migrations(db: &Database) -> Result<()> {
    db.with_conn(|conn| {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                name TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .map_err(|e| VaultimeError::Database(format!("failed to create _migrations table: {e}")))?;

        for &(name, sql) in MIGRATIONS {
            let already_applied: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = ?1)",
                    [name],
                    |row| row.get(0),
                )
                .map_err(|e| {
                    VaultimeError::Database(format!("failed to check migration {name}: {e}"))
                })?;

            if already_applied {
                continue;
            }

            info!("applying migration: {name}");

            conn.execute_batch(sql)
                .map_err(|e| VaultimeError::Database(format!("migration {name} failed: {e}")))?;

            conn.execute("INSERT INTO _migrations (name) VALUES (?1)", [name])
                .map_err(|e| {
                    VaultimeError::Database(format!("failed to record migration {name}: {e}"))
                })?;
        }

        Ok(())
    })
}
