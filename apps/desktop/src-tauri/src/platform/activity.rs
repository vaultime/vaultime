// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Platform activity probing for active window and idle detection.

use std::process::Command;
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

    #[cfg(not(target_os = "linux"))]
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

    #[cfg(not(target_os = "linux"))]
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

#[cfg(not(target_os = "linux"))]
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

#[cfg(not(target_os = "linux"))]
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

#[cfg(target_os = "linux")]
fn run_command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

#[cfg(not(target_os = "linux"))]
fn run_command(_program: &str, _args: &[&str]) -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
fn command_available(program: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {program} >/dev/null 2>&1")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "linux"))]
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
}
