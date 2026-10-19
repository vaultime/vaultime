// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finding games and the programs that are no game.

use std::sync::Arc;

use tauri::State;

use crate::assets::AssetManager;
use crate::db::connection::Database;
use crate::db::models::Game;
use crate::db::repo::ignored::{self, IgnoredProgram};
use crate::db::repo::offered;
use crate::discovery::auto::{FoundGames, NewGames};
use crate::discovery::{self, DiscoveredGame};
use crate::error::VaultimeError;

#[tauri::command(async)]
pub fn discover_games(
    db: State<'_, Arc<Database>>,
    paths: Vec<String>,
) -> Result<Vec<DiscoveredGame>, VaultimeError> {
    discovery::offer(&db, discovery::scanner::scan_folders(&db, &paths)?)
}

#[tauri::command(async)]
pub fn discover_steam_games(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<DiscoveredGame>, VaultimeError> {
    discovery::offer_launcher_games(&db, discovery::steam::discover_steam_games(&db)?)
}

#[tauri::command(async)]
pub fn get_default_scan_paths() -> Result<Vec<String>, VaultimeError> {
    Ok(discovery::scanner::default_scan_paths())
}

/// Games that launchers other than Steam installed here.
#[tauri::command(async)]
pub fn discover_launcher_games(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<DiscoveredGame>, VaultimeError> {
    discovery::offer_launcher_games(&db, discovery::discover_launcher_games(&db)?)
}

/// The games the look at start found that are still new.
#[tauri::command(async)]
pub fn list_found_games(
    db: State<'_, Arc<Database>>,
    found: State<'_, FoundGames>,
) -> Result<NewGames, VaultimeError> {
    discovery::auto::new_games(&db, &found)
}

/// Remembers that discovery showed these programs, so the look at start
/// never offers them again. Returns how many were not shown before.
#[tauri::command(async)]
pub fn remember_offered_games(
    db: State<'_, Arc<Database>>,
    paths: Vec<String>,
) -> Result<usize, VaultimeError> {
    offered::remember(&db, &paths)
}

/// Programs the player said are no game, newest first.
#[tauri::command(async)]
pub fn list_ignored_programs(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<IgnoredProgram>, VaultimeError> {
    ignored::list(&db)
}

/// Never offers this program again and never counts it.
#[tauri::command(async)]
pub fn ignore_program(
    db: State<'_, Arc<Database>>,
    path: String,
    title: String,
) -> Result<(), VaultimeError> {
    ignored::ignore(&db, &path, &title)
}

/// Lets discovery offer an ignored program again and the tracker count it.
#[tauri::command(async)]
pub fn allow_program(
    db: State<'_, Arc<Database>>,
    path_key: String,
) -> Result<bool, VaultimeError> {
    ignored::allow(&db, &path_key)
}

/// Adds the picked candidates. One marked as another install of a library
/// game counts as that game.
#[tauri::command(async)]
pub fn import_discovered_games(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    discoveries: Vec<DiscoveredGame>,
) -> Result<Vec<Game>, VaultimeError> {
    discovery::import(&db, &asset_manager, &discoveries)
}
