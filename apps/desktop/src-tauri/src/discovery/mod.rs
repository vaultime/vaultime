// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Game discovery through folder scans, the Steam library and the data of
//! other launchers.

#[cfg(windows)]
pub mod amazon;
pub mod auto;
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

use log::warn;
use serde::{Deserialize, Serialize};

use crate::assets::{self, AssetManager};
use crate::constants::{
    EXECUTABLE_SCAN_DEPTH, LAUNCHER_IDS_BACKFILL_SETTING, LAUNCHER_IDS_BACKFILL_VERSION,
    SHIPPING_BONUS_BYTES, STEAM_SOURCE, TITLE_MATCH_BONUS_BYTES, TITLE_WORD_MIN_CHARS,
    TOP_LEVEL_BONUS_BYTES,
};
use crate::db::connection::Database;
use crate::db::models::{CreateGame, Game};
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
    /// The library game this is most likely another install of, through
    /// another launcher. Adding it links it to that game, unless the player
    /// keeps them apart and clears this.
    #[serde(default)]
    pub same_game: Option<SameGame>,
}

impl DiscoveredGame {
    /// A candidate whose program was found at `program`, marked as added when
    /// `existing`, the path keys of the library's programs, holds it.
    pub(crate) fn new(
        existing: &HashSet<String>,
        title: String,
        program: &Path,
        install_folder: Option<String>,
        source: &str,
        source_id: Option<String>,
    ) -> Self {
        let executable_path = program.to_string_lossy().into_owned();
        Self {
            title,
            already_added: existing.contains(&path_key(&executable_path)),
            executable_path,
            install_folder,
            source: source.to_owned(),
            source_id,
            same_game: None,
        }
    }
}

/// A library game a candidate seems to be another install of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SameGame {
    /// The game the library shows, which the candidate would count as.
    pub game_id: String,
    pub title: String,
    /// The launcher of the install that matched.
    pub launcher_source: Option<String>,
}

/// Marks each new candidate that has the title of exactly one library game
/// from another launcher as another install of it. Kept conservative: a
/// title that a game from the candidate's own launcher has too, that fits
/// two games, or a candidate inside the matched game's install folder, marks
/// nothing.
pub fn mark_same_games(
    db: &Database,
    mut found: Vec<DiscoveredGame>,
) -> Result<Vec<DiscoveredGame>> {
    let library = games::list_local_games(db)?;
    let links = games::links(db)?;
    let mut by_title: HashMap<String, Vec<&Game>> = HashMap::new();
    for game in &library {
        let key = normalize(&game.title);
        if !key.is_empty() {
            by_title.entry(key).or_default().push(game);
        }
    }
    for candidate in found
        .iter_mut()
        .filter(|candidate| !candidate.already_added)
    {
        candidate.same_game = None;
        let Some(matches) = by_title.get(&normalize(&candidate.title)) else {
            continue;
        };
        let same_launcher = matches
            .iter()
            .any(|game| game.launcher_source.as_deref() == Some(candidate.source.as_str()));
        if same_launcher || matches.iter().any(|game| holds(game, candidate)) {
            continue;
        }
        let mut shown = matches
            .iter()
            .map(|game| links.get(&game.id).unwrap_or(&game.id));
        let Some(shown_id) = shown.next() else {
            continue;
        };
        if shown.any(|other| other != shown_id) {
            continue;
        }
        let Some(shown_game) = library.iter().find(|game| game.id == *shown_id) else {
            continue;
        };
        // The install shown when it matched, so the launcher named is its own.
        let install = matches
            .iter()
            .find(|game| game.id == shown_game.id)
            .unwrap_or(&matches[0]);
        candidate.same_game = Some(SameGame {
            game_id: shown_game.id.clone(),
            title: shown_game.title.clone(),
            launcher_source: install.launcher_source.clone(),
        });
    }
    Ok(found)
}

/// Whether a candidate lies in a library game's install folder, which makes
/// it part of that install rather than another one.
fn holds(game: &Game, candidate: &DiscoveredGame) -> bool {
    let Some(folder) = game.install_folder.as_deref().map(path_key) else {
        return false;
    };
    let folder = Path::new(&folder);
    Path::new(&path_key(&candidate.executable_path)).starts_with(folder)
        || candidate
            .install_folder
            .as_deref()
            .is_some_and(|other| Path::new(&path_key(other)) == folder)
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

/// What a scan offers: the candidates without programs the player ignored,
/// marked where they seem to be another install of a library game.
pub fn offer(db: &Database, found: Vec<DiscoveredGame>) -> Result<Vec<DiscoveredGame>> {
    mark_same_games(db, without_ignored(db, found)?)
}

/// What a scan of the launchers offers, see `offer`. Games already in the
/// library learn their launcher ids from it first.
pub fn offer_launcher_games(
    db: &Database,
    found: Vec<DiscoveredGame>,
) -> Result<Vec<DiscoveredGame>> {
    learn_launcher_ids(db, &found);
    offer(db, found)
}

/// `remember_launcher_ids` for a scan, which a failure here must not fail.
fn learn_launcher_ids(db: &Database, found: &[DiscoveredGame]) {
    if let Err(error) = remember_launcher_ids(db, found) {
        warn!("launcher ids not stored: {error}");
    }
}

/// Adds the picked candidates to the library, each with its launcher id and
/// artwork. One marked as another install of a library game counts as that
/// game. Candidates already added are skipped.
pub fn import(
    db: &Database,
    asset_manager: &AssetManager,
    picked: &[DiscoveredGame],
) -> Result<Vec<Game>> {
    let mut imported = Vec::new();
    for candidate in picked.iter().filter(|candidate| !candidate.already_added) {
        let input = CreateGame {
            title: candidate.title.clone(),
            executable_path: Some(candidate.executable_path.clone()),
            install_folder: candidate.install_folder.clone(),
            launcher_source: Some(candidate.source.clone()),
        };
        let mut game = games::create_game(db, &input)?;
        if let Some(launcher_id) = candidate.source_id.as_deref().filter(|id| !id.is_empty()) {
            game = games::set_launcher_id(db, &game.id, launcher_id)?;
        }
        // Its own cover too, so it looks right should it be unlinked.
        if let Err(error) = assets::scan_game_assets(db, asset_manager, &game.id) {
            warn!("artwork scan failed for {}: {error}", game.title);
        }
        if let Some(same) = &candidate.same_game {
            // The game may count as another one by now, which is then the one.
            let link = db
                .with_conn(|conn| games::shown_id(conn, &same.game_id))
                .and_then(|shown| games::link_game(db, &game.id, Some(&shown)));
            if let Err(error) = link {
                warn!("{} not linked to {}: {error}", game.title, same.title);
            }
        }
        imported.push(game);
    }
    Ok(imported)
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
    for game in games::list_local_games(db)? {
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
    for game in games::list_local_games(db)? {
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
    Ok(games::list_local_games(db)?
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
            let stem = normalize(stem);
            if title_words.iter().any(|word| stem.contains(word.as_str())) {
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
            same_game: None,
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

    #[test]
    fn a_game_from_another_launcher_is_marked_as_the_same_game() {
        let db = Database::open_in_memory().unwrap();
        let add = |title: &str, folder: &str, source: &str| {
            games::create_game(
                &db,
                &CreateGame {
                    title: title.into(),
                    executable_path: Some(format!("{folder}/game.exe")),
                    install_folder: Some(folder.into()),
                    launcher_source: Some(source.into()),
                },
            )
            .unwrap()
        };
        let cyberpunk = add("Cyberpunk 2077", "/steam/cyberpunk", "steam");
        let doom = add("DOOM", "/steam/doom", "steam");
        let doom_epic = add("Doom", "/epic/doom", "epic");
        let candidate = |title: &str, exe: &str, source: &str| DiscoveredGame {
            title: title.into(),
            executable_path: exe.into(),
            install_folder: None,
            source: source.into(),
            source_id: None,
            already_added: false,
            same_game: None,
        };
        let found = || {
            vec![
                candidate("Cyberpunk 2077™", "/gog/cyberpunk/Cyberpunk2077.exe", "gog"),
                candidate("Cyberpunk 2077", "/steam/cyberpunk2/game.exe", "steam"),
                candidate(
                    "Cyberpunk 2077",
                    "/steam/cyberpunk/bin/game.exe",
                    "folder_scan",
                ),
                candidate("Doom", "/gog/doom/doom.exe", "gog"),
                candidate("Hades", "/gog/hades/hades.exe", "gog"),
            ]
        };
        let same = |marked: &[DiscoveredGame]| -> Vec<Option<String>> {
            marked
                .iter()
                .map(|game| game.same_game.as_ref().map(|same| same.game_id.clone()))
                .collect()
        };

        let marked = mark_same_games(&db, found()).unwrap();
        // Another launcher's copy matches, its own launcher's or one inside
        // the install does not, and a title two games share stays unmarked.
        assert_eq!(
            same(&marked),
            [Some(cyberpunk.id.clone()), None, None, None, None]
        );
        let first = marked[0].same_game.as_ref().unwrap();
        assert_eq!(first.title, "Cyberpunk 2077");
        assert_eq!(first.launcher_source.as_deref(), Some("steam"));

        // Once the two Dooms are one game, a third copy matches that game.
        games::link_game(&db, &doom_epic.id, Some(&doom.id)).unwrap();
        let marked = mark_same_games(&db, found()).unwrap();
        assert_eq!(same(&marked)[3], Some(doom.id.clone()));
    }

    #[test]
    fn imports_new_candidates_with_their_launcher_id_and_link() {
        let db = Database::open_in_memory().unwrap();
        let cache = std::env::temp_dir().join(format!("vaultime-import-{}", uuid::Uuid::new_v4()));
        let assets = AssetManager::new(cache.clone());
        let steam = games::create_game(
            &db,
            &CreateGame {
                title: "Hades".into(),
                executable_path: Some("/steam/hades/hades.exe".into()),
                install_folder: Some("/steam/hades".into()),
                launcher_source: Some("steam".into()),
            },
        )
        .unwrap();
        let candidate = |exe: &str, already_added: bool| DiscoveredGame {
            title: "Hades".into(),
            executable_path: exe.into(),
            install_folder: None,
            source: "gog".into(),
            source_id: Some("1234".into()),
            already_added,
            same_game: Some(SameGame {
                game_id: steam.id.clone(),
                title: steam.title.clone(),
                launcher_source: steam.launcher_source.clone(),
            }),
        };

        let imported = import(
            &db,
            &assets,
            &[
                candidate("/gog/hades/hades.exe", false),
                candidate("/steam/hades/hades.exe", true),
            ],
        )
        .unwrap();

        assert_eq!(imported.len(), 1);
        assert_eq!(games::launcher_id(&imported[0]).as_deref(), Some("1234"));
        assert_eq!(
            games::links(&db).unwrap().get(&imported[0].id),
            Some(&steam.id)
        );
        let _ = std::fs::remove_dir_all(cache);
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
