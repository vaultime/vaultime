// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Game discovery through folder scans, the Steam library and the data of
//! other launchers.

#[cfg(windows)]
pub mod amazon;
#[cfg(windows)]
pub mod ea;
pub mod epic;
mod fuel;
pub mod gog;
pub mod heroic;
pub mod itch;
#[cfg(target_os = "linux")]
pub mod lutris;
pub mod metadata;
#[cfg(windows)]
pub mod registry;
pub mod scanner;
pub mod steam;
mod vdf;
#[cfg(windows)]
pub mod xbox;
#[cfg(windows)]
mod xml;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::constants::{
    EXECUTABLE_SCAN_DEPTH, LAUNCHER_IDS_BACKFILL_SETTING, LAUNCHER_IDS_BACKFILL_VERSION,
    SHIPPING_BONUS_BYTES, STEAM_SOURCE, TITLE_MATCH_BONUS_BYTES, TITLE_WORD_MIN_CHARS,
    TOP_LEVEL_BONUS_BYTES,
};
use crate::db::connection::Database;
use crate::db::models::Game;
use crate::db::repo::{games, settings};
use crate::error::Result;
use crate::platform::process::path_key;

/// A game candidate found during discovery. The user picks which ones to import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredGame {
    pub title: String,
    pub executable_path: String,
    /// Install folder, either reported by the launcher or the executable's parent.
    pub install_folder: Option<String>,
    /// Where the candidate came from, a launcher such as `steam`, or `folder_scan`.
    pub source: String,
    /// Launcher app id, for example the Steam app id.
    pub source_id: Option<String>,
    /// True when the library already has a game with this executable.
    pub already_added: bool,
}

/// The candidates without programs the player said are no game.
pub fn without_ignored(db: &Database, games: Vec<DiscoveredGame>) -> Result<Vec<DiscoveredGame>> {
    let ignored = crate::db::repo::ignored::keys(db)?;
    if ignored.is_empty() {
        return Ok(games);
    }
    Ok(games
        .into_iter()
        .filter(|game| !ignored.contains(&path_key(&game.executable_path)))
        .collect())
}

/// Games from every launcher other than Steam whose data exists here.
pub fn discover_launcher_games(db: &Database) -> Result<Vec<DiscoveredGame>> {
    let existing = library_executables(db)?;
    let mut games = epic::discover(&existing);
    games.extend(gog::discover(&existing));
    games.extend(heroic::discover(&existing));
    games.extend(itch::discover(&existing));
    #[cfg(windows)]
    {
        games.extend(registry::discover(&existing));
        games.extend(xbox::discover(&existing));
        games.extend(ea::discover(&existing));
        games.extend(amazon::discover(&existing));
    }
    #[cfg(target_os = "linux")]
    games.extend(lutris::discover(&existing));
    Ok(games)
}

/// Stores the launcher ids that `found` knows for games in the library that
/// came from the same launcher and have none yet, matched by executable or
/// install folder. Returns how many games got one.
pub fn remember_launcher_ids(db: &Database, found: &[DiscoveredGame]) -> Result<usize> {
    let mut missing: HashMap<(String, String), Game> = HashMap::new();
    for game in games::list_all_games(db)? {
        let Some(source) = game.launcher_source.clone() else {
            continue;
        };
        if games::launcher_id(&game).is_some() {
            continue;
        }
        for path in [&game.executable_path, &game.install_folder]
            .into_iter()
            .flatten()
        {
            missing.insert((source.clone(), path_key(path)), game.clone());
        }
    }
    let mut stored = HashSet::new();
    for discovered in found {
        let Some(id) = discovered.source_id.as_deref().filter(|id| !id.is_empty()) else {
            continue;
        };
        let paths = [
            Some(&discovered.executable_path),
            discovered.install_folder.as_ref(),
        ];
        let game = paths
            .into_iter()
            .flatten()
            .find_map(|path| missing.get(&(discovered.source.clone(), path_key(path))));
        if let Some(game) = game
            && stored.insert(game.id.clone())
        {
            games::set_launcher_id(db, &game.id, id)?;
        }
    }
    Ok(stored.len())
}

/// Reads the launcher ids of games imported before Vaultime kept them, once.
/// Steam games find theirs through Steam's manifests, the rest through the
/// launchers' own records. Returns how many games got one.
pub fn backfill_launcher_ids(db: &Database) -> Result<usize> {
    if settings::get_setting(db, LAUNCHER_IDS_BACKFILL_SETTING)?.as_deref()
        == Some(LAUNCHER_IDS_BACKFILL_VERSION)
    {
        return Ok(0);
    }
    let mut stored = 0;
    for game in games::list_all_games(db)? {
        if game.launcher_source.as_deref() != Some(STEAM_SOURCE)
            || games::launcher_id(&game).is_some()
        {
            continue;
        }
        if let Some(app_id) = game
            .install_folder
            .as_deref()
            .and_then(|folder| steam::app_id_for_install_folder(Path::new(folder)))
        {
            games::set_launcher_id(db, &game.id, &app_id)?;
            stored += 1;
        }
    }
    stored += remember_launcher_ids(db, &discover_launcher_games(db)?)?;
    settings::set_setting(
        db,
        LAUNCHER_IDS_BACKFILL_SETTING,
        LAUNCHER_IDS_BACKFILL_VERSION,
    )?;
    Ok(stored)
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
    use crate::db::models::CreateGame;

    #[test]
    fn games_learn_their_launcher_id_from_a_scan() {
        let db = Database::open_in_memory().unwrap();
        let add = |title: &str, exe: &str, source: Option<&str>| {
            games::create_game(
                &db,
                &CreateGame {
                    title: title.into(),
                    executable_path: Some(exe.into()),
                    install_folder: Some(format!("{exe}-folder")),
                    launcher_source: source.map(str::to_owned),
                },
            )
            .unwrap()
        };
        let by_exe = add("By exe", "/games/a/a.exe", Some("epic"));
        let by_folder = add("By folder", "/games/b/b.exe", Some("epic"));
        let other_launcher = add("Other launcher", "/games/c/c.exe", Some("gog"));
        let known = add("Known", "/games/d/d.exe", Some("epic"));
        games::set_launcher_id(&db, &known.id, "kept").unwrap();
        let found = |exe: &str, folder: &str, id: &str| DiscoveredGame {
            title: String::new(),
            executable_path: exe.into(),
            install_folder: Some(folder.into()),
            source: "epic".into(),
            source_id: Some(id.into()),
            already_added: true,
        };

        let stored = remember_launcher_ids(
            &db,
            &[
                found("/games/a/a.exe", "/elsewhere", "a-id"),
                found("/games/b/other.exe", "/games/b/b.exe-folder", "b-id"),
                found("/games/c/c.exe", "/games/c/c.exe-folder", "c-id"),
                found("/games/d/d.exe", "/games/d/d.exe-folder", "d-id"),
            ],
        )
        .unwrap();

        let id_of = |game: &Game| games::launcher_id(&games::get_game(&db, &game.id).unwrap());
        assert_eq!(stored, 2);
        assert_eq!(id_of(&by_exe).as_deref(), Some("a-id"));
        assert_eq!(id_of(&by_folder).as_deref(), Some("b-id"));
        assert_eq!(id_of(&other_launcher), None);
        assert_eq!(id_of(&known).as_deref(), Some("kept"));
    }

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
