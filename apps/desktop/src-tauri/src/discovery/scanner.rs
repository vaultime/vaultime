// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Folder scanner that finds game executables in common install locations.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use log::info;
use walkdir::WalkDir;

use crate::constants::DISCOVERY_SCAN_DEPTH;
use crate::db::connection::Database;
use crate::error::Result;
use crate::platform::process::path_key;

use super::{DiscoveredGame, is_executable, library_executables, metadata};

/// Scans the given folders for game executables.
///
/// Games already in the library come back with `already_added` set so the UI
/// can tell new finds from duplicates.
pub fn scan_folders(db: &Database, paths: &[String]) -> Result<Vec<DiscoveredGame>> {
    let existing = library_executables(db)?;
    let mut seen = HashSet::new();
    let mut results = Vec::new();

    for root in paths.iter().map(Path::new).filter(|root| root.is_dir()) {
        info!("scanning folder for games: {}", root.display());

        let entries = WalkDir::new(root)
            .max_depth(DISCOVERY_SCAN_DEPTH)
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file() && is_executable(entry.path()))
            .filter(|entry| {
                metadata::is_likely_game_executable(&entry.file_name().to_string_lossy())
            });

        for entry in entries {
            let exe_path = entry.path().to_string_lossy().into_owned();
            let key = path_key(&exe_path);
            if !seen.insert(key.clone()) {
                continue;
            }

            results.push(DiscoveredGame {
                title: metadata::infer_title(&exe_path),
                install_folder: entry
                    .path()
                    .parent()
                    .map(|parent| parent.to_string_lossy().into_owned()),
                already_added: existing.contains(&key),
                executable_path: exe_path,
                source: "folder_scan".into(),
                source_id: None,
            });
        }
    }

    info!("folder scan found {} candidate(s)", results.len());
    Ok(results)
}

/// Default folders to scan on this platform. Only existing folders are returned.
///
/// Steam libraries are left out because Steam discovery covers them.
pub fn default_scan_paths() -> Vec<String> {
    candidate_paths()
        .into_iter()
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

#[cfg(windows)]
fn candidate_paths() -> Vec<PathBuf> {
    // Default library folders of the common launchers, relative to a drive root.
    const LIBRARY_FOLDERS: &[&str] = &[
        "Games",
        "Epic Games",
        "GOG Games",
        "XboxGames",
        r"Program Files\Epic Games",
        r"Program Files (x86)\GOG Galaxy\Games",
        r"Program Files\EA Games",
        r"Program Files (x86)\Ubisoft\Ubisoft Game Launcher\games",
        r"Program Files (x86)\Battle.net\Games",
    ];

    fixed_drives()
        .into_iter()
        .flat_map(|drive| LIBRARY_FOLDERS.iter().map(move |folder| drive.join(folder)))
        .collect()
}

/// Roots of the local fixed drives, skipping removable and network drives.
#[cfg(windows)]
fn fixed_drives() -> Vec<PathBuf> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;

    // SAFETY: GetLogicalDrives has no preconditions.
    let mask = unsafe { GetLogicalDrives() };

    (0..26_u8)
        .filter(|index| mask & (1 << index) != 0)
        .map(|index| char::from(b'A' + index))
        .filter(|letter| {
            let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain([0]).collect();
            // SAFETY: `root` is a null-terminated UTF-16 string that outlives the call.
            unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_FIXED }
        })
        .map(|letter| PathBuf::from(format!("{letter}:\\")))
        .collect()
}

#[cfg(target_os = "linux")]
fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        // Heroic and Lutris both default to ~/Games.
        paths.push(home.join("Games"));
        paths.push(home.join("games"));
    }
    paths.push(PathBuf::from("/opt/games"));
    paths.push(PathBuf::from("/usr/games"));
    paths
}

#[cfg(target_os = "macos")]
fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join("Applications"));
    }
    paths
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn candidate_paths() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_exist() {
        assert!(
            default_scan_paths()
                .iter()
                .all(|path| Path::new(path).is_dir())
        );
    }

    #[cfg(windows)]
    #[test]
    fn system_drive_is_fixed() {
        assert!(
            fixed_drives()
                .iter()
                .any(|drive| drive.join("Windows").is_dir())
        );
    }
}
