// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tauri IPC command handlers.
//!
//! Commands that read the database or the disk use `#[tauri::command(async)]`,
//! so they run off the main thread and never freeze the window, not even while
//! another thread holds the database. Small writes stay on the main thread,
//! which runs them in the order the page sent them, so the last of two quick
//! changes wins. Cloud transfers and the OS credential store run on the
//! blocking pool, see `on_blocking_pool`.

use std::path::Path;
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
    POLL_INTERVAL, RECENT_PLAY_MAX_DAYS, STEAM_SOURCE, WINDOW_SIZE_SETTING,
};
use crate::db::connection::Database;
use crate::db::models::{
    BackupSnapshot, CreateGame, CropRect, Device, EarlierPlaytime, Game, GameStatusChange, Session,
    SessionEvent, SessionNote, Setting, UpdateGame,
};
use crate::db::repo::games::GameLink;
use crate::db::repo::ignored::{self, IgnoredProgram};
use crate::db::repo::{
    annotations, backup_snapshots, corrections, devices, earlier_playtime, games, offered,
    session_events, sessions, settings,
};
use crate::discovery::auto::{FoundGames, NewGames};
use crate::discovery::{self, DiscoveredGame};
use crate::earlier;
use crate::error::VaultimeError;
use crate::integrity::ledger::{self, LedgerReport};
use crate::platform::activity::{foreground_detection_strategy, idle_detection_strategy};
use crate::platform::controller;
use crate::playtime;
use crate::playtime::beside::StepsAsideHint;
use crate::playtime::journal::JournalWeek;
use crate::playtime::summaries::{RecentPlay, YearPlay};
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

/// Linked games, other installs here and games of other PCs, with the games
/// they count as.
#[tauri::command(async)]
pub fn list_game_links(db: State<'_, Arc<Database>>) -> Result<Vec<GameLink>, VaultimeError> {
    games::list_game_links(&db)
}

/// Makes a game count as a game of this PC, or as itself again, and works
/// out once more where the time of every game it now counts with fell, since
/// whether they step aside follows the game shown.
#[tauri::command(async)]
pub fn link_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
    linked_game_id: Option<String>,
) -> Result<(), VaultimeError> {
    games::link_game(&db, &game_id, linked_game_id.as_deref())?;
    let shown = db.with_conn(|conn| games::shown_id(conn, &game_id))?;
    playtime::slices::rebuild_for_game(&db, &shown)
}

/// What merging another PC's backup would bring in.
#[tauri::command(async)]
pub fn preview_merge(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    path: String,
    trust_new_keys: bool,
) -> Result<MergePreview, VaultimeError> {
    backup::merge::preview(&db, &app_context, Path::new(&path), trust_new_keys)
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
        Path::new(&path),
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
    /// Whether this PC has no history of its own yet, so restoring a backup
    /// of another PC can make it continue as that PC.
    pub fresh: bool,
}

fn this_pc(db: &Database, device_id: &str) -> Result<ThisPc, VaultimeError> {
    let device = devices::get_device(db, device_id)?;
    Ok(ThisPc {
        name: device.name.unwrap_or_else(|| device.id.clone()),
        device_id: device.id,
        ledger: db.with_conn(ledger::report)?,
        fresh: db.with_conn(ledger::fresh_install)?,
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
pub async fn load_cloud_session_secure() -> Result<Option<String>, VaultimeError> {
    on_blocking_pool(secure_storage::load_cloud_session).await
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
pub async fn has_cloud_backup_key_secure(account_id: String) -> Result<bool, VaultimeError> {
    on_blocking_pool(move || secure_storage::has_cloud_backup_key(&account_id)).await
}

/// Derives the key from the passphrase, which takes a while on purpose.
#[tauri::command]
pub async fn store_cloud_backup_key_secure(
    account_id: String,
    passphrase: String,
    expected_key_check: Option<String>,
) -> Result<bool, VaultimeError> {
    on_blocking_pool(move || {
        secure_storage::store_cloud_backup_key(
            &account_id,
            &passphrase,
            expected_key_check.as_deref(),
        )
    })
    .await?;
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
#[tauri::command(async)]
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

/// Every session, running ones as of the tracker's latest tick, each with
/// the time it set aside and the time it shares with another copy of its game.
#[tauri::command(async)]
pub fn list_sessions(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    app_context: State<'_, AppContext>,
) -> Result<Vec<Session>, VaultimeError> {
    let mut all = sessions::list_all_sessions(&db)?;
    as_of_now(&db, &live, &mut all)?;
    sessions::show_running_elsewhere(&db, &app_context.device_id, &mut all)?;
    Ok(all)
}

fn parse_day(text: &str) -> Result<chrono::NaiveDate, VaultimeError> {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map_err(|_| VaultimeError::Invalid(format!("not a date: {text}")))
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
    playtime::totals::play_totals(
        &db,
        &live,
        &chrono::Local,
        parse_day(&from)?,
        parse_day(&to)?,
        bucket,
        game_id.as_deref(),
        chrono::Utc::now().timestamp_millis(),
    )
}

/// The journal's week from the local day `from`, a Monday, "2026-10-05".
#[tauri::command(async)]
pub fn get_journal_week(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    app_context: State<'_, AppContext>,
    from: String,
) -> Result<JournalWeek, VaultimeError> {
    playtime::journal::journal_week(
        &db,
        &live,
        &chrono::Local,
        parse_day(&from)?,
        &app_context.device_id,
        chrono::Utc::now().timestamp_millis(),
    )
}

/// The year `year` for the stats page.
#[tauri::command(async)]
pub fn get_year_play(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    year: i32,
) -> Result<YearPlay, VaultimeError> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    playtime::summaries::year_play(
        &db,
        &live,
        &chrono::Local,
        year,
        playtime::summaries::today_of(&chrono::Local, now_ms),
        now_ms,
    )
}

/// The last `days` local days of play, today included, for the library.
#[tauri::command(async)]
pub fn get_recent_play(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    days: u32,
) -> Result<RecentPlay, VaultimeError> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    playtime::summaries::recent_play(
        &db,
        &live,
        &chrono::Local,
        days.min(RECENT_PLAY_MAX_DAYS),
        playtime::summaries::today_of(&chrono::Local, now_ms),
        now_ms,
    )
}

/// Why a game's page should suggest that it counts only while no other game
/// runs, or None.
#[tauri::command(async)]
pub fn get_steps_aside_hint(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    game_id: String,
) -> Result<Option<StepsAsideHint>, VaultimeError> {
    playtime::beside::steps_aside_hint(
        &db,
        &game_id,
        &app_context.device_id,
        chrono::Utc::now().timestamp_millis(),
    )
}

/// This PC's running sessions and today's play, for the live bar.
#[derive(Debug, Serialize)]
pub struct ActiveSessions {
    pub sessions: Vec<Session>,
    /// Clock time with a game running today, as the journal counts it.
    pub played_today_ms: i64,
}

/// The running sessions as of the tracker's latest tick, and today's play.
#[tauri::command(async)]
pub fn get_active_sessions(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    app_context: State<'_, AppContext>,
) -> Result<ActiveSessions, VaultimeError> {
    let mut active = sessions::get_active_sessions(&db, &app_context.device_id)?;
    as_of_now(&db, &live, &mut active)?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let played_today_ms = playtime::summaries::played_on(
        &db,
        &live,
        &chrono::Local,
        playtime::summaries::today_of(&chrono::Local, now_ms),
        now_ms,
    )?;
    Ok(ActiveSessions {
        sessions: active,
        played_today_ms,
    })
}

/// Brings sessions to the tracker's latest tick and to what counts of them,
/// each shown as the game it counts as.
fn as_of_now(
    db: &Database,
    live: &LiveSessions,
    sessions: &mut [Session],
) -> Result<(), VaultimeError> {
    for session in sessions.iter_mut() {
        live.apply(session);
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    playtime::totals::show_counted(db, live, sessions, now_ms)?;
    show_linked_games(db, sessions)
}

/// Shows a session of a linked game, another install here or a game of
/// another PC, as one of the game it is linked to, and says which game
/// recorded it. Only the shown game changes, the session's history keeps
/// the game it was recorded with.
fn show_linked_games(db: &Database, sessions: &mut [Session]) -> Result<(), VaultimeError> {
    let links = games::links(db)?;
    if links.is_empty() {
        return Ok(());
    }
    for session in sessions {
        if let Some(linked) = links.get(&session.game_id) {
            let recorded = std::mem::replace(&mut session.game_id, linked.clone());
            session.recorded_game_id = Some(recorded);
        }
    }
    Ok(())
}

/// The events of a game's sessions and those of the games linked to it,
/// newest first, without the checkpoints, which only place time.
#[tauri::command(async)]
pub fn get_notable_events_for_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
) -> Result<Vec<SessionEvent>, VaultimeError> {
    session_events::list_notable_events_for_game(&db, &game_id)
}

/// Shows in the tray and taskbar icon whether this PC is signed in. Runs off
/// the main thread, as a redraw may wait for it.
#[tauri::command(async)]
pub fn set_cloud_signed_in(app: AppHandle, signed_in: bool) {
    crate::tray::show_cloud_state(&app, signed_in);
}

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
    let summary = with_tracking_paused(&db, &engine, &app_context.device_id, || {
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

/// Release builds talk only to the Vaultime server, whatever the page asks.
/// Development builds follow the page, so a local server can be tried.
fn cloud_api_base_url(requested: &str) -> &str {
    if cfg!(debug_assertions) {
        requested
    } else {
        CLOUD_API_BASE_URL
    }
}

/// Runs cloud work and the OS credential store on the blocking pool.
/// reqwest's blocking client must not run on the async runtime, where debug
/// builds panic and release builds hold up a worker for the whole transfer.
/// The credential store may wait for the desktop's keyring service.
async fn on_blocking_pool<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, VaultimeError> + Send + 'static,
) -> Result<T, VaultimeError> {
    tauri::async_runtime::spawn_blocking(work)
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
    on_blocking_pool(move || {
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
    take_over: bool,
) -> Result<RemoteBackupRestoreResult, VaultimeError> {
    on_blocking_pool(move || {
        let db = app.state::<Arc<Database>>();
        let device_id = app.state::<AppContext>().device_id.clone();
        let result = with_tracking_paused(&db, &app.state::<TrackingEngine>(), &device_id, || {
            backup::remote::restore_remote_backup(
                &db,
                &app.state::<AssetManager>(),
                &app.state::<AppContext>(),
                cloud_api_base_url(&api_base_url),
                &access_token,
                &account_id,
                &backup_id,
                take_over,
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
/// because the restore replaces the session tables. After a restore that
/// made this PC continue as another one, tracking waits for the restart, as
/// the tracker still records with the id it started with.
fn with_tracking_paused<T>(
    db: &Database,
    engine: &TrackingEngine,
    device_id: &str,
    restore: impl FnOnce() -> Result<T, VaultimeError>,
) -> Result<T, VaultimeError> {
    engine.pause();
    let result = match sessions::get_active_sessions(db, device_id) {
        Ok(active) if !active.is_empty() => Err(VaultimeError::Backup(
            "close all running games before restoring a backup".into(),
        )),
        Ok(_) => restore(),
        Err(error) => Err(error),
    };
    if !ledger::this_device().is_ok_and(|now| now != device_id) {
        engine.resume();
    }
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

#[tauri::command(async)]
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

#[tauri::command(async)]
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
    discovery::offer(&db, discovery::scanner::scan_folders(&db, &paths)?)
}

/// Saves every finished session as CSV or JSON and returns how many.
#[tauri::command(async)]
pub fn export_sessions(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    path: String,
    format: crate::export::ExportFormat,
) -> Result<usize, VaultimeError> {
    crate::export::export_sessions(&db, Path::new(&path), format, &app_context.app_version)
}

/// Saves the year card, a PNG image as base64, to a `.png` file.
#[tauri::command(async)]
pub fn save_share_image(path: String, png_base64: String) -> Result<(), VaultimeError> {
    crate::share::save_share_image(Path::new(&path), &png_base64)
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
#[tauri::command(async)]
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

#[tauri::command(async)]
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

/// Playtime from before Vaultime, per game the library shows.
#[tauri::command(async)]
pub fn list_earlier_playtime(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<EarlierPlaytime>, VaultimeError> {
    earlier::shown_earlier_playtime(&db)
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
    discovery::offer_launcher_games(&db, discovery::steam::discover_steam_games(&db)?)
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
    discovery::offer_launcher_games(&db, discovery::discover_launcher_games(&db)?)
}

/// The games the look at start found that are still new.
#[tauri::command(async)]
pub fn list_found_games(
    db: State<'_, Arc<Database>>,
    found: State<'_, FoundGames>,
) -> Result<NewGames, VaultimeError> {
    discovery::auto::new_games(&db, &found)
}

/// Remembers that discovery showed these programs, so the look at start
/// never offers them again. Returns how many were not shown before.
#[tauri::command(async)]
pub fn remember_offered_games(
    db: State<'_, Arc<Database>>,
    paths: Vec<String>,
) -> Result<usize, VaultimeError> {
    offered::remember(&db, &paths)
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

/// Adds the picked candidates. One marked as another install of a library
/// game counts as that game.
#[tauri::command(async)]
pub fn import_discovered_games(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    discoveries: Vec<DiscoveredGame>,
) -> Result<Vec<Game>, VaultimeError> {
    discovery::import(&db, &asset_manager, &discoveries)
}
