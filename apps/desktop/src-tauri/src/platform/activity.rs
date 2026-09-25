// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Foreground window and user idle detection.

use std::time::Duration;

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
    ActivitySnapshot {
        foreground_pid: imp::foreground_pid(),
        foreground_supported: foreground_detection_strategy() != HEURISTIC,
        idle_for: imp::idle_duration(),
        idle_supported: idle_detection_strategy() != HEURISTIC,
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
    use std::process::Command;
    use std::sync::OnceLock;
    use std::time::Duration;

    use super::HEURISTIC;

    pub fn foreground_strategy() -> &'static str {
        if xprop_available() { "x11" } else { HEURISTIC }
    }

    pub fn idle_strategy() -> &'static str {
        if xprintidle_available() {
            "x11"
        } else {
            HEURISTIC
        }
    }

    pub fn foreground_pid() -> Option<u32> {
        if !xprop_available() {
            return None;
        }

        let root = run("xprop", &["-root", "_NET_ACTIVE_WINDOW"])?;
        let window_id = parse_window_id(&root)?;
        let pid = run("xprop", &["-id", &window_id, "_NET_WM_PID"])?;
        parse_pid(&pid)
    }

    pub fn idle_duration() -> Option<Duration> {
        if !xprintidle_available() {
            return None;
        }

        let idle_ms = run("xprintidle", &[])?.trim().parse::<u64>().ok()?;
        Some(Duration::from_millis(idle_ms))
    }

    // Tool availability does not change while the app runs, so check it once.
    fn xprop_available() -> bool {
        static AVAILABLE: OnceLock<bool> = OnceLock::new();
        *AVAILABLE.get_or_init(|| x11_session() && on_path("xprop"))
    }

    fn xprintidle_available() -> bool {
        static AVAILABLE: OnceLock<bool> = OnceLock::new();
        *AVAILABLE.get_or_init(|| x11_session() && on_path("xprintidle"))
    }

    fn x11_session() -> bool {
        std::env::var_os("DISPLAY").is_some()
    }

    fn on_path(program: &str) -> bool {
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join(program).is_file())
        })
    }

    fn run(program: &str, args: &[&str]) -> Option<String> {
        let output = Command::new(program).args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout).ok()
    }

    fn parse_window_id(output: &str) -> Option<String> {
        output
            .split('#')
            .nth(1)
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "0x0")
            .map(ToOwned::to_owned)
    }

    fn parse_pid(output: &str) -> Option<u32> {
        output
            .split('=')
            .nth(1)
            .map(str::trim)
            .and_then(|value| value.parse::<u32>().ok())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_window_id_from_xprop_output() {
            let output = "_NET_ACTIVE_WINDOW(WINDOW): window id # 0x4c00007";
            assert_eq!(parse_window_id(output).as_deref(), Some("0x4c00007"));
        }

        #[test]
        fn ignores_empty_window_id() {
            let output = "_NET_ACTIVE_WINDOW(WINDOW): window id # 0x0";
            assert_eq!(parse_window_id(output), None);
        }

        #[test]
        fn parses_pid_from_xprop_output() {
            let output = "_NET_WM_PID(CARDINAL) = 4242";
            assert_eq!(parse_pid(output), Some(4242));
        }
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
