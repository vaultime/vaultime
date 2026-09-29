// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! GOG games. Every GOG install carries a `goggame-<id>.info` file with the
//! title and the main program, whether it came from GOG Galaxy, Heroic or an
//! offline installer. GOG Galaxy lists its installs in the Windows registry.

use std::collections::HashSet;
use std::fs;
use std::path::{MAIN_SEPARATOR_STR, Path};

use serde::Deserialize;

use super::DiscoveredGame;
use crate::platform::process::path_key;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameInfo {
    game_id: String,
    name: String,
    #[serde(default)]
    root_game_id: Option<String>,
    #[serde(default)]
    play_tasks: Vec<PlayTask>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayTask {
    #[serde(default)]
    is_primary: bool,
    #[serde(default)]
    path: Option<String>,
    #[serde(rename = "type")]
    kind: String,
}

/// The game installed in `folder`, from its GOG info file. DLCs keep info
/// files in the same folder, the one that is its own root game wins.
pub(crate) fn game_from_folder(
    folder: &Path,
    source: &str,
    existing: &HashSet<String>,
) -> Option<DiscoveredGame> {
    fs::read_dir(folder)
        .ok()?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("info"))
                && path.file_stem().is_some_and(|stem| {
                    stem.to_string_lossy()
                        .to_ascii_lowercase()
                        .starts_with("goggame-")
                })
        })
        .filter_map(|path| fs::read_to_string(path).ok())
        .find_map(|content| parse_info(&content, folder, source, existing))
}

fn parse_info(
    content: &str,
    folder: &Path,
    source: &str,
    existing: &HashSet<String>,
) -> Option<DiscoveredGame> {
    let info: GameInfo = serde_json::from_str(content).ok()?;
    if info
        .root_game_id
        .as_deref()
        .is_some_and(|root| root != info.game_id)
    {
        return None;
    }
    let relative = info
        .play_tasks
        .iter()
        .find(|task| task.is_primary && task.kind == "FileTask")
        .and_then(|task| task.path.as_deref())?;

    let executable = folder.join(relative.replace(['/', '\\'], MAIN_SEPARATOR_STR));
    let executable_path = executable.to_string_lossy().into_owned();
    Some(DiscoveredGame {
        title: info.name,
        already_added: existing.contains(&path_key(&executable_path)),
        executable_path,
        install_folder: Some(folder.to_string_lossy().into_owned()),
        source: source.into(),
        source_id: Some(info.game_id),
    })
}

/// Games installed through GOG Galaxy.
#[cfg(windows)]
pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    use windows_registry::LOCAL_MACHINE;

    let Ok(games_key) = LOCAL_MACHINE.open(r"SOFTWARE\WOW6432Node\GOG.com\Games") else {
        return Vec::new();
    };
    let Ok(ids) = games_key.keys() else {
        return Vec::new();
    };
    let games: Vec<DiscoveredGame> = ids
        .filter_map(|id| {
            games_key
                .open(&id)
                .and_then(|key| key.get_string("path"))
                .ok()
        })
        .filter_map(|folder| game_from_folder(Path::new(&folder), "gog", existing))
        .collect();
    log::info!("GOG Galaxy discovery found {} game(s)", games.len());
    games
}

#[cfg(not(windows))]
pub(crate) fn discover(_existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARDEW: &str = r#"{
        "gameId": "1453375253",
        "rootGameId": "1453375253",
        "name": "Stardew Valley",
        "playTasks": [
            { "category": "document", "isPrimary": false, "path": "manual.pdf", "type": "FileTask" },
            { "category": "game", "isPrimary": true, "path": "Stardew Valley.exe", "type": "FileTask" }
        ]
    }"#;

    const DLC: &str = r#"{
        "gameId": "2000000000",
        "rootGameId": "1453375253",
        "name": "Some DLC",
        "playTasks": []
    }"#;

    fn folder_with(files: &[(&str, &str)]) -> std::path::PathBuf {
        let folder =
            std::env::temp_dir().join(format!("vaultime-gog-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        for (name, content) in files {
            fs::write(folder.join(name), content).unwrap();
        }
        folder
    }

    #[test]
    fn reads_the_primary_program_from_the_info_file() {
        let folder = folder_with(&[
            ("goggame-2000000000.info", DLC),
            ("goggame-1453375253.info", STARDEW),
        ]);
        let game = game_from_folder(&folder, "gog", &HashSet::new()).unwrap();
        assert_eq!(game.title, "Stardew Valley");
        assert_eq!(game.source_id.as_deref(), Some("1453375253"));
        assert!(game.executable_path.ends_with("Stardew Valley.exe"));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn ignores_folders_without_a_game() {
        let folder = folder_with(&[("goggame-2000000000.info", DLC), ("readme.txt", "hello")]);
        assert!(game_from_folder(&folder, "gog", &HashSet::new()).is_none());
        fs::remove_dir_all(folder).unwrap();
    }
}
