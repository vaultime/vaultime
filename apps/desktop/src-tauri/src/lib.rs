// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Vaultime core library — Tauri application setup and command registration.

pub mod assets;
pub mod cloud;
pub mod db;
pub mod error;
pub mod integrity;
pub mod platform;
pub mod tracking;

/// Runs the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running Vaultime");
}

/// Temporary greeting command to verify IPC bridge works.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! Vaultime is running.", name)
}
