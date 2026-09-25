// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Vaultime core library — Tauri application setup and command registration.

pub mod assets;
pub mod backup;
pub mod cloud;
pub mod commands;
pub mod db;
pub mod error;
pub mod integrity;
pub mod platform;
pub mod tracking;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use log::info;
use tauri::Manager;

use assets::AssetManager;
use cloud::auth::AuthManager;
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
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data directory");

            fs::create_dir_all(&app_dir).expect("failed to create app data directory");
            let asset_cache_dir = app_dir.join("asset-cache");
            fs::create_dir_all(&asset_cache_dir).expect("failed to create asset cache directory");

            let db_path = app_dir.join("vaultime.db");
            let database = Arc::new(Database::open(&db_path).expect("failed to open database"));

            // Register this device.
            let device_id = machine_id();
            let platform = std::env::consts::OS.to_string();
            let version = env!("CARGO_PKG_VERSION").to_string();
            devices::ensure_device(&database, &device_id, &platform, &version)
                .expect("failed to register device");
            info!("device registered: {device_id} ({platform} v{version})");

            // Start tracking engine.
            let engine = TrackingEngine::start(Arc::clone(&database), device_id.clone());

            // Initialize cloud auth manager.
            let auth_manager = AuthManager::new(app_dir.clone());

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
            app.manage(auth_manager);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::list_settings,
            commands::set_setting,
            commands::get_tracking_status,
            commands::get_tracking_diagnostics,
            commands::cloud_sign_up,
            commands::cloud_sign_in,
            commands::cloud_sign_out,
            commands::cloud_refresh_token,
            commands::cloud_get_session,
            commands::cloud_get_config,
            commands::cloud_register_device,
            commands::cloud_sync_events,
            commands::cloud_get_unsynced_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vaultime");
}

/// Returns a stable identifier for this machine.
///
/// Uses the hostname as a simple device identifier. A more robust approach
/// would use a persisted UUID, but this is sufficient for the local-first MVP.
fn machine_id() -> String {
    hostname::get().map_or_else(
        |_| uuid::Uuid::new_v4().to_string(),
        |h| h.to_string_lossy().into_owned(),
    )
}
