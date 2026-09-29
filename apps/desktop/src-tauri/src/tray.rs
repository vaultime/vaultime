// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tray icon, so tracking keeps running while the window is closed.

use log::{info, warn};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, Webview};

use crate::db::connection::Database;
use crate::db::repo::settings;

/// Passed by the login item, so Vaultime starts in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";

/// Setting that decides whether closing the window keeps Vaultime in the tray.
pub const CLOSE_TO_TRAY_SETTING: &str = "close_to_tray";

/// Tray and window icon while signed in to cloud backup, violet like the logo
/// in the app. `scripts/build-brand.py` describes how it is made.
const SIGNED_IN_ICON: &[u8] = include_bytes!("../icons/signed-in.png");
const TRAY_ID: &str = "main";
const TOOLTIP: &str = "Vaultime";
const SIGNED_IN_TOOLTIP: &str = "Vaultime, signed in to cloud backup";

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

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(TOOLTIP)
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
        let webview: &Webview<R> = window.as_ref();
        let _ = webview.show();
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Shows in the tray and the taskbar whether this PC is signed in to cloud
/// backup, the way the logo in the app does.
pub fn show_cloud_state<R: Runtime>(app: &AppHandle<R>, signed_in: bool) {
    let icon = if signed_in {
        Image::from_bytes(SIGNED_IN_ICON)
            .inspect_err(|e| warn!("failed to load the signed in icon: {e}"))
            .ok()
    } else {
        app.default_window_icon().cloned()
    };
    let Some(icon) = icon else {
        return;
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon.clone()));
        let _ = tray.set_tooltip(Some(if signed_in {
            SIGNED_IN_TOOLTIP
        } else {
            TOOLTIP
        }));
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_icon(icon);
    }
}

/// Hides the main window into the tray. Hiding the window alone leaves the
/// page running as if it were seen, so the webview is hidden too. It then
/// stops drawing and the page pauses its timers.
pub fn hide_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        let webview: &Webview<R> = window.as_ref();
        let _ = webview.hide();
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
