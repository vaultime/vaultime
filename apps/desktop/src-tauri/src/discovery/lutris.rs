// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Games installed through Lutris, from its database and game configs.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use log::info;
use rusqlite::{Connection, OpenFlags};

use super::DiscoveredGame;
use crate::platform::process::path_key;

/// Lutris data folders, the regular one and the Flatpak one.
fn data_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Some(data) = dirs::data_dir() {
        folders.push(data.join("lutris"));
    }
    if let Some(home) = dirs::home_dir() {
        folders.push(home.join(".var/app/net.lutris.Lutris/data/lutris"));
    }
    folders
}

/// Folders that hold the per game `.yml` configs, old and new layout.
fn config_folders(data_folder: &Path) -> Vec<PathBuf> {
    let mut folders = vec![data_folder.join("games")];
    if let Some(config) = dirs::config_dir() {
        folders.push(config.join("lutris").join("games"));
    }
    folders
}

pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let mut games = Vec::new();
    for data in data_folders() {
        let database = data.join("pga.db");
        if !database.is_file() {
            continue;
        }
        match installed_games(&database) {
            Ok(rows) => {
                for (name, slug, config) in rows {
                    let executable = config_folders(&data)
                        .iter()
                        .find_map(|folder| {
                            std::fs::read_to_string(folder.join(format!("{config}.yml"))).ok()
                        })
                        .and_then(|yml| game_executable(&yml));
                    let Some(executable_path) = executable else {
                        continue;
                    };
                    let install_folder = Path::new(&executable_path)
                        .parent()
                        .map(|folder| folder.to_string_lossy().into_owned());
                    games.push(DiscoveredGame {
                        title: name,
                        already_added: existing.contains(&path_key(&executable_path)),
                        executable_path,
                        install_folder,
                        source: "lutris".into(),
                        source_id: Some(slug),
                    });
                }
            }
            Err(error) => log::warn!("could not read the Lutris library: {error}"),
        }
    }
    info!("Lutris discovery found {} game(s)", games.len());
    games
}

/// Name, slug and config file name of every installed game.
fn installed_games(database: &Path) -> rusqlite::Result<Vec<(String, String, String)>> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare(
        "SELECT name, slug, configpath FROM games WHERE installed = 1 AND configpath IS NOT NULL AND configpath != ''",
    )?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect()
}

/// The `exe` value inside the `game:` section of a Lutris config.
fn game_executable(yml: &str) -> Option<String> {
    let mut in_game_section = false;
    for line in yml.lines() {
        if !line.starts_with(' ') && !line.trim().is_empty() {
            in_game_section = line.trim_end() == "game:";
            continue;
        }
        if in_game_section && let Some(value) = line.trim().strip_prefix("exe:") {
            let value = value.trim().trim_matches(['"', '\'']);
            return (!value.is_empty()).then(|| value.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_exe_of_the_game_section() {
        let yml = "game:\n  args: -windowed\n  exe: /home/me/Games/some-game/drive_c/Game/Game.exe\n  prefix: /home/me/Games/some-game\nsystem:\n  exe: /usr/bin/other\nwine:\n  version: lutris-7\n";
        assert_eq!(
            game_executable(yml).as_deref(),
            Some("/home/me/Games/some-game/drive_c/Game/Game.exe")
        );
        assert_eq!(
            game_executable("game:\n  exe: 'quoted path'\n").as_deref(),
            Some("quoted path")
        );
        assert_eq!(game_executable("system:\n  exe: /usr/bin/other\n"), None);
    }

    #[test]
    fn lists_installed_games_from_the_database() {
        let path =
            std::env::temp_dir().join(format!("vaultime-lutris-test-{}.db", uuid::Uuid::new_v4()));
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE games (name TEXT, slug TEXT, configpath TEXT, installed INTEGER);
                     INSERT INTO games VALUES ('Some Game', 'some-game', 'some-game-1690000000', 1);
                     INSERT INTO games VALUES ('Not Installed', 'not-installed', 'not-installed-1', 0);",
                )
                .unwrap();
        }
        let games = installed_games(&path).unwrap();
        assert_eq!(
            games,
            [(
                "Some Game".into(),
                "some-game".into(),
                "some-game-1690000000".into()
            )]
        );
        std::fs::remove_file(path).unwrap();
    }
}
