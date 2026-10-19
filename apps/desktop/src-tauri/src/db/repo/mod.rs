// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Repository implementations for each domain entity.

pub mod annotations;
pub mod backup_snapshots;
pub mod corrections;
pub mod devices;
pub mod earlier_playtime;
pub mod game_assets;
pub mod games;
pub mod ignored;
pub mod offered;
pub mod session_events;
pub mod sessions;
pub mod settings;

use crate::error::VaultimeError;

pub(crate) fn map_db(error: rusqlite::Error) -> VaultimeError {
    match error {
        rusqlite::Error::QueryReturnedNoRows => {
            VaultimeError::NotFound("it is no longer there, it may have been removed".into())
        }
        error => VaultimeError::Database(error.to_string()),
    }
}
