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

use crate::constants::{
    STEAM_EXECUTABLE_SCAN_DEPTH, STEAM_SHIPPING_BONUS_BYTES, STEAM_TITLE_MATCH_BONUS_BYTES,
    STEAM_TOP_LEVEL_BONUS_BYTES, TITLE_WORD_MIN_CHARS,
};
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

    let executable = find_main_executable(&install_folder, &name)?;
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

/// Portrait covers in Steam's library cache, newest name first.
const STEAM_COVER_FILES: [&str; 2] = ["library_600x900.jpg", "library_capsule.jpg"];

/// The portrait cover the Steam client keeps for the game installed in
/// `install_folder`, if the game is a Steam install and Steam cached one.
pub(crate) fn cached_cover(install_folder: &Path) -> Option<PathBuf> {
    let app_id = app_id_for_install_folder(install_folder)?;
    let cache = find_steam_root()?
        .join("appcache")
        .join("librarycache")
        .join(app_id);
    STEAM_COVER_FILES
        .iter()
        .find_map(|name| find_in_cache(&cache, name))
}

/// Finds the app id through the manifests of the library that holds the
/// folder, `steamapps/common/<installdir>`.
fn app_id_for_install_folder(install_folder: &Path) -> Option<String> {
    let install_dir = install_folder.file_name()?.to_string_lossy().into_owned();
    let steamapps = install_folder.parent()?.parent()?;
    find_app_manifests(steamapps)
        .into_iter()
        .find_map(|manifest| {
            let content = fs::read_to_string(manifest).ok()?;
            let dir = extract_acf_field(&content, "installdir")?;
            if dir.eq_ignore_ascii_case(&install_dir) {
                extract_acf_field(&content, "appid")
            } else {
                None
            }
        })
}

/// Newer Steam clients keep each image in a hashed folder below the app's
/// cache folder, older ones directly in it.
fn find_in_cache(folder: &Path, name: &str) -> Option<PathBuf> {
    let direct = folder.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    fs::read_dir(folder)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path().join(name))
        .find(|path| path.is_file())
}

/// Picks the most likely game binary in an install folder.
///
/// The game binary is usually the biggest program, so size is the base score.
/// An Unreal `-Shipping` build always wins, a name that matches the title gets
/// a big head start and files in the top folder a small one.
fn find_main_executable(install_dir: &Path, title: &str) -> Option<PathBuf> {
    let title_words = significant_words(title);
    walkdir::WalkDir::new(install_dir)
        .max_depth(STEAM_EXECUTABLE_SCAN_DEPTH)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file() && is_executable(entry.path()))
        .filter(|entry| metadata::is_likely_game_executable(&entry.file_name().to_string_lossy()))
        .max_by_key(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let stem = name.strip_suffix(".exe").unwrap_or(&name);
            let mut score = entry.metadata().map_or(0, |meta| meta.len());
            if stem.ends_with("-shipping") {
                score += STEAM_SHIPPING_BONUS_BYTES;
            }
            if title_words
                .iter()
                .any(|word| normalize(stem).contains(word.as_str()))
            {
                score += STEAM_TITLE_MATCH_BONUS_BYTES;
            }
            if entry.depth() <= 1 {
                score += STEAM_TOP_LEVEL_BONUS_BYTES;
            }
            score
        })
        .map(walkdir::DirEntry::into_path)
}

/// Lowercase letters and digits only, "Hades II" gives "hadesii".
fn normalize(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Title words long enough to identify a game, "Slay the Spire 2" gives
/// "slay" and "spire".
fn significant_words(title: &str) -> Vec<String> {
    title
        .split(|c: char| !c.is_ascii_alphanumeric())
        .map(normalize)
        .filter(|word| word.len() >= TITLE_WORD_MIN_CHARS && word != "the")
        .collect()
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

    /// A game folder with executables of the given sizes, in a fresh temp folder.
    #[cfg(windows)]
    fn game_folder(files: &[(&str, u64)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("vaultime-exe-test-{}", uuid::Uuid::new_v4()));
        for (path, size) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::File::create(&path).unwrap().set_len(*size).unwrap();
        }
        root
    }

    #[cfg(windows)]
    fn picked(files: &[(&str, u64)], title: &str) -> String {
        let root = game_folder(files);
        let exe = find_main_executable(&root, title).unwrap();
        let relative = exe
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        fs::remove_dir_all(root).unwrap();
        relative
    }

    // The layouts below are real Steam installs, with their file sizes. They
    // need .exe files, so they run on Windows.

    #[cfg(windows)]
    #[test]
    fn prefers_the_executable_named_like_the_game() {
        let files = [
            ("Ship/F10.exe", 9_206_784),
            ("Release/F10.exe", 9_206_784),
            ("Release/Hades2.exe", 7_721_032),
            ("Ship/Hades2.exe", 6_432_432),
            ("Ship/crashpad_handler.exe", 814_080),
        ];
        assert!(picked(&files, "Hades II").ends_with("Hades2.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn prefers_the_unreal_shipping_build() {
        let files = [
            ("SB/Binaries/Win64/SB-Win64-Shipping.exe", 359_186_432),
            ("Engine/Binaries/Win64/UnrealCEFSubProcess.exe", 3_648_512),
            ("crs-handler.exe", 1_266_856),
            ("crs-uploader.exe", 859_304),
            ("SB.exe", 459_776),
        ];
        assert_eq!(
            picked(&files, "Stellar Blade"),
            "SB/Binaries/Win64/SB-Win64-Shipping.exe"
        );
    }

    #[cfg(windows)]
    #[test]
    fn skips_source_engine_tools_and_servers() {
        let files = [
            ("bin/x64/qc_eyes.exe", 3_694_744),
            ("bin/x64/elementviewer.exe", 3_636_376),
            ("bin/x64/studiomdl.exe", 2_459_800),
            ("srcds_win64.exe", 2_000_000),
            ("cstrike_win64.exe", 800_000),
        ];
        assert_eq!(
            picked(&files, "Counter-Strike: Source"),
            "cstrike_win64.exe"
        );
    }

    #[cfg(windows)]
    #[test]
    fn finds_executables_four_folders_deep() {
        let files = [
            ("game/bin/win64/vconsole2.exe", 5_105_304),
            ("game/bin/win64/cs2.exe", 2_967_704),
            ("game/csgo/bin/legacy/csgo_legacy_app.exe", 1_728_360),
        ];
        assert_eq!(picked(&files, "Counter-Strike 2"), "game/bin/win64/cs2.exe");
    }

    #[test]
    fn title_words_skip_short_and_filler_words() {
        assert_eq!(significant_words("Slay the Spire 2"), ["slay", "spire"]);
        assert_eq!(significant_words("Hades II"), ["hades"]);
    }

    #[test]
    fn finds_the_app_id_and_the_cached_cover() {
        let root =
            std::env::temp_dir().join(format!("vaultime-steam-test-{}", uuid::Uuid::new_v4()));
        let steamapps = root.join("steamapps");
        let install = steamapps.join("common").join("Balatro");
        fs::create_dir_all(&install).unwrap();
        fs::write(
            steamapps.join("appmanifest_2379780.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"2379780\"\n\t\"installdir\"\t\t\"Balatro\"\n}\n",
        )
        .unwrap();
        assert_eq!(
            app_id_for_install_folder(&install).as_deref(),
            Some("2379780")
        );
        assert_eq!(
            app_id_for_install_folder(&steamapps.join("common").join("Other")),
            None
        );

        let cache = root.join("librarycache").join("2379780");
        let hashed = cache.join("137bbd62c036ea008fef89cbf5d6ad884c2735cf");
        fs::create_dir_all(&hashed).unwrap();
        fs::write(hashed.join("library_600x900.jpg"), b"").unwrap();
        assert_eq!(
            find_in_cache(&cache, "library_600x900.jpg"),
            Some(hashed.join("library_600x900.jpg"))
        );
        fs::write(cache.join("library_600x900.jpg"), b"").unwrap();
        assert_eq!(
            find_in_cache(&cache, "library_600x900.jpg"),
            Some(cache.join("library_600x900.jpg"))
        );
        assert_eq!(find_in_cache(&cache, "library_capsule.jpg"), None);
        fs::remove_dir_all(root).unwrap();
    }

    /// Prints the cached Steam cover for every game in a real library.
    /// Same setup as `report_local_library`.
    #[test]
    #[ignore = "reads a local Steam library"]
    fn report_local_covers() {
        let steamapps = PathBuf::from(std::env::var("VAULTIME_STEAMAPPS").unwrap());
        for manifest in find_app_manifests(&steamapps) {
            let content = fs::read_to_string(&manifest).unwrap();
            let Some(dir) = extract_acf_field(&content, "installdir") else {
                continue;
            };
            let install = steamapps.join("common").join(&dir);
            println!("{dir} | {:?}", cached_cover(&install));
        }
    }

    /// Prints what discovery makes of a real library, to check the picked
    /// executables by eye. Run with the steamapps folder in
    /// `VAULTIME_STEAMAPPS` and `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads a local Steam library"]
    fn report_local_library() {
        let steamapps = PathBuf::from(std::env::var("VAULTIME_STEAMAPPS").unwrap());
        for manifest in find_app_manifests(&steamapps) {
            match parse_app_manifest(&manifest, &steamapps, &HashSet::new()) {
                Some(game) => println!("{} | {}", game.title, game.executable_path),
                None => println!("skipped {}", manifest.display()),
            }
        }
    }

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
