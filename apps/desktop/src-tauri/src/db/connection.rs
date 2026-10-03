// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! `SQLite` connection management.

use std::path::Path;
use std::sync::Mutex;

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

        // WAL needs fewer disk syncs per commit, which suits the tracker's frequent small writes.
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
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
        let conn = self.conn.lock().map_err(|error| {
            VaultimeError::Database(format!("connection lock poisoned: {error}"))
        })?;
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
