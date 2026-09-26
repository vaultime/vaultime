// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Device registration.

use rusqlite::{Row, params};

use crate::db::connection::Database;
use crate::db::models::Device;
use crate::error::{Result, VaultimeError};

fn row_to_device(row: &Row) -> rusqlite::Result<Device> {
    Ok(Device {
        id: row.get("id")?,
        platform: row.get("platform")?,
        app_version: row.get("app_version")?,
        key_id: row.get("key_id")?,
        registered_at: row.get("registered_at")?,
    })
}

fn map_db(e: rusqlite::Error) -> VaultimeError {
    VaultimeError::Database(format!("{e}"))
}

/// Ensures a device record exists for this machine. Returns the device.
pub fn ensure_device(db: &Database, id: &str, platform: &str, app_version: &str) -> Result<Device> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO devices (id, platform, app_version)
             VALUES (?1, ?2, ?3)",
            params![id, platform, app_version],
        )
        .map_err(map_db)?;

        // Update version on subsequent launches.
        conn.execute(
            "UPDATE devices SET app_version = ?1 WHERE id = ?2",
            params![app_version, id],
        )
        .map_err(map_db)?;

        conn.query_row("SELECT * FROM devices WHERE id = ?1", [id], row_to_device)
            .map_err(map_db)
    })
}
