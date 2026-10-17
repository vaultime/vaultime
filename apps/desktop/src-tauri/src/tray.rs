// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tray icon, so tracking keeps running while the window is closed.

use std::sync::Arc;

use chrono::{DateTime, Local, TimeDelta, Utc};
use log::{debug, info, warn};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, Webview};

use crate::constants::{CLOSE_TO_TRAY_SETTING, TRAY_STATUS_INTERVAL, TRAY_TODAY_MIN_PLAYED_MS};
use crate::db::connection::Database;
use crate::db::repo::sessions::{self, SessionSpan};
use crate::db::repo::{map_db, settings};
use crate::error::Result;
use crate::integrity;
use crate::playtime::{summaries, totals};
use crate::tracking::live::LiveSessions;
use crate::window_look::{WindowLookState, draw_icon};

/// Passed by the login item, so Vaultime starts in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";

/// Tray and window icon while signed in to cloud backup, in violet, until the
/// page tells the core its accent. `scripts/build-brand.py` describes how it
/// is made.
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
        Ok((playing, today)) => {
            info!("tray icon created");
            start_status_updates(app.clone(), playing, today);
            TrayState { available: true }
        }
        Err(error) => {
            warn!("failed to create the tray icon: {error}");
            TrayState { available: false }
        }
    }
}

/// Builds the tray icon and returns the two status lines of its menu.
fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<(MenuItem<R>, MenuItem<R>)> {
    let playing = MenuItem::with_id(app, "playing", NOTHING_RUNNING, false, None::<&str>)?;
    let today = MenuItem::with_id(app, "today", NOTHING_TODAY, false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Vaultime", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Vaultime", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&playing, &today, &separator, &open, &quit])?;

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
    Ok((playing, today))
}

const NOTHING_RUNNING: &str = "No game running";
const NOTHING_TODAY: &str = "Nothing played today";

/// Keeps the status lines of the tray menu current. They change at most once
/// a minute, so the menu is only touched when their text does.
fn start_status_updates<R: Runtime>(app: AppHandle<R>, playing: MenuItem<R>, today: MenuItem<R>) {
    let db = Arc::clone(&app.state::<Arc<Database>>());
    let live = Arc::clone(&app.state::<Arc<LiveSessions>>());
    let device_id = app.state::<crate::AppContext>().device_id.clone();
    let spawned = std::thread::Builder::new()
        .name("vaultime-tray".into())
        .spawn(move || {
            let mut shown = (NOTHING_RUNNING.to_string(), NOTHING_TODAY.to_string());
            loop {
                match status_lines(&db, &live, &device_id, Utc::now()) {
                    Ok(lines) if lines != shown => {
                        let _ = playing.set_text(&lines.0);
                        let _ = today.set_text(&lines.1);
                        shown = lines;
                    }
                    Ok(_) => {}
                    Err(error) => debug!("tray status not updated: {error}"),
                }
                std::thread::sleep(TRAY_STATUS_INTERVAL);
            }
        });
    if let Err(error) = spawned {
        warn!("could not start the tray status: {error}");
    }
}

fn status_lines(
    db: &Database,
    live: &LiveSessions,
    device_id: &str,
    now: DateTime<Utc>,
) -> Result<(String, String)> {
    let since = integrity::format_timestamp(local_day_start(now));
    let mut spans = sessions::spans_since(db, &since, device_id)?;
    spans.retain(|span| span.ended_at_wall.is_none());
    let now_ms = now.timestamp_millis();
    // The time the live bar shows, without what a launcher set aside.
    let mut running = db.with_conn(|conn| {
        spans
            .iter()
            .map(|span| sessions::stored_session(conn, &span.session_id).map_err(map_db))
            .collect::<Result<Vec<_>>>()
    })?;
    for session in &mut running {
        live.apply(session);
    }
    totals::show_counted(db, live, &mut running, now_ms)?;
    for (span, session) in spans.iter_mut().zip(&running) {
        span.runtime_ms = session.runtime_ms;
    }
    let played = summaries::played_on(
        db,
        live,
        &Local,
        summaries::today_of(&Local, now_ms),
        now_ms,
    )?;
    Ok(describe_status(&spans, played))
}

/// The first moment of the local day `now` falls in. Where the clocks jump
/// forward at midnight, the day starts at the first hour that exists.
fn local_day_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let day = now.with_timezone(&Local).date_naive();
    (0..2)
        .filter_map(|hour| day.and_hms_opt(hour, 0, 0))
        .find_map(|start| start.and_local_timezone(Local).earliest())
        .map_or(now, |start| start.with_timezone(&Utc))
}

/// "Playing Elden Ring, 1 h 24" and "2 h 40 played today", from the
/// running sessions and today's play as the journal counts it.
fn describe_status(running: &[SessionSpan], played_today_ms: i64) -> (String, String) {
    // Two linked installs of one game running at once show as one game.
    let mut shown: Vec<&SessionSpan> = Vec::new();
    for span in running {
        if !shown
            .iter()
            .any(|other| other.game_title == span.game_title)
        {
            shown.push(span);
        }
    }
    let playing = match shown.as_slice() {
        [] => NOTHING_RUNNING.to_string(),
        [only] => format!(
            "Playing {}, {}",
            only.game_title,
            format_hours_minutes(only.runtime_ms)
        ),
        [first @ .., last] => {
            let names: Vec<&str> = first.iter().map(|span| span.game_title.as_str()).collect();
            format!("Playing {} and {}", names.join(", "), last.game_title)
        }
    };
    let today = if played_today_ms < TRAY_TODAY_MIN_PLAYED_MS {
        NOTHING_TODAY.to_string()
    } else {
        format!("{} played today", format_hours_minutes(played_today_ms))
    };
    (playing, today)
}

/// "1 h 24" or "40 min". Same as `formatHoursMinutes` in `lib/time.ts`.
fn format_hours_minutes(ms: i64) -> String {
    let time = TimeDelta::milliseconds(ms.max(0));
    if time < TimeDelta::hours(1) {
        return format!("{} min", time.num_minutes());
    }
    let minutes = (time - TimeDelta::hours(time.num_hours())).num_minutes();
    format!("{} h {minutes:02}", time.num_hours())
}

/// Brings the main window back from the tray or the taskbar.
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let webview: &Webview<R> = window.as_ref();
        let _ = webview.show();
        let _ = window.show();
        let _ = window.unminimize();
        // The screen may have changed while the window was away.
        crate::window_size::fit_main_window(app, false);
        let _ = window.set_focus();
    }
}

/// Shows in the tray and the taskbar whether this PC is signed in to cloud
/// backup, the way the logo in the app does, in the accent of the app.
pub fn show_cloud_state<R: Runtime>(app: &AppHandle<R>, signed_in: bool) {
    if let Some(state) = app.try_state::<WindowLookState>() {
        state.set_signed_in(signed_in);
    }
    crate::window_look::redraw(app);
}

/// Puts the logo in `accent` on the tray, the taskbar and the title bar, or
/// the bundled logo until the page has told the core its accent.
pub fn show_icon<R: Runtime>(app: &AppHandle<R>, accent: Option<&str>, signed_in: bool) {
    let icon = accent
        .and_then(|accent| draw_icon(accent, signed_in))
        .or_else(|| {
            if signed_in {
                Image::from_bytes(SIGNED_IN_ICON)
                    .inspect_err(|error| warn!("failed to load the signed in icon: {error}"))
                    .ok()
            } else {
                app.default_window_icon().cloned()
            }
        });
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
        #[cfg(windows)]
        crate::window_look::set_taskbar_icon(&window, &icon);
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

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE_MS: i64 = 60_000;

    fn running(title: &str, minutes: i64) -> SessionSpan {
        SessionSpan {
            session_id: format!("{title}-{minutes}"),
            game_title: title.into(),
            started_at_wall: "2026-09-30T18:00:00.000Z".into(),
            ended_at_wall: None,
            runtime_ms: minutes * MINUTE_MS,
        }
    }

    #[test]
    fn names_the_running_game_and_the_day() {
        let (playing, today) = describe_status(&[running("Elden Ring", 84)], 144 * MINUTE_MS);
        assert_eq!(playing, "Playing Elden Ring, 1 h 24");
        assert_eq!(today, "2 h 24 played today");
        assert_eq!(
            describe_status(&[], 0),
            (NOTHING_RUNNING.into(), NOTHING_TODAY.into())
        );
    }

    #[test]
    fn lists_games_running_side_by_side() {
        let spans = [
            running("Elden Ring", 60),
            running("Balatro", 30),
            running("Celeste", 10),
        ];
        let (playing, _) = describe_status(&spans, 60 * MINUTE_MS);
        assert_eq!(playing, "Playing Elden Ring, Balatro and Celeste");
    }

    #[test]
    fn two_installs_of_one_game_show_once() {
        let spans = [running("Cyberpunk 2077", 60), running("Cyberpunk 2077", 30)];
        let (playing, _) = describe_status(&spans, 60 * MINUTE_MS);
        assert_eq!(playing, "Playing Cyberpunk 2077, 1 h 00");
    }

    #[test]
    fn a_launcher_shows_without_the_time_it_set_aside() {
        use crate::playtime::testing::{Mark, database, game, recorded, utc};
        use crate::tracking::live::LiveCounters;

        // The PC the tests act as.
        let here = integrity::ledger::this_device().unwrap();
        let db = database();
        crate::db::repo::devices::ensure_device(&db, &here, "test", "0.1.0").unwrap();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        crate::db::repo::games::set_steps_aside(&db, &client, true).unwrap();
        let start = utc("2026-10-02T09:00:00Z");
        let session = sessions::create_session_at(&db, &client, &here, start).unwrap();
        // A match beside it for the last hour of its three.
        recorded(
            &db,
            &match_game,
            &here,
            "2026-10-02T11:00:00Z",
            &[Mark::Check(60, 60, 60)],
        );
        let live = LiveSessions::default();
        let runtime_ms = 180 * MINUTE_MS;
        live.update(
            &session.id,
            LiveCounters {
                runtime_ms,
                active_ms: runtime_ms,
                idle_ms: 0,
            },
        );
        let (playing, _) =
            status_lines(&db, &live, &here, start + TimeDelta::minutes(180)).unwrap();
        assert_eq!(playing, "Playing Client, 2 h 00");
    }

    #[test]
    fn the_day_starts_before_now() {
        let now = Utc::now();
        let start = local_day_start(now);
        assert!(start <= now);
        assert!(now - start < TimeDelta::days(1));
    }

    #[test]
    fn formats_like_the_app() {
        assert_eq!(format_hours_minutes(0), "0 min");
        assert_eq!(format_hours_minutes(40 * MINUTE_MS + 59_999), "40 min");
        assert_eq!(format_hours_minutes(60 * MINUTE_MS), "1 h 00");
        assert_eq!(format_hours_minutes(84 * MINUTE_MS), "1 h 24");
        assert_eq!(format_hours_minutes(-5), "0 min");
    }
}
