// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sync folder: where it is, the PCs in it and their backups.

use std::path::Path;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use super::backups::record_snapshot;
use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::cloud_merge::AutoMerge;
use crate::backup::folder::{self, FolderPc};
use crate::backup::merge::{GameChoice, MergePreview, MergeSummary};
use crate::backup::{LocalBackupSummary, auto};
use crate::constants::{LIBRARY_CHANGED_EVENT, SYNC_FOLDER_SETTING};
use crate::db::connection::Database;
use crate::db::repo::settings;
use crate::error::VaultimeError;
use crate::tracking::engine::TrackingEngine;

/// Label of a restore from the sync folder in the backup history.
const RESTORE_LABEL: &str = "Sync folder restore";

/// Where backups sync, if anywhere.
#[tauri::command(async)]
pub fn get_sync_folder(db: State<'_, Arc<Database>>) -> Option<String> {
    folder::root(&db).map(|root| root.to_string_lossy().into_owned())
}

/// Keeps backups in sync in `path`, or stops with none. A new folder gets a
/// backup of this PC right away, so the other PCs find it.
#[tauri::command(async)]
pub fn set_sync_folder(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    path: Option<String>,
) -> Result<Option<String>, VaultimeError> {
    match path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        Some(path) => {
            if !Path::new(path).is_dir() {
                return Err(VaultimeError::Invalid("That folder is not there.".into()));
            }
            let before = settings::get_setting(&db, SYNC_FOLDER_SETTING)?.unwrap_or_default();
            settings::set_setting(&db, SYNC_FOLDER_SETTING, path)?;
            // A folder that takes no backup, like a full or read-only one, is not kept.
            if let Err(error) = auto::back_up_to_sync_folder(&db, &asset_manager, &app_context) {
                settings::set_setting(&db, SYNC_FOLDER_SETTING, &before)?;
                return Err(error);
            }
        }
        None => settings::set_setting(&db, SYNC_FOLDER_SETTING, "")?,
    }
    Ok(folder::root(&db).map(|root| root.to_string_lossy().into_owned()))
}

/// The newest backup of every PC in the sync folder.
#[tauri::command(async)]
pub fn list_sync_folder_pcs(db: State<'_, Arc<Database>>) -> Result<Vec<FolderPc>, VaultimeError> {
    folder::list_pcs(&db)
}

/// What merging the newest sync folder backup of a PC would bring in.
#[tauri::command(async)]
pub fn preview_sync_merge(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    device_id: String,
    trust_new_keys: bool,
) -> Result<MergePreview, VaultimeError> {
    folder::preview_merge(&db, &app_context, &device_id, trust_new_keys)
}

/// Merges the sync folder backup the preview showed, with the player's
/// choice for each game.
#[tauri::command(async)]
pub fn merge_sync_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    device_id: String,
    choices: Vec<GameChoice>,
    trust_new_keys: bool,
) -> Result<MergeSummary, VaultimeError> {
    folder::merge_backup(
        &db,
        &asset_manager,
        &app_context,
        &device_id,
        &choices,
        trust_new_keys,
    )
}

/// Lets go of a sync folder backup a preview put together.
#[tauri::command(async)]
pub fn discard_sync_merge(app_context: State<'_, AppContext>, device_id: String) {
    folder::discard(&app_context, &device_id);
}

/// Merges the newer sync folder backups of the PCs merged here before, as
/// far as they need no choice of the player.
#[tauri::command(async)]
pub fn merge_due_sync_backups(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
) -> Result<Vec<AutoMerge>, VaultimeError> {
    let done = folder::merge_due(&db, &asset_manager, &app_context)?;
    if done
        .iter()
        .any(|merge| merge.sessions_added + merge.sessions_grown > 0)
    {
        let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
    }
    Ok(done)
}

/// The newest sync folder backup of a PC as a restore would see it.
#[tauri::command(async)]
pub fn inspect_sync_backup(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    device_id: String,
) -> Result<LocalBackupSummary, VaultimeError> {
    folder::inspect(&db, &app_context, &device_id)
}

/// Restores the sync folder backup of a PC that the inspection showed.
#[tauri::command(async)]
pub fn restore_sync_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    engine: State<'_, TrackingEngine>,
    device_id: String,
    take_over: bool,
) -> Result<LocalBackupSummary, VaultimeError> {
    let summary = engine.while_paused(&db, &app_context.device_id, || {
        folder::restore(&db, &asset_manager, &app_context, &device_id, take_over)
    })?;
    let location = folder::root(&db)
        .map(|root| root.join(&device_id).to_string_lossy().into_owned())
        .unwrap_or_default();
    record_snapshot(
        &db,
        &summary.source_device_id,
        &summary.overall_checksum,
        &location,
        RESTORE_LABEL,
    );
    Ok(summary)
}
