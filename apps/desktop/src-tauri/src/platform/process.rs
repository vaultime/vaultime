// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Process enumeration and game executable matching.

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// A snapshot of a running process relevant for game matching.
#[derive(Debug, Clone)]
pub struct RunningProcess {
    /// OS process ID.
    pub pid: u32,
    /// Process name (e.g. "game.exe").
    pub name: String,
    /// Full executable path, if available.
    pub exe_path: Option<String>,
    /// First word of the command line. Wine puts the Windows path of the game
    /// there, while the executable path is Wine's own.
    pub command: Option<String>,
    /// Percent CPU usage since the previous refresh.
    pub cpu_usage: f32,
}

/// Refreshes a long-lived `sysinfo::System` instance and returns the snapshot.
pub fn refresh_running_processes(sys: &mut System) -> Vec<RunningProcess> {
    // Paths and command lines do not change while a process runs, so they are
    // read once per process.
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cpu()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_cmd(UpdateKind::OnlyIfNotSet),
    );

    sys.processes()
        .values()
        .map(|p| RunningProcess {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            exe_path: p.exe().map(|e| e.to_string_lossy().into_owned()),
            command: p
                .cmd()
                .first()
                .map(|arg| arg.to_string_lossy().into_owned()),
            cpu_usage: p.cpu_usage(),
        })
        .collect()
}

/// Checks whether a running process matches a game's registered executable.
///
/// The full path decides when the process has one, so two games that share a
/// file name like `Game.exe` stay apart. A different path still matches when
/// both lead to the same file, for example through a junction or a symlinked
/// library folder. The file name alone decides when the path is hidden, as for
/// elevated processes on Windows, or when it belongs to a loader such as Wine,
/// which keeps the game's name but reports its own path.
pub fn matches_executable(process: &RunningProcess, game_executable: &str) -> bool {
    let Some(game_name) = file_name(game_executable) else {
        return false;
    };
    let Some(exe) = &process.exe_path else {
        return names_equal(&process.name, game_name);
    };
    if paths_equal(exe, game_executable) {
        return true;
    }

    match file_name(exe) {
        Some(exe_name) if names_equal(exe_name, game_name) => same_file(exe, game_executable),
        // A loader such as Wine. Linux cuts process names to 15 characters,
        // so the Windows path at the start of the command line is checked too.
        Some(exe_name) if !names_equal(exe_name, &process.name) => {
            names_equal(&process.name, game_name)
                || process
                    .command
                    .as_deref()
                    .and_then(file_name)
                    .is_some_and(|name| names_equal(name, game_name))
        }
        _ => false,
    }
}

/// Whether the process runs a program from inside `folder`, at any depth.
/// The folder is also compared with links resolved, so a library behind a
/// junction or symlink still counts.
pub fn runs_from_folder(process: &RunningProcess, folder: &str) -> bool {
    let Some(exe) = &process.exe_path else {
        return false;
    };
    let exe = path_key(exe);
    let resolved = std::fs::canonicalize(folder)
        .ok()
        .map(|path| strip_verbatim_prefix(&path.to_string_lossy()).to_owned());
    std::iter::once(folder.to_owned())
        .chain(resolved)
        .any(|candidate| is_inside(&exe, &path_key(&candidate)))
}

/// True when `path` lies below `folder`, both given as path keys.
fn is_inside(path: &str, folder: &str) -> bool {
    let folder = folder.trim_end_matches(['/', '\\']);
    !folder.is_empty()
        && path.len() > folder.len()
        && path.starts_with(folder)
        && path[folder.len()..].starts_with(['/', '\\'])
}

/// Windows canonical paths start with `\\?\`, process paths do not.
fn strip_verbatim_prefix(path: &str) -> &str {
    path.strip_prefix(r"\\?\")
        .filter(|rest| !rest.starts_with("UNC"))
        .unwrap_or(path)
}

/// Whether the process file at `exe` is the game file, with links resolved.
/// When the process path cannot be resolved from here, for example because
/// the game runs in a container, the matching file name has to do.
fn same_file(exe: &str, game_executable: &str) -> bool {
    let Ok(exe) = std::fs::canonicalize(exe) else {
        return true;
    };
    std::fs::canonicalize(game_executable)
        .is_ok_and(|game| paths_equal(&exe.to_string_lossy(), &game.to_string_lossy()))
}

/// Last path segment, splitting on both separators so Windows paths parse on any OS.
pub(crate) fn file_name(path: &str) -> Option<&str> {
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
            command: None,
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
            command: None,
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
            command: None,
            cpu_usage: 0.0,
        };
        assert!(!matches_executable(&proc, "/opt/games/cool-game/game"));
    }

    /// Two folders with a file of the same name, in a fresh temp folder.
    fn same_named_games() -> (std::path::PathBuf, String, String) {
        let root =
            std::env::temp_dir().join(format!("vaultime-match-test-{}", uuid::Uuid::new_v4()));
        let first = root.join("first").join("game");
        let second = root.join("second").join("game");
        for path in [&first, &second] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"").unwrap();
        }
        (
            root,
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        )
    }

    #[test]
    fn same_name_at_another_path_does_not_match() {
        let (root, first, second) = same_named_games();
        let proc = RunningProcess {
            pid: 5,
            name: "game".into(),
            exe_path: Some(second),
            command: None,
            cpu_usage: 0.0,
        };
        assert!(!matches_executable(&proc, &first));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unresolvable_process_path_falls_back_to_the_name() {
        let proc = RunningProcess {
            pid: 6,
            name: "game".into(),
            exe_path: Some("/container/only/game".into()),
            command: None,
            cpu_usage: 0.0,
        };
        assert!(matches_executable(&proc, "/opt/games/cool-game/game"));
    }

    #[test]
    fn wine_game_with_a_long_name_matches_by_command_line() {
        let proc = RunningProcess {
            pid: 12,
            name: "SomeVeryLongGam".into(),
            exe_path: Some("/usr/bin/wine64-preloader".into()),
            command: Some(r"Z:\home\me\Games\SomeVeryLongGameName.exe".into()),
            cpu_usage: 0.0,
        };
        assert!(matches_executable(
            &proc,
            "/home/me/Games/SomeVeryLongGameName.exe"
        ));
        assert!(!matches_executable(&proc, "/home/me/Games/OtherGame.exe"));
    }

    #[test]
    fn loader_process_matches_by_name() {
        let proc = RunningProcess {
            pid: 7,
            name: "game.exe".into(),
            exe_path: Some("/usr/bin/wine64-preloader".into()),
            command: None,
            cpu_usage: 0.0,
        };
        assert!(matches_executable(&proc, "/home/me/Games/Cool/game.exe"));
    }

    #[test]
    fn other_program_with_a_path_does_not_match_by_name() {
        let proc = RunningProcess {
            pid: 8,
            name: "firefox".into(),
            exe_path: Some("/usr/bin/firefox".into()),
            command: None,
            cpu_usage: 0.0,
        };
        assert!(!matches_executable(&proc, "/opt/games/firefox-game/game"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_library_matches() {
        let (root, first, _) = same_named_games();
        let link = root.join("library");
        std::os::unix::fs::symlink(root.join("first"), &link).unwrap();
        let proc = RunningProcess {
            pid: 9,
            name: "game".into(),
            exe_path: Some(first),
            command: None,
            cpu_usage: 0.0,
        };
        assert!(matches_executable(
            &proc,
            &link.join("game").to_string_lossy()
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn library_behind_a_junction_matches() {
        let (root, first, _) = same_named_games();
        let link = root.join("library");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(root.join("first"))
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        let proc = RunningProcess {
            pid: 10,
            name: "game".into(),
            exe_path: Some(first),
            command: None,
            cpu_usage: 0.0,
        };
        assert!(matches_executable(
            &proc,
            &link.join("game").to_string_lossy()
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    fn running(exe: &str) -> RunningProcess {
        RunningProcess {
            pid: 11,
            name: file_name(exe).unwrap().into(),
            exe_path: Some(exe.into()),
            command: None,
            cpu_usage: 0.0,
        }
    }

    #[cfg(windows)]
    #[test]
    fn runs_from_folder_at_any_depth() {
        let folder = r"C:\Games\Hades II";
        assert!(runs_from_folder(
            &running(r"C:\Games\Hades II\Ship\Hades2.exe"),
            folder
        ));
        assert!(runs_from_folder(
            &running(r"c:\games\hades ii\Release\Hades2.exe"),
            r"C:\Games\Hades II\"
        ));
        assert!(!runs_from_folder(
            &running(r"C:\Games\Hades II Demo\Hades2.exe"),
            folder
        ));
        assert!(!runs_from_folder(&running(r"C:\Games\Hades II"), folder));
    }

    #[cfg(unix)]
    #[test]
    fn runs_from_folder_at_any_depth() {
        let folder = "/games/hades";
        assert!(runs_from_folder(&running("/games/hades/bin/hades"), folder));
        assert!(!runs_from_folder(
            &running("/games/hades-demo/hades"),
            folder
        ));
        assert!(!runs_from_folder(&running("/games/other/hades"), "/"));
    }

    #[test]
    fn windows_style_path_matches_by_file_name() {
        let proc = RunningProcess {
            pid: 4,
            name: "game.exe".into(),
            exe_path: None,
            command: None,
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
        let procs = refresh_running_processes(&mut System::new());
        // Should at least find the current test process.
        assert!(!procs.is_empty());
    }
}
