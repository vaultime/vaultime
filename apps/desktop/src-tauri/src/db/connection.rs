// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! SQLite connection management.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::{Result, VaultimeError};

use super::migrate;

/// Thread-safe wrapper around a SQLite connection.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Opens (or creates) the database at `path` and runs pending migrations.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| {
            VaultimeError::Database(format!("failed to open database: {e}"))
        })?;

        // Enable WAL mode for better concurrent read performance.
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| {
                VaultimeError::Database(format!("failed to set pragmas: {e}"))
            })?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        migrate::run_migrations(&db)?;

        Ok(db)
    }

    /// Opens an in-memory database (useful for tests).
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| {
            VaultimeError::Database(format!("failed to open in-memory db: {e}"))
        })?;

        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| {
                VaultimeError::Database(format!("failed to set pragmas: {e}"))
            })?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        migrate::run_migrations(&db)?;

        Ok(db)
    }

    /// Acquires the connection lock and runs a closure with it.
    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock().map_err(|e| {
            VaultimeError::Database(format!("connection lock poisoned: {e}"))
        })?;
        f(&conn)
    }
}
