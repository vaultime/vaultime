// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Steam library discovery.
//!
//! Reads `libraryfolders.vdf` for the library folders, then every
//! `appmanifest_*.acf` inside them. Both are simple key-value text files, so a
//! small parser is enough.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use log::info;

use crate::db::connection::Database;
use crate::error::Result;
use crate::platform::process::path_key;

use super::{DiscoveredGame, is_executable, library_executables, metadata};

/// Finds every installed Steam game with a launchable executable.
pub fn discover_steam_games(db: &Database) -> Result<Vec<DiscoveredGame>> {
    let Some(root) = find_steam_root() else {
        info!("Steam installation not found");
        return Ok(Vec::new());
    };
    info!("found Steam root: {}", root.display());

    let existing = library_executables(db)?;
    let mut results = Vec::new();

    for library in find_library_folders(&root) {
        let steamapps = library.join("steamapps");
        for manifest in find_app_manifests(&steamapps) {
            if let Some(game) = parse_app_manifest(&manifest, &steamapps, &existing) {
                results.push(game);
            }
        }
    }

    info!("Steam discovery found {} game(s)", results.len());
    Ok(results)
}

fn find_steam_root() -> Option<PathBuf> {
    steam_root_candidates()
        .into_iter()
        .find(|path| path.is_dir())
}

fn steam_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(windows)]
    {
        candidates.extend(registry_steam_root());
        candidates.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        candidates.push(PathBuf::from(r"C:\Program Files\Steam"));
    }

    #[cfg(target_os = "linux")]
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".steam/steam"));
        candidates.push(home.join(".local/share/Steam"));
        candidates.push(home.join(".steam/debian-installation"));
        candidates.push(home.join(".var/app/com.valvesoftware.Steam/.steam/steam"));
        candidates.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
        candidates.push(home.join("snap/steam/common/.local/share/Steam"));
    }

    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("Library/Application Support/Steam"));
    }

    candidates
}

/// Steam writes its install location to the registry, which also covers
/// installs outside Program Files.
#[cfg(windows)]
fn registry_steam_root() -> Option<PathBuf> {
    use windows_registry::{CURRENT_USER, LOCAL_MACHINE};

    CURRENT_USER
        .open(r"Software\Valve\Steam")
        .and_then(|key| key.get_string("SteamPath"))
        .or_else(|_| {
            LOCAL_MACHINE
                .open(r"SOFTWARE\WOW6432Node\Valve\Steam")
                .and_then(|key| key.get_string("InstallPath"))
        })
        .ok()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

/// The Steam root plus every extra library listed in `libraryfolders.vdf`.
fn find_library_folders(steam_root: &Path) -> Vec<PathBuf> {
    let mut folders = vec![steam_root.to_path_buf()];
    let mut seen: HashSet<String> = HashSet::from([path_key(&steam_root.to_string_lossy())]);

    let vdf = steam_root.join("steamapps/libraryfolders.vdf");
    if let Ok(content) = fs::read_to_string(vdf) {
        for path in parse_library_paths(&content) {
            let folder = PathBuf::from(&path);
            if folder.is_dir() && seen.insert(path_key(&path)) {
                folders.push(folder);
            }
        }
    }

    folders
}

fn parse_library_paths(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| extract_vdf_value(line, "path"))
        .collect()
}

fn find_app_manifests(steamapps_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(steamapps_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            name.starts_with("appmanifest_")
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("acf"))
        })
        .collect()
}

fn parse_app_manifest(
    manifest_path: &Path,
    steamapps_dir: &Path,
    existing: &HashSet<String>,
) -> Option<DiscoveredGame> {
    let content = fs::read_to_string(manifest_path).ok()?;

    let app_id = extract_acf_field(&content, "appid")?;
    let name = extract_acf_field(&content, "name")?;
    let install_dir_name = extract_acf_field(&content, "installdir")?;

    if is_steam_tool(&name, &app_id) {
        return None;
    }

    let install_folder = steamapps_dir.join("common").join(&install_dir_name);
    if !install_folder.is_dir() {
        return None;
    }

    let executable = find_main_executable(&install_folder)?;
    let executable_path = executable.to_string_lossy().into_owned();
    let already_added = existing.contains(&path_key(&executable_path));

    Some(DiscoveredGame {
        title: name,
        executable_path,
        install_folder: Some(install_folder.to_string_lossy().into_owned()),
        source: "steam".into(),
        source_id: Some(app_id),
        already_added,
    })
}

/// Picks the most likely game binary in an install folder.
///
/// The largest executable wins since the game binary is usually bigger than
/// helpers and tools. Files in the top folder get a 100 MB head start.
fn find_main_executable(install_dir: &Path) -> Option<PathBuf> {
    const TOP_LEVEL_BONUS: u64 = 100_000_000;

    walkdir::WalkDir::new(install_dir)
        .max_depth(3)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file() && is_executable(entry.path()))
        .filter(|entry| metadata::is_likely_game_executable(&entry.file_name().to_string_lossy()))
        .max_by_key(|entry| {
            let size = entry.metadata().map_or(0, |meta| meta.len());
            if entry.depth() <= 1 {
                size + TOP_LEVEL_BONUS
            } else {
                size
            }
        })
        .map(walkdir::DirEntry::into_path)
}

/// True for redistributables, Proton builds and other Steam tools.
fn is_steam_tool(name: &str, app_id: &str) -> bool {
    let lower = name.to_lowercase();

    if lower.contains("redistributable")
        || lower.contains("redist")
        || lower.contains("proton")
        || lower.contains("steam linux runtime")
        || lower.contains("steamworks")
        || lower.starts_with("steam ")
        || lower.contains("directx")
        || lower.contains("vcredist")
    {
        return true;
    }

    matches!(
        app_id,
        "228980"  // Steamworks Common Redistributables
        | "1070560" // Steam Linux Runtime
        | "1887720" // Proton
        | "2180100" // Proton Hotfix
        | "2348590" // Proton 9
    )
}

/// Reads the value of a `"key"  "value"` line.
fn extract_vdf_value(line: &str, key: &str) -> Option<String> {
    let rest = line.trim().strip_prefix(&format!("\"{key}\""))?;
    let inner = rest.trim().strip_prefix('"')?;

    // Values escape backslashes and quotes, which matters for Windows paths.
    let mut value = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(value),
            '\\' => value.push(chars.next()?),
            _ => value.push(ch),
        }
    }
    None
}

fn extract_acf_field(content: &str, key: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| extract_vdf_value(line, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vdf_value() {
        assert_eq!(
            extract_vdf_value(r#"		"path"		"/mnt/games/SteamLibrary""#, "path"),
            Some("/mnt/games/SteamLibrary".into())
        );
    }

    #[test]
    fn parse_vdf_value_unescapes_windows_paths() {
        assert_eq!(
            extract_vdf_value(r#"		"path"		"D:\\SteamLibrary""#, "path"),
            Some(r"D:\SteamLibrary".into())
        );
    }

    #[test]
    fn parse_vdf_value_not_matching() {
        assert_eq!(
            extract_vdf_value(r#"		"name"		"Counter-Strike 2""#, "path"),
            None
        );
    }

    #[test]
    fn parse_library_paths_multi() {
        let content = r#"
"libraryfolders"
{
  "0"
  {
    "path"    "/home/user/.steam/steam"
    "label"   ""
  }
  "1"
  {
    "path"    "/mnt/games/SteamLibrary"
    "label"   "Games Drive"
  }
}
"#;
        let paths = parse_library_paths(content);
        assert_eq!(
            paths,
            ["/home/user/.steam/steam", "/mnt/games/SteamLibrary"]
        );
    }

    #[test]
    fn parse_acf_fields() {
        let content = r#"
"AppState"
{
    "appid"        "730"
    "Universe"     "1"
    "name"         "Counter-Strike 2"
    "StateFlags"   "4"
    "installdir"   "Counter-Strike Global Offensive"
}
"#;
        assert_eq!(extract_acf_field(content, "appid"), Some("730".into()));
        assert_eq!(
            extract_acf_field(content, "name"),
            Some("Counter-Strike 2".into())
        );
        assert_eq!(
            extract_acf_field(content, "installdir"),
            Some("Counter-Strike Global Offensive".into())
        );
    }

    #[test]
    fn steam_tools_filtered() {
        assert!(is_steam_tool(
            "Steamworks Common Redistributables",
            "228980"
        ));
        assert!(is_steam_tool("Proton 9.0-4", "2348590"));
        assert!(!is_steam_tool("Counter-Strike 2", "730"));
    }

    #[test]
    fn steam_roots_returns_list() {
        assert!(!steam_root_candidates().is_empty());
    }
}
