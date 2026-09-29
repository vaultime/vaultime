// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Games from the itch.io app, from the `butler.db` database it keeps.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use log::{info, warn};
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;

use super::{DiscoveredGame, find_main_executable};
use crate::platform::process::path_key;

/// butler's name for programs that run natively here.
#[cfg(windows)]
const NATIVE_FLAVOR: &str = "windows";
#[cfg(not(windows))]
const NATIVE_FLAVOR: &str = "linux";

/// The programs butler found when it installed a game.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Verdict {
    #[serde(default)]
    base_path: String,
    #[serde(default)]
    candidates: Option<Vec<LaunchCandidate>>,
}

#[derive(Deserialize)]
struct LaunchCandidate {
    path: String,
    #[serde(default)]
    flavor: String,
}

struct Install {
    id: i64,
    title: String,
    verdict: Option<String>,
    folder: String,
}

fn databases() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(config) = dirs::config_dir() {
        paths.push(config.join("itch").join("db").join("butler.db"));
    }
    #[cfg(target_os = "linux")]
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".var/app/io.itch.itch/config/itch/db/butler.db"));
    }
    paths
}

pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let mut games = Vec::new();
    for database in databases().into_iter().filter(|path| path.is_file()) {
        match installed_games(&database) {
            Ok(installs) => games.extend(
                installs
                    .into_iter()
                    .filter_map(|install| game(install, existing)),
            ),
            Err(error) => warn!("could not read the itch.io library: {error}"),
        }
    }
    info!("itch.io discovery found {} game(s)", games.len());
    games
}

/// Every installed game. A custom install folder wins over the install
/// location plus the folder name.
fn installed_games(database: &Path) -> rusqlite::Result<Vec<Install>> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare(
        "SELECT games.id, games.title, caves.verdict, caves.custom_install_folder,
                install_locations.path, caves.install_folder_name
         FROM caves
         JOIN games ON games.id = caves.game_id
         LEFT JOIN install_locations ON install_locations.id = caves.install_location_id
         WHERE games.classification = 'game'",
    )?;
    statement
        .query_map([], |row| {
            let custom: Option<String> = row.get(3)?;
            let location: Option<String> = row.get(4)?;
            let name: Option<String> = row.get(5)?;
            let folder = custom
                .filter(|folder| !folder.is_empty())
                .or_else(|| {
                    Some(
                        Path::new(&location?)
                            .join(name?)
                            .to_string_lossy()
                            .into_owned(),
                    )
                })
                .unwrap_or_default();
            Ok(Install {
                id: row.get(0)?,
                title: row.get(1)?,
                verdict: row.get(2)?,
                folder,
            })
        })?
        .collect()
}

fn game(install: Install, existing: &HashSet<String>) -> Option<DiscoveredGame> {
    let folder = Path::new(&install.folder);
    if install.folder.is_empty() {
        return None;
    }
    let program = install
        .verdict
        .as_deref()
        .and_then(|verdict| verdict_program(verdict, folder))
        .filter(|path| path.is_file())
        .or_else(|| find_main_executable(folder, &install.title))?;
    let executable_path = program.to_string_lossy().into_owned();
    Some(DiscoveredGame {
        title: install.title,
        already_added: existing.contains(&path_key(&executable_path)),
        executable_path,
        install_folder: Some(install.folder),
        source: "itch".into(),
        source_id: Some(install.id.to_string()),
    })
}

/// The program butler found for this platform, relative to its base folder.
fn verdict_program(verdict: &str, folder: &Path) -> Option<PathBuf> {
    let verdict: Verdict = serde_json::from_str(verdict).ok()?;
    let candidates = verdict.candidates.unwrap_or_default();
    let candidate = candidates
        .iter()
        .find(|candidate| candidate.flavor == NATIVE_FLAVOR)?;
    let base = if verdict.base_path.is_empty() {
        folder.to_path_buf()
    } else {
        PathBuf::from(verdict.base_path)
    };
    Some(base.join(&candidate.path))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn lists_installed_games_with_their_native_programs() {
        let root =
            std::env::temp_dir().join(format!("vaultime-itch-test-{}", uuid::Uuid::new_v4()));
        let library = root.join("apps");
        let folder = library.join("some-game");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("Game.exe"), b"").unwrap();
        fs::write(folder.join("game.x86_64"), b"").unwrap();
        let verdict = serde_json::json!({
            "basePath": folder,
            "candidates": [
                { "path": "Game.exe", "flavor": "windows", "depth": 1 },
                { "path": "game.x86_64", "flavor": "linux", "depth": 1 }
            ]
        });
        let database = root.join("butler.db");
        {
            let connection = Connection::open(&database).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE games (id INTEGER, title TEXT, classification TEXT);
                     CREATE TABLE install_locations (id TEXT, path TEXT);
                     CREATE TABLE caves (id TEXT, game_id INTEGER, verdict TEXT,
                         install_location_id TEXT, install_folder_name TEXT, custom_install_folder TEXT);
                     INSERT INTO games VALUES (1, 'Some Game', 'game');
                     INSERT INTO games VALUES (2, 'Some Tool', 'tool');",
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO install_locations VALUES ('loc', ?1)",
                    [library.to_string_lossy()],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO caves VALUES ('cave', 1, ?1, 'loc', 'some-game', NULL)",
                    [verdict.to_string()],
                )
                .unwrap();
            connection
                .execute_batch(
                    "INSERT INTO caves VALUES ('tool', 2, NULL, 'loc', 'some-tool', NULL);",
                )
                .unwrap();
        }

        let installs = installed_games(&database).unwrap();
        assert_eq!(installs.len(), 1, "tools are no games");
        let game = game(installs.into_iter().next().unwrap(), &HashSet::new()).unwrap();
        assert_eq!(game.title, "Some Game");
        let native = if cfg!(windows) {
            "Game.exe"
        } else {
            "game.x86_64"
        };
        assert!(game.executable_path.ends_with(native));
        assert_eq!(game.source, "itch");
        fs::remove_dir_all(root).unwrap();
    }
}
