// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The app itself: its version, settings, window and what the tracker detects.

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppContext;
use crate::constants::{PAGE_SETTINGS, POLL_INTERVAL, WINDOW_SIZE_SETTING};
use crate::db::connection::Database;
use crate::db::models::Setting;
use crate::db::repo::settings;
use crate::error::VaultimeError;
use crate::platform::activity::{foreground_detection_strategy, idle_detection_strategy};
use crate::platform::controller;
use crate::tracking::engine::TrackingEngine;
use crate::window_size::{self, WindowSizeState};

#[tauri::command]
pub fn get_app_version(app_context: State<'_, AppContext>) -> Result<String, VaultimeError> {
    Ok(app_context.app_version.clone())
}

/// Whether this is a development build. It keeps its cloud sign-in apart
/// from the installed app's but shares its settings, so what its own
/// sign-in says must not decide a setting for both.
#[tauri::command]
pub fn is_development_build() -> bool {
    cfg!(debug_assertions)
}

#[tauri::command]
pub fn get_device_id(app_context: State<'_, AppContext>) -> Result<String, VaultimeError> {
    Ok(app_context.device_id.clone())
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

/// Saves the year card, a PNG image as base64, to a `.png` file.
#[tauri::command(async)]
pub fn save_share_image(path: String, png_base64: String) -> Result<(), VaultimeError> {
    crate::share::save_share_image(Path::new(&path), &png_base64)
}
