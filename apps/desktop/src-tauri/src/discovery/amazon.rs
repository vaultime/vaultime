// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Games from the Amazon Games app, from its install database and the
//! `fuel.json` that names the program of each game.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use log::{info, warn};
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;

use super::{DiscoveredGame, find_main_executable};
use crate::platform::process::path_key;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Fuel {
    main: Option<FuelMain>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FuelMain {
    command: Option<String>,
}

fn database() -> Option<PathBuf> {
    Some(
        dirs::data_local_dir()?
            .join("Amazon Games")
            .join("Data")
            .join("Games")
            .join("Sql")
            .join("GameInstallInfo.sqlite"),
    )
}

pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let Some(database) = database().filter(|path| path.is_file()) else {
        return Vec::new();
    };
    let games: Vec<DiscoveredGame> = match installed_games(&database) {
        Ok(rows) => rows
            .into_iter()
            .filter_map(|(id, title, folder)| game(id, title, &folder, existing))
            .collect(),
        Err(error) => {
            warn!("could not read the Amazon Games library: {error}");
            Vec::new()
        }
    };
    info!("Amazon Games discovery found {} game(s)", games.len());
    games
}

/// Id, title and install folder of every installed game.
fn installed_games(database: &Path) -> rusqlite::Result<Vec<(String, String, String)>> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare(
        "SELECT Id, ProductTitle, InstallDirectory FROM DbSet WHERE Installed = 1 AND InstallDirectory != ''",
    )?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect()
}

fn game(
    id: String,
    title: String,
    folder: &str,
    existing: &HashSet<String>,
) -> Option<DiscoveredGame> {
    let folder_path = Path::new(folder);
    let program = fs::read_to_string(folder_path.join("fuel.json"))
        .ok()
        .and_then(|json| fuel_command(&json))
        .map(|command| folder_path.join(command))
        .filter(|path| path.is_file())
        .or_else(|| find_main_executable(folder_path, &title))?;
    let executable_path = program.to_string_lossy().into_owned();
    Some(DiscoveredGame {
        title,
        already_added: existing.contains(&path_key(&executable_path)),
        executable_path,
        install_folder: Some(folder.to_owned()),
        source: "amazon".into(),
        source_id: Some(id),
    })
}

/// The program `fuel.json` starts, relative to the game folder.
fn fuel_command(json: &str) -> Option<String> {
    serde_json::from_str::<Fuel>(json)
        .ok()?
        .main?
        .command
        .filter(|command| !command.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_program_from_fuel_json() {
        let json = r#"{"SchemaVersion": "2", "Main": {"Command": "Bin/Game.exe", "Args": []}}"#;
        assert_eq!(fuel_command(json).as_deref(), Some("Bin/Game.exe"));
        assert_eq!(fuel_command(r#"{"Main": {"Command": ""}}"#), None);
        assert_eq!(fuel_command("not json"), None);
    }

    #[test]
    fn lists_installed_games_with_their_programs() {
        let root =
            std::env::temp_dir().join(format!("vaultime-amazon-test-{}", uuid::Uuid::new_v4()));
        let folder = root.join("Some Game");
        fs::create_dir_all(folder.join("Bin")).unwrap();
        fs::write(
            folder.join("fuel.json"),
            r#"{"Main": {"Command": "Bin/Game.exe"}}"#,
        )
        .unwrap();
        fs::write(folder.join("Bin").join("Game.exe"), b"").unwrap();
        let database = root.join("GameInstallInfo.sqlite");
        {
            let connection = Connection::open(&database).unwrap();
            connection
                .execute_batch(&format!(
                    "CREATE TABLE DbSet (Id TEXT, ProductTitle TEXT, InstallDirectory TEXT, Installed INTEGER);
                     INSERT INTO DbSet VALUES ('amzn1.adg.product.1', 'Some Game', '{}', 1);
                     INSERT INTO DbSet VALUES ('amzn1.adg.product.2', 'Removed Game', 'C:\\Nowhere', 0);",
                    folder.to_string_lossy()
                ))
                .unwrap();
        }

        let rows = installed_games(&database).unwrap();
        assert_eq!(rows.len(), 1);
        let (id, title, install) = rows.into_iter().next().unwrap();
        let game = game(id, title, &install, &HashSet::new()).unwrap();
        assert_eq!(game.title, "Some Game");
        assert!(game.executable_path.ends_with("Game.exe"));
        assert_eq!(game.source, "amazon");
        fs::remove_dir_all(root).unwrap();
    }
}
