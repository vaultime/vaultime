// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wargaming.net Game Center games. The Game Center keeps a file per game in
//! `ProgramData\Wargaming.net\GameCenter\apps` that holds the game's folder.
//! The folder says in `game_info.xml` whether the game is installed, and in
//! `game_metadata\metadata.xml` its id, name and programs.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::xml::{attribute, elements, read_text, texts};
use super::{DiscoveredGame, find_main_executable};
use crate::platform::process::path_key;

const APPS_FOLDER: &str = r"Wargaming.net\GameCenter\apps";
/// The apps folder holds a folder per game with a file per install.
const APPS_DEPTH: usize = 3;
const GAME_INFO: &str = "game_info.xml";
const METADATA: &str = r"game_metadata\metadata.xml";
/// Marks the programs of the Game Center for macOS, which runs the Windows
/// games in Wine.
const MAC_EMULATION: &str = "wgc_mac";

pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let program_data = std::env::var_os("ProgramData")
        .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from);
    let games = games_in(&program_data.join(APPS_FOLDER), existing);
    log::info!("Wargaming discovery found {} game(s)", games.len());
    games
}

/// The installed games the apps folder `apps` names, each folder once.
fn games_in(apps: &Path, existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let mut seen = HashSet::new();
    app_folders(apps)
        .into_iter()
        .filter(|folder| seen.insert(path_key(&folder.to_string_lossy())))
        .filter_map(|folder| game_from_folder(&folder, existing))
        .collect()
}

/// The folders the files in the apps folder hold.
fn app_folders(apps: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(apps)
        .max_depth(APPS_DEPTH)
        .follow_links(false)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| fs::read(entry.path()).ok())
        .filter_map(|content| {
            let text = String::from_utf8_lossy(&content);
            let folder = text.trim().trim_matches('"');
            (!folder.is_empty()).then(|| PathBuf::from(folder))
        })
        .collect()
}

fn game_from_folder(folder: &Path, existing: &HashSet<String>) -> Option<DiscoveredGame> {
    let info = read_text(&folder.join(GAME_INFO))?;
    if texts(&info, "installed").first().map(String::as_str) != Some("true") {
        return None;
    }
    let metadata = parse_metadata(&read_text(&folder.join(METADATA))?);
    let title = metadata.name.unwrap_or_else(|| {
        folder
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    });
    let program = metadata
        .programs
        .iter()
        .map(|program| folder.join(program))
        .find(|program| program.is_file())
        .or_else(|| find_main_executable(folder, &title))?;
    Some(DiscoveredGame::new(
        existing,
        title,
        &program,
        Some(folder.to_string_lossy().into_owned()),
        "wargaming",
        metadata.app_id,
    ))
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Metadata {
    /// Like `WOT.EU.PRODUCTION`.
    app_id: Option<String>,
    name: Option<String>,
    /// The Windows programs, relative to the game folder.
    programs: Vec<String>,
}

/// Version 6 keeps the fields in `predefined_section` with a list of
/// `executable` programs, version 5 at the top with one `executable_name`.
fn parse_metadata(xml: &str) -> Metadata {
    let first = |name: &str| texts(xml, name).into_iter().find(|text| !text.is_empty());
    let mut programs: Vec<String> = elements(xml, "executable")
        .into_iter()
        .filter(|(attributes, _)| attribute(attributes, "emul").as_deref() != Some(MAC_EMULATION))
        .map(|(_, program)| program)
        .collect();
    programs.extend(texts(xml, "executable_name"));
    programs.retain(|program| !program.is_empty());
    Metadata {
        app_id: first("app_id"),
        name: first("shortcut_name"),
        programs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA_V6: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<protocol name="metadata" version="6.15">
  <predefined_section>
    <app_id>WOT.EU.PRODUCTION</app_id>
    <shortcut_name>World of Tanks</shortcut_name>
    <executables>
      <executable emul="wgc_mac" arch="x64">win64\WorldOfTanks.exe</executable>
      <executable arch="x64">win64\WorldOfTanks.exe</executable>
    </executables>
  </predefined_section>
</protocol>"#;

    const METADATA_V5: &str = r#"<protocol name="metadata" version="5.3">
  <app_id>WOWS.EU.PRODUCTION</app_id>
  <shortcut_name>World of Warships</shortcut_name>
  <executable_name>WorldOfWarships.exe</executable_name>
</protocol>"#;

    const INSTALLED: &str = r#"<protocol name="game_info" version="2.9"><game><installed>true</installed></game></protocol>"#;

    #[test]
    fn reads_both_metadata_versions() {
        assert_eq!(
            parse_metadata(METADATA_V6),
            Metadata {
                app_id: Some("WOT.EU.PRODUCTION".into()),
                name: Some("World of Tanks".into()),
                programs: vec![r"win64\WorldOfTanks.exe".into()],
            }
        );
        assert_eq!(
            parse_metadata(METADATA_V5).programs,
            ["WorldOfWarships.exe"]
        );
    }

    #[test]
    fn finds_installed_games_from_the_apps_folder() {
        let root = std::env::temp_dir().join(format!("vaultime-wgc-test-{}", uuid::Uuid::new_v4()));
        let apps = root.join("apps");
        let tanks = root.join("World_of_Tanks_EU");
        let ships = root.join("World_of_Warships");
        for folder in [
            apps.join("wot"),
            apps.join("wows"),
            tanks.join(r"win64"),
            tanks.join("game_metadata"),
            ships.join("game_metadata"),
        ] {
            fs::create_dir_all(folder).unwrap();
        }
        fs::write(
            apps.join(r"wot\WOT.EU.PRODUCTION"),
            format!("{}\n", tanks.display()),
        )
        .unwrap();
        // The same game named twice, and a game that is not installed.
        fs::write(
            apps.join(r"wot\WOT.EU.PRODUCTION.copy"),
            tanks.display().to_string(),
        )
        .unwrap();
        fs::write(
            apps.join(r"wows\WOWS.EU.PRODUCTION"),
            ships.display().to_string(),
        )
        .unwrap();
        fs::write(tanks.join(GAME_INFO), INSTALLED).unwrap();
        fs::write(tanks.join(METADATA), METADATA_V6).unwrap();
        fs::write(tanks.join(r"win64\WorldOfTanks.exe"), b"MZ").unwrap();
        fs::write(ships.join(GAME_INFO), INSTALLED.replace("true", "false")).unwrap();
        fs::write(ships.join(METADATA), METADATA_V5).unwrap();

        let games = games_in(&apps, &HashSet::new());
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "World of Tanks");
        assert_eq!(games[0].source, "wargaming");
        assert_eq!(games[0].source_id.as_deref(), Some("WOT.EU.PRODUCTION"));
        assert_eq!(
            Path::new(&games[0].executable_path),
            tanks.join(r"win64\WorldOfTanks.exe")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
