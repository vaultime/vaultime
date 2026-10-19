// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Games of the library, their links, statuses and artwork.

use std::sync::Arc;

use tauri::State;

use crate::assets::crop::ArtworkSource;
use crate::assets::{self, AssetManager, GameAssetView};
use crate::db::connection::Database;
use crate::db::models::{CreateGame, CropRect, Game, GameStatusChange, UpdateGame};
use crate::db::repo::games::GameLink;
use crate::db::repo::{annotations, games};
use crate::error::VaultimeError;
use crate::library;

/// Linked games, other installs here and games of other PCs, with the games
/// they count as.
#[tauri::command(async)]
pub fn list_game_links(db: State<'_, Arc<Database>>) -> Result<Vec<GameLink>, VaultimeError> {
    games::list_game_links(&db)
}

/// Makes a game count as a game of this PC, or as itself again, and works
/// out once more where the time of every game it now counts with fell, since
/// whether they step aside follows the game shown.
#[tauri::command(async)]
pub fn link_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
    linked_game_id: Option<String>,
) -> Result<(), VaultimeError> {
    library::link_game(&db, &game_id, linked_game_id.as_deref())
}

/// Every game the library shows, hidden ones too. Hidden games are still
/// tracked and their sessions count, only the library views leave them out.
/// Games of other PCs that are linked to a game of this PC count as that
/// game and are left out.
#[tauri::command(async)]
pub fn list_games(db: State<'_, Arc<Database>>) -> Result<Vec<Game>, VaultimeError> {
    games::list_shown_games(&db)
}

#[tauri::command]
pub fn create_game(db: State<'_, Arc<Database>>, input: CreateGame) -> Result<Game, VaultimeError> {
    games::create_game(&db, &input)
}

#[tauri::command]
pub fn update_game(
    db: State<'_, Arc<Database>>,
    id: String,
    input: UpdateGame,
) -> Result<Game, VaultimeError> {
    games::update_game(&db, &id, &input)
}

#[tauri::command(async)]
pub fn delete_game(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    id: String,
) -> Result<bool, VaultimeError> {
    library::delete_game(&db, &asset_manager, &id)
}

/// Makes a game count only while no other game runs, like a launcher, or
/// always again, and works out where all play fell once more.
#[tauri::command(async)]
pub fn set_game_steps_aside(
    db: State<'_, Arc<Database>>,
    game_id: String,
    steps_aside: bool,
) -> Result<Game, VaultimeError> {
    library::set_steps_aside(&db, &game_id, steps_aside)
}

#[tauri::command(async)]
pub fn list_game_assets(
    db: State<'_, Arc<Database>>,
    game_id: String,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::list_game_assets(&db, &game_id)
}

#[tauri::command(async)]
pub fn list_preferred_game_assets(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::list_preferred_game_assets(&db)
}

#[tauri::command(async)]
pub fn scan_game_assets(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::scan_game_assets(&db, &asset_manager, &game_id)
}

#[tauri::command(async)]
pub fn open_artwork_file(source_path: String) -> Result<ArtworkSource, VaultimeError> {
    assets::open_artwork_file(&source_path)
}

#[tauri::command(async)]
pub fn open_game_asset_source(
    db: State<'_, Arc<Database>>,
    game_id: String,
    asset_id: String,
) -> Result<ArtworkSource, VaultimeError> {
    assets::open_game_asset_source(&db, &game_id, &asset_id)
}

#[tauri::command(async)]
pub fn import_game_asset(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
    source_path: String,
    crop: Option<CropRect>,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::import_game_asset(&db, &asset_manager, &game_id, &source_path, crop)
}

#[tauri::command(async)]
pub fn crop_game_asset(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
    asset_id: String,
    crop: CropRect,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::crop_game_asset(&db, &asset_manager, &game_id, &asset_id, crop)
}

#[tauri::command(async)]
pub fn delete_game_asset(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
    asset_id: String,
) -> Result<Vec<GameAssetView>, VaultimeError> {
    assets::delete_game_asset(&db, &asset_manager, &game_id, &asset_id)
}

#[tauri::command]
pub fn set_preferred_game_asset(
    db: State<'_, Arc<Database>>,
    game_id: String,
    asset_id: String,
) -> Result<bool, VaultimeError> {
    assets::set_preferred_game_asset(&db, &game_id, &asset_id)
}

/// Every status change of every game, oldest first.
#[tauri::command(async)]
pub fn list_status_changes(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<GameStatusChange>, VaultimeError> {
    annotations::list_status_changes(&db)
}

/// Records a new status for a game, `none` clears it.
#[tauri::command]
pub fn set_game_status(
    db: State<'_, Arc<Database>>,
    game_id: String,
    status: String,
) -> Result<Option<GameStatusChange>, VaultimeError> {
    annotations::set_game_status(&db, &game_id, &status)
}
