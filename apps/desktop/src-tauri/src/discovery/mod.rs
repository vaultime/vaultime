// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Game discovery through folder scans, the Steam library and the data of
//! other launchers.

pub mod epic;
pub mod gog;
pub mod heroic;
#[cfg(target_os = "linux")]
pub mod lutris;
pub mod metadata;
#[cfg(windows)]
pub mod registry;
pub mod scanner;
pub mod steam;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::constants::{
    EXECUTABLE_SCAN_DEPTH, SHIPPING_BONUS_BYTES, TITLE_MATCH_BONUS_BYTES, TITLE_WORD_MIN_CHARS,
    TOP_LEVEL_BONUS_BYTES,
};
use crate::db::connection::Database;
use crate::db::repo::games;
use crate::error::Result;
use crate::platform::process::path_key;

/// A game candidate found during discovery. The user picks which ones to import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredGame {
    pub title: String,
    pub executable_path: String,
    /// Install folder, either reported by the launcher or the executable's parent.
    pub install_folder: Option<String>,
    /// Where the candidate came from: `steam`, `epic`, `gog`, `heroic`,
    /// `lutris` or `folder_scan`.
    pub source: String,
    /// Launcher app id, for example the Steam app id.
    pub source_id: Option<String>,
    /// True when the library already has a game with this executable.
    pub already_added: bool,
}

/// Games from every launcher other than Steam whose data exists here.
pub fn discover_launcher_games(db: &Database) -> Result<Vec<DiscoveredGame>> {
    let existing = library_executables(db)?;
    let mut games = epic::discover(&existing);
    games.extend(gog::discover(&existing));
    games.extend(heroic::discover(&existing));
    #[cfg(windows)]
    games.extend(registry::discover(&existing));
    #[cfg(target_os = "linux")]
    games.extend(lutris::discover(&existing));
    Ok(games)
}

/// Path keys of every executable already in the library.
fn library_executables(db: &Database) -> Result<HashSet<String>> {
    Ok(games::list_all_games(db)?
        .into_iter()
        .filter_map(|game| game.executable_path)
        .map(|path| path_key(&path))
        .collect())
}

/// Picks the most likely game binary in an install folder.
///
/// The game binary is usually the biggest program, so size is the base score.
/// An Unreal `-Shipping` build always wins, a name that matches the title gets
/// a big head start and files in the top folder a small one.
pub(crate) fn find_main_executable(install_dir: &Path, title: &str) -> Option<PathBuf> {
    let title_words = significant_words(title);
    walkdir::WalkDir::new(install_dir)
        .max_depth(EXECUTABLE_SCAN_DEPTH)
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
                score += SHIPPING_BONUS_BYTES;
            }
            if title_words
                .iter()
                .any(|word| normalize(stem).contains(word.as_str()))
            {
                score += TITLE_MATCH_BONUS_BYTES;
            }
            if entry.depth() <= 1 {
                score += TOP_LEVEL_BONUS_BYTES;
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

/// Whether a file can be launched as a program on this platform.
fn is_executable(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    #[cfg(windows)]
    {
        ext == "exe"
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Shared libraries often carry the execute bit but are never games.
        let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase());
        if ext == "so" || name.is_some_and(|n| n.contains(".so.")) {
            return false;
        }

        matches!(ext.as_str(), "sh" | "x86_64" | "x86")
            || path
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    }

    #[cfg(not(any(windows, unix)))]
    {
        let _ = ext;
        false
    }
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
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::File::create(&path)
                .unwrap()
                .set_len(*size)
                .unwrap();
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
        std::fs::remove_dir_all(root).unwrap();
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

    #[cfg(windows)]
    #[test]
    fn picks_hoyoplay_games_over_their_helpers() {
        let genshin = [
            ("GenshinImpact.exe", 444_260_776),
            ("GenshinImpact_Data/upload_crash.exe", 9_457_480),
            ("GenshinImpact_Data/Plugins/crashreport.exe", 2_045_864),
            ("BeyondAssets/BeyondAssistEditor/BeyondEditor.exe", 364_968),
        ];
        assert_eq!(picked(&genshin, "Genshin Impact"), "GenshinImpact.exe");

        // The game is smaller than a launcher plugin next to it.
        let zenless = [
            ("ZenlessZoneZero.exe", 10_394_416),
            ("LauncherPlugins/hyp_questr_app.exe", 27_007_664),
            ("APMCrashReporter/crashreport.exe", 9_334_576),
            ("UnityCrashHandler64.exe", 1_145_136),
        ];
        assert_eq!(picked(&zenless, "Zenless Zone Zero"), "ZenlessZoneZero.exe");
    }

    #[test]
    fn title_words_skip_short_and_filler_words() {
        assert_eq!(significant_words("Slay the Spire 2"), ["slay", "spire"]);
        assert_eq!(significant_words("Hades II"), ["hades"]);
    }
}
