// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! `SQLite` connection management.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use log::warn;
use rusqlite::{Connection, OpenFlags};

use crate::constants::{DATABASE_BUSY_TIMEOUT, WAL_SIZE_LIMIT_BYTES};
use crate::db::repo::{busy, is_busy};
use crate::error::{Result, VaultimeError};

use super::migrate;

thread_local! {
    /// Set while this thread writes what only the live views show.
    static LIVE_WRITES: Cell<bool> = const { Cell::new(false) };
}

/// Thread-safe wrapper around a `SQLite` connection.
pub struct Database {
    conn: Mutex<Connection>,
    /// The file, none for a database in memory.
    path: Option<PathBuf>,
    /// Moves with every write the library loads, so the page reloads only
    /// when it did.
    revision: AtomicU64,
}

impl Database {
    /// Creates the database when it is missing and runs pending migrations.
    pub fn open(path: &Path) -> Result<Self> {
        let failed = |what: &str, error: rusqlite::Error| {
            if is_busy(&error) {
                busy()
            } else {
                VaultimeError::Database(format!("failed to {what}: {error}"))
            }
        };
        let conn = Connection::open(path).map_err(|error| failed("open database", error))?;
        // Tests wait only briefly, so one that holds the lock on purpose stays fast.
        let busy_wait = if cfg!(test) {
            std::time::Duration::from_millis(100)
        } else {
            DATABASE_BUSY_TIMEOUT
        };
        conn.busy_timeout(busy_wait)
            .map_err(|error| failed("set the busy timeout", error))?;

        // WAL suits the tracker's frequent small writes. Every commit still
        // waits for the disk, so a power cut never takes back a checkpoint, a
        // ledger pin or a takeover whose key files are already written.
        conn.execute_batch(&format!(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
             PRAGMA journal_size_limit={WAL_SIZE_LIMIT_BYTES};"
        ))
        .map_err(|error| failed("set pragmas", error))?;
        // Test databases never have to outlast a power cut, so their commits
        // skip waiting for the disk, which halves the time of the merge tests.
        #[cfg(test)]
        conn.execute_batch("PRAGMA synchronous=OFF;")
            .map_err(|error| VaultimeError::Database(format!("failed to set pragmas: {error}")))?;

        let db = Self {
            conn: Mutex::new(conn),
            path: Some(path.to_path_buf()),
            revision: AtomicU64::new(0),
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
            path: None,
            revision: AtomicU64::new(0),
        };

        migrate::run_migrations(&db)?;

        Ok(db)
    }

    /// The database file, none for a database in memory.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// A read-only connection to the database file, for reads beside the
    /// app's own connection. None for a database in memory.
    pub fn reader(&self) -> Result<Option<Connection>> {
        let Some(path) = &self.path else {
            return Ok(None);
        };
        let failed = |error: rusqlite::Error| {
            VaultimeError::Database(format!("failed to open a reader: {error}"))
        };
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI,
        )
        .map_err(failed)?;
        conn.busy_timeout(DATABASE_BUSY_TIMEOUT).map_err(failed)?;
        Ok(Some(conn))
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
        let before = conn.total_changes();
        let result = f(&conn);
        if conn.total_changes() != before && !LIVE_WRITES.get() {
            self.bump_revision();
        }
        result
    }

    /// The revision of the library. It moves with every write the pages load
    /// and stays while only the counters of running sessions move.
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    /// Moves the revision for a change in `live_writes` that the library
    /// shows, like a running session found suspicious.
    pub fn bump_revision(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    /// Runs `f` with the writes of this thread left out of the revision, for
    /// what only the live views show, like the checkpoints of running
    /// sessions.
    pub fn live_writes<T>(f: impl FnOnce() -> T) -> T {
        struct Restore(bool);
        impl Drop for Restore {
            fn drop(&mut self) {
                LIVE_WRITES.set(self.0);
            }
        }
        let _restore = Restore(LIVE_WRITES.replace(true));
        f()
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
    fn the_revision_moves_with_writes_and_not_with_reads_or_live_writes() {
        let db = Database::open_in_memory().unwrap();
        let write = || {
            db.with_conn(|conn| {
                conn.execute(
                    "INSERT INTO settings (key, value) VALUES ('probe', 'x')
                     ON CONFLICT(key) DO UPDATE SET value = value || 'x'",
                    [],
                )
                .map_err(map_db)
            })
            .unwrap();
        };
        let start = db.revision();
        db.with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM settings", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(map_db)
        })
        .unwrap();
        assert_eq!(db.revision(), start);

        write();
        assert_eq!(db.revision(), start + 1);
        db.with_transaction(|conn| {
            conn.execute("DELETE FROM settings WHERE key = 'probe'", [])
                .map_err(map_db)
        })
        .unwrap();
        assert_eq!(db.revision(), start + 2);

        Database::live_writes(write);
        assert_eq!(db.revision(), start + 2);
        // A panic in live writes leaves the thread counting again.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Database::live_writes(|| panic!("a tick that fails"))
        }));
        write();
        assert_eq!(db.revision(), start + 3);
    }

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
