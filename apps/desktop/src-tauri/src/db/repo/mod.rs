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
        rusqlite::Error::QueryReturnedNoRows => VaultimeError::NotFound(
            "the record it looked for is missing, it may have been removed".into(),
        ),
        error if is_busy(&error) => busy(),
        error => VaultimeError::Database(error.to_string()),
    }
}

/// Whether `error` says another connection holds the database.
pub(crate) fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

pub(crate) fn busy() -> VaultimeError {
    VaultimeError::Busy(
        "another program is using the database. Close it or wait a moment, then try again".into(),
    )
}
