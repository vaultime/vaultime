// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tray icon, so tracking keeps running while the window is closed.

use log::{info, warn};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::db::connection::Database;
use crate::db::repo::settings;

/// Passed by the login item, so Vaultime starts in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";

/// Setting that decides whether closing the window keeps Vaultime in the tray.
pub const CLOSE_TO_TRAY_SETTING: &str = "close_to_tray";

/// Whether this system can show a tray icon at all.
pub struct TrayState {
    pub available: bool,
}

/// Creates the tray icon when the system supports one.
pub fn create<R: Runtime>(app: &AppHandle<R>) -> TrayState {
    if !system_supports_tray() {
        warn!("no tray on this system, install libayatana-appindicator3 for one");
        return TrayState { available: false };
    }
    match build(app) {
        Ok(()) => {
            info!("tray icon created");
            TrayState { available: true }
        }
        Err(e) => {
            warn!("failed to create the tray icon: {e}");
            TrayState { available: false }
        }
    }
}

fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Vaultime", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Vaultime", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Vaultime")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Brings the main window back from the tray or the taskbar.
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Whether closing the window should hide it instead of quitting.
pub fn close_to_tray<R: Runtime>(app: &AppHandle<R>) -> bool {
    if !app.state::<TrayState>().available {
        return false;
    }
    let db = app.state::<std::sync::Arc<Database>>();
    settings::get_setting(&db, CLOSE_TO_TRAY_SETTING)
        .ok()
        .flatten()
        .is_none_or(|value| value != "false")
}

/// Linux shows tray icons through `AppIndicator`, which is not installed
/// everywhere. Tauri panics without it, so check that it loads first.
#[cfg(target_os = "linux")]
fn system_supports_tray() -> bool {
    [
        "libayatana-appindicator3.so.1",
        "libappindicator3.so.1",
        "libayatana-appindicator3.so",
        "libappindicator3.so",
    ]
    .iter()
    // SAFETY: these libraries run no initialization code with preconditions.
    .any(|name| unsafe { libloading::Library::new(*name) }.is_ok())
}

#[cfg(not(target_os = "linux"))]
fn system_supports_tray() -> bool {
    true
}
