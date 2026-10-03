// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device registration.

use rusqlite::{Row, params};

use crate::constants::DEVICE_NAME_MAX_CHARS;
use crate::db::connection::Database;
use crate::db::models::Device;
use crate::db::repo::map_db;
use crate::error::{Result, VaultimeError};

fn row_to_device(row: &Row) -> rusqlite::Result<Device> {
    Ok(Device {
        id: row.get("id")?,
        platform: row.get("platform")?,
        app_version: row.get("app_version")?,
        key_id: row.get("key_id")?,
        registered_at: row.get("registered_at")?,
        name: row.get("name")?,
        merged_at: row.get("merged_at")?,
    })
}

/// Every PC this database knows, this one included.
pub fn list_devices(db: &Database) -> Result<Vec<Device>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT * FROM devices ORDER BY registered_at")
            .map_err(map_db)?;
        let rows = stmt.query_map([], row_to_device).map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })
}

/// The name of the PC when its sessions came here by merge, so only that PC
/// may change them. None for this PC and PCs never merged.
pub fn merged_pc_name(conn: &rusqlite::Connection, device_id: &str) -> Result<Option<String>> {
    use rusqlite::OptionalExtension;
    // This PC is never merged into itself.
    if crate::integrity::ledger::this_device().is_ok_and(|this| this == device_id) {
        return Ok(None);
    }
    conn.query_row(
        "SELECT COALESCE(name, id) FROM devices WHERE id = ?1 AND merged_at IS NOT NULL",
        [device_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(map_db)
}

/// Registers this PC on its first start and keeps its app version current.
pub fn ensure_device(db: &Database, id: &str, platform: &str, app_version: &str) -> Result<Device> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO devices (id, platform, app_version)
             VALUES (?1, ?2, ?3)",
            params![id, platform, app_version],
        )
        .map_err(map_db)?;

        conn.execute(
            "UPDATE devices SET app_version = ?1 WHERE id = ?2",
            params![app_version, id],
        )
        .map_err(map_db)?;

        conn.query_row("SELECT * FROM devices WHERE id = ?1", [id], row_to_device)
            .map_err(map_db)
    })
}

pub fn get_device(db: &Database, id: &str) -> Result<Device> {
    db.with_conn(|conn| {
        conn.query_row("SELECT * FROM devices WHERE id = ?1", [id], row_to_device)
            .map_err(map_db)
    })
}

/// Records the key this PC signs its ledger with, and gives the PC a first
/// name when it has none.
pub fn describe_this_device(db: &Database, id: &str, key_id: &str, name: &str) -> Result<()> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE devices SET key_id = ?1, name = COALESCE(name, ?2) WHERE id = ?3",
            params![key_id, clean_name(name), id],
        )
        .map_err(map_db)?;
        Ok(())
    })
}

/// Renames a PC. The name is trimmed and cut to `DEVICE_NAME_MAX_CHARS`.
pub fn rename_device(db: &Database, id: &str, name: &str) -> Result<Device> {
    let name = clean_name(name);
    if name.is_empty() {
        return Err(VaultimeError::Invalid("A PC needs a name.".into()));
    }
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE devices SET name = ?1 WHERE id = ?2",
            params![name, id],
        )
        .map_err(map_db)?;
        conn.query_row("SELECT * FROM devices WHERE id = ?1", [id], row_to_device)
            .map_err(map_db)
    })
}

fn clean_name(name: &str) -> String {
    name.trim().chars().take(DEVICE_NAME_MAX_CHARS).collect()
}
