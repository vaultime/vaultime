// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Key value store for app settings.

use rusqlite::{OptionalExtension, Row, params};

use crate::db::connection::Database;
use crate::db::models::Setting;
use crate::db::repo::map_db;
use crate::error::Result;

fn row_to_setting(row: &Row) -> rusqlite::Result<Setting> {
    Ok(Setting {
        key: row.get("key")?,
        value: row.get("value")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_settings(db: &Database) -> Result<Vec<Setting>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM settings ORDER BY key")
            .map_err(map_db)?;

        let rows = stmt.query_map([], row_to_setting).map_err(map_db)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

/// `None` when the setting was never stored.
pub fn get_setting(db: &Database, key: &str) -> Result<Option<String>> {
    db.with_conn(|conn| {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(map_db)
    })
}

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
