// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tauri IPC command handlers.
//!
//! Commands that touch the disk or many rows use `#[tauri::command(async)]` so
//! they run off the main thread and never freeze the window. Cloud transfers
//! run on the blocking pool, see `on_blocking_pool`.

use std::sync::Arc;

use log::warn;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppContext;
use crate::assets::crop::ArtworkSource;
use crate::assets::{self, AssetManager, GameAssetView};
use crate::backup::merge::{GameChoice, MergePreview, MergeSummary};
use crate::backup::remote::{RemoteBackupRestoreResult, RemoteBackupUploadResult};
use crate::backup::{self, LocalBackupSummary};
use crate::constants::{
    BACKUP_HISTORY_LIMIT, CLOUD_API_BASE_URL, PAGE_SETTINGS, PLAY_SLICES_VERSION_SETTING,
    POLL_INTERVAL, STEAM_SOURCE, WINDOW_SIZE_SETTING,
};
use crate::db::connection::Database;
use crate::db::models::{
    BackupSnapshot, CreateGame, CropRect, Device, EarlierPlaytime, Game, GameStatusChange, Session,
    SessionEvent, SessionNote, Setting, UpdateGame,
};
use crate::db::repo::games::GameLink;
use crate::db::repo::ignored::{self, IgnoredProgram};
use crate::db::repo::{
    annotations, backup_snapshots, corrections, devices, earlier_playtime, games, session_events,
    sessions, settings,
};
use crate::discovery::{self, DiscoveredGame};
use crate::earlier;
use crate::error::VaultimeError;
use crate::integrity::ledger::{self, LedgerReport};
use crate::platform::activity::{foreground_detection_strategy, idle_detection_strategy};
use crate::platform::controller;
use crate::playtime;
use crate::playtime::totals::{Bucket, PlayTotal};
use crate::secure_storage;
use crate::tracking::engine::TrackingEngine;
use crate::tracking::live::LiveSessions;
use crate::window_size::{self, WindowSizeState};

#[tauri::command]
pub fn get_app_version(app_context: State<'_, AppContext>) -> Result<String, VaultimeError> {
    Ok(app_context.app_version.clone())
}

#[tauri::command]
pub fn get_device_id(app_context: State<'_, AppContext>) -> Result<String, VaultimeError> {
    Ok(app_context.device_id.clone())
}

/// Every PC this database knows, this one included.
#[tauri::command(async)]
pub fn list_devices(db: State<'_, Arc<Database>>) -> Result<Vec<Device>, VaultimeError> {
    devices::list_devices(&db)
}

/// Games of other PCs and the games of this PC they count as.
#[tauri::command(async)]
pub fn list_game_links(db: State<'_, Arc<Database>>) -> Result<Vec<GameLink>, VaultimeError> {
    games::list_game_links(&db)
}

/// Makes a game of another PC count as a game of this PC, or as itself again,
/// and works out where its time fell once more.
#[tauri::command(async)]
pub fn link_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
    linked_game_id: Option<String>,
) -> Result<(), VaultimeError> {
    games::link_game(&db, &game_id, linked_game_id.as_deref())?;
    playtime::slices::rebuild_for_game(&db, &game_id)
}

/// What merging another PC's backup would bring in.
#[tauri::command(async)]
pub fn preview_merge(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    path: String,
    trust_new_keys: bool,
) -> Result<MergePreview, VaultimeError> {
    backup::merge::preview(
        &db,
        &app_context,
        std::path::Path::new(&path),
        trust_new_keys,
    )
}

/// Merges another PC's backup, with the player's choice for each game.
#[tauri::command(async)]
pub fn merge_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    path: String,
    choices: Vec<GameChoice>,
    trust_new_keys: bool,
) -> Result<MergeSummary, VaultimeError> {
    backup::merge::apply(
        &db,
        &asset_manager,
        &app_context,
        std::path::Path::new(&path),
        &choices,
        trust_new_keys,
    )
}

/// This PC: its name and what the ledgers say about the history.
#[derive(Debug, Serialize)]
pub struct ThisPc {
    pub device_id: String,
    pub name: String,
    pub ledger: LedgerReport,
}

fn this_pc(db: &Database, device_id: &str) -> Result<ThisPc, VaultimeError> {
    let device = devices::get_device(db, device_id)?;
    Ok(ThisPc {
        name: device.name.unwrap_or_else(|| device.id.clone()),
        device_id: device.id,
        ledger: db.with_conn(ledger::report)?,
    })
}

#[tauri::command(async)]
pub fn get_this_pc(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
) -> Result<ThisPc, VaultimeError> {
    this_pc(&db, &app_context.device_id)
}

#[tauri::command(async)]
pub fn rename_this_pc(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    name: String,
) -> Result<ThisPc, VaultimeError> {
    devices::rename_device(&db, &app_context.device_id, &name)?;
    this_pc(&db, &app_context.device_id)
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

/// Every game the library shows, hidden ones too. Hidden games are still
/// tracked and their sessions count, only the library views leave them out.
/// Games of other PCs that are linked to a game of this PC count as that
/// game and are left out.
#[tauri::command]
pub fn list_games(db: State<'_, Arc<Database>>) -> Result<Vec<Game>, VaultimeError> {
    games::list_shown_games(&db)
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
    // Games that stepped aside for it count that time again.
    let spans = playtime::slices::spans_beside(&db, &id)?;
    let deleted = assets::delete_game(&db, &asset_manager, &id)?;
    if let Err(error) = playtime::slices::rebuild_aside_around_spans(&db, &spans) {
        // The game is gone either way, so the next start builds every slice.
        warn!("slices beside a deleted game not rebuilt: {error}");
        if let Err(error) = settings::set_setting(&db, PLAY_SLICES_VERSION_SETTING, "0") {
            warn!("slices not marked for a rebuild: {error}");
        }
    }
    Ok(deleted)
}

/// Makes a game count only while no other game runs, like a launcher, or
/// always again, and works out where all play fell once more.
#[tauri::command(async)]
pub fn set_game_steps_aside(
    db: State<'_, Arc<Database>>,
    game_id: String,
    steps_aside: bool,
) -> Result<Game, VaultimeError> {
    let game = games::set_steps_aside(&db, &game_id, steps_aside)?;
    playtime::slices::rebuild_for_game(&db, &game_id)?;
    Ok(game)
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

#[tauri::command(async)]
pub fn delete_game_asset(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
    asset_id: String,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::delete_game_asset(&db, &asset_manager, &game_id, &asset_id)
}

#[tauri::command]
pub fn set_preferred_game_asset(
    db: State<'_, Arc<Database>>,
    game_id: String,
    asset_id: String,
) -> Result<bool, VaultimeError> {
    assets::set_preferred_game_asset(&db, &game_id, &asset_id)
}

/// Every session, running ones as of the tracker's latest tick.
#[tauri::command(async)]
pub fn list_sessions(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
) -> Result<Vec<Session>, VaultimeError> {
    let mut all = sessions::list_all_sessions(&db)?;
    for session in &mut all {
        live.apply(session);
    }
    playtime::totals::fill_set_aside(&db, &live, &mut all, chrono::Utc::now().timestamp_millis())?;
    show_linked_games(&db, &mut all)?;
    Ok(all)
}

/// Each game's play time per day, week, month, year or hour of the week,
/// from the local day `from` up to but not including `to`, both "2026-10-02".
#[tauri::command(async)]
pub fn get_play_totals(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    from: String,
    to: String,
    bucket: Bucket,
    game_id: Option<String>,
) -> Result<Vec<PlayTotal>, VaultimeError> {
    let parse = |text: &str| {
        chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map_err(|_| VaultimeError::Invalid(format!("not a date: {text}")))
    };
    playtime::totals::play_totals(
        &db,
        &live,
        &chrono::Local,
        parse(&from)?,
        parse(&to)?,
        bucket,
        game_id.as_deref(),
        chrono::Utc::now().timestamp_millis(),
    )
}

/// The running sessions as of the tracker's latest tick.
#[tauri::command(async)]
pub fn get_active_sessions(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
) -> Result<Vec<Session>, VaultimeError> {
    let mut active = sessions::get_active_sessions(&db)?;
    for session in &mut active {
        live.apply(session);
    }
    playtime::totals::fill_set_aside(
        &db,
        &live,
        &mut active,
        chrono::Utc::now().timestamp_millis(),
    )?;
    show_linked_games(&db, &mut active)?;
    Ok(active)
}

/// Shows a session of a game of another PC as one of the game of this PC
/// it is linked to. Only the shown game changes, the session's history keeps
/// the game it was recorded with.
fn show_linked_games(db: &Database, sessions: &mut [Session]) -> Result<(), VaultimeError> {
    let links = games::links(db)?;
    if links.is_empty() {
        return Ok(());
    }
    for session in sessions {
        if let Some(linked) = links.get(&session.game_id) {
            session.game_id.clone_from(linked);
        }
    }
    Ok(())
}

#[tauri::command(async)]
pub fn get_session_events_for_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
) -> Result<Vec<SessionEvent>, VaultimeError> {
    session_events::list_events_for_game(&db, &game_id)
}

/// Shows in the tray and taskbar icon whether this PC is signed in. Runs off
/// the main thread, as a redraw may wait for it.
#[tauri::command(async)]
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

/// Runs cloud work on the blocking pool. reqwest's blocking client must not
/// run on the async runtime, where debug builds panic and release builds hold
/// up a worker for the whole transfer.
async fn on_blocking_pool<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&AppHandle) -> Result<T, VaultimeError> + Send + 'static,
) -> Result<T, VaultimeError> {
    tauri::async_runtime::spawn_blocking(move || work(&app))
        .await
        .map_err(|error| VaultimeError::Cloud(format!("the cloud task stopped: {error}")))?
}

#[tauri::command]
pub async fn upload_remote_backup(
    app: AppHandle,
    api_base_url: String,
    access_token: String,
    account_id: String,
    client_device_id: Option<String>,
    label: Option<String>,
) -> Result<RemoteBackupUploadResult, VaultimeError> {
    on_blocking_pool(app, move |app| {
        let db = app.state::<Arc<Database>>();
        let result = backup::remote::upload_remote_backup(
            &db,
            &app.state::<AssetManager>(),
            &app.state::<AppContext>(),
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
    })
    .await
}

#[tauri::command]
pub async fn restore_remote_backup(
    app: AppHandle,
    api_base_url: String,
    access_token: String,
    account_id: String,
    backup_id: String,
) -> Result<RemoteBackupRestoreResult, VaultimeError> {
    on_blocking_pool(app, move |app| {
        let db = app.state::<Arc<Database>>();
        let result = with_tracking_paused(&db, &app.state::<TrackingEngine>(), || {
            backup::remote::restore_remote_backup(
                &db,
                &app.state::<AssetManager>(),
                &app.state::<AppContext>(),
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
    })
    .await
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
    discovery::without_ignored(&db, discovery::scanner::scan_folders(&db, &paths)?)
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

/// What a closed session would keep when it counted only up to `ended_at`.
#[tauri::command(async)]
pub fn preview_trim(
    db: State<'_, Arc<Database>>,
    session_id: String,
    ended_at: String,
) -> Result<corrections::TrimPreview, VaultimeError> {
    corrections::preview_trim(&db, &session_id, &ended_at)
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
    let found = discovery::steam::discover_steam_games(&db)?;
    remember_launcher_ids(&db, &found);
    discovery::without_ignored(&db, found)
}

/// Games already in the library learn their launcher id from a scan. A
/// failure here must not fail the scan.
fn remember_launcher_ids(db: &Database, found: &[DiscoveredGame]) {
    if let Err(error) = discovery::remember_launcher_ids(db, found) {
        warn!("launcher ids not stored: {error}");
    }
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
    let found = discovery::discover_launcher_games(&db)?;
    remember_launcher_ids(&db, &found);
    discovery::without_ignored(&db, found)
}

/// Programs the player said are no game, newest first.
#[tauri::command(async)]
pub fn list_ignored_programs(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<IgnoredProgram>, VaultimeError> {
    ignored::list(&db)
}

/// Never offers this program again and never counts it.
#[tauri::command(async)]
pub fn ignore_program(
    db: State<'_, Arc<Database>>,
    path: String,
    title: String,
) -> Result<(), VaultimeError> {
    ignored::ignore(&db, &path, &title)
}

/// Lets discovery offer an ignored program again and the tracker count it.
#[tauri::command(async)]
pub fn allow_program(
    db: State<'_, Arc<Database>>,
    path_key: String,
) -> Result<bool, VaultimeError> {
    ignored::allow(&db, &path_key)
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

        let mut game = games::create_game(&db, &input)?;
        if let Some(launcher_id) = disc.source_id.as_deref().filter(|id| !id.is_empty()) {
            game = games::set_launcher_id(&db, &game.id, launcher_id)?;
        }
        if let Err(error) = assets::scan_game_assets(&db, &asset_manager, &game.id) {
            warn!("artwork scan failed for {}: {error}", game.title);
        }

        imported.push(game);
    }

    Ok(imported)
}
