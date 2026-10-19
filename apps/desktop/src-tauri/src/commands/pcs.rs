// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! This PC, the other PCs whose sessions are here and merging their backups.

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::merge::{GameChoice, MergePreview, MergeSummary};
use crate::backup::{self};
use crate::db::connection::Database;
use crate::db::models::Device;
use crate::db::repo::devices;
use crate::error::VaultimeError;
use crate::integrity::ledger::{self, LedgerReport};

/// Every PC this database knows, this one included.
#[tauri::command(async)]
pub fn list_devices(db: State<'_, Arc<Database>>) -> Result<Vec<Device>, VaultimeError> {
    devices::list_devices(&db)
}

/// What merging another PC's backup would bring in.
#[tauri::command(async)]
pub fn preview_merge(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    path: String,
    trust_new_keys: bool,
) -> Result<MergePreview, VaultimeError> {
    backup::merge::preview(&db, &app_context, Path::new(&path), trust_new_keys)
}

/// Merges another PC's backup, with the player's choice for each game.
#[tauri::command(async)]
pub fn merge_backup(
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    app_context: State<'_, AppContext>,
    path: String,
    choices: Vec<GameChoice>,
    trust_new_keys: bool,
) -> Result<MergeSummary, VaultimeError> {
    backup::merge::apply(
        &db,
        &asset_manager,
        &app_context,
        Path::new(&path),
        &choices,
        trust_new_keys,
    )
}

/// This PC: its name and what the ledgers say about the history.
#[derive(Debug, Serialize)]
pub struct ThisPc {
    pub device_id: String,
    pub name: String,
    pub ledger: LedgerReport,
    /// Whether this PC has no history of its own yet, so restoring a backup
    /// of another PC can make it continue as that PC.
    pub fresh: bool,
}

fn this_pc(db: &Database, device_id: &str) -> Result<ThisPc, VaultimeError> {
    let device = devices::get_device(db, device_id)?;
    Ok(ThisPc {
        name: device.name.unwrap_or_else(|| device.id.clone()),
        device_id: device.id,
        ledger: db.with_conn(ledger::report)?,
        fresh: db.with_conn(ledger::fresh_install)?,
    })
}

#[tauri::command(async)]
pub fn get_this_pc(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
) -> Result<ThisPc, VaultimeError> {
    this_pc(&db, &app_context.device_id)
}

#[tauri::command(async)]
pub fn rename_this_pc(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    name: String,
) -> Result<ThisPc, VaultimeError> {
    devices::rename_device(&db, &app_context.device_id, &name)?;
    this_pc(&db, &app_context.device_id)
}
