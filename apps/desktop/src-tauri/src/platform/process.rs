// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Cross-platform process enumeration and matching.

use std::path::Path;

use sysinfo::System;

/// A snapshot of a running process relevant for game matching.
#[derive(Debug, Clone)]
pub struct RunningProcess {
    /// OS process ID.
    pub pid: u32,
    /// Process name (e.g. "game.exe").
    pub name: String,
    /// Full executable path, if available.
    pub exe_path: Option<String>,
    /// Percent CPU usage since the previous refresh.
    pub cpu_usage: f32,
}

/// Returns a list of currently running processes.
pub fn list_running_processes() -> Vec<RunningProcess> {
    let mut sys = System::new_all();
    refresh_running_processes(&mut sys)
}

/// Refreshes a long-lived `sysinfo::System` instance and returns the snapshot.
pub fn refresh_running_processes(sys: &mut System) -> Vec<RunningProcess> {
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    sys.processes()
        .values()
        .map(|p| RunningProcess {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            exe_path: p.exe().map(|e| e.to_string_lossy().into_owned()),
            cpu_usage: p.cpu_usage(),
        })
        .collect()
}

/// Checks whether a running process matches a game's registered executable path.
///
/// Matching strategy (in priority order):
/// 1. Full path match — the process `exe_path` equals the game's `executable_path`.
/// 2. File-name match — the process name matches the file name component of the
///    game's `executable_path` (handles cases where the OS reports a short name).
pub fn matches_executable(process: &RunningProcess, game_executable: &str) -> bool {
    // Full path comparison (case-sensitive on Linux, case-insensitive on Windows).
    if let Some(ref exe) = process.exe_path {
        if paths_equal(exe, game_executable) {
            return true;
        }
    }

    // File-name-only comparison.
    if let Some(game_filename) = Path::new(game_executable).file_name() {
        let game_name = game_filename.to_string_lossy();
        if names_equal(&process.name, &game_name) {
            return true;
        }
    }

    false
}

#[cfg(target_os = "windows")]
fn paths_equal(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[cfg(not(target_os = "windows"))]
fn paths_equal(a: &str, b: &str) -> bool {
    a == b
}

#[cfg(target_os = "windows")]
fn names_equal(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[cfg(not(target_os = "windows"))]
fn names_equal(a: &str, b: &str) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_path_match() {
        let proc = RunningProcess {
            pid: 1,
            name: "game".into(),
            exe_path: Some("/opt/games/cool-game/game".into()),
            cpu_usage: 0.0,
        };
        assert!(matches_executable(&proc, "/opt/games/cool-game/game"));
    }

    #[test]
    fn filename_match() {
        let proc = RunningProcess {
            pid: 2,
            name: "game".into(),
            exe_path: None,
            cpu_usage: 0.0,
        };
        assert!(matches_executable(&proc, "/opt/games/cool-game/game"));
    }

    #[test]
    fn no_match() {
        let proc = RunningProcess {
            pid: 3,
            name: "firefox".into(),
            exe_path: Some("/usr/bin/firefox".into()),
            cpu_usage: 0.0,
        };
        assert!(!matches_executable(&proc, "/opt/games/cool-game/game"));
    }

    #[test]
    fn list_processes_runs() {
        let procs = list_running_processes();
        // Should at least find the current test process.
        assert!(!procs.is_empty());
    }
}
