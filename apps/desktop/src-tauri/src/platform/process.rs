// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Process enumeration and game executable matching.

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

/// Checks whether a running process matches a game's registered executable.
///
/// The full path is compared first. The file name is the fallback because some
/// processes, for example elevated ones on Windows, hide their full path.
pub fn matches_executable(process: &RunningProcess, game_executable: &str) -> bool {
    if let Some(exe) = &process.exe_path
        && paths_equal(exe, game_executable)
    {
        return true;
    }

    file_name(game_executable).is_some_and(|game_name| names_equal(&process.name, game_name))
}

/// Last path segment, splitting on both separators so Windows paths parse on any OS.
fn file_name(path: &str) -> Option<&str> {
    path.rsplit(['/', '\\']).find(|segment| !segment.is_empty())
}

fn paths_equal(a: &str, b: &str) -> bool {
    path_key(a) == path_key(b)
}

/// Comparison key for executable paths. On Windows it ignores case and
/// collapses separators, so `C:/Games//x.exe` equals `c:\games\X.EXE`.
#[cfg(windows)]
pub fn path_key(path: &str) -> String {
    path.split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("\\")
        .to_lowercase()
}

/// Comparison key for executable paths. Unix paths are compared as is.
#[cfg(not(windows))]
pub fn path_key(path: &str) -> String {
    path.to_owned()
}

#[cfg(windows)]
fn names_equal(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[cfg(not(windows))]
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
    fn windows_style_path_matches_by_file_name() {
        let proc = RunningProcess {
            pid: 4,
            name: "game.exe".into(),
            exe_path: None,
            cpu_usage: 0.0,
        };
        assert!(matches_executable(&proc, r"C:\Games\Cool\game.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_ignore_case_and_separators() {
        assert!(paths_equal(
            r"C:\Games\Cool\Game.exe",
            "c:/games//cool/game.EXE"
        ));
    }

    #[test]
    fn list_processes_runs() {
        let procs = list_running_processes();
        // Should at least find the current test process.
        assert!(!procs.is_empty());
    }
}
