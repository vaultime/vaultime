// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tauri application setup and command registration.

pub mod appearance;
pub mod assets;
pub mod backup;
pub mod commands;
pub mod constants;
pub mod db;
pub mod discovery;
pub mod earlier;
pub mod error;
pub mod export;
pub mod hex;
pub mod integrity;
pub mod platform;
pub mod playtime;
pub mod secure_storage;
pub mod tracking;
pub mod tray;
pub mod window_look;
pub mod window_size;

#[cfg(test)]
mod perf;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::{LevelFilter, info};
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use appearance::BackgroundStore;
use assets::AssetManager;
use constants::{
    APPEARANCE_DIR, ASSET_CACHE_DIR, AUTO_BACKUP_CHECK_INTERVAL, AUTO_BACKUP_INTERVAL,
    AUTO_BACKUP_ON_QUIT_MIN_AGE, DATABASE_FILE, DEVICE_ID_FILE, LIBRARY_CHANGED_EVENT,
    LOG_FILES_KEPT, LOG_MAX_FILE_BYTES,
};
use db::connection::Database;
use db::repo::devices;
use tracking::engine::TrackingEngine;
use tracking::live::LiveSessions;

#[derive(Debug, Clone)]
pub struct AppContext {
    pub app_dir: PathBuf,
    pub device_id: String,
    pub app_version: String,
}

pub fn run() {
    tauri::Builder::default()
        // Must be registered first. A second instance would track every game twice.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(log_plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .arg(tray::MINIMIZED_ARG)
                .build(),
        )
        .setup(setup)
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                WindowEvent::CloseRequested { api, .. }
                    if tray::close_to_tray(window.app_handle()) =>
                {
                    api.prevent_close();
                    tray::hide_main_window(window.app_handle());
                }
                WindowEvent::Resized(_) => window_size::on_main_resized(window.app_handle()),
                _ => {}
            }
        })
        .invoke_handler(command_handler())
        .build(tauri::generate_context!())
        .expect("error while building Vaultime")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(engine) = app.try_state::<TrackingEngine>() {
                    engine.shutdown();
                }
                back_up_on_quit(app);
            }
        });
}

/// Logs to the console and to rotated files in the app's log folder.
fn log_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_log::Builder::new()
        .level(LevelFilter::Info)
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: None }),
        ])
        .max_file_size(LOG_MAX_FILE_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(LOG_FILES_KEPT))
        .build()
}

/// Every command the page may call.
fn command_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::get_app_version,
        commands::get_device_id,
        commands::get_this_pc,
        commands::rename_this_pc,
        commands::list_devices,
        commands::list_game_links,
        commands::link_game,
        commands::preview_merge,
        commands::merge_backup,
        commands::load_cloud_session_secure,
        commands::has_cloud_backup_key_secure,
        commands::store_cloud_session_secure,
        commands::clear_cloud_session_secure,
        commands::store_cloud_backup_key_secure,
        commands::clear_cloud_backup_key_secure,
        commands::list_games,
        commands::create_game,
        commands::update_game,
        commands::delete_game,
        commands::list_game_assets,
        commands::list_preferred_game_assets,
        commands::scan_game_assets,
        commands::open_artwork_file,
        commands::open_game_asset_source,
        commands::import_game_asset,
        commands::crop_game_asset,
        commands::delete_game_asset,
        commands::set_preferred_game_asset,
        commands::list_sessions,
        commands::get_active_sessions,
        commands::get_play_totals,
        commands::set_game_steps_aside,
        commands::list_ignored_programs,
        commands::ignore_program,
        commands::allow_program,
        commands::get_session_events_for_game,
        commands::list_backup_snapshots,
        commands::get_auto_backup_folder,
        commands::export_local_backup,
        commands::inspect_local_backup,
        commands::import_local_backup,
        commands::upload_remote_backup,
        commands::restore_remote_backup,
        commands::list_settings,
        commands::set_setting,
        commands::get_tracking_diagnostics,
        commands::tray_available,
        commands::get_window_size,
        commands::set_window_size,
        commands::set_cloud_signed_in,
        commands::discover_games,
        commands::discover_steam_games,
        commands::discover_launcher_games,
        commands::get_default_scan_paths,
        commands::import_discovered_games,
        commands::list_earlier_playtime,
        commands::export_sessions,
        commands::trim_session,
        commands::preview_trim,
        commands::discard_session,
        commands::add_manual_session,
        commands::list_status_changes,
        commands::set_game_status,
        commands::list_session_notes,
        commands::set_session_note,
        commands::preview_steam_playtime,
        commands::import_steam_playtime,
        commands::remove_steam_playtime,
        appearance::get_background_image,
        appearance::set_background_image,
        appearance::set_background_from_game,
        appearance::clear_background_image,
        window_look::set_window_look,
    ]
}

/// Opens the database, registers this device, starts tracking and creates the tray.
fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_dir = app
        .path()
        .app_data_dir()
        .expect("failed to resolve app data directory");

    fs::create_dir_all(&app_dir).expect("failed to create app data directory");
    let asset_cache_dir = app_dir.join(ASSET_CACHE_DIR);
    let appearance_dir = app_dir.join(APPEARANCE_DIR);
    fs::create_dir_all(&asset_cache_dir).expect("failed to create asset cache directory");

    let db_path = app_dir.join(DATABASE_FILE);
    let first_start = !db_path.exists();
    let device_id = device_id(&app_dir, !first_start);
    let database = Arc::new(Database::open(&db_path).expect("failed to open database"));
    app.manage(window_look::WindowLookState::load(&database));

    let platform = std::env::consts::OS.to_string();
    let version = env!("CARGO_PKG_VERSION").to_string();
    devices::ensure_device(&database, &device_id, &platform, &version)
        .expect("failed to register device");
    info!("device registered: {device_id} ({platform} v{version})");
    integrity::ledger::load_key(&app_dir, &device_id).expect("failed to load the ledger key");
    let pc_name = hostname::get().map_or_else(
        |_| platform.clone(),
        |name| name.to_string_lossy().into_owned(),
    );
    devices::describe_this_device(
        &database,
        &device_id,
        &integrity::ledger::key_id().expect("ledger key loaded"),
        &pc_name,
    )
    .expect("failed to describe this PC");
    match database.with_transaction(|conn| integrity::ledger::begin_if_empty(conn, "first_start")) {
        Ok(Some(count)) => info!("the ledger of this PC began with {count} sessions"),
        Ok(None) => {}
        Err(error) => log::warn!("the ledger of this PC did not begin: {error}"),
    }

    // Before the tracker starts, so no session ends halfway through.
    match playtime::slices::ensure_current(&database) {
        Ok(0) => {}
        Ok(count) => info!("worked out the play slices of {count} sessions"),
        Err(error) => log::warn!("play slices not built: {error}"),
    }

    let live = Arc::new(LiveSessions::default());
    let engine = TrackingEngine::start(Arc::clone(&database), device_id.clone(), Arc::clone(&live));

    app.manage(AppContext {
        app_dir,
        device_id,
        app_version: version,
    });
    start_automatic_backups(
        Arc::clone(&database),
        AssetManager::new(asset_cache_dir.clone()),
        app.state::<AppContext>().inner().clone(),
    );

    // Existing Steam games pick up Steam's covers without a manual scan.
    let backfill_db = Arc::clone(&database);
    let backfill_assets = AssetManager::new(asset_cache_dir.clone());
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        match assets::backfill_steam_covers(&backfill_db, &backfill_assets) {
            Ok(0) => {}
            Ok(count) => {
                info!("scanned {count} Steam games for their covers");
                let _ = handle.emit(LIBRARY_CHANGED_EVENT, ());
            }
            Err(error) => log::warn!("artwork backfill failed: {error}"),
        }
        match discovery::backfill_launcher_ids(&backfill_db) {
            Ok(0) => {}
            Ok(count) => info!("stored the launcher ids of {count} games"),
            Err(error) => log::warn!("launcher id backfill failed: {error}"),
        }
        match assets::sweep_cache(&backfill_db, &backfill_assets) {
            Ok(0) => {}
            Ok(count) => info!("removed {count} cached images no game uses"),
            Err(error) => log::warn!("artwork cache sweep failed: {error}"),
        }
    });

    app.manage(database);
    app.manage(AssetManager::new(asset_cache_dir));
    app.manage(BackgroundStore::new(appearance_dir));
    app.manage(engine);
    app.manage(live);

    // New installs start with the system, so the first game of the day counts.
    // Later starts point a login item at this executable, so it never keeps
    // starting an older copy. Development builds leave the login items alone.
    if !cfg!(debug_assertions) {
        if first_start {
            enable_autostart(app.handle());
        } else {
            refresh_autostart(app.handle());
        }
    }

    // The window starts hidden and a login item leaves it that way, in the tray
    // or without one. Opening Vaultime again shows the window of this instance.
    // It takes its size while still hidden, so it never shows at another one.
    app.manage(tray::create(app.handle()));
    window_look::show_stored(app.handle());
    window_size::fit_main_window(app.handle(), true);
    if std::env::args().any(|arg| arg == tray::MINIMIZED_ARG) {
        tray::hide_main_window(app.handle());
    } else {
        tray::show_main_window(app.handle());
    }

    Ok(())
}

/// Checks every `AUTO_BACKUP_CHECK_INTERVAL` whether the daily automatic
/// backup is due.
fn start_automatic_backups(database: Arc<Database>, assets: AssetManager, context: AppContext) {
    let spawned = std::thread::Builder::new()
        .name("vaultime-auto-backup".into())
        .spawn(move || {
            loop {
                if let Err(error) =
                    backup::auto::back_up_if_due(&database, &assets, &context, AUTO_BACKUP_INTERVAL)
                {
                    log::warn!("automatic backup failed: {error}");
                }
                std::thread::sleep(AUTO_BACKUP_CHECK_INTERVAL);
            }
        });
    if let Err(error) = spawned {
        log::warn!("could not start automatic backups: {error}");
    }
}

/// Quitting saves the day's play, unless the last backup is younger than
/// `AUTO_BACKUP_ON_QUIT_MIN_AGE`.
fn back_up_on_quit(app: &tauri::AppHandle) {
    let (Some(database), Some(assets), Some(context)) = (
        app.try_state::<Arc<Database>>(),
        app.try_state::<AssetManager>(),
        app.try_state::<AppContext>(),
    ) else {
        return;
    };
    if let Err(error) =
        backup::auto::back_up_if_due(&database, &assets, &context, AUTO_BACKUP_ON_QUIT_MIN_AGE)
    {
        log::warn!("automatic backup on quit failed: {error}");
    }
}

fn enable_autostart(app: &tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;

    // The autostart library creates ~/.config/autostart but not ~/.config.
    #[cfg(target_os = "linux")]
    if let Some(home) = dirs::home_dir() {
        let _ = fs::create_dir_all(home.join(".config").join("autostart"));
    }

    match app.autolaunch().enable() {
        Ok(()) => info!("new install, starts with the system from now on"),
        Err(error) => log::warn!("could not turn on starting with the system: {error}"),
    }
}

/// The login item stores the path of the executable that enabled it, which an
/// install to another folder leaves pointing at the old one.
fn refresh_autostart(app: &tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;

    let autolaunch = app.autolaunch();
    if !autolaunch.is_enabled().unwrap_or(false) {
        return;
    }
    if let Err(error) = autolaunch.enable() {
        log::warn!("could not update the login item: {error}");
    }
}

/// Identifier for this PC, kept in a file next to the database. A backup
/// restored from another PC therefore never brings that PC's id along, and a
/// new hostname does not make a new device. Installs from before the file
/// keep their hostname, which their sessions already use.
fn device_id(app_dir: &Path, existing_install: bool) -> String {
    let path = app_dir.join(DEVICE_ID_FILE);
    if let Ok(saved) = fs::read_to_string(&path)
        && !saved.trim().is_empty()
    {
        return saved.trim().to_owned();
    }

    let id = existing_install
        .then(|| hostname::get().ok())
        .flatten()
        .map_or_else(
            || uuid::Uuid::new_v4().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
    if let Err(error) = fs::write(&path, &id) {
        log::warn!("could not save the device id: {error}");
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_is_created_once_and_kept() {
        let dir =
            std::env::temp_dir().join(format!("vaultime-device-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();

        let fresh = device_id(&dir, false);
        assert!(uuid::Uuid::parse_str(&fresh).is_ok());
        assert_eq!(device_id(&dir, true), fresh);

        fs::remove_file(dir.join(DEVICE_ID_FILE)).unwrap();
        let existing = device_id(&dir, true);
        assert_eq!(existing, hostname::get().unwrap().to_string_lossy());
        fs::remove_dir_all(dir).unwrap();
    }
}
