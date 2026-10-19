// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! `SQLite` connection management.

use std::path::Path;
use std::sync::Mutex;

use log::warn;
use rusqlite::Connection;

use crate::error::{Result, VaultimeError};

use super::migrate;

/// Thread-safe wrapper around a `SQLite` connection.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Creates the database when it is missing and runs pending migrations.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|error| {
            VaultimeError::Database(format!("failed to open database: {error}"))
        })?;

        // WAL suits the tracker's frequent small writes. Every commit still
        // waits for the disk, so a power cut never takes back a checkpoint, a
        // ledger pin or a takeover whose key files are already written.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )
        .map_err(|error| VaultimeError::Database(format!("failed to set pragmas: {error}")))?;
        // Test databases never have to outlast a power cut, so their commits
        // skip waiting for the disk, which halves the time of the merge tests.
        #[cfg(test)]
        conn.execute_batch("PRAGMA synchronous=OFF;")
            .map_err(|error| VaultimeError::Database(format!("failed to set pragmas: {error}")))?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        migrate::run_migrations(&db)?;

        Ok(db)
    }

    /// An in-memory database, for tests and as the schema a backup has to match.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|error| {
            VaultimeError::Database(format!("failed to open in-memory db: {error}"))
        })?;

        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|error| VaultimeError::Database(format!("failed to set pragmas: {error}")))?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        migrate::run_migrations(&db)?;

        Ok(db)
    }

    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| {
            // A panic while the connection was in use. Dropping its
            // transaction on the way out rolled it back, so the connection
            // still works and every later call would fail for nothing.
            let conn = poisoned.into_inner();
            self.conn.clear_poison();
            if !conn.is_autocommit() {
                let _ = conn.execute_batch("ROLLBACK");
            }
            warn!("the database connection was in use during a panic and carries on");
            conn
        });
        f(&conn)
    }

    /// Like `with_conn`, in one transaction, so a session row and the event
    /// that records its change are stored together or not at all.
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        self.with_conn(|conn| {
            let failed = |error: rusqlite::Error| {
                VaultimeError::Database(format!("transaction failed: {error}"))
            };
            let transaction = conn.unchecked_transaction().map_err(failed)?;
            let value = f(&transaction)?;
            transaction.commit().map_err(failed)?;
            Ok(value)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::map_db;

    #[test]
    fn a_panic_inside_a_transaction_leaves_the_connection_usable() {
        let db = Database::open_in_memory().unwrap();
        let write = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            db.with_transaction(|conn| -> Result<()> {
                conn.execute(
                    "INSERT INTO settings (key, value) VALUES ('probe', 'half')",
                    [],
                )
                .map_err(map_db)?;
                panic!("in the middle of a write");
            })
        }));
        assert!(write.is_err());

        // The write was rolled back and the next ones go through.
        let count = |db: &Database| -> i64 {
            db.with_conn(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM settings WHERE key = 'probe'",
                    [],
                    |row| row.get(0),
                )
                .map_err(map_db)
            })
            .unwrap()
        };
        assert_eq!(count(&db), 0);
        db.with_transaction(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('probe', 'whole')",
                [],
            )
            .map_err(map_db)
        })
        .unwrap();
        assert_eq!(count(&db), 1);
    }
}
