// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Local backups and restores.

use std::path::Path;
use std::sync::Arc;

use log::warn;
use tauri::State;

use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::{self, LocalBackupSummary};
use crate::constants::BACKUP_HISTORY_LIMIT;
use crate::db::connection::Database;
use crate::db::models::BackupSnapshot;
use crate::db::repo::backup_snapshots;
use crate::error::VaultimeError;
use crate::tracking::engine::TrackingEngine;

/// The folder automatic backups go to right now.
#[tauri::command(async)]
pub fn get_auto_backup_folder(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
) -> String {
    backup::auto::folder(&db, &app_context.app_dir)
        .to_string_lossy()
        .into_owned()
}

#[tauri::command(async)]
pub fn list_backup_snapshots(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<BackupSnapshot>, VaultimeError> {
    backup_snapshots::list_snapshots(&db, BACKUP_HISTORY_LIMIT)
}

#[tauri::command(async)]
pub fn export_local_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    destination_dir: String,
) -> Result<LocalBackupSummary, VaultimeError> {
    let summary = backup::export_local_backup(
        &db,
        &asset_manager,
        &app_context,
        Path::new(&destination_dir),
    )?;

    record_snapshot(
        &db,
        &app_context.device_id,
        &summary.overall_checksum,
        &summary.backup_path,
        "Saved backup",
    );

    Ok(summary)
}

#[tauri::command(async)]
pub fn inspect_local_backup(
    db: State<'_, Arc<Database>>,
    path: String,
) -> Result<LocalBackupSummary, VaultimeError> {
    backup::inspect_local_backup(&db, Path::new(&path))
}

#[tauri::command(async)]
pub fn import_local_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    engine: State<'_, TrackingEngine>,
    path: String,
    take_over: bool,
) -> Result<LocalBackupSummary, VaultimeError> {
    let summary = engine.while_paused(&db, &app_context.device_id, || {
        backup::import_local_backup(
            &db,
            &asset_manager,
            &app_context,
            Path::new(&path),
            take_over,
        )
    })?;

    record_snapshot(
        &db,
        &summary.source_device_id,
        &summary.overall_checksum,
        &summary.backup_path,
        "Local restore",
    );

    Ok(summary)
}

/// Records a backup in the local history. A failure here must not fail the backup.
pub(super) fn record_snapshot(
    db: &Database,
    device_id: &str,
    checksum: &str,
    location: &str,
    label: &str,
) {
    if let Err(error) = backup_snapshots::create_snapshot(
        db,
        Some(device_id),
        checksum,
        Some(location),
        Some(label),
    ) {
        warn!("failed to record backup snapshot: {error}");
    }
}
