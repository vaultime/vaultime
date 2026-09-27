// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

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
    use std::sync::{Mutex, OnceLock, PoisonError};
    use std::time::Duration;

    use log::info;
    use x11rb::connection::Connection;
    use x11rb::protocol::screensaver::ConnectionExt as _;
    use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, Window};
    use x11rb::rust_connection::RustConnection;

    use super::HEURISTIC;

    const GNOME_IDLE_MONITOR: &str = "org.gnome.Mutter.IdleMonitor";
    const GNOME_IDLE_MONITOR_PATH: &str = "/org/gnome/Mutter/IdleMonitor/Core";

    pub fn foreground_strategy() -> &'static str {
        if x11().is_some() { "x11" } else { HEURISTIC }
    }

    pub fn idle_strategy() -> &'static str {
        static STRATEGY: OnceLock<&'static str> = OnceLock::new();
        STRATEGY.get_or_init(|| {
            // On Wayland the X server only sees input for X11 apps, so the
            // desktop's own idle monitor is the better source.
            let strategy = if wayland_session() && gnome_idle().is_some() {
                "gnome_dbus"
            } else if x11().is_some_and(|x11| {
                x11.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .screensaver
            }) {
                "x11"
            } else {
                HEURISTIC
            };
            info!("idle detection: {strategy}");
            strategy
        })
    }

    pub fn foreground_pid() -> Option<u32> {
        x11()?
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .foreground_pid()
    }

    pub fn idle_duration() -> Option<Duration> {
        match idle_strategy() {
            "gnome_dbus" => gnome_idle(),
            "x11" => x11()?.lock().unwrap_or_else(PoisonError::into_inner).idle(),
            _ => None,
        }
    }

    fn wayland_session() -> bool {
        std::env::var_os("WAYLAND_DISPLAY").is_some()
    }

    /// A connection to the X server, on Wayland the one for X11 apps, opened on
    /// first use.
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
        fn property(&self, window: Window, property: Atom, kind: AtomEnum) -> Option<u32> {
            let reply = self
                .connection
                .get_property(false, window, property, kind, 0, 1)
                .ok()?
                .reply()
                .ok()?;
            reply.value32()?.next()
        }

        fn foreground_pid(&self) -> Option<u32> {
            let window = self.property(self.root, self.active_window, AtomEnum::WINDOW)?;
            if window == 0 {
                return None;
            }
            self.property(window, self.window_pid, AtomEnum::CARDINAL)
        }

        fn idle(&self) -> Option<Duration> {
            let info = self
                .connection
                .screensaver_query_info(self.root)
                .ok()?
                .reply()
                .ok()?;
            Some(Duration::from_millis(u64::from(info.ms_since_user_input)))
        }
    }

    fn x11() -> Option<&'static Mutex<X11>> {
        static X11_CONNECTION: OnceLock<Option<Mutex<X11>>> = OnceLock::new();
        X11_CONNECTION
            .get_or_init(|| X11::connect().map(Mutex::new))
            .as_ref()
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
