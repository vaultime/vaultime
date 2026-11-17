// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tray icon, so tracking keeps running while the window is closed.

use std::sync::Arc;

use chrono::{DateTime, Local, TimeDelta, Utc};
use log::{debug, info, warn};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime, Webview};

use crate::constants::{
    CLOSE_TO_TRAY_SETTING, LIBRARY_CHANGED_EVENT, TRAY_MIN_PLAYED_MS, TRAY_RECENT_DAYS,
    TRAY_STATUS_INTERVAL,
};
use crate::db::connection::Database;
use crate::db::repo::sessions::{self, SessionSpan};
use crate::db::repo::{map_db, settings};
use crate::error::Result;
use crate::integrity;
use crate::playtime::{summaries, totals};
use crate::tracking::live::LiveSessions;
use crate::window_look::draw_icon;

/// Passed by the login item, so Vaultime starts in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";

const TRAY_ID: &str = "main";
const TOOLTIP: &str = "Vaultime";

/// Whether this system can show a tray icon at all.
pub struct TrayState {
    pub available: bool,
    /// Whether the desktop shows the icon. GNOME without an `AppIndicator`
    /// extension runs it unseen.
    pub shown: bool,
}

/// Where closing the window leaves Vaultime, for the settings page.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayStatus {
    /// In the tray.
    Shown,
    /// Minimized, as the desktop shows no tray icons.
    Hidden,
    /// Nowhere, closing the window quits.
    None,
}

impl TrayState {
    pub fn status(&self) -> TrayStatus {
        match (self.available, self.shown) {
            (false, _) => TrayStatus::None,
            (true, false) => TrayStatus::Hidden,
            (true, true) => TrayStatus::Shown,
        }
    }
}

/// Creates the tray icon when the system supports one.
pub fn create<R: Runtime>(app: &AppHandle<R>) -> TrayState {
    let missing = TrayState {
        available: false,
        shown: false,
    };
    if !system_supports_tray() {
        warn!("no tray on this system, install libayatana-appindicator3 for one");
        return missing;
    }
    match build(app) {
        Ok(items) => {
            let shown = desktop_shows_tray();
            if shown {
                info!("tray icon created");
            } else {
                warn!("tray icon created, but this desktop shows none, so closing minimizes");
            }
            start_status_updates(app.clone(), items);
            TrayState {
                available: true,
                shown,
            }
        }
        Err(error) => {
            warn!("failed to create the tray icon: {error}");
            missing
        }
    }
}

/// The status lines of the tray menu and their menu items.
struct StatusItems<R: Runtime> {
    playing: MenuItem<R>,
    today: MenuItem<R>,
    recent: MenuItem<R>,
}

/// What the status lines of the tray menu say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusLines {
    playing: String,
    today: String,
    recent: String,
}

impl Default for StatusLines {
    fn default() -> Self {
        describe_status(&[], 0, 0)
    }
}

/// Builds the tray icon and returns the status lines of its menu.
fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<StatusItems<R>> {
    let lines = StatusLines::default();
    let playing = MenuItem::with_id(app, "playing", &lines.playing, false, None::<&str>)?;
    let today = MenuItem::with_id(app, "today", &lines.today, false, None::<&str>)?;
    let recent = MenuItem::with_id(app, "recent", &lines.recent, false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Vaultime", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Vaultime", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&playing, &today, &recent, &separator, &open, &quit])?;

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
    Ok(StatusItems {
        playing,
        today,
        recent,
    })
}

const NOTHING_RUNNING: &str = "No game running";
const NOTHING_TODAY: &str = "Nothing played today";

/// Keeps the status lines of the tray menu current. They change at most once
/// a minute, so the menu is only touched when their text does. A session that
/// starts or ends reloads the library of the page, which polls only while it
/// is seen, so the window and its icons follow the game played.
fn start_status_updates<R: Runtime>(app: AppHandle<R>, items: StatusItems<R>) {
    let db = Arc::clone(&app.state::<Arc<Database>>());
    let live = Arc::clone(&app.state::<Arc<LiveSessions>>());
    let device_id = app.state::<crate::AppContext>().device_id.clone();
    let spawned = std::thread::Builder::new()
        .name("vaultime-tray".into())
        .spawn(move || {
            let mut shown = StatusLines::default();
            let mut running: Option<Vec<String>> = None;
            loop {
                match status_lines(&db, &live, &device_id, Utc::now()) {
                    Ok((lines, now_running)) => {
                        if lines != shown {
                            let _ = items.playing.set_text(&lines.playing);
                            let _ = items.today.set_text(&lines.today);
                            let _ = items.recent.set_text(&lines.recent);
                            shown = lines;
                        }
                        if running
                            .as_ref()
                            .is_some_and(|before| *before != now_running)
                        {
                            let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
                        }
                        running = Some(now_running);
                    }
                    Err(error) => debug!("tray status not updated: {error}"),
                }
                std::thread::sleep(TRAY_STATUS_INTERVAL);
            }
        });
    if let Err(error) = spawned {
        warn!("could not start the tray status: {error}");
    }
}

/// The status lines and the ids of the sessions running on this PC, sorted.
pub(crate) fn status_lines(
    db: &Database,
    live: &LiveSessions,
    device_id: &str,
    now: DateTime<Utc>,
) -> Result<(StatusLines, Vec<String>)> {
    let since = integrity::format_timestamp(local_day_start(now));
    let mut spans = sessions::spans_since(db, &since, device_id)?;
    spans.retain(|span| span.ended_at_wall.is_none());
    let mut running_ids: Vec<String> = spans.iter().map(|span| span.session_id.clone()).collect();
    running_ids.sort();
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
    let today = summaries::today_of(&Local, now_ms);
    let played = summaries::played_on(db, live, &Local, today, now_ms)?;
    let recent = summaries::played_in_last(db, live, &Local, TRAY_RECENT_DAYS, today, now_ms)?;
    Ok((describe_status(&spans, played, recent), running_ids))
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

/// "Playing Elden Ring, 1h 24m", "2h 40m played today" and "17h 06m in the
/// past 7 days", from the running sessions and the play of today and the
/// recent days as the journal and Home count it.
fn describe_status(
    running: &[SessionSpan],
    played_today_ms: i64,
    played_recent_ms: i64,
) -> StatusLines {
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
    let today = if played_today_ms < TRAY_MIN_PLAYED_MS {
        NOTHING_TODAY.to_string()
    } else {
        format!("{} played today", format_hours_minutes(played_today_ms))
    };
    let recent = if played_recent_ms < TRAY_MIN_PLAYED_MS {
        format!("Nothing played in the past {TRAY_RECENT_DAYS} days")
    } else {
        format!(
            "{} in the past {TRAY_RECENT_DAYS} days",
            format_hours_minutes(played_recent_ms)
        )
    };
    StatusLines {
        playing,
        today,
        recent,
    }
}

/// "1h 24m" or "40m". Same as `formatHoursMinutes` in `lib/time.ts`.
fn format_hours_minutes(ms: i64) -> String {
    let time = TimeDelta::milliseconds(ms.max(0));
    if time < TimeDelta::hours(1) {
        return format!("{}m", time.num_minutes());
    }
    let minutes = (time - TimeDelta::hours(time.num_hours())).num_minutes();
    format!("{}h {minutes:02}m", time.num_hours())
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

/// Puts the logo in `accent` on the tray, the taskbar and the title bar, or
/// the bundled logo until the page has told the core its accent.
pub fn show_icon<R: Runtime>(app: &AppHandle<R>, accent: Option<&str>) {
    let icon = accent
        .and_then(draw_icon)
        .or_else(|| app.default_window_icon().cloned());
    let Some(icon) = icon else {
        return;
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon.clone()));
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

/// Starts Vaultime out of sight for a login item: in the tray, or minimized
/// where the desktop shows no tray icons, so it never runs without a window
/// to come back to.
pub fn start_minimized<R: Runtime>(app: &AppHandle<R>) {
    if app.state::<TrayState>().shown {
        hide_main_window(app);
    } else if let Some(window) = app.get_webview_window("main") {
        let webview: &Webview<R> = window.as_ref();
        let _ = webview.show();
        let _ = window.minimize();
        let _ = window.show();
        // GTK on Wayland drops a minimize sent before the window is mapped.
        let _ = window.minimize();
    }
}

/// Puts the window away on close: into the tray, or down to the taskbar or
/// dock when the desktop shows no tray icons, so Vaultime stays in sight.
pub fn put_main_window_away<R: Runtime>(app: &AppHandle<R>) {
    if app.state::<TrayState>().shown {
        hide_main_window(app);
    } else if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
    }
}

/// Whether closing the window should keep Vaultime running instead of quitting.
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

/// The service a Linux desktop runs to show `AppIndicator` icons.
#[cfg(target_os = "linux")]
const NOTIFIER_WATCHER: &str = "org.kde.StatusNotifierWatcher";

/// Whether the desktop shows tray icons: through a status notifier watcher,
/// or an X tray that `AppIndicator` falls back to without one. GNOME has
/// neither unless an `AppIndicator` extension runs.
#[cfg(target_os = "linux")]
fn desktop_shows_tray() -> bool {
    notifier_watcher_running() || x_tray_running()
}

#[cfg(not(target_os = "linux"))]
fn desktop_shows_tray() -> bool {
    true
}

#[cfg(target_os = "linux")]
fn notifier_watcher_running() -> bool {
    let Ok(bus) = zbus::blocking::Connection::session() else {
        return false;
    };
    bus.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "NameHasOwner",
        &NOTIFIER_WATCHER,
    )
    .ok()
    .and_then(|reply| reply.body().deserialize::<bool>().ok())
    .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn x_tray_running() -> bool {
    use x11rb::protocol::xproto::ConnectionExt as _;

    let Ok((connection, screen)) = x11rb::connect(None) else {
        return false;
    };
    let selection = format!("_NET_SYSTEM_TRAY_S{screen}");
    connection
        .intern_atom(true, selection.as_bytes())
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .filter(|reply| reply.atom != x11rb::NONE)
        .and_then(|reply| connection.get_selection_owner(reply.atom).ok())
        .and_then(|cookie| cookie.reply().ok())
        .is_some_and(|reply| reply.owner != x11rb::NONE)
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
    fn names_the_running_game_the_day_and_the_week() {
        let lines = describe_status(
            &[running("Elden Ring", 84)],
            144 * MINUTE_MS,
            1026 * MINUTE_MS,
        );
        assert_eq!(lines.playing, "Playing Elden Ring, 1h 24m");
        assert_eq!(lines.today, "2h 24m played today");
        assert_eq!(lines.recent, "17h 06m in the past 7 days");
        let quiet = describe_status(&[], 0, 30 * MINUTE_MS);
        assert_eq!(quiet.playing, NOTHING_RUNNING);
        assert_eq!(quiet.today, NOTHING_TODAY);
        assert_eq!(quiet.recent, "30m in the past 7 days");
        assert_eq!(
            StatusLines::default().recent,
            "Nothing played in the past 7 days"
        );
    }

    #[test]
    fn lists_games_running_side_by_side() {
        let spans = [
            running("Elden Ring", 60),
            running("Balatro", 30),
            running("Celeste", 10),
        ];
        let lines = describe_status(&spans, 60 * MINUTE_MS, 60 * MINUTE_MS);
        assert_eq!(lines.playing, "Playing Elden Ring, Balatro and Celeste");
    }

    #[test]
    fn two_installs_of_one_game_show_once() {
        let spans = [running("Cyberpunk 2077", 60), running("Cyberpunk 2077", 30)];
        let lines = describe_status(&spans, 60 * MINUTE_MS, 60 * MINUTE_MS);
        assert_eq!(lines.playing, "Playing Cyberpunk 2077, 1h 00m");
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
            chrono::Utc::now(),
        );
        let (lines, running) =
            status_lines(&db, &live, &here, start + TimeDelta::minutes(180)).unwrap();
        assert_eq!(lines.playing, "Playing Client, 2h 00m");
        assert_eq!(running, [session.id]);
    }

    #[test]
    fn closing_goes_where_the_icon_shows() {
        let state = |available, shown| TrayState { available, shown };
        assert_eq!(state(true, true).status(), TrayStatus::Shown);
        assert_eq!(state(true, false).status(), TrayStatus::Hidden);
        assert_eq!(state(false, false).status(), TrayStatus::None);
        assert_eq!(state(false, true).status(), TrayStatus::None);
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
        assert_eq!(format_hours_minutes(0), "0m");
        assert_eq!(format_hours_minutes(40 * MINUTE_MS + 59_999), "40m");
        assert_eq!(format_hours_minutes(60 * MINUTE_MS), "1h 00m");
        assert_eq!(format_hours_minutes(84 * MINUTE_MS), "1h 24m");
        assert_eq!(format_hours_minutes(-5), "0m");
    }
}
