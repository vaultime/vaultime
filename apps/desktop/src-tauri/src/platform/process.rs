// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Process enumeration and game executable matching.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Component, Path};

#[cfg(not(windows))]
use sysinfo::UpdateKind;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// A snapshot of a running process relevant for game matching.
#[derive(Debug, Clone)]
pub struct RunningProcess {
    /// OS process ID.
    pub pid: u32,
    /// Process name, for example "game.exe".
    pub name: String,
    /// Full executable path, if available.
    pub exe_path: Option<String>,
    /// First word of the command line. Wine puts the Windows path of the game
    /// there, while the executable path is Wine's own.
    pub command: Option<String>,
    /// `path_key` of `exe_path`, worked out once for all the install folders
    /// a tick checks it against.
    exe_key: Option<String>,
}

impl RunningProcess {
    pub fn new(pid: u32, name: String, exe_path: Option<String>, command: Option<String>) -> Self {
        let exe_key = exe_path.as_deref().map(path_key);
        Self {
            pid,
            name,
            exe_path,
            command,
            exe_key,
        }
    }
}

/// The running processes, read once per tick.
///
/// On Windows each tick takes the list of process ids and the program of
/// each id, read without opening the process, which any process allows. An
/// id that still names the program it named before is the same process, so
/// only new processes are opened, once, for the path Windows shows for their
/// program. Elsewhere sysinfo reads the list, with each path and command line
/// read once per process.
pub struct ProcessList {
    /// Measures the CPU usage of game processes, where it stands in for
    /// input, and lists the processes outside Windows.
    system: System,
    #[cfg(windows)]
    known: HashMap<u32, Known>,
    /// Takes the paths read, kept between ticks.
    #[cfg(windows)]
    buffer: Vec<u16>,
}

/// A process seen in an earlier tick.
#[cfg(any(windows, test))]
#[derive(Debug, Clone, PartialEq)]
struct Known {
    /// The program as the id names it, in the form of the kernel.
    image: String,
    name: String,
    exe_path: Option<String>,
}

impl Default for ProcessList {
    fn default() -> Self {
        Self {
            // sysinfo fails only on systems it does not support, and Windows
            // and Linux never do.
            system: System::new().expect("sysinfo supports this system"),
            #[cfg(windows)]
            known: HashMap::new(),
            #[cfg(windows)]
            buffer: Vec::new(),
        }
    }
}

impl ProcessList {
    /// The processes running now. Empty only when the list failed to load.
    #[cfg(windows)]
    pub fn refresh(&mut self) -> Vec<RunningProcess> {
        let Some(ids) = windows_list::process_ids() else {
            return Vec::new();
        };
        let buffer = &mut self.buffer;
        self.known = carry_over(
            std::mem::take(&mut self.known),
            ids,
            |pid| windows_list::image_by_id(pid, buffer),
            windows_list::shown_path,
        );
        self.known
            .iter()
            .map(|(&pid, process)| {
                RunningProcess::new(pid, process.name.clone(), process.exe_path.clone(), None)
            })
            .collect()
    }

    /// The processes running now. Empty only when the list failed to load.
    #[cfg(not(windows))]
    pub fn refresh(&mut self) -> Vec<RunningProcess> {
        // Paths and command lines do not change while a process runs, so they
        // are read once per process. CPU usage is left out, reading it for
        // every process costs twice as much as the whole list.
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_exe(UpdateKind::OnlyIfNotSet)
                .with_cmd(UpdateKind::OnlyIfNotSet),
        );
        self.system
            .processes()
            .values()
            .map(|p| {
                RunningProcess::new(
                    p.pid().as_u32(),
                    p.name().to_string_lossy().into_owned(),
                    p.exe().map(|e| e.to_string_lossy().into_owned()),
                    p.cmd()
                        .first()
                        .map(|arg| arg.to_string_lossy().into_owned()),
                )
            })
            .collect()
    }

    /// CPU usage in percent of the given processes since their previous
    /// measurement. A process measured for the first time reads 0.
    pub fn cpu_usage(&mut self, pids: &[u32]) -> HashMap<u32, f32> {
        let pids: Vec<Pid> = pids.iter().copied().map(Pid::from_u32).collect();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            false,
            ProcessRefreshKind::nothing().with_cpu(),
        );
        pids.iter()
            .filter_map(|pid| Some((pid.as_u32(), self.system.process(*pid)?.cpu_usage())))
            .collect()
    }
}

/// The processes of `ids`, those of `known` kept where their id still names
/// the same program. Processes that ended since the list was taken, and those
/// that run no program file, like the system's own, are left out. The path of
/// a new process is read once, through `shown_path`.
#[cfg(any(windows, test))]
fn carry_over(
    mut known: HashMap<u32, Known>,
    ids: Vec<u32>,
    mut image_of: impl FnMut(u32) -> Option<String>,
    mut shown_path: impl FnMut(u32) -> Option<String>,
) -> HashMap<u32, Known> {
    let mut current = HashMap::with_capacity(ids.len());
    for pid in ids {
        let Some(image) = image_of(pid) else {
            continue;
        };
        let process = match known.remove(&pid) {
            Some(process) if process.image == image => process,
            _ => {
                let Some(name) = file_name(&image).map(str::to_owned) else {
                    continue;
                };
                Known {
                    exe_path: shown_path(pid),
                    image,
                    name,
                }
            }
        };
        current.insert(pid, process);
    }
    current
}

#[cfg(windows)]
mod windows_list {
    use windows_sys::Wdk::System::SystemInformation::{
        NtQuerySystemInformation, SYSTEM_INFORMATION_CLASS,
    };
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_INSUFFICIENT_BUFFER, GetLastError, HANDLE, STATUS_INFO_LENGTH_MISMATCH,
        UNICODE_STRING,
    };
    use windows_sys::Win32::System::ProcessStatus::EnumProcesses;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };

    use crate::constants::{PROCESS_IDS_FIRST, PROCESS_PATH_UNITS_FIRST};

    /// The class of `NtQuerySystemInformation` that names the program of
    /// one process by its id. Windows has kept it since Vista without
    /// documenting it.
    const SYSTEM_PROCESS_ID_INFORMATION: SYSTEM_INFORMATION_CLASS = 88;

    /// The longest path Windows knows, in UTF-16 units.
    const LONG_PATH_UNITS: usize = 32_768;

    /// The most UTF-16 units a kernel string holds, its length being 16 bits
    /// of bytes.
    const NAME_UNITS_MAX: usize = 32_767;

    #[repr(C)]
    struct ProcessIdInformation {
        process_id: usize,
        image_name: UNICODE_STRING,
    }

    /// The ids of the running processes. Processes that ended are left out,
    /// also while something still holds them open.
    pub fn process_ids() -> Option<Vec<u32>> {
        let mut ids = vec![0_u32; PROCESS_IDS_FIRST];
        loop {
            let bytes = u32::try_from(size_of_val(ids.as_slice())).ok()?;
            let mut used = 0;
            // SAFETY: `ids` holds `bytes` bytes and outlives the call, which
            // writes how many it filled into `used`.
            if unsafe { EnumProcesses(ids.as_mut_ptr(), bytes, &raw mut used) } == 0 {
                return None;
            }
            // A list that fills the room may have been cut short.
            if used < bytes {
                ids.truncate(used as usize / size_of::<u32>());
                return Some(ids);
            }
            ids.resize(ids.len() * 2, 0);
        }
    }

    /// The program of a process, as the kernel names it, read without
    /// opening the process. None once the process ended, and empty for one
    /// that runs no program file.
    pub fn image_by_id(pid: u32, buffer: &mut Vec<u16>) -> Option<String> {
        if buffer.len() < PROCESS_PATH_UNITS_FIRST {
            buffer.resize(PROCESS_PATH_UNITS_FIRST, 0);
        }
        loop {
            let room = u16::try_from(buffer.len() * 2).unwrap_or(u16::MAX - 1);
            let mut info = ProcessIdInformation {
                process_id: usize::try_from(pid).ok()?,
                image_name: UNICODE_STRING {
                    Length: 0,
                    MaximumLength: room,
                    Buffer: buffer.as_mut_ptr(),
                },
            };
            let size = u32::try_from(size_of::<ProcessIdInformation>()).ok()?;
            // SAFETY: `info` points at `buffer`, which holds `room` bytes,
            // and both outlive the call.
            let status = unsafe {
                NtQuerySystemInformation(
                    SYSTEM_PROCESS_ID_INFORMATION,
                    (&raw mut info).cast(),
                    size,
                    std::ptr::null_mut(),
                )
            };
            // The call names the room the path needs when it lacked it, and
            // where it names none the path gets all the room there is.
            let needed = usize::from(info.image_name.MaximumLength).div_ceil(2);
            let room_after = if needed > buffer.len() {
                needed
            } else {
                NAME_UNITS_MAX
            };
            if status == STATUS_INFO_LENGTH_MISMATCH && room_after > buffer.len() {
                buffer.resize(room_after, 0);
                continue;
            }
            if status < 0 {
                return None;
            }
            let units = usize::from(info.image_name.Length) / 2;
            return Some(String::from_utf16_lossy(&buffer[..units]));
        }
    }

    /// The path of the program a process runs, as Windows shows it. Reading
    /// it opens the process, which elevated processes allow too, but not
    /// those of other users or protected ones.
    pub fn shown_path(pid: u32) -> Option<String> {
        // SAFETY: plain call, a failure returns null.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let path = path_of(handle);
        // SAFETY: the handle came from OpenProcess and is closed once.
        unsafe { CloseHandle(handle) };
        path
    }

    fn path_of(handle: HANDLE) -> Option<String> {
        let mut path = vec![0_u16; PROCESS_PATH_UNITS_FIRST];
        loop {
            let mut units = u32::try_from(path.len()).ok()?;
            // SAFETY: `path` holds `units` units and outlives the call, which
            // writes how many it used back into `units`.
            let read = unsafe {
                QueryFullProcessImageNameW(
                    handle,
                    PROCESS_NAME_WIN32,
                    path.as_mut_ptr(),
                    &raw mut units,
                )
            };
            if read != 0 {
                return Some(String::from_utf16_lossy(&path[..units as usize]));
            }
            // SAFETY: plain call, about the call above on this thread.
            if unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER
                || path.len() >= LONG_PATH_UNITS
            {
                return None;
            }
            path.resize(LONG_PATH_UNITS, 0);
        }
    }
}

/// Checks whether a running process matches a game's registered executable.
///
/// The full path decides when the process has one, so two games that share a
/// file name like `Game.exe` stay apart. A different path still matches when
/// both lead to the same file, for example through a junction or a symlinked
/// library folder. The file name alone decides when the path is hidden, as for
/// processes of other users or protected ones, or when it belongs to a loader
/// such as Wine, which keeps the game's name but reports its own path.
pub fn matches_executable(process: &RunningProcess, game_executable: &str) -> bool {
    let Some(game_name) = file_name(game_executable) else {
        return false;
    };
    let Some(exe) = &process.exe_path else {
        return names_equal(&process.name, game_name);
    };

    // Names first, they rule out almost every process without allocating.
    match file_name(exe) {
        Some(exe_name) if names_equal(exe_name, game_name) => {
            paths_equal(exe, game_executable) || same_file(exe, game_executable)
        }
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
#[cfg(test)]
pub fn runs_from_folder(process: &RunningProcess, folder: &str) -> bool {
    InstallFolder::new(folder).contains(process)
}

/// A game's install folder, resolved once, so checking many processes
/// against it needs no file system calls. It is also compared with links
/// resolved, so a library behind a junction or symlink still counts.
pub struct InstallFolder {
    keys: Vec<String>,
}

impl InstallFolder {
    pub fn new(folder: &str) -> Self {
        // The root of a drive or share holds every program on it, so it
        // never counts as a game's folder, also where a link leads there.
        if !below_a_root(folder) {
            return Self { keys: Vec::new() };
        }
        let resolved = std::fs::canonicalize(folder)
            .ok()
            .map(|path| strip_verbatim_prefix(&path.to_string_lossy()).to_owned());
        // A share that is not open now still names its folder.
        let checked = resolved.as_deref().unwrap_or(folder);
        if is_mount_point(checked) || is_network_place(checked) {
            return Self { keys: Vec::new() };
        }
        let resolved = resolved.filter(|path| below_a_root(path));
        let mut keys: Vec<String> = Vec::new();
        for candidate in std::iter::once(folder).chain(resolved.as_deref()) {
            let key = path_key(candidate).trim_end_matches(['/', '\\']).to_owned();
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        Self { keys }
    }

    /// Whether the process runs a program from inside the folder, at any depth.
    pub fn contains(&self, process: &RunningProcess) -> bool {
        let Some(exe) = &process.exe_key else {
            return false;
        };
        self.keys.iter().any(|folder| is_inside(exe, folder))
    }

    /// Whether the folder is a whole drive or share, so no program counts as
    /// inside it.
    pub fn is_refused(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Whether `path` names a folder below the root of a drive, a share or the
/// file system.
fn below_a_root(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|component| matches!(component, Component::Normal(_)))
}

/// Whether a drive or share is mounted at `path`. Linux lists every mount in
/// its mount table, where a btrfs subvolume that was never mounted is a
/// folder like any other.
#[cfg(target_os = "linux")]
fn is_mount_point(path: &str) -> bool {
    std::fs::read_to_string("/proc/self/mountinfo").map_or_else(
        |_| on_another_device(path),
        |table| mount_points(&table).any(|point| Path::new(&point) == Path::new(path)),
    )
}

#[cfg(all(unix, not(target_os = "linux")))]
fn is_mount_point(path: &str) -> bool {
    on_another_device(path)
}

/// Whether the folder lies on another device than the one above it, as a
/// drive mounted at a folder does.
#[cfg(unix)]
fn on_another_device(path: &str) -> bool {
    use std::os::unix::fs::MetadataExt;
    let path = Path::new(path);
    let device = |path: &Path| std::fs::metadata(path).map(|meta| meta.dev()).ok();
    path.parent()
        .and_then(device)
        .is_some_and(|above| device(path).is_some_and(|here| here != above))
}

/// The mount points of a table in the format of `/proc/self/mountinfo`,
/// whose fifth field is the mount point.
#[cfg(any(target_os = "linux", test))]
fn mount_points(table: &str) -> impl Iterator<Item = String> + '_ {
    table
        .lines()
        .filter_map(|line| line.split(' ').nth(4))
        .map(unescape_octal)
}

/// Undoes the `\ooo` escapes the kernel writes in the mount table for a
/// space, tab, newline or backslash.
#[cfg(any(target_os = "linux", test))]
fn unescape_octal(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut unescaped = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'\\')
            .then(|| field.get(index + 1..index + 4))
            .flatten()
            .filter(|digits| digits.bytes().all(|digit| (b'0'..=b'7').contains(&digit)))
            .and_then(|digits| u8::from_str_radix(digits, 8).ok());
        if let Some(byte) = escaped {
            unescaped.push(byte);
            index += 4;
        } else {
            unescaped.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&unescaped).into_owned()
}

/// Whether `path` is a share, or a folder above shares, where GNOME and KDE
/// open network places: a share is a folder of its own in
/// `/run/user/<uid>/gvfs`, and `<protocol>/<host>/<share>` in
/// `/run/user/<uid>/kio-fuse-*`.
fn is_network_place(path: &str) -> bool {
    let Ok(rest) = Path::new(path).strip_prefix("/run/user") else {
        return false;
    };
    // The parts of a path given as text are text.
    let parts: Vec<&str> = rest.iter().filter_map(OsStr::to_str).collect();
    let is_uid = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    match parts.as_slice() {
        [uid, "gvfs", below @ ..] if is_uid(uid) => below.len() <= 1,
        [uid, place, below @ ..] if is_uid(uid) && place.starts_with("kio-fuse-") => {
            below.len() <= 3
        }
        _ => false,
    }
}

/// Whether a drive is mounted at `path`: the folder is the root of the volume
/// it lies on, as a drive letter's root is, or a folder a drive is mounted at.
#[cfg(windows)]
fn is_mount_point(path: &str) -> bool {
    use windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW;
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    // The root is the path or a part of it, with a backslash at the end.
    let mut root = vec![0_u16; wide.len() + 1];
    let Ok(capacity) = u32::try_from(root.len()) else {
        return false;
    };
    // SAFETY: `wide` is null-terminated and `root` holds `capacity` units,
    // both outlive the call.
    if unsafe { GetVolumePathNameW(wide.as_ptr(), root.as_mut_ptr(), capacity) } == 0 {
        return false;
    }
    let end = root
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(root.len());
    path_key(&String::from_utf16_lossy(&root[..end])) == path_key(path)
}

#[cfg(not(any(unix, windows)))]
fn is_mount_point(_path: &str) -> bool {
    false
}

/// True when `path` lies below `folder`, both given as path keys.
fn is_inside(path: &str, folder: &str) -> bool {
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

/// The comparison key of `path` with links and short names resolved, or of
/// the path as given when it cannot be resolved.
#[cfg(windows)]
pub(crate) fn resolved_key(path: &str) -> String {
    std::fs::canonicalize(path).map_or_else(
        |_| path_key(path),
        |resolved| path_key(strip_verbatim_prefix(&resolved.to_string_lossy())),
    )
}

/// Last path segment, splitting on both separators so Windows paths parse on any OS.
pub(crate) fn file_name(path: &str) -> Option<&str> {
    path.rsplit(['/', '\\']).find(|segment| !segment.is_empty())
}

fn paths_equal(a: &str, b: &str) -> bool {
    path_key(a) == path_key(b)
}

/// Comparison key for executable paths. On Windows it ignores case and the
/// `\\?\` resolved paths start with, and collapses separators, so
/// `C:/Games//x.exe` equals `c:\games\X.EXE`, and `\\?\UNC\nas\games\x.exe`
/// the `\\nas\games\x.exe` a program on a mapped drive runs as.
#[cfg(windows)]
pub fn path_key(path: &str) -> String {
    let path = path
        .strip_prefix(r"\\?\UNC\")
        .or_else(|| path.strip_prefix(r"\\?\"))
        .unwrap_or(path);
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

/// Ignores case like `path_key`, without allocating.
#[cfg(windows)]
fn names_equal(a: &str, b: &str) -> bool {
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(b);
    }
    a.chars()
        .flat_map(char::to_lowercase)
        .eq(b.chars().flat_map(char::to_lowercase))
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
        let proc = RunningProcess::new(
            1,
            "game".into(),
            Some("/opt/games/cool-game/game".into()),
            None,
        );
        assert!(matches_executable(&proc, "/opt/games/cool-game/game"));
    }

    #[test]
    fn filename_match() {
        let proc = RunningProcess::new(2, "game".into(), None, None);
        assert!(matches_executable(&proc, "/opt/games/cool-game/game"));
    }

    #[test]
    fn no_match() {
        let proc = RunningProcess::new(3, "firefox".into(), Some("/usr/bin/firefox".into()), None);
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
        let proc = RunningProcess::new(5, "game".into(), Some(second), None);
        assert!(!matches_executable(&proc, &first));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unresolvable_process_path_falls_back_to_the_name() {
        let proc = RunningProcess::new(6, "game".into(), Some("/container/only/game".into()), None);
        assert!(matches_executable(&proc, "/opt/games/cool-game/game"));
    }

    #[test]
    fn wine_game_with_a_long_name_matches_by_command_line() {
        let proc = RunningProcess::new(
            12,
            "SomeVeryLongGam".into(),
            Some("/usr/bin/wine64-preloader".into()),
            Some(r"Z:\home\me\Games\SomeVeryLongGameName.exe".into()),
        );
        assert!(matches_executable(
            &proc,
            "/home/me/Games/SomeVeryLongGameName.exe"
        ));
        assert!(!matches_executable(&proc, "/home/me/Games/OtherGame.exe"));
    }

    #[test]
    fn loader_process_matches_by_name() {
        let proc = RunningProcess::new(
            7,
            "game.exe".into(),
            Some("/usr/bin/wine64-preloader".into()),
            None,
        );
        assert!(matches_executable(&proc, "/home/me/Games/Cool/game.exe"));
    }

    #[test]
    fn other_program_with_a_path_does_not_match_by_name() {
        let proc = RunningProcess::new(8, "firefox".into(), Some("/usr/bin/firefox".into()), None);
        assert!(!matches_executable(&proc, "/opt/games/firefox-game/game"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_library_matches() {
        let (root, first, _) = same_named_games();
        let link = root.join("library");
        std::os::unix::fs::symlink(root.join("first"), &link).unwrap();
        let proc = RunningProcess::new(9, "game".into(), Some(first), None);
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
        let proc = RunningProcess::new(10, "game".into(), Some(first), None);
        assert!(matches_executable(
            &proc,
            &link.join("game").to_string_lossy()
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    fn running(exe: &str) -> RunningProcess {
        RunningProcess::new(11, file_name(exe).unwrap().into(), Some(exe.into()), None)
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

    #[cfg(windows)]
    #[test]
    fn the_root_of_a_drive_or_share_is_no_install_folder() {
        let browser = running(r"C:\Program Files\Browser\browser.exe");
        for root in [r"C:\", "C:", "c:/", r"\\?\C:\"] {
            assert!(!runs_from_folder(&browser, root), "{root}");
        }
        assert!(!below_a_root(r"\\server\share"));
        assert!(!below_a_root(r"\\?\UNC\server\share\"));
        assert!(below_a_root(r"\\server\share\Game"));
        assert!(below_a_root(r"D:\Game"));
    }

    #[cfg(windows)]
    #[test]
    fn a_resolved_path_has_the_key_of_the_path_it_names() {
        assert_eq!(path_key(r"\\?\C:\Games\Foo"), path_key(r"C:\Games\Foo"));
        assert_eq!(
            path_key(r"\\?\UNC\nas\games\Foo"),
            path_key(r"\\nas\games\Foo")
        );
        // A game on a mapped drive runs from the share the drive stands for.
        assert!(is_inside(
            &path_key(r"\\nas\games\Foo\Binaries\Foo.exe"),
            &path_key(r"\\?\UNC\nas\games\Foo")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn the_root_of_a_volume_is_a_mount_point_and_a_folder_on_it_is_not() {
        let drive = std::env::var("SystemDrive").unwrap();
        assert!(is_mount_point(&format!(r"{drive}\")));
        let folder = std::env::temp_dir().join(format!("vaultime-mount-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let resolved = std::fs::canonicalize(&folder).unwrap();
        assert!(!is_mount_point(strip_verbatim_prefix(
            &resolved.to_string_lossy()
        )));
        let game = running(&format!(r"{}\game.exe", folder.display()));
        assert!(runs_from_folder(&game, &folder.to_string_lossy()));
        std::fs::remove_dir(&folder).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_drive_mounted_at_a_folder_is_no_install_folder() {
        // /proc is a file system of its own, as a drive mounted at a folder is.
        assert!(!runs_from_folder(&running("/proc/self/exe"), "/proc"));
        let folder = std::env::temp_dir().join(format!("vaultime-mount-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let game = running(&format!("{}/game", folder.display()));
        assert!(runs_from_folder(&game, &folder.to_string_lossy()));
        std::fs::remove_dir(&folder).unwrap();
    }

    #[test]
    fn the_mount_table_names_its_mount_points() {
        let table = r"22 1 0:21 / / rw,relatime shared:1 - btrfs /dev/nvme0n1p2 rw,subvol=/root
35 22 0:21 /home /home rw,relatime shared:28 - btrfs /dev/nvme0n1p2 rw,subvol=/home
61 22 8:17 / /mnt/My\040Games rw,relatime shared:33 - ext4 /dev/sdb1 rw
70 22 0:55 / /run/user/1000/gvfs rw,nosuid,nodev,relatime shared:40 - fuse.gvfsd-fuse gvfsd-fuse rw
";
        assert_eq!(
            mount_points(table).collect::<Vec<_>>(),
            ["/", "/home", "/mnt/My Games", "/run/user/1000/gvfs"]
        );
        assert_eq!(unescape_octal(r"/mnt/a\134b\011c"), "/mnt/a\\b\tc");
        assert_eq!(unescape_octal(r"/mnt/a\9b\04"), r"/mnt/a\9b\04");
    }

    #[test]
    fn a_share_the_desktop_opened_is_no_install_folder() {
        for place in [
            "/run/user/1000/gvfs/smb-share:server=nas,share=games",
            "/run/user/1000/gvfs",
            "/run/user/1000/kio-fuse-AbCdEf/smb/nas/games",
            "/run/user/1000/kio-fuse-AbCdEf/smb/nas",
        ] {
            assert!(is_network_place(place), "{place}");
        }
        for folder in [
            "/run/user/1000/gvfs/smb-share:server=nas,share=games/Hades",
            "/run/user/1000/kio-fuse-AbCdEf/smb/nas/games/Hades",
            "/run/user/me/gvfs/share",
            "/home/me/gvfs/share",
        ] {
            assert!(!is_network_place(folder), "{folder}");
        }
        if cfg!(unix) {
            // Also while the share is not open, so its folder does not exist.
            let share = "/run/user/1000/gvfs/smb-share:server=nas,share=games";
            assert!(!runs_from_folder(
                &running(&format!("{share}/Hades/hades")),
                share
            ));
        }
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
        let proc = RunningProcess::new(4, "game.exe".into(), None, None);
        assert!(matches_executable(&proc, r"C:\Games\Cool\game.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_ignore_case_and_separators() {
        assert!(paths_equal(
            r"C:\Games\Cool\Game.exe",
            "c:/games//cool/game.EXE"
        ));
        assert!(names_equal("Ärger.exe", "äRGER.EXE"));
        assert!(!names_equal("Ärger.exe", "Arger.exe"));
    }

    #[test]
    fn an_id_keeps_its_process_while_it_names_the_same_program() {
        let game = r"\Device\HarddiskVolume3\Games\game.exe";
        let tool = r"\Device\HarddiskVolume3\Tools\tool.exe";
        let other = r"\Device\HarddiskVolume3\Tools\other.exe";
        let image = |images: &[(u32, &str)], pid: u32| {
            images
                .iter()
                .find(|(id, _)| *id == pid)
                .map(|(_, image)| (*image).to_owned())
        };
        let mut opened = Vec::new();
        // 2 runs no program file, 3 ended since the list was taken.
        let first = carry_over(
            HashMap::new(),
            vec![1, 2, 3, 4],
            |pid| image(&[(1, game), (2, ""), (4, tool)], pid),
            |pid| {
                opened.push(pid);
                Some(format!("C:\\shown\\{pid}"))
            },
        );
        let mut ids: Vec<u32> = first.keys().copied().collect();
        ids.sort_unstable();
        assert_eq!(ids, [1, 4]);
        assert_eq!(first[&1].name, "game.exe");
        assert_eq!(first[&1].exe_path.as_deref(), Some("C:\\shown\\1"));
        opened.sort_unstable();
        assert_eq!(opened, [1, 4]);

        // The id of the tool passed on to another program.
        let mut opened = Vec::new();
        let second = carry_over(
            first.clone(),
            vec![1, 4],
            |pid| image(&[(1, game), (4, other)], pid),
            |pid| {
                opened.push(pid);
                None
            },
        );
        assert_eq!(opened, [4]);
        assert_eq!(second[&1], first[&1]);
        assert_eq!(second[&4].name, "other.exe");
        assert!(second[&4].exe_path.is_none());

        let third = carry_over(second, vec![4], |pid| image(&[(4, other)], pid), |_| None);
        assert_eq!(third.keys().copied().collect::<Vec<_>>(), [4]);
    }

    /// A process that waits half a minute, for a test to see.
    fn sleeper() -> std::process::Child {
        let mut command = if cfg!(windows) {
            let mut command = std::process::Command::new("ping");
            command.args(["-n", "30", "127.0.0.1"]);
            command
        } else {
            let mut command = std::process::Command::new("sleep");
            command.arg("30");
            command
        };
        command.stdout(std::process::Stdio::null()).spawn().unwrap()
    }

    #[test]
    fn the_list_shows_a_process_from_its_start_to_its_end() {
        let mut list = ProcessList::default();
        list.refresh();
        let mut child = sleeper();
        let pid = child.id();
        let name = if cfg!(windows) { "ping.exe" } else { "sleep" };
        // Linux may list the child before it runs its program, under the
        // name of the thread that started it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let listed = loop {
            let listed = list
                .refresh()
                .into_iter()
                .find(|process| process.pid == pid)
                .expect("the new process is listed");
            if names_equal(&listed.name.to_lowercase(), name)
                || std::time::Instant::now() > deadline
            {
                break listed;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        assert!(names_equal(&listed.name.to_lowercase(), name), "{listed:?}");
        assert!(listed.exe_path.is_some(), "{listed:?}");
        child.kill().unwrap();
        child.wait().unwrap();
        // Windows may list a process for a moment after it ended.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while list.refresh().iter().any(|process| process.pid == pid) {
            assert!(
                std::time::Instant::now() < deadline,
                "the ended process is gone"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn list_processes_runs() {
        let procs = ProcessList::default().refresh();
        // Should at least find the current test process.
        assert!(!procs.is_empty());
    }
}
