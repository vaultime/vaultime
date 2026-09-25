// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tauri IPC command handlers.

use std::sync::Arc;

use tauri::State;

use crate::db::connection::Database;
use crate::db::models::{CreateGame, Game, Session, UpdateGame};
use crate::db::repo::{games, sessions};
use crate::error::VaultimeError;
use crate::tracking::engine::TrackingEngine;

// ---------------------------------------------------------------------------
// Game commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_games(db: State<'_, Arc<Database>>) -> Result<Vec<Game>, VaultimeError> {
    games::list_games(&db)
}

#[tauri::command]
pub fn get_game(db: State<'_, Arc<Database>>, id: String) -> Result<Game, VaultimeError> {
    games::get_game(&db, &id)
}

#[tauri::command]
pub fn create_game(
    db: State<'_, Arc<Database>>,
    input: CreateGame,
) -> Result<Game, VaultimeError> {
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

#[tauri::command]
pub fn delete_game(db: State<'_, Arc<Database>>, id: String) -> Result<bool, VaultimeError> {
    games::delete_game(&db, &id)
}

// ---------------------------------------------------------------------------
// Session commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_sessions(db: State<'_, Arc<Database>>) -> Result<Vec<Session>, VaultimeError> {
    sessions::list_all_sessions(&db)
}

#[tauri::command]
pub fn get_sessions_for_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
) -> Result<Vec<Session>, VaultimeError> {
    sessions::list_sessions_for_game(&db, &game_id)
}

#[tauri::command]
pub fn get_active_sessions(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Session>, VaultimeError> {
    sessions::get_active_sessions(&db)
}

// ---------------------------------------------------------------------------
// Tracking commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_tracking_status(
    engine: State<'_, TrackingEngine>,
) -> Result<bool, VaultimeError> {
    Ok(engine.is_running())
}
