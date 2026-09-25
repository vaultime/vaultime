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

use tauri::Manager;

use db::connection::Database;

/// Runs the Tauri application.
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data directory");

            fs::create_dir_all(&app_dir)
                .expect("failed to create app data directory");

            let db_path = app_dir.join("vaultime.db");
            let database = Database::open(&db_path)
                .expect("failed to open database");

            app.manage(database);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_games,
            commands::get_game,
            commands::create_game,
            commands::update_game,
            commands::delete_game,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vaultime");
}
