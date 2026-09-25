// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tauri IPC command handlers.

use tauri::State;

use crate::db::connection::Database;
use crate::db::models::{CreateGame, Game, UpdateGame};
use crate::db::repo::games;
use crate::error::VaultimeError;

// ---------------------------------------------------------------------------
// Game commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_games(db: State<'_, Database>) -> Result<Vec<Game>, VaultimeError> {
    games::list_games(&db)
}

#[tauri::command]
pub fn get_game(db: State<'_, Database>, id: String) -> Result<Game, VaultimeError> {
    games::get_game(&db, &id)
}

#[tauri::command]
pub fn create_game(db: State<'_, Database>, input: CreateGame) -> Result<Game, VaultimeError> {
    games::create_game(&db, &input)
}

#[tauri::command]
pub fn update_game(
    db: State<'_, Database>,
    id: String,
    input: UpdateGame,
) -> Result<Game, VaultimeError> {
    games::update_game(&db, &id, &input)
}

#[tauri::command]
pub fn delete_game(db: State<'_, Database>, id: String) -> Result<bool, VaultimeError> {
    games::delete_game(&db, &id)
}
