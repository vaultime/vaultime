// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Games installed through the Epic Games Launcher, read from its manifests.

use std::collections::HashSet;
use std::fs;
use std::path::{MAIN_SEPARATOR_STR, Path, PathBuf};

use log::info;
use serde::Deserialize;

use super::DiscoveredGame;
use crate::platform::process::path_key;

/// The fields Vaultime needs from an Epic `.item` manifest.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Manifest {
    display_name: String,
    install_location: String,
    launch_executable: String,
    app_name: String,
    #[serde(default)]
    main_game_app_name: Option<String>,
    #[serde(default)]
    app_categories: Vec<String>,
    #[serde(default, rename = "bIsIncompleteInstall")]
    incomplete: bool,
}

/// Epic keeps one `.item` file per installed app under `ProgramData`.
fn manifests_folder() -> Option<PathBuf> {
    let program_data = std::env::var_os("ProgramData")?;
    Some(
        PathBuf::from(program_data)
            .join("Epic")
            .join("EpicGamesLauncher")
            .join("Data")
            .join("Manifests"),
    )
}

pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let Some(folder) = manifests_folder() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&folder) else {
        return Vec::new();
    };
    let games: Vec<DiscoveredGame> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("item"))
        })
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|content| parse_manifest(&content, existing))
        .collect();
    info!("Epic discovery found {} game(s)", games.len());
    games
}

/// A game from one manifest. Add-ons, apps that are not games and
/// unfinished installs are skipped.
fn parse_manifest(content: &str, existing: &HashSet<String>) -> Option<DiscoveredGame> {
    let manifest: Manifest = serde_json::from_str(content).ok()?;
    let is_game = manifest
        .app_categories
        .iter()
        .any(|category| category == "games");
    let is_addon = manifest
        .main_game_app_name
        .as_deref()
        .is_some_and(|main| main != manifest.app_name);
    if !is_game || is_addon || manifest.incomplete || manifest.launch_executable.is_empty() {
        return None;
    }

    let executable = Path::new(&manifest.install_location)
        .join(manifest.launch_executable.replace('/', MAIN_SEPARATOR_STR));
    let executable_path = executable.to_string_lossy().into_owned();
    Some(DiscoveredGame {
        title: manifest.display_name,
        already_added: existing.contains(&path_key(&executable_path)),
        executable_path,
        install_folder: Some(manifest.install_location),
        source: "epic".into(),
        source_id: Some(manifest.app_name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(app_name: &str, main_game: &str, categories: &str, incomplete: bool) -> String {
        format!(
            r#"{{
                "FormatVersion": 0,
                "bIsIncompleteInstall": {incomplete},
                "LaunchExecutable": "Game/Binaries/Win64/Game-Win64-Shipping.exe",
                "DisplayName": "Some Game",
                "InstallLocation": "C:\\Games\\Epic\\SomeGame",
                "AppName": "{app_name}",
                "MainGameAppName": "{main_game}",
                "AppCategories": [{categories}]
            }}"#
        )
    }

    #[test]
    fn reads_an_installed_game() {
        let content = manifest(
            "Sugar",
            "Sugar",
            r#""public", "games", "applications""#,
            false,
        );
        let game = parse_manifest(&content, &HashSet::new()).unwrap();
        assert_eq!(game.title, "Some Game");
        assert_eq!(game.source, "epic");
        assert_eq!(game.source_id.as_deref(), Some("Sugar"));
        assert!(game.executable_path.ends_with("Game-Win64-Shipping.exe"));
        assert_eq!(
            game.install_folder.as_deref(),
            Some(r"C:\Games\Epic\SomeGame")
        );
    }

    #[test]
    fn skips_addons_apps_and_unfinished_installs() {
        let games = r#""public", "games""#;
        assert!(
            parse_manifest(
                &manifest("SugarDlc", "Sugar", games, false),
                &HashSet::new()
            )
            .is_none()
        );
        assert!(
            parse_manifest(
                &manifest("Tool", "Tool", r#""applications""#, false),
                &HashSet::new()
            )
            .is_none()
        );
        assert!(
            parse_manifest(&manifest("Sugar", "Sugar", games, true), &HashSet::new()).is_none()
        );
        assert!(parse_manifest("not json", &HashSet::new()).is_none());
    }
}
