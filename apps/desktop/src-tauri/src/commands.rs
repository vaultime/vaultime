// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tauri IPC command handlers.
//!
//! Commands that touch the disk, the network or many rows use
//! `#[tauri::command(async)]` so they run off the main thread and never freeze
//! the window.

use std::sync::Arc;

use log::warn;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppContext;
use crate::assets::crop::ArtworkSource;
use crate::assets::{self, AssetManager, GameAssetView};
use crate::backup::remote::{RemoteBackupRestoreResult, RemoteBackupUploadResult};
use crate::backup::{self, LocalBackupSummary};
use crate::constants::{
    BACKUP_HISTORY_LIMIT, CLOUD_API_BASE_URL, PAGE_SETTINGS, POLL_INTERVAL, STEAM_SOURCE,
    WINDOW_SIZE_SETTING,
};
use crate::db::connection::Database;
use crate::db::models::{
    BackupSnapshot, CreateGame, CropRect, EarlierPlaytime, Game, GameStatusChange, Session,
    SessionEvent, SessionNote, Setting, UpdateGame,
};
use crate::db::repo::{
    annotations, backup_snapshots, corrections, earlier_playtime, games, session_events, sessions,
    settings,
};
use crate::discovery::{self, DiscoveredGame};
use crate::earlier;
use crate::error::VaultimeError;
use crate::platform::activity::{foreground_detection_strategy, idle_detection_strategy};
use crate::platform::controller;
use crate::secure_storage;
use crate::tracking::engine::TrackingEngine;
use crate::window_size::{self, WindowSizeState};

#[tauri::command]
pub fn get_app_version(app_context: State<'_, AppContext>) -> Result<String, VaultimeError> {
    Ok(app_context.app_version.clone())
}

#[tauri::command]
pub fn get_device_id(app_context: State<'_, AppContext>) -> Result<String, VaultimeError> {
    Ok(app_context.device_id.clone())
}

#[tauri::command]
pub fn load_cloud_session_secure() -> Result<Option<String>, VaultimeError> {
    secure_storage::load_cloud_session()
}

#[tauri::command]
pub fn store_cloud_session_secure(session_json: String) -> Result<bool, VaultimeError> {
    secure_storage::store_cloud_session(&session_json)?;
    Ok(true)
}

#[tauri::command]
pub fn clear_cloud_session_secure() -> Result<bool, VaultimeError> {
    secure_storage::clear_cloud_session()?;
    Ok(true)
}

#[tauri::command]
pub fn has_cloud_backup_key_secure(account_id: String) -> Result<bool, VaultimeError> {
    secure_storage::has_cloud_backup_key(&account_id)
}

#[tauri::command]
pub fn store_cloud_backup_key_secure(
    account_id: String,
    passphrase: String,
    expected_key_check: Option<String>,
) -> Result<bool, VaultimeError> {
    secure_storage::store_cloud_backup_key(
        &account_id,
        &passphrase,
        expected_key_check.as_deref(),
    )?;
    Ok(true)
}

#[tauri::command]
pub fn clear_cloud_backup_key_secure(account_id: String) -> Result<bool, VaultimeError> {
    secure_storage::clear_cloud_backup_key(&account_id)?;
    Ok(true)
}

/// Every game, hidden ones too. Hidden games are still tracked and their
/// sessions count, only the library views leave them out.
#[tauri::command]
pub fn list_games(db: State<'_, Arc<Database>>) -> Result<Vec<Game>, VaultimeError> {
    games::list_all_games(&db)
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

#[tauri::command(async)]
pub fn delete_game(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    id: String,
) -> Result<bool, VaultimeError> {
    assets::delete_game(&db, &asset_manager, &id)
}

#[tauri::command(async)]
pub fn list_game_assets(
    db: State<'_, Arc<Database>>,
    game_id: String,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::list_game_assets(&db, &game_id)
}

#[tauri::command(async)]
pub fn list_preferred_game_assets(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::list_preferred_game_assets(&db)
}

#[tauri::command(async)]
pub fn scan_game_assets(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::scan_game_assets(&db, &asset_manager, &game_id)
}

#[tauri::command(async)]
pub fn open_artwork_file(source_path: String) -> Result<ArtworkSource, VaultimeError> {
    assets::open_artwork_file(&source_path)
}

#[tauri::command(async)]
pub fn open_game_asset_source(
    db: State<'_, Arc<Database>>,
    game_id: String,
    asset_id: String,
) -> Result<ArtworkSource, VaultimeError> {
    assets::open_game_asset_source(&db, &game_id, &asset_id)
}

#[tauri::command(async)]
pub fn import_game_asset(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
    source_path: String,
    crop: Option<CropRect>,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::import_game_asset(&db, &asset_manager, &game_id, &source_path, crop)
}

#[tauri::command(async)]
pub fn crop_game_asset(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
    asset_id: String,
    crop: CropRect,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::crop_game_asset(&db, &asset_manager, &game_id, &asset_id, crop)
}

#[tauri::command]
pub fn set_preferred_game_asset(
    db: State<'_, Arc<Database>>,
    game_id: String,
    asset_id: String,
) -> Result<bool, VaultimeError> {
    assets::set_preferred_game_asset(&db, &game_id, &asset_id)
}

#[tauri::command(async)]
pub fn list_sessions(db: State<'_, Arc<Database>>) -> Result<Vec<Session>, VaultimeError> {
    sessions::list_all_sessions(&db)
}

#[tauri::command(async)]
pub fn get_active_sessions(db: State<'_, Arc<Database>>) -> Result<Vec<Session>, VaultimeError> {
    sessions::get_active_sessions(&db)
}

#[tauri::command(async)]
pub fn get_session_events_for_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
) -> Result<Vec<SessionEvent>, VaultimeError> {
    session_events::list_events_for_game(&db, &game_id)
}

/// Turns the tray and taskbar icon into the accent while signed in to cloud backup.
#[tauri::command]
pub fn set_cloud_signed_in(app: tauri::AppHandle, signed_in: bool) {
    crate::tray::show_cloud_state(&app, signed_in);
}

/// The folder automatic backups go to right now.
#[tauri::command]
pub fn get_auto_backup_folder(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
) -> String {
    backup::auto::folder(&db, &app_context.app_dir)
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
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
        std::path::Path::new(&destination_dir),
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
pub fn inspect_local_backup(path: String) -> Result<LocalBackupSummary, VaultimeError> {
    backup::inspect_local_backup(std::path::Path::new(&path))
}

#[tauri::command(async)]
pub fn import_local_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    engine: State<'_, TrackingEngine>,
    path: String,
) -> Result<LocalBackupSummary, VaultimeError> {
    let summary = with_tracking_paused(&db, &engine, || {
        backup::import_local_backup(
            &db,
            &asset_manager,
            &app_context,
            std::path::Path::new(&path),
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

/// Release builds talk only to the Vaultime server, whatever the page asks.
/// Development builds follow the page, so a local server can be tried.
fn cloud_api_base_url(requested: &str) -> &str {
    if cfg!(debug_assertions) {
        requested
    } else {
        CLOUD_API_BASE_URL
    }
}

#[tauri::command(async)]
#[expect(clippy::too_many_arguments)]
pub fn upload_remote_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    api_base_url: String,
    access_token: String,
    account_id: String,
    client_device_id: Option<String>,
    label: Option<String>,
) -> Result<RemoteBackupUploadResult, VaultimeError> {
    let result = backup::remote::upload_remote_backup(
        &db,
        &asset_manager,
        &app_context,
        cloud_api_base_url(&api_base_url),
        &access_token,
        &account_id,
        client_device_id.as_deref(),
        label.as_deref(),
    )?;

    record_snapshot(
        &db,
        &result.payload_summary.source_device_id,
        &result.payload_summary.overall_checksum,
        &result.backup.storage_key,
        "Cloud backup",
    );

    Ok(result)
}

#[tauri::command(async)]
#[expect(clippy::too_many_arguments)]
pub fn restore_remote_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    engine: State<'_, TrackingEngine>,
    api_base_url: String,
    access_token: String,
    account_id: String,
    backup_id: String,
) -> Result<RemoteBackupRestoreResult, VaultimeError> {
    let result = with_tracking_paused(&db, &engine, || {
        backup::remote::restore_remote_backup(
            &db,
            &asset_manager,
            &app_context,
            cloud_api_base_url(&api_base_url),
            &access_token,
            &account_id,
            &backup_id,
        )
    })?;

    record_snapshot(
        &db,
        &result.restored_summary.source_device_id,
        &result.restored_summary.overall_checksum,
        &result.backup.storage_key,
        "Cloud restore",
    );

    Ok(result)
}

/// Runs a restore with tracking paused. Refuses while a game is being tracked,
/// because the restore replaces the session tables.
fn with_tracking_paused<T>(
    db: &Database,
    engine: &TrackingEngine,
    restore: impl FnOnce() -> Result<T, VaultimeError>,
) -> Result<T, VaultimeError> {
    engine.pause();
    let result = match sessions::get_active_sessions(db) {
        Ok(active) if !active.is_empty() => Err(VaultimeError::Backup(
            "close all running games before restoring a backup".into(),
        )),
        Ok(_) => restore(),
        Err(error) => Err(error),
    };
    engine.resume();
    result
}

/// Records a backup in the local history. A failure here must not fail the backup.
fn record_snapshot(db: &Database, device_id: &str, checksum: &str, location: &str, label: &str) {
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
    if !PAGE_SETTINGS.contains(&key.as_str()) {
        return Err(VaultimeError::Invalid(format!(
            "the setting {key} cannot be changed from the app"
        )));
    }
    settings::set_setting(&db, &key, &value)?;
    Ok(true)
}

#[derive(Debug, Serialize)]
pub struct TrackingDiagnostics {
    pub platform: String,
    pub running: bool,
    pub foreground_detection: String,
    pub idle_detection: String,
    pub controller_detection: String,
    pub controllers_connected: usize,
    pub poll_interval_seconds: u64,
}

/// Whether this system shows a tray icon, so closing the window can keep tracking.
#[tauri::command]
pub fn tray_available(tray: State<'_, crate::tray::TrayState>) -> bool {
    tray.available
}

fn main_window(app: &AppHandle) -> Result<tauri::WebviewWindow, VaultimeError> {
    app.get_webview_window("main")
        .ok_or_else(|| VaultimeError::Invalid("the Vaultime window is not open".into()))
}

/// The window size picked on this PC and which presets fit its screen.
#[tauri::command]
pub fn get_window_size(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
) -> Result<WindowSizeState, VaultimeError> {
    Ok(window_size::state(
        &main_window(&app)?,
        window_size::saved_choice(&db),
    ))
}

/// Saves a window size for this PC and gives the window that size right away.
#[tauri::command]
pub fn set_window_size(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    choice: String,
) -> Result<WindowSizeState, VaultimeError> {
    if !window_size::is_choice(&choice) {
        return Err(VaultimeError::Invalid(format!(
            "{choice} is not a window size"
        )));
    }
    settings::set_setting(&db, WINDOW_SIZE_SETTING, &choice)?;
    Ok(window_size::apply(&main_window(&app)?, &choice, false))
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
        controller_detection: controller::detection_strategy().into(),
        controllers_connected: controller::connected_count(),
        poll_interval_seconds: POLL_INTERVAL.as_secs(),
    })
}

#[tauri::command(async)]
pub fn discover_games(
    db: State<'_, Arc<Database>>,
    paths: Vec<String>,
) -> Result<Vec<DiscoveredGame>, VaultimeError> {
    discovery::scanner::scan_folders(&db, &paths)
}

/// Saves every finished session as CSV or JSON and returns how many.
#[tauri::command(async)]
pub fn export_sessions(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    path: String,
    format: crate::export::ExportFormat,
) -> Result<usize, VaultimeError> {
    crate::export::export_sessions(
        &db,
        std::path::Path::new(&path),
        format,
        &app_context.app_version,
    )
}

/// Counts a closed session only up to `ended_at`, with a reason.
#[tauri::command]
pub fn trim_session(
    db: State<'_, Arc<Database>>,
    session_id: String,
    ended_at: String,
    reason: String,
) -> Result<Session, VaultimeError> {
    corrections::trim_session(&db, &session_id, &ended_at, &reason)
}

/// Takes all time out of a closed session, with a reason.
#[tauri::command]
pub fn discard_session(
    db: State<'_, Arc<Database>>,
    session_id: String,
    reason: String,
) -> Result<Session, VaultimeError> {
    corrections::discard_session(&db, &session_id, &reason)
}

/// Adds play Vaultime did not see, labeled Manual. `launcher` is `steam`
/// when Steam counted the play too.
#[tauri::command]
pub fn add_manual_session(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    game_id: String,
    started_at: String,
    runtime_ms: i64,
    reason: String,
    launcher: Option<String>,
) -> Result<Session, VaultimeError> {
    corrections::add_manual_session(
        &db,
        &game_id,
        &app_context.device_id,
        &started_at,
        runtime_ms,
        &reason,
        launcher.as_deref(),
    )
}

/// Every status change of every game, oldest first.
#[tauri::command]
pub fn list_status_changes(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<GameStatusChange>, VaultimeError> {
    annotations::list_status_changes(&db)
}

/// Records a new status for a game, `none` clears it.
#[tauri::command]
pub fn set_game_status(
    db: State<'_, Arc<Database>>,
    game_id: String,
    status: String,
) -> Result<Option<GameStatusChange>, VaultimeError> {
    annotations::set_game_status(&db, &game_id, &status)
}

#[tauri::command]
pub fn list_session_notes(db: State<'_, Arc<Database>>) -> Result<Vec<SessionNote>, VaultimeError> {
    annotations::list_session_notes(&db)
}

/// Sets the note of a session, an empty note removes it.
#[tauri::command]
pub fn set_session_note(
    db: State<'_, Arc<Database>>,
    session_id: String,
    note: String,
) -> Result<Option<SessionNote>, VaultimeError> {
    annotations::set_session_note(&db, &session_id, &note)
}

/// Playtime from before Vaultime, per game.
#[tauri::command]
pub fn list_earlier_playtime(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<EarlierPlaytime>, VaultimeError> {
    earlier_playtime::list_earlier_playtime(&db)
}

/// What an import from Steam would add, without storing anything.
#[tauri::command(async)]
pub fn preview_steam_playtime(
    db: State<'_, Arc<Database>>,
) -> Result<earlier::SteamPlaytimePreview, VaultimeError> {
    earlier::preview_steam(&db)
}

#[tauri::command(async)]
pub fn import_steam_playtime(
    db: State<'_, Arc<Database>>,
) -> Result<earlier::SteamPlaytimePreview, VaultimeError> {
    earlier::import_steam(&db)
}

/// Removes all playtime imported from Steam. Returns how many games had some.
#[tauri::command]
pub fn remove_steam_playtime(db: State<'_, Arc<Database>>) -> Result<usize, VaultimeError> {
    earlier_playtime::clear_earlier_playtime(&db, STEAM_SOURCE)
}

#[tauri::command(async)]
pub fn discover_steam_games(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<DiscoveredGame>, VaultimeError> {
    discovery::steam::discover_steam_games(&db)
}

#[tauri::command(async)]
pub fn get_default_scan_paths() -> Result<Vec<String>, VaultimeError> {
    Ok(discovery::scanner::default_scan_paths())
}

/// Games that launchers other than Steam installed here.
#[tauri::command(async)]
pub fn discover_launcher_games(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<DiscoveredGame>, VaultimeError> {
    discovery::discover_launcher_games(&db)
}

#[tauri::command(async)]
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
        if let Err(error) = assets::scan_game_assets(&db, &asset_manager, &game.id) {
            warn!("artwork scan failed for {}: {error}", game.title);
        }

        imported.push(game);
    }

    Ok(imported)
}
