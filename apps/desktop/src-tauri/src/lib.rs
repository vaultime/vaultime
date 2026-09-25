// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Vaultime core library — Tauri application setup and command registration.

pub mod assets;
pub mod cloud;
pub mod commands;
pub mod db;
pub mod error;
pub mod integrity;
pub mod platform;
pub mod tracking;

use std::fs;
use std::sync::Arc;

use log::info;
use tauri::Manager;

use db::connection::Database;
use db::repo::devices;
use tracking::engine::TrackingEngine;

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

            fs::create_dir_all(&app_dir)
                .expect("failed to create app data directory");

            let db_path = app_dir.join("vaultime.db");
            let database = Arc::new(
                Database::open(&db_path).expect("failed to open database"),
            );

            // Register this device.
            let device_id = machine_id();
            let platform = std::env::consts::OS.to_string();
            let version = env!("CARGO_PKG_VERSION").to_string();
            devices::ensure_device(&database, &device_id, &platform, &version)
                .expect("failed to register device");
            info!("device registered: {device_id} ({platform} v{version})");

            // Start tracking engine.
            let engine = TrackingEngine::start(
                Arc::clone(&database),
                device_id,
            );

            app.manage(database);
            app.manage(engine);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_games,
            commands::get_game,
            commands::create_game,
            commands::update_game,
            commands::delete_game,
            commands::list_sessions,
            commands::get_sessions_for_game,
            commands::get_active_sessions,
            commands::get_tracking_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vaultime");
}

/// Returns a stable identifier for this machine.
///
/// Uses the hostname as a simple device identifier. A more robust approach
/// would use a persisted UUID, but this is sufficient for the local-first MVP.
fn machine_id() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
}
