// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Games installed through the Heroic Games Launcher, for Epic through its
//! Legendary backend, for Amazon through Nile and for GOG.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{MAIN_SEPARATOR_STR, Path, PathBuf};

use log::info;
use serde::Deserialize;

use super::{DiscoveredGame, find_main_executable, fuel, gog};
use crate::platform::process::path_key;

#[derive(Deserialize)]
struct LegendaryGame {
    app_name: String,
    title: String,
    install_path: String,
    executable: String,
    #[serde(default)]
    is_dlc: bool,
}

#[derive(Deserialize)]
struct NileInstall {
    id: String,
    path: String,
}

#[derive(Deserialize)]
struct NileGame {
    id: String,
    #[serde(default)]
    product: NileProduct,
}

#[derive(Deserialize, Default)]
struct NileProduct {
    #[serde(default)]
    title: Option<String>,
}

#[derive(Deserialize)]
struct GogInstalled {
    #[serde(default)]
    installed: Vec<GogInstall>,
}

#[derive(Deserialize)]
struct GogInstall {
    install_path: String,
}

/// Heroic's config folder, the Flatpak one on Linux when that is in use.
fn config_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Some(config) = dirs::config_dir() {
        folders.push(config.join("heroic"));
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            folders.push(home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic"));
        }
    }
    folders
}

pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let mut games = Vec::new();
    for folder in config_folders()
        .into_iter()
        .filter(|folder| folder.is_dir())
    {
        let legendary = folder
            .join("legendaryConfig")
            .join("legendary")
            .join("installed.json");
        if let Ok(content) = fs::read_to_string(legendary) {
            games.extend(parse_legendary(&content, existing));
        }
        let nile = folder.join("nile_config").join("nile");
        if let Ok(content) = fs::read_to_string(nile.join("installed.json")) {
            let library = fs::read_to_string(nile.join("library.json")).unwrap_or_default();
            games.extend(parse_nile(&content, &nile_titles(&library), existing));
        }
        let gog = folder.join("gog_store").join("installed.json");
        if let Ok(content) = fs::read_to_string(gog) {
            games.extend(
                gog_install_paths(&content)
                    .iter()
                    .filter_map(|path| gog::game_from_folder(Path::new(path), "heroic", existing)),
            );
        }
    }
    info!("Heroic discovery found {} game(s)", games.len());
    games
}

/// Epic games from Legendary's list of installs, without DLCs.
fn parse_legendary(content: &str, existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let Ok(installed) = serde_json::from_str::<HashMap<String, LegendaryGame>>(content) else {
        return Vec::new();
    };
    installed
        .into_values()
        .filter(|game| !game.is_dlc && !game.executable.is_empty())
        .map(|game| {
            let executable = Path::new(&game.install_path)
                .join(game.executable.replace(['/', '\\'], MAIN_SEPARATOR_STR));
            let executable_path = executable.to_string_lossy().into_owned();
            DiscoveredGame {
                title: game.title,
                already_added: existing.contains(&path_key(&executable_path)),
                executable_path,
                install_folder: Some(game.install_path),
                source: "heroic".into(),
                source_id: Some(game.app_name),
            }
        })
        .collect()
}

/// Amazon games from Nile's list of installs. The program comes from the
/// game's `fuel.json`, the title from Nile's library, which may lack it.
fn parse_nile(
    content: &str,
    titles: &HashMap<String, String>,
    existing: &HashSet<String>,
) -> Vec<DiscoveredGame> {
    let installs: Vec<NileInstall> = serde_json::from_str(content).unwrap_or_default();
    installs
        .into_iter()
        .filter_map(|install| {
            let folder = Path::new(&install.path);
            let title = titles.get(&install.id).cloned().unwrap_or_else(|| {
                folder
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            let program = fuel::program(folder).or_else(|| find_main_executable(folder, &title))?;
            let executable_path = program.to_string_lossy().into_owned();
            Some(DiscoveredGame {
                title,
                already_added: existing.contains(&path_key(&executable_path)),
                executable_path,
                install_folder: Some(install.path),
                source: "heroic".into(),
                source_id: Some(install.id),
            })
        })
        .collect()
}

fn nile_titles(library: &str) -> HashMap<String, String> {
    serde_json::from_str::<Vec<NileGame>>(library)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|game| Some((game.id, game.product.title?)))
        .collect()
}

/// Install folders of GOG games. Their titles come from GOG's info files.
fn gog_install_paths(content: &str) -> Vec<String> {
    serde_json::from_str::<GogInstalled>(content)
        .map(|installed| {
            installed
                .installed
                .into_iter()
                .map(|game| game.install_path)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_legendary_installs_without_dlcs() {
        let content = r#"{
            "Sugar": {
                "app_name": "Sugar", "title": "Some Game", "install_path": "/home/me/Games/Heroic/SomeGame",
                "executable": "Binaries/Win64/Game.exe", "is_dlc": false, "platform": "Windows"
            },
            "SugarDlc": {
                "app_name": "SugarDlc", "title": "Some DLC", "install_path": "/home/me/Games/Heroic/SomeGame",
                "executable": "", "is_dlc": true, "platform": "Windows"
            }
        }"#;
        let games = parse_legendary(content, &HashSet::new());
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Some Game");
        assert_eq!(games[0].source, "heroic");
        assert!(games[0].executable_path.ends_with("Game.exe"));
    }

    #[test]
    fn reads_nile_installs_with_titles_and_fuel_programs() {
        let root =
            std::env::temp_dir().join(format!("vaultime-nile-test-{}", uuid::Uuid::new_v4()));
        let folder = root.join("SomeAmazonGame");
        fs::create_dir_all(folder.join("Bin")).unwrap();
        fs::write(
            folder.join("fuel.json"),
            r#"{"Main": {"Command": "Bin/Game.exe"}}"#,
        )
        .unwrap();
        fs::write(folder.join("Bin").join("Game.exe"), b"").unwrap();
        let installed =
            serde_json::json!([{ "id": "amzn1.adg.product.1", "version": "1", "path": folder }]);
        let library = r#"[{"id": "amzn1.adg.product.1", "product": {"id": "x", "title": "Some Amazon Game"}}]"#;

        let games = parse_nile(
            &installed.to_string(),
            &nile_titles(library),
            &HashSet::new(),
        );
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Some Amazon Game");
        assert!(games[0].executable_path.ends_with("Game.exe"));
        assert_eq!(games[0].source_id.as_deref(), Some("amzn1.adg.product.1"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reads_gog_install_folders() {
        let content = r#"{ "installed": [ { "platform": "windows", "executable": "", "install_path": "/home/me/Games/Heroic/Stardew Valley", "appName": "1453375253" } ] }"#;
        assert_eq!(
            gog_install_paths(content),
            ["/home/me/Games/Heroic/Stardew Valley"]
        );
        assert!(gog_install_paths("[]").is_empty());
    }
}
