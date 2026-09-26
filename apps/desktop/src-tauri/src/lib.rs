// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tauri application setup and command registration.

pub mod assets;
pub mod backup;
pub mod commands;
pub mod constants;
pub mod db;
pub mod discovery;
pub mod error;
pub mod hex;
pub mod integrity;
pub mod platform;
pub mod secure_storage;
pub mod tracking;
pub mod tray;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use log::{LevelFilter, info};
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use assets::AssetManager;
use constants::{LOG_FILES_KEPT, LOG_MAX_FILE_BYTES};
use db::connection::Database;
use db::repo::devices;
use tracking::engine::TrackingEngine;

#[derive(Debug, Clone)]
pub struct AppContext {
    pub app_dir: PathBuf,
    pub asset_cache_dir: PathBuf,
    pub db_path: PathBuf,
    pub device_id: String,
    pub app_version: String,
}

/// Runs the Tauri application.
pub fn run() {
    tauri::Builder::default()
        // Must be registered first. A second instance would track every game twice.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(LevelFilter::Info)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .max_file_size(LOG_MAX_FILE_BYTES)
                .rotation_strategy(RotationStrategy::KeepSome(LOG_FILES_KEPT))
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
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
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
                && tray::close_to_tray(window.app_handle())
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_version,
            commands::load_cloud_session_secure,
            commands::has_cloud_backup_key_secure,
            commands::store_cloud_session_secure,
            commands::clear_cloud_session_secure,
            commands::store_cloud_backup_key_secure,
            commands::clear_cloud_backup_key_secure,
            commands::list_games,
            commands::get_game,
            commands::create_game,
            commands::update_game,
            commands::delete_game,
            commands::list_game_assets,
            commands::list_preferred_game_assets,
            commands::scan_game_assets,
            commands::import_game_asset,
            commands::set_preferred_game_asset,
            commands::list_sessions,
            commands::get_sessions_for_game,
            commands::get_active_sessions,
            commands::get_session_events_for_game,
            commands::list_backup_snapshots,
            commands::export_local_backup,
            commands::inspect_local_backup,
            commands::import_local_backup,
            commands::upload_remote_backup,
            commands::restore_remote_backup,
            commands::list_settings,
            commands::set_setting,
            commands::get_tracking_diagnostics,
            commands::tray_available,
            commands::discover_games,
            commands::discover_steam_games,
            commands::get_default_scan_paths,
            commands::import_discovered_games,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Vaultime")
        .run(|app, event| {
            if let RunEvent::Exit = event
                && let Some(engine) = app.try_state::<TrackingEngine>()
            {
                engine.shutdown();
            }
        });
}

/// Opens the database, registers this device, starts tracking and creates the tray.
fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_dir = app
        .path()
        .app_data_dir()
        .expect("failed to resolve app data directory");

    fs::create_dir_all(&app_dir).expect("failed to create app data directory");
    let asset_cache_dir = app_dir.join("asset-cache");
    fs::create_dir_all(&asset_cache_dir).expect("failed to create asset cache directory");

    let db_path = app_dir.join("vaultime.db");
    let database = Arc::new(Database::open(&db_path).expect("failed to open database"));

    let device_id = machine_id();
    let platform = std::env::consts::OS.to_string();
    let version = env!("CARGO_PKG_VERSION").to_string();
    devices::ensure_device(&database, &device_id, &platform, &version)
        .expect("failed to register device");
    info!("device registered: {device_id} ({platform} v{version})");

    let engine = TrackingEngine::start(Arc::clone(&database), device_id.clone());

    app.manage(AppContext {
        app_dir,
        asset_cache_dir: asset_cache_dir.clone(),
        db_path,
        device_id,
        app_version: version,
    });
    app.manage(database);
    app.manage(AssetManager::new(asset_cache_dir));
    app.manage(engine);

    // The window starts hidden. A login item starts in the tray, when there is one.
    let tray = tray::create(app.handle());
    let start_in_tray = tray.available && std::env::args().any(|arg| arg == tray::MINIMIZED_ARG);
    app.manage(tray);
    if !start_in_tray {
        tray::show_main_window(app.handle());
    }

    Ok(())
}

/// Identifier for this machine. The hostname is used so existing session rows
/// keep matching, with a random id as the fallback.
fn machine_id() -> String {
    hostname::get().map_or_else(
        |_| uuid::Uuid::new_v4().to_string(),
        |h| h.to_string_lossy().into_owned(),
    )
}
