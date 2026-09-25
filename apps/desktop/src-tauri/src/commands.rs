// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tauri IPC command handlers.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::AppContext;
use crate::assets::{self, AssetManager, GameAssetView};
use crate::backup::remote::{RemoteBackupRestoreResult, RemoteBackupUploadResult};
use crate::backup::{self, LocalBackupSummary};
use crate::db::connection::Database;
use crate::db::models::{
    BackupSnapshot, CreateGame, Game, Session, SessionEvent, Setting, UpdateGame,
};
use crate::db::repo::{backup_snapshots, games, session_events, sessions, settings};
use crate::discovery::{self, DiscoveredGame};
use crate::error::VaultimeError;
use crate::platform::activity::{foreground_detection_strategy, idle_detection_strategy};
use crate::tracking::engine::TrackingEngine;

// ---------------------------------------------------------------------------
// App commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_app_version(app_context: State<'_, AppContext>) -> Result<String, VaultimeError> {
    Ok(app_context.app_version.clone())
}

// ---------------------------------------------------------------------------
// Game commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_games(db: State<'_, Arc<Database>>) -> Result<Vec<Game>, VaultimeError> {
    games::list_games(&db)
}

#[tauri::command]
pub fn get_game(db: State<'_, Arc<Database>>, id: String) -> Result<Game, VaultimeError> {
    games::get_game(&db, &id)
}

#[tauri::command]
pub fn create_game(db: State<'_, Arc<Database>>, input: CreateGame) -> Result<Game, VaultimeError> {
    games::create_game(&db, &input)
}

#[tauri::command]
pub fn update_game(
    db: State<'_, Arc<Database>>,
    id: String,
    input: UpdateGame,
) -> Result<Game, VaultimeError> {
    games::update_game(&db, &id, &input)
}

#[tauri::command]
pub fn delete_game(db: State<'_, Arc<Database>>, id: String) -> Result<bool, VaultimeError> {
    games::delete_game(&db, &id)
}

// ---------------------------------------------------------------------------
// Asset commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_game_assets(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::list_game_assets(&db, &asset_manager, &game_id)
}

#[tauri::command]
pub fn list_preferred_game_assets(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::list_preferred_game_assets(&db, &asset_manager)
}

#[tauri::command]
pub fn scan_game_assets(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::scan_game_assets(&db, &asset_manager, &game_id)
}

#[tauri::command]
pub fn import_game_asset(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
    source_path: String,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::import_game_asset(&db, &asset_manager, &game_id, &source_path)
}

#[tauri::command]
pub fn set_preferred_game_asset(
    db: State<'_, Arc<Database>>,
    game_id: String,
    asset_id: String,
) -> Result<bool, VaultimeError> {
    assets::set_preferred_game_asset(&db, &game_id, &asset_id)
}

// ---------------------------------------------------------------------------
// Session commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_sessions(db: State<'_, Arc<Database>>) -> Result<Vec<Session>, VaultimeError> {
    sessions::list_all_sessions(&db)
}

#[tauri::command]
pub fn get_sessions_for_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
) -> Result<Vec<Session>, VaultimeError> {
    sessions::list_sessions_for_game(&db, &game_id)
}

#[tauri::command]
pub fn get_active_sessions(db: State<'_, Arc<Database>>) -> Result<Vec<Session>, VaultimeError> {
    sessions::get_active_sessions(&db)
}

#[tauri::command]
pub fn get_session_events_for_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
) -> Result<Vec<SessionEvent>, VaultimeError> {
    session_events::list_events_for_game(&db, &game_id)
}

// ---------------------------------------------------------------------------
// Backup commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_backup_snapshots(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<BackupSnapshot>, VaultimeError> {
    backup_snapshots::list_snapshots(&db, 8)
}

#[tauri::command]
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
        std::path::Path::new(&destination_dir),
    )?;

    let _ = backup_snapshots::create_snapshot(
        &db,
        Some(&app_context.device_id),
        &summary.overall_checksum,
        Some(&summary.backup_path),
        Some("Local export"),
    );

    Ok(summary)
}

#[tauri::command]
pub fn inspect_local_backup(path: String) -> Result<LocalBackupSummary, VaultimeError> {
    backup::inspect_local_backup(std::path::Path::new(&path))
}

#[tauri::command]
pub fn import_local_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    engine: State<'_, TrackingEngine>,
    path: String,
) -> Result<LocalBackupSummary, VaultimeError> {
    if !sessions::get_active_sessions(&db)?.is_empty() {
        return Err(VaultimeError::Backup(
            "close all live sessions before restoring a local backup".into(),
        ));
    }

    engine.stop();

    let summary = backup::import_local_backup(
        &db,
        &asset_manager,
        &app_context,
        std::path::Path::new(&path),
    )?;

    let _ = backup_snapshots::create_snapshot(
        &db,
        Some(&summary.source_device_id),
        &summary.overall_checksum,
        Some(&summary.backup_path),
        Some("Local restore"),
    );

    Ok(summary)
}

#[tauri::command]
pub fn upload_remote_backup(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    api_base_url: String,
    access_token: String,
    client_device_id: Option<String>,
    label: Option<String>,
) -> Result<RemoteBackupUploadResult, VaultimeError> {
    let result = backup::remote::upload_remote_backup(
        &db,
        &app_context,
        &api_base_url,
        &access_token,
        client_device_id.as_deref(),
        label.as_deref(),
    )?;

    let _ = backup_snapshots::create_snapshot(
        &db,
        Some(&result.payload_summary.source_device_id),
        &result.payload_summary.overall_checksum,
        Some(&result.backup.storage_key),
        Some("Remote backup"),
    );

    Ok(result)
}

#[tauri::command]
pub fn restore_remote_backup(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    engine: State<'_, TrackingEngine>,
    api_base_url: String,
    access_token: String,
    backup_id: String,
) -> Result<RemoteBackupRestoreResult, VaultimeError> {
    if !sessions::get_active_sessions(&db)?.is_empty() {
        return Err(VaultimeError::Backup(
            "close all live sessions before restoring a remote backup".into(),
        ));
    }

    engine.stop();

    let result = backup::remote::restore_remote_backup(
        &db,
        &app_context,
        &api_base_url,
        &access_token,
        &backup_id,
    )?;

    let _ = backup_snapshots::create_snapshot(
        &db,
        Some(&result.restored_summary.source_device_id),
        &result.restored_summary.overall_checksum,
        Some(&result.backup.storage_key),
        Some("Remote restore"),
    );

    Ok(result)
}

// ---------------------------------------------------------------------------
// Settings commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_settings(db: State<'_, Arc<Database>>) -> Result<Vec<Setting>, VaultimeError> {
    settings::list_settings(&db)
}

#[tauri::command]
pub fn set_setting(
    db: State<'_, Arc<Database>>,
    key: String,
    value: String,
) -> Result<bool, VaultimeError> {
    settings::set_setting(&db, &key, &value)?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Tracking commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TrackingDiagnostics {
    pub platform: String,
    pub running: bool,
    pub foreground_detection: String,
    pub idle_detection: String,
    pub poll_interval_seconds: u64,
}

#[tauri::command]
pub fn get_tracking_status(engine: State<'_, TrackingEngine>) -> Result<bool, VaultimeError> {
    Ok(engine.is_running())
}

#[tauri::command]
pub fn get_tracking_diagnostics(
    engine: State<'_, TrackingEngine>,
) -> Result<TrackingDiagnostics, VaultimeError> {
    Ok(TrackingDiagnostics {
        platform: std::env::consts::OS.into(),
        running: engine.is_running(),
        foreground_detection: foreground_detection_strategy().into(),
        idle_detection: idle_detection_strategy().into(),
        poll_interval_seconds: 5,
    })
}

// ---------------------------------------------------------------------------
// Discovery commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn discover_games(
    db: State<'_, Arc<Database>>,
    paths: Vec<String>,
) -> Result<Vec<DiscoveredGame>, VaultimeError> {
    discovery::scanner::scan_folders(&db, &paths)
}

#[tauri::command]
pub fn discover_steam_games(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<DiscoveredGame>, VaultimeError> {
    discovery::steam::discover_steam_games(&db)
}

#[tauri::command]
pub fn get_default_scan_paths() -> Result<Vec<String>, VaultimeError> {
    Ok(discovery::scanner::default_scan_paths())
}

#[tauri::command]
pub fn import_discovered_games(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    discoveries: Vec<DiscoveredGame>,
) -> Result<Vec<Game>, VaultimeError> {
    let mut imported = Vec::new();

    for disc in &discoveries {
        if disc.already_added {
            continue;
        }

        let input = CreateGame {
            title: disc.title.clone(),
            executable_path: Some(disc.executable_path.clone()),
            install_folder: disc.install_folder.clone(),
            launcher_source: Some(disc.source.clone()),
        };

        let game = games::create_game(&db, &input)?;

        // Trigger asset scanning for the newly imported game.
        let _ = assets::scan_game_assets(&db, &asset_manager, &game.id);

        imported.push(game);
    }

    Ok(imported)
}
