// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tauri IPC command handlers.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::AppContext;
use crate::assets::{self, AssetManager, GameAssetView};
use crate::backup::{self, LocalBackupSummary};
use crate::cloud::auth::AuthManager;
use crate::cloud::sync::{self, SyncResult};
use crate::cloud::types::{AuthCredentials, CloudSession};
use crate::db::connection::Database;
use crate::db::models::{
    BackupSnapshot, CreateGame, Game, Session, SessionEvent, Setting, UpdateGame,
};
use crate::db::repo::{backup_snapshots, games, session_events, sessions, settings};
use crate::error::VaultimeError;
use crate::platform::activity::{foreground_detection_strategy, idle_detection_strategy};
use crate::tracking::engine::TrackingEngine;

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
// Cloud commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CloudConfig {
    pub configured: bool,
}

#[tauri::command]
pub fn cloud_get_config(auth: State<'_, AuthManager>) -> Result<CloudConfig, VaultimeError> {
    Ok(CloudConfig {
        configured: auth.is_configured(),
    })
}

#[tauri::command]
pub fn cloud_get_session(
    auth: State<'_, AuthManager>,
) -> Result<Option<CloudSession>, VaultimeError> {
    Ok(auth.current_session())
}

#[tauri::command]
pub async fn cloud_sign_up(
    auth: State<'_, AuthManager>,
    input: AuthCredentials,
) -> Result<CloudSession, VaultimeError> {
    auth.sign_up(&input.email, &input.password).await
}

#[tauri::command]
pub async fn cloud_sign_in(
    auth: State<'_, AuthManager>,
    input: AuthCredentials,
) -> Result<CloudSession, VaultimeError> {
    auth.sign_in(&input.email, &input.password).await
}

#[tauri::command]
pub async fn cloud_sign_out(auth: State<'_, AuthManager>) -> Result<bool, VaultimeError> {
    auth.sign_out().await?;
    Ok(true)
}

#[tauri::command]
pub async fn cloud_refresh_token(
    auth: State<'_, AuthManager>,
) -> Result<CloudSession, VaultimeError> {
    auth.refresh_token().await
}

#[tauri::command]
pub async fn cloud_register_device(
    auth: State<'_, AuthManager>,
    app_context: State<'_, AppContext>,
) -> Result<bool, VaultimeError> {
    auth.register_device(
        &app_context.device_id,
        &app_context.device_id, // device_name = hostname for now
        std::env::consts::OS,
        &app_context.app_version,
    )
    .await?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Sync commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn cloud_sync_events(
    db: State<'_, Arc<Database>>,
    auth: State<'_, AuthManager>,
    app_context: State<'_, AppContext>,
) -> Result<SyncResult, VaultimeError> {
    sync::sync_events(&db, &auth, &app_context.device_id).await
}

#[tauri::command]
pub fn cloud_get_unsynced_count(
    db: State<'_, Arc<Database>>,
) -> Result<usize, VaultimeError> {
    let events = session_events::list_unsynced_events(&db, 1)?;
    // Return 0 or 1+ as a cheap "has unsynced" indicator.
    // A full count would be wasteful; the UI just needs to know if sync is needed.
    Ok(events.len())
}
