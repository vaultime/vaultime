// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Games from the Xbox app and PC Game Pass. A drive that holds such games has
//! a `.GamingRoot` file naming its library folders, older installs sit in
//! `Program Files\ModifiableWindowsApps`. Every game keeps a
//! `MicrosoftGame.config`, usually in its `Content` folder.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::xml::{attribute, read_text, tags};
use super::{DiscoveredGame, find_main_executable, metadata, scanner};
use crate::platform::process::path_key;

const GAMING_ROOT: &str = ".GamingRoot";
const GAMING_ROOT_MAGIC: &[u8] = b"RGBX";
const MODIFIABLE_APPS: &str = r"Program Files\ModifiableWindowsApps";
const CONTENT: &str = "Content";
const CONFIG: &str = "MicrosoftGame.config";
/// Localized names point into resource files, the folder name is used instead.
const RESOURCE_PREFIX: &str = "ms-resource:";

pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let games: Vec<DiscoveredGame> = scanner::fixed_drives()
        .into_iter()
        .flat_map(|drive| library_folders(&drive))
        .filter_map(|library| fs::read_dir(library).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| game_from_folder(&entry.path(), existing))
        .collect();
    log::info!("Xbox discovery found {} game(s)", games.len());
    games
}

fn library_folders(drive: &Path) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = fs::read(drive.join(GAMING_ROOT))
        .map(|bytes| parse_gaming_root(&bytes))
        .unwrap_or_default()
        .into_iter()
        .map(|folder| drive.join(folder.trim_start_matches(['\\', '/'])))
        .collect();
    folders.push(drive.join(MODIFIABLE_APPS));
    folders
}

/// Library folders from a `.GamingRoot` file: `RGBX`, a 32 bit count, then
/// the folders as UTF-16 text, each ending in a zero.
fn parse_gaming_root(bytes: &[u8]) -> Vec<String> {
    let Some(rest) = bytes.strip_prefix(GAMING_ROOT_MAGIC) else {
        return Vec::new();
    };
    let Some((count, text)) = rest.split_first_chunk::<4>() else {
        return Vec::new();
    };
    let units: Vec<u16> = text
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    units
        .split(|&unit| unit == 0)
        .filter(|folder| !folder.is_empty())
        .take(u32::from_le_bytes(*count) as usize)
        .map(String::from_utf16_lossy)
        .collect()
}

fn game_from_folder(folder: &Path, existing: &HashSet<String>) -> Option<DiscoveredGame> {
    // Program names in the config are relative to the folder that holds it.
    let content = [folder.join(CONTENT), folder.to_path_buf()]
        .into_iter()
        .find(|base| base.join(CONFIG).is_file())?;
    let config = parse_config(&read_text(&content.join(CONFIG))?);
    let title = config.title.unwrap_or_else(|| {
        folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    // Some games name a launch helper here, the scorer then finds the game.
    let program = config
        .executable
        .map(|name| content.join(name))
        .filter(|path| {
            path.is_file()
                && path.file_name().is_some_and(|name| {
                    metadata::is_likely_game_executable(&name.to_string_lossy())
                })
        })
        .or_else(|| find_main_executable(&content, &title))?;
    let executable_path = program.to_string_lossy().into_owned();
    Some(DiscoveredGame {
        title,
        already_added: existing.contains(&path_key(&executable_path)),
        executable_path,
        install_folder: Some(folder.to_string_lossy().into_owned()),
        source: "xbox".into(),
        source_id: config.identity,
    })
}

#[derive(Debug, Default, PartialEq, Eq)]
struct GameConfig {
    title: Option<String>,
    executable: Option<String>,
    identity: Option<String>,
}

fn parse_config(xml: &str) -> GameConfig {
    let executables = tags(xml, "Executable");
    GameConfig {
        title: tags(xml, "ShellVisuals")
            .first()
            .and_then(|tag| attribute(tag, "DefaultDisplayName"))
            .filter(|title| !title.is_empty() && !title.starts_with(RESOURCE_PREFIX)),
        executable: executables
            .iter()
            .find(|tag| attribute(tag, "Id").as_deref() == Some("Game"))
            .or(executables.first())
            .and_then(|tag| attribute(tag, "Name")),
        identity: tags(xml, "Identity")
            .first()
            .and_then(|tag| attribute(tag, "Name")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Game configVersion="1">
  <Identity Name="Microsoft.SomeGame" Publisher="CN=Someone" Version="1.2.3.0" />
  <ExecutableList>
    <Executable Name="gamelaunchhelper.exe" TargetDeviceFamily="PC" Id="Game" />
  </ExecutableList>
  <ShellVisuals DefaultDisplayName="Tom &amp; Jerry's Race" PublisherDisplayName="Someone" />
</Game>"#;

    #[test]
    fn reads_the_gaming_root_file() {
        // The file on a drive with the default library folder.
        let mut bytes = b"RGBX\x01\x00\x00\x00".to_vec();
        for unit in "XboxGames".encode_utf16().chain([0]) {
            bytes.extend(unit.to_le_bytes());
        }
        assert_eq!(parse_gaming_root(&bytes), ["XboxGames"]);
        assert!(parse_gaming_root(b"nope").is_empty());
    }

    #[test]
    fn reads_title_program_and_identity() {
        let config = parse_config(CONFIG_XML);
        assert_eq!(config.title.as_deref(), Some("Tom & Jerry's Race"));
        assert_eq!(config.executable.as_deref(), Some("gamelaunchhelper.exe"));
        assert_eq!(config.identity.as_deref(), Some("Microsoft.SomeGame"));

        let localized = CONFIG_XML.replace(
            "Tom &amp; Jerry's Race",
            "ms-resource:ApplicationDisplayName",
        );
        assert_eq!(parse_config(&localized).title, None);
    }

    #[test]
    fn skips_the_launch_helper_for_the_game() {
        let folder = std::env::temp_dir()
            .join(format!("vaultime-xbox-test-{}", uuid::Uuid::new_v4()))
            .join("Some Game");
        let content = folder.join(CONTENT);
        fs::create_dir_all(&content).unwrap();
        let localized = CONFIG_XML.replace("Tom &amp; Jerry's Race", "ms-resource:Name");
        fs::write(content.join(CONFIG), localized).unwrap();
        fs::File::create(content.join("gamelaunchhelper.exe"))
            .unwrap()
            .set_len(200_000)
            .unwrap();
        fs::File::create(content.join("SomeGame.exe"))
            .unwrap()
            .set_len(90_000_000)
            .unwrap();

        let game = game_from_folder(&folder, &HashSet::new()).unwrap();
        assert_eq!(game.title, "Some Game", "the folder names localized games");
        assert!(
            game_from_folder(&folder.join(CONTENT), &HashSet::new()).is_some(),
            "a config in the root counts too"
        );
        assert!(game.executable_path.ends_with("SomeGame.exe"));
        assert_eq!(game.source, "xbox");
        fs::remove_dir_all(folder.parent().unwrap()).unwrap();
    }
}
