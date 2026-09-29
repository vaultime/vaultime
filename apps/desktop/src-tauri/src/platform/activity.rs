// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Foreground window and user idle detection.

use std::time::Duration;

use super::controller;

/// Snapshot of activity-related platform signals.
#[derive(Debug, Clone, Copy)]
pub struct ActivitySnapshot {
    pub foreground_pid: Option<u32>,
    pub foreground_supported: bool,
    pub idle_for: Option<Duration>,
    pub idle_supported: bool,
}

/// Returns the best-effort activity snapshot for the current platform.
pub fn capture_activity_snapshot() -> ActivitySnapshot {
    let controller_idle = controller::idle_duration();
    ActivitySnapshot {
        foreground_pid: imp::foreground_pid(),
        foreground_supported: foreground_detection_strategy() != HEURISTIC,
        idle_for: with_controller_input(imp::idle_duration(), controller_idle),
        idle_supported: idle_detection_strategy() != HEURISTIC,
    }
}

/// The desktop idle time, cut short by later controller input. Controller
/// input alone cannot tell idle time where the desktop reports none.
fn with_controller_input(
    desktop: Option<Duration>,
    controller: Option<Duration>,
) -> Option<Duration> {
    match (desktop, controller) {
        (Some(desktop), Some(controller)) => Some(desktop.min(controller)),
        (desktop, _) => desktop,
    }
}

/// Short label for the foreground detection strategy, shown in diagnostics.
pub fn foreground_detection_strategy() -> &'static str {
    imp::foreground_strategy()
}

/// Short label for the idle detection strategy, shown in diagnostics.
pub fn idle_detection_strategy() -> &'static str {
    imp::idle_strategy()
}

const HEURISTIC: &str = "heuristic";

#[cfg(windows)]
mod imp {
    use std::time::Duration;

    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    pub fn foreground_strategy() -> &'static str {
        "win32_api"
    }

    pub fn idle_strategy() -> &'static str {
        "win32_api"
    }

    pub fn foreground_pid() -> Option<u32> {
        // SAFETY: both calls only read window manager state and `pid` outlives the call.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() {
                return None;
            }

            let mut pid = 0_u32;
            GetWindowThreadProcessId(window, &raw mut pid);
            (pid != 0).then_some(pid)
        }
    }

    pub fn idle_duration() -> Option<Duration> {
        let mut info = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        // SAFETY: `info` is a valid LASTINPUTINFO with `cbSize` set.
        if unsafe { GetLastInputInfo(&raw mut info) } == 0 {
            return None;
        }

        // `dwTime` is a 32-bit tick count. Subtracting from the 32-bit clock with
        // wrapping keeps the result correct after the 49.7 day rollover.
        // SAFETY: GetTickCount has no preconditions.
        let now = unsafe { GetTickCount() };
        Some(Duration::from_millis(u64::from(
            now.wrapping_sub(info.dwTime),
        )))
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock, PoisonError};
    use std::thread;
    use std::time::{Duration, Instant};

    use log::{info, warn};
    use x11rb::connection::Connection;
    use x11rb::cookie::VoidCookie;
    use x11rb::errors::ReplyError;
    use x11rb::protocol::Event;
    use x11rb::protocol::screensaver::ConnectionExt as _;
    use x11rb::protocol::xinput::{ConnectionExt as _, Device, EventMask, XIEventMask};
    use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, Window};
    use x11rb::rust_connection::RustConnection;

    use super::HEURISTIC;

    const GNOME_IDLE_MONITOR: &str = "org.gnome.Mutter.IdleMonitor";
    const GNOME_IDLE_MONITOR_PATH: &str = "/org/gnome/Mutter/IdleMonitor/Core";

    pub fn foreground_strategy() -> &'static str {
        static STRATEGY: OnceLock<&'static str> = OnceLock::new();
        STRATEGY.get_or_init(|| {
            if with_x11(|_| Ok(Some(()))).is_some() {
                "x11"
            } else {
                HEURISTIC
            }
        })
    }

    pub fn idle_strategy() -> &'static str {
        static STRATEGY: OnceLock<&'static str> = OnceLock::new();
        STRATEGY.get_or_init(|| {
            // On Wayland the X server only sees input for X11 apps, so the
            // desktop's own idle monitor is the better source.
            let strategy = if wayland_session() && gnome_idle().is_some() {
                "gnome_dbus"
            } else if raw_input_idle().is_some() {
                "x11_input"
            } else if with_x11(|x11| Ok(Some(x11.screensaver))) == Some(true) {
                "x11"
            } else {
                HEURISTIC
            };
            info!("idle detection: {strategy}");
            strategy
        })
    }

    pub fn foreground_pid() -> Option<u32> {
        with_x11(X11::foreground_pid)
    }

    pub fn idle_duration() -> Option<Duration> {
        match idle_strategy() {
            "gnome_dbus" => gnome_idle(),
            "x11_input" => raw_input_idle().or_else(|| with_x11(X11::idle)),
            "x11" => with_x11(X11::idle),
            _ => None,
        }
    }

    fn wayland_session() -> bool {
        std::env::var_os("WAYLAND_DISPLAY").is_some()
    }

    /// A connection to the X server, on Wayland the one for X11 apps.
    struct X11 {
        connection: RustConnection,
        root: Window,
        active_window: Atom,
        window_pid: Atom,
        screensaver: bool,
    }

    impl X11 {
        fn connect() -> Option<Self> {
            let (connection, screen) = x11rb::connect(None).ok()?;
            let root = connection.setup().roots.get(screen)?.root;
            let atom = |name: &[u8]| -> Option<Atom> {
                Some(connection.intern_atom(false, name).ok()?.reply().ok()?.atom)
            };
            let active_window = atom(b"_NET_ACTIVE_WINDOW")?;
            let window_pid = atom(b"_NET_WM_PID")?;
            let screensaver = connection
                .screensaver_query_version(1, 1)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .is_some();
            info!("connected to the X server, screensaver extension {screensaver}");
            Some(Self {
                connection,
                root,
                active_window,
                window_pid,
                screensaver,
            })
        }

        /// Reads one 32 bit value of a window property.
        fn property(
            &self,
            window: Window,
            property: Atom,
            kind: AtomEnum,
        ) -> Result<Option<u32>, ReplyError> {
            let reply = self
                .connection
                .get_property(false, window, property, kind, 0, 1)?
                .reply()?;
            Ok(reply.value32().and_then(|mut values| values.next()))
        }

        fn foreground_pid(&self) -> Result<Option<u32>, ReplyError> {
            let window = self.property(self.root, self.active_window, AtomEnum::WINDOW)?;
            match window {
                None | Some(0) => Ok(None),
                Some(window) => self.property(window, self.window_pid, AtomEnum::CARDINAL),
            }
        }

        fn idle(&self) -> Result<Option<Duration>, ReplyError> {
            if !self.screensaver {
                return Ok(None);
            }
            let info = self.connection.screensaver_query_info(self.root)?.reply()?;
            Ok(Some(Duration::from_millis(u64::from(
                info.ms_since_user_input,
            ))))
        }
    }

    /// Runs `read` on the X connection. A broken connection is dropped and
    /// opened again on a later call, which recovers from an X server restart.
    fn with_x11<T>(read: impl FnOnce(&X11) -> Result<Option<T>, ReplyError>) -> Option<T> {
        static CONNECTION: Mutex<Option<X11>> = Mutex::new(None);

        let mut connection = CONNECTION.lock().unwrap_or_else(PoisonError::into_inner);
        if connection.is_none() {
            *connection = X11::connect();
        }
        match read(connection.as_ref()?) {
            Ok(value) => value,
            // For example a window that closed between two requests.
            Err(ReplyError::X11Error(_)) => None,
            Err(ReplyError::ConnectionError(error)) => {
                warn!("lost the connection to the X server: {error}");
                *connection = None;
                None
            }
        }
    }

    /// Keyboard and mouse input as `XInput2` raw events, on a connection of its
    /// own. The idle counter of the screensaver extension is no help while a
    /// game runs, because SDL resets it every 30 seconds.
    struct RawInput {
        connection: RustConnection,
        root: Window,
        last_input: Mutex<Instant>,
        watching_motion: AtomicBool,
        alive: AtomicBool,
    }

    impl RawInput {
        fn connect() -> Option<Self> {
            let (connection, screen) = x11rb::connect(None).ok()?;
            let root = connection.setup().roots.get(screen)?.root;
            let version = connection
                .xinput_xi_query_version(2, 2)
                .ok()?
                .reply()
                .ok()?;
            // Raw events reach every client that asks for them since XInput 2.1.
            if (version.major_version, version.minor_version) < (2, 1) {
                return None;
            }
            // The screensaver counter is the best guess until the first event.
            let idle = connection
                .screensaver_query_info(root)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .map_or(Duration::ZERO, |info| {
                    Duration::from_millis(u64::from(info.ms_since_user_input))
                });
            let now = Instant::now();
            let input = Self {
                connection,
                root,
                last_input: Mutex::new(now.checked_sub(idle).unwrap_or(now)),
                watching_motion: AtomicBool::new(true),
                alive: AtomicBool::new(true),
            };
            input.select(true)?.check().ok()?;
            Some(input)
        }

        /// Connects and starts the thread that reads the events.
        fn start() -> Option<Arc<Self>> {
            let input = Arc::new(Self::connect()?);
            let reader = Arc::clone(&input);
            thread::Builder::new()
                .name("vaultime-x11-input".into())
                .spawn(move || reader.run())
                .ok()?;
            info!("reading X11 input events");
            Some(input)
        }

        fn select(&self, motion: bool) -> Option<VoidCookie<'_, RustConnection>> {
            let mut mask = XIEventMask::RAW_KEY_PRESS
                | XIEventMask::RAW_BUTTON_PRESS
                | XIEventMask::RAW_TOUCH_BEGIN;
            if motion {
                mask |= XIEventMask::RAW_MOTION;
            }
            let cookie = self
                .connection
                .xinput_xi_select_events(
                    self.root,
                    &[EventMask {
                        deviceid: Device::ALL.into(),
                        mask: vec![mask],
                    }],
                )
                .ok()?;
            self.connection.flush().ok()?;
            self.watching_motion.store(motion, Ordering::Relaxed);
            Some(cookie)
        }

        /// Notes the time of every input until the connection ends. After a
        /// mouse move, motion is left out until the next tracking tick, so a
        /// moving mouse does not wake this thread a thousand times a second.
        fn run(&self) {
            loop {
                match self.connection.wait_for_event() {
                    Ok(Event::XinputRawMotion(_)) => {
                        self.note_input();
                        // Moves already queued still arrive, so one request to stop is enough.
                        if self.watching_motion.load(Ordering::Relaxed)
                            && let Some(cookie) = self.select(false)
                        {
                            cookie.ignore_error();
                        }
                    }
                    Ok(
                        Event::XinputRawKeyPress(_)
                        | Event::XinputRawButtonPress(_)
                        | Event::XinputRawTouchBegin(_),
                    ) => self.note_input(),
                    Ok(_) => {}
                    Err(error) => {
                        warn!("lost the X11 input events: {error}");
                        self.alive.store(false, Ordering::Relaxed);
                        return;
                    }
                }
            }
        }

        fn note_input(&self) {
            *self
                .last_input
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Instant::now();
        }

        fn idle(&self) -> Duration {
            if !self.watching_motion.load(Ordering::Relaxed)
                && let Some(cookie) = self.select(true)
            {
                cookie.ignore_error();
            }
            self.last_input
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .elapsed()
        }
    }

    /// Time since the last input from the raw events. A lost connection is
    /// opened again on a later call.
    fn raw_input_idle() -> Option<Duration> {
        static RAW_INPUT: Mutex<Option<Arc<RawInput>>> = Mutex::new(None);

        let mut input = RAW_INPUT.lock().unwrap_or_else(PoisonError::into_inner);
        if input
            .as_ref()
            .is_none_or(|input| !input.alive.load(Ordering::Relaxed))
        {
            *input = RawInput::start();
        }
        Some(input.as_ref()?.idle())
    }

    /// GNOME reports the time since the last input over D-Bus, on X11 and Wayland.
    fn gnome_idle() -> Option<Duration> {
        static SESSION_BUS: OnceLock<Option<zbus::blocking::Connection>> = OnceLock::new();
        let bus = SESSION_BUS
            .get_or_init(|| zbus::blocking::Connection::session().ok())
            .as_ref()?;
        let reply = bus
            .call_method(
                Some(GNOME_IDLE_MONITOR),
                GNOME_IDLE_MONITOR_PATH,
                Some(GNOME_IDLE_MONITOR),
                "GetIdletime",
                &(),
            )
            .ok()?;
        let idle_ms: u64 = reply.body().deserialize().ok()?;
        Some(Duration::from_millis(idle_ms))
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::process::Command;
    use std::time::Duration;

    pub fn foreground_strategy() -> &'static str {
        "macos_system"
    }

    pub fn idle_strategy() -> &'static str {
        "macos_ioreg"
    }

    pub fn foreground_pid() -> Option<u32> {
        let script = "tell application \"System Events\" to unix id of first application process whose frontmost is true";
        run("osascript", &["-e", script])?.trim().parse().ok()
    }

    pub fn idle_duration() -> Option<Duration> {
        let output = run("ioreg", &["-c", "IOHIDSystem"])?;
        output.lines().find_map(|line| {
            if !line.contains("HIDIdleTime") {
                return None;
            }
            line.split('=')
                .nth(1)
                .and_then(|value| value.trim().parse().ok())
                .map(Duration::from_nanos)
        })
    }

    fn run(program: &str, args: &[&str]) -> Option<String> {
        let output = Command::new(program).args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout).ok()
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod imp {
    use std::time::Duration;

    use super::HEURISTIC;

    pub fn foreground_strategy() -> &'static str {
        HEURISTIC
    }

    pub fn idle_strategy() -> &'static str {
        HEURISTIC
    }

    pub fn foreground_pid() -> Option<u32> {
        None
    }

    pub fn idle_duration() -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints the signals this desktop offers. Run it in a desktop session
    /// or under `xvfb-run` with `cargo test -- --ignored --nocapture signals`.
    #[test]
    #[ignore = "needs a desktop session"]
    fn report_activity_signals() {
        println!(
            "foreground {}, idle {}",
            foreground_detection_strategy(),
            idle_detection_strategy()
        );
        println!("{:?}", capture_activity_snapshot());
        assert_ne!(idle_detection_strategy(), HEURISTIC);
    }

    #[test]
    fn controller_input_shortens_idle_time() {
        let (minute, second) = (Duration::from_secs(60), Duration::from_secs(1));
        assert_eq!(
            with_controller_input(Some(minute), Some(second)),
            Some(second)
        );
        assert_eq!(
            with_controller_input(Some(second), Some(minute)),
            Some(second)
        );
        assert_eq!(with_controller_input(Some(minute), None), Some(minute));
        assert_eq!(with_controller_input(None, Some(second)), None);
    }

    /// Screensaver resets, which SDL games send every 30 seconds, must not
    /// hide that the user left. Needs an X server with `xdotool` and `xset`:
    /// `xvfb-run cargo test -- --ignored --nocapture screensaver_resets`.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "needs an X server with xdotool and xset"]
    fn x11_idle_ignores_screensaver_resets() {
        use std::process::Command;
        use std::thread::sleep;

        let run = |program: &str, args: &[&str]| {
            assert!(Command::new(program).args(args).status().unwrap().success());
        };
        let idle = || capture_activity_snapshot().idle_for.unwrap();
        let settle = Duration::from_millis(500);
        let away = Duration::from_secs(3);
        assert_eq!(idle_detection_strategy(), "x11_input");

        run("xdotool", &["mousemove_relative", "20", "20"]);
        sleep(settle);
        assert!(idle() < away);

        // Games that warp the pointer and SDL's screensaver resets are no input.
        // The XWayland of a headless Weston goes down on a warp.
        sleep(away);
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            run("xdotool", &["mousemove", "10", "10"]);
        }
        run("xset", &["s", "reset"]);
        sleep(settle);
        assert!(idle() >= away);

        // The tick above turned motion back on after the first move.
        run("xdotool", &["mousemove_relative", "20", "20"]);
        sleep(settle);
        assert!(idle() < away);

        sleep(away);
        run("xdotool", &["click", "1"]);
        sleep(settle);
        assert!(idle() < away);

        sleep(away);
        run("xdotool", &["key", "shift"]);
        sleep(settle);
        assert!(idle() < away);
    }

    /// The X connections come back after the X server restarts. Needs a
    /// compositor that starts it on demand, like `weston --xwayland`, and `xdotool`:
    /// `cargo test -- --ignored --nocapture xwayland_restart`.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "needs Weston with XWayland"]
    fn x11_input_survives_an_xwayland_restart() {
        use std::process::Command;
        use std::thread::sleep;

        let run = |program: &str, args: &[&str]| {
            assert!(Command::new(program).args(args).status().unwrap().success());
        };
        let idle = || capture_activity_snapshot().idle_for.unwrap();
        let settle = Duration::from_millis(500);
        let away = Duration::from_secs(3);
        assert_eq!(idle_detection_strategy(), "x11_input");
        idle();

        run("pkill", &["-x", "Xwayland"]);
        sleep(settle);
        assert!(idle() < away, "a new connection starts from now");

        sleep(away);
        assert!(idle() >= away);
        run("xdotool", &["key", "shift"]);
        sleep(settle);
        assert!(idle() < away, "input reaches the new connection");
    }

    #[test]
    fn snapshot_flags_match_strategies() {
        let snapshot = capture_activity_snapshot();
        assert_eq!(
            snapshot.foreground_supported,
            foreground_detection_strategy() != HEURISTIC
        );
        assert_eq!(
            snapshot.idle_supported,
            idle_detection_strategy() != HEURISTIC
        );
    }
}
