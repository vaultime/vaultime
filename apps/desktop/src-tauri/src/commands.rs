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
use crate::cloud::backup as cloud_backup;
use crate::cloud::billing;
use crate::cloud::sync::{self, SyncResult};
use crate::cloud::types::{
    AuthCredentials, CloudBackupRecord, CloudBackupRestorePreview, CloudBackupRestoreResult,
    CloudBackupUploadResult, CloudSession, Subscription, SyncStatus,
};
use crate::db::connection::Database;
use crate::db::models::{
    BackupSnapshot, CreateGame, Game, Session, SessionEvent, Setting, UpdateGame,
};
use crate::db::repo::{backup_snapshots, games, session_events, sessions, settings};
use crate::discovery::{self, DiscoveredGame};
use crate::error::VaultimeError;
use crate::platform::activity::{foreground_detection_strategy, idle_detection_strategy};
use crate::tracking::engine::TrackingEngine;

async fn ensure_premium_access(auth: &AuthManager) -> Result<(), VaultimeError> {
    let sub = billing::get_subscription(auth).await?;
    if sub.has_premium_access() {
        Ok(())
    } else {
        Err(VaultimeError::Cloud(
            "an active Pro subscription is required for cloud sync and backup".into(),
        ))
    }
}

async fn ensure_cloud_device_registered(
    auth: &AuthManager,
    app_context: &AppContext,
) -> Result<(), VaultimeError> {
    if auth
        .current_session()
        .as_ref()
        .is_some_and(|session| session.device_registered)
    {
        return Ok(());
    }

    auth.register_device(
        &app_context.device_id,
        &app_context.device_id,
        std::env::consts::OS,
        &app_context.app_version,
    )
    .await
}

fn normalize_optional_setting(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

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

// ---------------------------------------------------------------------------
// Cloud commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CloudConfig {
    pub configured: bool,
    pub billing_enabled: bool,
}

#[tauri::command]
pub fn cloud_get_config(auth: State<'_, AuthManager>) -> Result<CloudConfig, VaultimeError> {
    Ok(CloudConfig {
        configured: auth.is_configured(),
        billing_enabled: crate::cloud::config::is_billing_enabled(),
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
// Billing commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn cloud_get_subscription(
    auth: State<'_, AuthManager>,
) -> Result<Subscription, VaultimeError> {
    billing::get_subscription(&auth).await
}

#[tauri::command]
pub async fn cloud_create_checkout_url(
    auth: State<'_, AuthManager>,
) -> Result<String, VaultimeError> {
    billing::create_checkout_url(&auth).await
}

#[tauri::command]
pub async fn cloud_create_portal_url(
    auth: State<'_, AuthManager>,
) -> Result<String, VaultimeError> {
    billing::create_portal_url(&auth).await
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
    ensure_premium_access(&auth).await?;
    ensure_cloud_device_registered(&auth, &app_context).await?;
    sync::sync_events(&db, &auth, &app_context.device_id).await
}

#[tauri::command]
pub fn cloud_get_unsynced_count(db: State<'_, Arc<Database>>) -> Result<u64, VaultimeError> {
    session_events::count_unsynced_events(&db)
}

#[tauri::command]
pub fn cloud_get_sync_status(
    db: State<'_, Arc<Database>>,
    auth: State<'_, AuthManager>,
) -> Result<SyncStatus, VaultimeError> {
    Ok(SyncStatus {
        connected: auth.current_session().is_some(),
        last_sync_at: normalize_optional_setting(settings::get_setting(&db, "cloud_last_sync_at")?),
        last_backup_at: normalize_optional_setting(settings::get_setting(
            &db,
            "cloud_last_backup_at",
        )?),
        pending_events: session_events::count_unsynced_events(&db)?,
    })
}

#[tauri::command]
pub async fn cloud_list_backups(
    auth: State<'_, AuthManager>,
) -> Result<Vec<CloudBackupRecord>, VaultimeError> {
    cloud_backup::list_backups(&auth).await
}

#[tauri::command]
pub async fn cloud_create_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    auth: State<'_, AuthManager>,
) -> Result<CloudBackupUploadResult, VaultimeError> {
    ensure_premium_access(&auth).await?;
    ensure_cloud_device_registered(&auth, &app_context).await?;

    let uploaded = cloud_backup::create_backup(
        &db,
        &asset_manager,
        &app_context,
        &auth,
        Some("Cloud snapshot"),
    )
    .await?;

    let _ = backup_snapshots::create_snapshot(
        &db,
        Some(&app_context.device_id),
        &uploaded.summary.overall_checksum,
        Some(&uploaded.backup.storage_path),
        Some("Cloud snapshot"),
    );

    Ok(uploaded)
}

#[tauri::command]
pub async fn cloud_get_restore_preview(
    db: State<'_, Arc<Database>>,
    auth: State<'_, AuthManager>,
    backup_id: String,
) -> Result<CloudBackupRestorePreview, VaultimeError> {
    cloud_backup::get_restore_preview(&db, &auth, &backup_id).await
}

#[tauri::command]
pub async fn cloud_restore_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    engine: State<'_, TrackingEngine>,
    auth: State<'_, AuthManager>,
    backup_id: String,
    force: bool,
) -> Result<CloudBackupRestoreResult, VaultimeError> {
    if !sessions::get_active_sessions(&db)?.is_empty() {
        return Err(VaultimeError::Cloud(
            "close all live sessions before restoring a cloud backup".into(),
        ));
    }

    engine.stop();

    let (backup, summary) =
        cloud_backup::restore_backup(&db, &asset_manager, &app_context, &auth, &backup_id, force)
            .await?;

    let _ = backup_snapshots::create_snapshot(
        &db,
        Some(&summary.source_device_id),
        &summary.overall_checksum,
        Some(&backup.storage_path),
        Some("Cloud restore"),
    );

    Ok(CloudBackupRestoreResult {
        backup,
        restart_required: summary.restart_required,
    })
}
