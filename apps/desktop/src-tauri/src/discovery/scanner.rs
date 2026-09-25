// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Folder scanner — finds game executables in common install locations.

use std::collections::HashSet;
use std::path::Path;

use log::info;
use walkdir::WalkDir;

use crate::db::connection::Database;
use crate::db::repo::games;
use crate::error::Result;

use super::DiscoveredGame;
use super::metadata;

/// Maximum directory depth when scanning for executables.
const MAX_SCAN_DEPTH: usize = 4;

/// Scan the given directories for game executables.
///
/// Returns a list of discovered game candidates. Games already in the
/// library are marked with `already_added = true` so the UI can
/// distinguish new finds from duplicates.
pub fn scan_folders(db: &Database, paths: &[String]) -> Result<Vec<DiscoveredGame>> {
    let existing_exes = collect_existing_executables(db)?;
    let mut results: Vec<DiscoveredGame> = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();

    for scan_root in paths {
        let root = Path::new(scan_root);
        if !root.is_dir() {
            continue;
        }

        info!("scanning folder for games: {}", root.display());

        for entry in WalkDir::new(root)
            .max_depth(MAX_SCAN_DEPTH)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if !is_executable(path) {
                continue;
            }

            let filename = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();

            if !metadata::is_likely_game_executable(&filename) {
                continue;
            }

            let exe_path = path.to_string_lossy().into_owned();

            // Deduplicate within this scan.
            if !seen_paths.insert(exe_path.clone()) {
                continue;
            }

            let install_folder = path.parent().map(|p| p.to_string_lossy().into_owned());

            let title = metadata::infer_title(&exe_path);
            let already_added = existing_exes.contains(&exe_path);

            results.push(DiscoveredGame {
                title,
                executable_path: exe_path,
                install_folder,
                source: "folder_scan".into(),
                source_id: None,
                already_added,
            });
        }
    }

    info!("folder scan found {} candidate(s)", results.len());
    Ok(results)
}

/// Returns the default set of directories to scan on the current platform.
pub fn default_scan_paths() -> Vec<String> {
    let mut paths = Vec::new();

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            // Common Linux game locations.
            let candidates = [
                home.join("Games"),
                home.join("games"),
                home.join(".local/share/applications"),
            ];
            for p in &candidates {
                if p.is_dir() {
                    paths.push(p.to_string_lossy().into_owned());
                }
            }
        }
        // Flatpak and system locations.
        for p in &["/opt/games", "/usr/games"] {
            if Path::new(p).is_dir() {
                paths.push((*p).to_string());
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let candidates: Vec<PathBuf> = vec![
            PathBuf::from(r"C:\Program Files"),
            PathBuf::from(r"C:\Program Files (x86)"),
            PathBuf::from(r"D:\Games"),
            PathBuf::from(r"E:\Games"),
        ];
        for p in &candidates {
            if p.is_dir() {
                paths.push(p.to_string_lossy().into_owned());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        paths.push("/Applications".into());
        if let Some(home) = dirs::home_dir() {
            let app_support = home.join("Applications");
            if app_support.is_dir() {
                paths.push(app_support.to_string_lossy().into_owned());
            }
        }
    }

    paths
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if a file is likely an executable based on extension / permissions.
fn is_executable(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        matches!(ext.as_str(), "exe" | "bat" | "cmd")
    }

    #[cfg(target_os = "macos")]
    {
        if ext == "app" {
            return true;
        }
        has_execute_permission(path)
    }

    #[cfg(target_os = "linux")]
    {
        if matches!(ext.as_str(), "sh" | "x86_64" | "x86") {
            return true;
        }
        has_execute_permission(path)
    }
}

#[cfg(unix)]
fn has_execute_permission(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Collect the set of executable paths already registered in the library.
fn collect_existing_executables(db: &Database) -> Result<HashSet<String>> {
    let all_games = games::list_all_games(db)?;
    Ok(all_games
        .into_iter()
        .filter_map(|g| g.executable_path)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_returns_list() {
        let paths = default_scan_paths();
        // Should return at least an empty list without panicking.
        assert!(paths.len() >= 0);
    }
}
