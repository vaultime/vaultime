// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cloud backup: the sign-in in the OS credential store and backups on the server.

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::backups::record_snapshot;
use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::remote::{RemoteBackupRestoreResult, RemoteBackupUploadResult};
use crate::backup::{self};
use crate::constants::CLOUD_API_BASE_URL;
use crate::db::connection::Database;
use crate::error::VaultimeError;
use crate::secure_storage;
use crate::tracking::engine::TrackingEngine;

#[tauri::command]
pub async fn load_cloud_session_secure() -> Result<Option<String>, VaultimeError> {
    on_blocking_pool(secure_storage::load_cloud_session).await
}

#[tauri::command]
pub async fn store_cloud_session_secure(session_json: String) -> Result<bool, VaultimeError> {
    on_blocking_pool(move || secure_storage::store_cloud_session(&session_json)).await?;
    Ok(true)
}

#[tauri::command]
pub async fn clear_cloud_session_secure() -> Result<bool, VaultimeError> {
    on_blocking_pool(secure_storage::clear_cloud_session).await?;
    Ok(true)
}

#[tauri::command]
pub async fn has_cloud_backup_key_secure(account_id: String) -> Result<bool, VaultimeError> {
    on_blocking_pool(move || secure_storage::has_cloud_backup_key(&account_id)).await
}

/// Derives the key from the passphrase, which takes a while on purpose.
#[tauri::command]
pub async fn store_cloud_backup_key_secure(
    account_id: String,
    passphrase: String,
    expected_key_check: Option<String>,
) -> Result<bool, VaultimeError> {
    on_blocking_pool(move || {
        secure_storage::store_cloud_backup_key(
            &account_id,
            &passphrase,
            expected_key_check.as_deref(),
        )
    })
    .await?;
    Ok(true)
}

#[tauri::command]
pub async fn clear_cloud_backup_key_secure(account_id: String) -> Result<bool, VaultimeError> {
    on_blocking_pool(move || secure_storage::clear_cloud_backup_key(&account_id)).await?;
    Ok(true)
}

/// Shows in the tray and taskbar icon whether this PC is signed in. Runs off
/// the main thread, as a redraw may wait for it.
#[tauri::command(async)]
pub fn set_cloud_signed_in(app: AppHandle, signed_in: bool) {
    crate::tray::show_cloud_state(&app, signed_in);
}

/// Release builds talk only to the Vaultime server, whatever the page asks.
/// Development builds follow the page, so a local server can be tried.
fn cloud_api_base_url(requested: &str) -> &str {
    if cfg!(debug_assertions) {
        requested
    } else {
        CLOUD_API_BASE_URL
    }
}

/// Runs cloud work and the OS credential store on the blocking pool.
/// reqwest's blocking client must not run on the async runtime, where debug
/// builds panic and release builds hold up a worker for the whole transfer.
/// The credential store may wait for the desktop's keyring service.
async fn on_blocking_pool<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, VaultimeError> + Send + 'static,
) -> Result<T, VaultimeError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| VaultimeError::Cloud(format!("the cloud task stopped: {error}")))?
}

#[tauri::command]
pub async fn upload_remote_backup(
    app: AppHandle,
    api_base_url: String,
    access_token: String,
    account_id: String,
    client_device_id: Option<String>,
    label: Option<String>,
) -> Result<RemoteBackupUploadResult, VaultimeError> {
    on_blocking_pool(move || {
        let db = app.state::<Arc<Database>>();
        let result = backup::remote::upload_remote_backup(
            &db,
            &app.state::<AssetManager>(),
            &app.state::<AppContext>(),
            cloud_api_base_url(&api_base_url),
            &access_token,
            &account_id,
            client_device_id.as_deref(),
            label.as_deref(),
        )?;

        record_snapshot(
            &db,
            &result.payload_summary.source_device_id,
            &result.payload_summary.overall_checksum,
            &result.backup.storage_key,
            "Cloud backup",
        );

        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn restore_remote_backup(
    app: AppHandle,
    api_base_url: String,
    access_token: String,
    account_id: String,
    backup_id: String,
    take_over: bool,
) -> Result<RemoteBackupRestoreResult, VaultimeError> {
    on_blocking_pool(move || {
        let db = app.state::<Arc<Database>>();
        let device_id = app.state::<AppContext>().device_id.clone();
        let engine = app.state::<TrackingEngine>();
        let result = engine.while_paused(&db, &device_id, || {
            backup::remote::restore_remote_backup(
                &db,
                &app.state::<AssetManager>(),
                &app.state::<AppContext>(),
                cloud_api_base_url(&api_base_url),
                &access_token,
                &account_id,
                &backup_id,
                take_over,
            )
        })?;

        record_snapshot(
            &db,
            &result.restored_summary.source_device_id,
            &result.restored_summary.overall_checksum,
            &result.backup.storage_key,
            "Cloud restore",
        );

        Ok(result)
    })
    .await
}
