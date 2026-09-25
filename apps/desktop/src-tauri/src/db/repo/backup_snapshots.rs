// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! History of local and remote backup operations.

use rusqlite::{Row, params};

use crate::db::connection::Database;
use crate::db::models::BackupSnapshot;
use crate::error::{Result, VaultimeError};

fn row_to_backup_snapshot(row: &Row) -> rusqlite::Result<BackupSnapshot> {
    Ok(BackupSnapshot {
        id: row.get("id")?,
        created_at: row.get("created_at")?,
        source_device_id: row.get("source_device_id")?,
        checksum: row.get("checksum")?,
        remote_path: row.get("remote_path")?,
        restore_point_label: row.get("restore_point_label")?,
    })
}

fn map_db(error: rusqlite::Error) -> VaultimeError {
    VaultimeError::Database(format!("{error}"))
}

pub fn create_snapshot(
    db: &Database,
    source_device_id: Option<&str>,
    checksum: &str,
    location_path: Option<&str>,
    label: Option<&str>,
) -> Result<BackupSnapshot> {
    let id = uuid::Uuid::new_v4().to_string();

    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO backup_snapshots
                (id, source_device_id, checksum, remote_path, restore_point_label)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, source_device_id, checksum, location_path, label],
        )
        .map_err(map_db)?;

        conn.query_row(
            "SELECT * FROM backup_snapshots WHERE id = ?1",
            [&id],
            row_to_backup_snapshot,
        )
        .map_err(map_db)
    })
}

pub fn list_snapshots(db: &Database, limit: usize) -> Result<Vec<BackupSnapshot>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM backup_snapshots
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )
            .map_err(map_db)?;

        let rows = stmt
            .query_map([limit as i64], row_to_backup_snapshot)
            .map_err(map_db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}
