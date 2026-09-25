// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Settings repository — key/value store for app configuration.

use rusqlite::{params, Row};

use crate::db::connection::Database;
use crate::db::models::Setting;
use crate::error::{Result, VaultimeError};

fn row_to_setting(row: &Row) -> rusqlite::Result<Setting> {
    Ok(Setting {
        key: row.get("key")?,
        value: row.get("value")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_db(e: rusqlite::Error) -> VaultimeError {
    VaultimeError::Database(format!("{e}"))
}

/// Returns all settings.
pub fn list_settings(db: &Database) -> Result<Vec<Setting>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM settings ORDER BY key")
            .map_err(map_db)?;

        let rows = stmt.query_map([], row_to_setting).map_err(map_db)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// Gets a single setting value by key.
pub fn get_setting(db: &Database, key: &str) -> Result<Option<String>> {
    db.with_conn(|conn| {
        match conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        ) {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(VaultimeError::Database(format!("{e}"))),
        }
    })
}

/// Sets a setting, inserting or updating as needed.
pub fn set_setting(db: &Database, key: &str, value: &str) -> Result<()> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
            params![key, value],
        )
        .map_err(map_db)?;
        Ok(())
    })
}
