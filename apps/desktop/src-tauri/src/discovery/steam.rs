// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Steam launcher adapter.
//!
//! Discovers installed Steam games by reading `libraryfolders.vdf` to find
//! library directories, then parsing `appmanifest_*.acf` files in each
//! library's `steamapps/` folder.
//!
//! The VDF and ACF formats are simple key-value text files — this module
//! includes a lightweight parser rather than pulling in a full VDF crate.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use log::info;

use crate::db::connection::Database;
use crate::db::repo::games;
use crate::error::Result;

use super::DiscoveredGame;

/// Discover all installed Steam games.
///
/// Returns one `DiscoveredGame` per installed app with its executable path,
/// install folder, title from the manifest, and the Steam app ID.
pub fn discover_steam_games(db: &Database) -> Result<Vec<DiscoveredGame>> {
    let existing_exes = collect_existing_executables(db)?;
    let steam_root = find_steam_root();

    let Some(root) = steam_root else {
        info!("Steam installation not found");
        return Ok(Vec::new());
    };

    info!("found Steam root: {}", root.display());

    let library_folders = find_library_folders(&root);
    let mut results = Vec::new();

    for library_dir in &library_folders {
        let steamapps = library_dir.join("steamapps");
        if !steamapps.is_dir() {
            continue;
        }

        let manifests = find_app_manifests(&steamapps);
        for manifest_path in &manifests {
            if let Some(game) = parse_app_manifest(manifest_path, &steamapps, &existing_exes) {
                results.push(game);
            }
        }
    }

    info!("Steam discovery found {} game(s)", results.len());
    Ok(results)
}

// ---------------------------------------------------------------------------
// Steam root detection
// ---------------------------------------------------------------------------

fn find_steam_root() -> Option<PathBuf> {
    let candidates = steam_root_candidates();
    candidates.into_iter().find(|p| p.is_dir())
}

fn steam_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join(".steam/steam"));
            candidates.push(home.join(".local/share/Steam"));
            candidates.push(home.join(".steam/debian-installation"));
            // Flatpak Steam
            candidates.push(home.join(".var/app/com.valvesoftware.Steam/.steam/steam"));
            candidates.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        candidates.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        candidates.push(PathBuf::from(r"C:\Program Files\Steam"));
        candidates.push(PathBuf::from(r"D:\Steam"));
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join("Library/Application Support/Steam"));
        }
    }

    candidates
}

// ---------------------------------------------------------------------------
// Library folder discovery
// ---------------------------------------------------------------------------

/// Parse `libraryfolders.vdf` to find all Steam library directories.
///
/// Falls back to just the Steam root if the VDF file is missing or
/// unparseable.
fn find_library_folders(steam_root: &Path) -> Vec<PathBuf> {
    let vdf_path = steam_root.join("steamapps/libraryfolders.vdf");

    let mut folders = Vec::new();

    // The Steam root itself is always a library folder.
    folders.push(steam_root.to_path_buf());

    if let Ok(content) = fs::read_to_string(&vdf_path) {
        for path in parse_library_paths(&content) {
            let p = PathBuf::from(&path);
            if p.is_dir() && p != steam_root {
                folders.push(p);
            }
        }
    }

    folders
}

/// Extract library directory paths from `libraryfolders.vdf` content.
///
/// The file format looks like:
/// ```text
/// "libraryfolders"
/// {
///   "0"
///   {
///     "path"    "/home/user/.steam/steam"
///     ...
///   }
///   "1"
///   {
///     "path"    "/mnt/games/SteamLibrary"
///     ...
///   }
/// }
/// ```
fn parse_library_paths(content: &str) -> Vec<String> {
    let mut paths = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // Look for "path" key-value pairs.
        if let Some(value) = extract_vdf_value(trimmed, "path") {
            paths.push(value);
        }
    }

    paths
}

// ---------------------------------------------------------------------------
// App manifest parsing
// ---------------------------------------------------------------------------

/// Find all `appmanifest_*.acf` files in a steamapps directory.
fn find_app_manifests(steamapps_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(steamapps_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.starts_with("appmanifest_")
                && std::path::Path::new(&name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("acf"))
        })
        .map(|e| e.path())
        .collect()
}

/// Parse a single `appmanifest_*.acf` file into a `DiscoveredGame`.
///
/// ACF format is similar to VDF:
/// ```text
/// "AppState"
/// {
///   "appid"        "730"
///   "name"         "Counter-Strike 2"
///   "installdir"   "Counter-Strike Global Offensive"
///   ...
/// }
/// ```
fn parse_app_manifest(
    manifest_path: &Path,
    steamapps_dir: &Path,
    existing_exes: &HashSet<String>,
) -> Option<DiscoveredGame> {
    let content = fs::read_to_string(manifest_path).ok()?;

    let app_id = extract_acf_field(&content, "appid")?;
    let name = extract_acf_field(&content, "name")?;
    let install_dir_name = extract_acf_field(&content, "installdir")?;

    // Skip Steamworks redistributables, Proton, tools, etc.
    if is_steam_tool(&name, &app_id) {
        return None;
    }

    let install_folder = steamapps_dir.join("common").join(&install_dir_name);
    if !install_folder.is_dir() {
        return None;
    }

    // Try to find the main executable in the install folder.
    let executable_path = find_main_executable(&install_folder)?;
    let exe_str = executable_path.to_string_lossy().into_owned();
    let already_added = existing_exes.contains(&exe_str);

    Some(DiscoveredGame {
        title: name,
        executable_path: exe_str,
        install_folder: Some(install_folder.to_string_lossy().into_owned()),
        source: "steam".into(),
        source_id: Some(app_id),
        already_added,
    })
}

/// Attempt to find the main game executable in an install directory.
///
/// Heuristics:
/// 1. Look for executables at the top level first (most common).
/// 2. Look one level deeper (e.g. `Binaries/Win64/Game.exe`).
/// 3. Prefer larger executables (the game binary is usually the biggest).
fn find_main_executable(install_dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<(PathBuf, u64)> = Vec::new();

    for entry in walkdir::WalkDir::new(install_dir)
        .max_depth(3)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if !is_executable_file(path) {
            continue;
        }

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        if !super::metadata::is_likely_game_executable(&filename) {
            continue;
        }

        let size = path.metadata().map(|m| m.len()).unwrap_or(0);

        // Boost top-level executables.
        let depth = entry.depth();
        let effective_size = if depth <= 1 { size + 100_000_000 } else { size };

        candidates.push((path.to_path_buf(), effective_size));
    }

    // Pick the candidate with the highest effective size.
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.into_iter().next().map(|(p, _)| p)
}

/// Check if a path is an executable file (platform-aware).
fn is_executable_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        ext == "exe"
    }

    #[cfg(target_os = "macos")]
    {
        ext == "app" || has_unix_execute(path)
    }

    #[cfg(target_os = "linux")]
    {
        if matches!(ext.as_str(), "sh" | "x86_64" | "x86") {
            return true;
        }
        has_unix_execute(path)
    }
}

#[cfg(unix)]
fn has_unix_execute(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Returns `true` for Steam tools, redistributables, and Proton versions
/// that should be excluded from game discovery.
fn is_steam_tool(name: &str, app_id: &str) -> bool {
    let lower = name.to_lowercase();

    // Common Steam tool patterns.
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

    // Known tool app IDs.
    matches!(
        app_id,
        "228980"  // Steamworks Common Redistributables
        | "1070560" // Steam Linux Runtime
        | "1887720" // Proton
        | "2180100" // Proton Hotfix
        | "2348590" // Proton 9
    )
}

// ---------------------------------------------------------------------------
// Simple VDF/ACF parser helpers
// ---------------------------------------------------------------------------

/// Extract the value for a key from a VDF/ACF line.
///
/// Handles lines like: `"key"    "value"`
fn extract_vdf_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    let target = format!("\"{key}\"");

    if !trimmed.starts_with(&target) {
        return None;
    }

    // Find the value after the key.
    let rest = trimmed[target.len()..].trim();
    extract_quoted_string(rest)
}

/// Extract a field from ACF content (searches all lines).
fn extract_acf_field(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(value) = extract_vdf_value(line, key) {
            return Some(value);
        }
    }
    None
}

/// Extract a double-quoted string value.
fn extract_quoted_string(s: &str) -> Option<String> {
    let s = s.trim();
    if !s.starts_with('"') {
        return None;
    }
    let inner = &s[1..];
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

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
    fn parse_vdf_value() {
        assert_eq!(
            extract_vdf_value(r#"		"path"		"/mnt/games/SteamLibrary""#, "path"),
            Some("/mnt/games/SteamLibrary".into())
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
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], "/home/user/.steam/steam");
        assert_eq!(paths[1], "/mnt/games/SteamLibrary");
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
        let candidates = steam_root_candidates();
        // Should return at least one candidate on any platform.
        assert!(!candidates.is_empty());
    }
}
