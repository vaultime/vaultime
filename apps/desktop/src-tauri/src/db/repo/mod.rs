// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Repository implementations for each domain entity.

pub mod backup_snapshots;
pub mod devices;
pub mod game_assets;
pub mod games;
pub mod session_events;
pub mod sessions;
pub mod settings;

use crate::error::VaultimeError;

pub(crate) fn map_db(error: rusqlite::Error) -> VaultimeError {
    VaultimeError::Database(error.to_string())
}
