// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! EA app games. There is no plain list of installed games, so Vaultime reads
//! the `__Installer\installerdata.xml` every EA game keeps, in the default EA
//! library folders on every drive and in the folders of EA's uninstall entries.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::registry::{InstalledProgram, installed_programs};
use super::xml::{read_text, texts};
use super::{DiscoveredGame, find_main_executable, metadata, scanner};
use crate::platform::process::path_key;

const PUBLISHER: &str = "Electronic Arts";
const LAUNCHERS: &[&str] = &["EA app", "Origin"];
/// Default library folders of the EA app and Origin, relative to a drive root.
const LIBRARY_FOLDERS: &[&str] = &[
    r"Program Files\EA Games",
    r"Program Files (x86)\EA Games",
    r"Program Files (x86)\Origin Games",
    "EA Games",
];
const INSTALLER_DATA: &str = r"__Installer\installerdata.xml";

pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let mut seen = HashSet::new();
    let games: Vec<DiscoveredGame> = registered_folders(&installed_programs())
        .into_iter()
        .chain(library_folders())
        .filter(|(folder, _)| seen.insert(path_key(&folder.to_string_lossy())))
        .filter_map(|(folder, title)| game_from_folder(&folder, title, existing))
        .collect();
    log::info!("EA discovery found {} game(s)", games.len());
    games
}

/// Folders and titles of EA's uninstall entries, without the launchers.
fn registered_folders(programs: &[InstalledProgram]) -> Vec<(PathBuf, Option<String>)> {
    programs
        .iter()
        .filter(|program| {
            program.publisher.starts_with(PUBLISHER)
                && !LAUNCHERS.contains(&program.name.as_str())
                && !program.folder.is_empty()
        })
        .map(|program| {
            let folder = program
                .folder
                .trim()
                .trim_matches('"')
                .trim_end_matches(['\\', '/']);
            (PathBuf::from(folder), Some(program.name.clone()))
        })
        .collect()
}

fn library_folders() -> Vec<(PathBuf, Option<String>)> {
    scanner::fixed_drives()
        .into_iter()
        .flat_map(|drive| LIBRARY_FOLDERS.iter().map(move |folder| drive.join(folder)))
        .filter_map(|library| fs::read_dir(library).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| (entry.path(), None))
        .collect()
}

/// A game from its installer data, or from its uninstall entry's title when it
/// has none. Folders with neither are left to the folder scan.
fn game_from_folder(
    folder: &Path,
    registered_title: Option<String>,
    existing: &HashSet<String>,
) -> Option<DiscoveredGame> {
    let data = read_text(&folder.join(INSTALLER_DATA)).map(|xml| parse_installer_data(&xml));
    if data.is_none() && registered_title.is_none() {
        return None;
    }
    let data = data.unwrap_or_default();
    let title = data.title.or(registered_title).unwrap_or_else(|| {
        folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    // Some games start through an anti-cheat launcher, the scorer then finds the game.
    let program = data
        .program
        .map(|program| folder.join(program))
        .filter(|path| {
            path.is_file()
                && path.file_name().is_some_and(|name| {
                    metadata::is_likely_game_executable(&name.to_string_lossy())
                })
        })
        .or_else(|| find_main_executable(folder, &title))?;
    let executable_path = program.to_string_lossy().into_owned();
    Some(DiscoveredGame {
        title,
        already_added: existing.contains(&path_key(&executable_path)),
        executable_path,
        install_folder: Some(folder.to_string_lossy().into_owned()),
        source: "ea".into(),
        source_id: data.id,
    })
}

#[derive(Debug, Default, PartialEq, Eq)]
struct InstallerData {
    title: Option<String>,
    program: Option<String>,
    id: Option<String>,
}

/// Title, program and content id. The program is the first launcher that is
/// no trial, a 64 bit one first.
fn parse_installer_data(xml: &str) -> InstallerData {
    let first = |xml: &str, name: &str| texts(xml, name).into_iter().find(|text| !text.is_empty());
    let launchers: Vec<(String, bool)> = texts(xml, "launcher")
        .into_iter()
        .filter(|launcher| first(launcher, "trial").as_deref() != Some("true"))
        .filter_map(|launcher| {
            let path = first(&launcher, "filePath")?;
            let x64 = first(&launcher, "requires64BitOS").as_deref() == Some("true");
            Some((strip_registry_prefix(&path).to_owned(), x64))
        })
        .collect();
    InstallerData {
        title: first(xml, "gameTitle").or_else(|| first(xml, "title")),
        program: launchers
            .iter()
            .find(|(_, x64)| *x64)
            .or(launchers.first())
            .map(|(path, _)| path.clone()),
        id: first(xml, "contentID"),
    }
}

/// `[HKEY_LOCAL_MACHINE\SOFTWARE\Respawn\Apex\Install Dir]r5apex.exe` gives
/// `r5apex.exe`, the part after the registry value of the install folder.
fn strip_registry_prefix(path: &str) -> &str {
    let path = path.trim();
    path.strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .map_or(path, |(_, rest)| rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTALLER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DiPManifest version="4.0">
  <gameTitles>
    <gameTitle locale="en_US">Some EA Game</gameTitle>
    <gameTitle locale="de_DE">Ein EA Spiel</gameTitle>
  </gameTitles>
  <contentIDs><contentID>1234567</contentID></contentIDs>
  <runtime>
    <launcher uid="trial">
      <filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\EA Games\Some EA Game\Install Dir]Trial.exe</filePath>
      <trial>true</trial>
    </launcher>
    <launcher uid="32">
      <filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\EA Games\Some EA Game\Install Dir]Game32.exe</filePath>
      <trial>false</trial>
      <requires64BitOS>false</requires64BitOS>
    </launcher>
    <launcher uid="64">
      <filePath>[HKEY_LOCAL_MACHINE\SOFTWARE\EA Games\Some EA Game\Install Dir]Bin\Game.exe</filePath>
      <trial>false</trial>
      <requires64BitOS>true</requires64BitOS>
    </launcher>
  </runtime>
</DiPManifest>"#;

    #[test]
    fn reads_title_id_and_the_64_bit_program() {
        let data = parse_installer_data(INSTALLER_XML);
        assert_eq!(data.title.as_deref(), Some("Some EA Game"));
        assert_eq!(data.id.as_deref(), Some("1234567"));
        assert_eq!(data.program.as_deref(), Some(r"Bin\Game.exe"));
    }

    #[test]
    fn strips_the_registry_prefix() {
        assert_eq!(
            strip_registry_prefix(
                r"[HKEY_LOCAL_MACHINE\SOFTWARE\Respawn\Apex\Install Dir]r5apex.exe"
            ),
            "r5apex.exe"
        );
        assert_eq!(strip_registry_prefix("Game.exe"), "Game.exe");
    }

    #[test]
    fn finds_the_game_behind_an_anti_cheat_launcher() {
        let folder = std::env::temp_dir()
            .join(format!("vaultime-ea-test-{}", uuid::Uuid::new_v4()))
            .join("Apex");
        fs::create_dir_all(folder.join("__Installer")).unwrap();
        let xml = INSTALLER_XML.replace(r"Bin\Game.exe", "EasyAntiCheat_launcher.exe");
        fs::write(folder.join(INSTALLER_DATA), xml).unwrap();
        fs::File::create(folder.join("EasyAntiCheat_launcher.exe"))
            .unwrap()
            .set_len(2_000_000)
            .unwrap();
        fs::File::create(folder.join("r5apex.exe"))
            .unwrap()
            .set_len(90_000_000)
            .unwrap();

        let game = game_from_folder(&folder, None, &HashSet::new()).unwrap();
        assert_eq!(game.title, "Some EA Game");
        assert!(game.executable_path.ends_with("r5apex.exe"));
        assert_eq!(game.source, "ea");

        let unknown = folder.parent().unwrap().join("Not A Game");
        fs::create_dir_all(&unknown).unwrap();
        assert!(game_from_folder(&unknown, None, &HashSet::new()).is_none());
        fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }
}
