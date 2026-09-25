// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Platform activity probing for active window and idle detection.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::ffi::c_void;

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
        foreground_pid: detect_foreground_pid(),
        foreground_supported: foreground_detection_strategy() != "heuristic",
        idle_for: detect_idle_duration(),
        idle_supported: idle_detection_strategy() != "heuristic",
    }
}

/// Returns a short label describing the current foreground detection strategy.
pub fn foreground_detection_strategy() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        if can_use_x11_tools() && command_available("xprop") {
            "x11"
        } else {
            "heuristic"
        }
    }

    #[cfg(target_os = "windows")]
    {
        "win32_api"
    }

    #[cfg(target_os = "macos")]
    {
        if command_available("osascript") {
            "macos_system"
        } else {
            "heuristic"
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[cfg(not(target_os = "windows"))]
    #[cfg(not(target_os = "macos"))]
    {
        "heuristic"
    }
}

/// Returns a short label describing the current idle detection strategy.
pub fn idle_detection_strategy() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        if can_use_x11_tools() && command_available("xprintidle") {
            "x11"
        } else {
            "heuristic"
        }
    }

    #[cfg(target_os = "windows")]
    {
        "win32_api"
    }

    #[cfg(target_os = "macos")]
    {
        if command_available("ioreg") {
            "macos_ioreg"
        } else {
            "heuristic"
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[cfg(not(target_os = "windows"))]
    #[cfg(not(target_os = "macos"))]
    {
        "heuristic"
    }
}

#[cfg(target_os = "linux")]
fn detect_foreground_pid() -> Option<u32> {
    if !can_use_x11_tools() || !command_available("xprop") {
        return None;
    }

    let root_output = run_command("xprop", &["-root", "_NET_ACTIVE_WINDOW"])?;
    let window_id = parse_window_id(&root_output)?;

    let pid_output = run_command("xprop", &["-id", &window_id, "_NET_WM_PID"])?;
    parse_pid(&pid_output)
}

#[cfg(target_os = "windows")]
fn detect_foreground_pid() -> Option<u32> {
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return None;
        }

        let mut pid = 0u32;
        let _thread_id = GetWindowThreadProcessId(window, &mut pid);
        if pid == 0 { None } else { Some(pid) }
    }
}

#[cfg(target_os = "macos")]
fn detect_foreground_pid() -> Option<u32> {
    if !command_available("osascript") {
        return None;
    }

    let output = run_command(
        "osascript",
        &[
            "-e",
            "tell application \"System Events\" to unix id of first application process whose frontmost is true",
        ],
    )?;

    output.trim().parse::<u32>().ok()
}

#[cfg(not(target_os = "linux"))]
#[cfg(not(target_os = "windows"))]
#[cfg(not(target_os = "macos"))]
fn detect_foreground_pid() -> Option<u32> {
    None
}

#[cfg(target_os = "linux")]
fn detect_idle_duration() -> Option<Duration> {
    if !can_use_x11_tools() || !command_available("xprintidle") {
        return None;
    }

    let output = run_command("xprintidle", &[])?;
    let idle_ms = output.trim().parse::<u64>().ok()?;
    Some(Duration::from_millis(idle_ms))
}

#[cfg(target_os = "windows")]
fn detect_idle_duration() -> Option<Duration> {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        if GetLastInputInfo(&mut info) == 0 {
            return None;
        }

        let now_ms = GetTickCount64();
        let idle_ms = now_ms.saturating_sub(info.dwTime as u64);
        Some(Duration::from_millis(idle_ms))
    }
}

#[cfg(target_os = "macos")]
fn detect_idle_duration() -> Option<Duration> {
    if !command_available("ioreg") {
        return None;
    }

    let output = run_command("ioreg", &["-c", "IOHIDSystem"])?;
    let idle_nanos = parse_macos_idle_nanos(&output)?;
    Some(Duration::from_nanos(idle_nanos))
}

#[cfg(not(target_os = "linux"))]
#[cfg(not(target_os = "windows"))]
#[cfg(not(target_os = "macos"))]
fn detect_idle_duration() -> Option<Duration> {
    None
}

#[cfg(target_os = "linux")]
fn can_use_x11_tools() -> bool {
    std::env::var_os("DISPLAY").is_some()
}

#[cfg(not(target_os = "linux"))]
fn can_use_x11_tools() -> bool {
    false
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn run_command(_program: &str, _args: &[&str]) -> Option<String> {
    None
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn command_available(program: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {program} >/dev/null 2>&1")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn command_available(_program: &str) -> bool {
    false
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

#[cfg(target_os = "macos")]
fn parse_macos_idle_nanos(output: &str) -> Option<u64> {
    output.lines().find_map(|line| {
        if !line.contains("HIDIdleTime") {
            return None;
        }

        line.split('=')
            .nth(1)
            .map(str::trim)
            .and_then(|value| value.parse::<u64>().ok())
    })
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct LASTINPUTINFO {
    cbSize: u32,
    dwTime: u32,
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowThreadProcessId(window: *mut c_void, process_id: *mut u32) -> u32;
    fn GetLastInputInfo(info: *mut LASTINPUTINFO) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetTickCount64() -> u64;
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
    fn parses_pid_from_xprop_output() {
        let output = "_NET_WM_PID(CARDINAL) = 4242";
        assert_eq!(parse_pid(output), Some(4242));
    }

    #[test]
    fn ignores_empty_window_id() {
        let output = "_NET_ACTIVE_WINDOW(WINDOW): window id # 0x0";
        assert_eq!(parse_window_id(output), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn parses_macos_idle_time() {
        let output = "\"HIDIdleTime\" = 123456789";
        assert_eq!(parse_macos_idle_nanos(output), Some(123456789));
    }
}
