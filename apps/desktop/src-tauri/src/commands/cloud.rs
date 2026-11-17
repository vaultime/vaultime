// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cloud backup: the sign-in in the OS credential store and backups on the server.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use super::backups::record_snapshot;
use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::cloud_merge::{AutoMerge, CloudPc};
use crate::backup::merge::{GameChoice, MergePreview, MergeSummary};
use crate::backup::remote::{RemoteBackupRestoreResult, RemoteBackupUploadResult};
use crate::backup::server::{Reply, ServerInfo};
use crate::backup::{self};
use crate::constants::{BACKUP_KEY_BYTES, LIBRARY_CHANGED_EVENT};
use crate::db::connection::Database;
use crate::db::repo::devices;
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

/// This PC's name sealed with the account's backup key, for its registration
/// with the server, which cannot read it. None without a key or a name.
#[tauri::command]
pub async fn seal_pc_name(
    app: AppHandle,
    account_id: String,
) -> Result<Option<String>, VaultimeError> {
    on_blocking_pool(move || {
        let Some(key) = secure_storage::find_cloud_backup_key(&account_id)? else {
            return Ok(None);
        };
        // The PC the ledger names, as for backups, which a takeover changes
        // before the app restarts.
        let this_pc = crate::integrity::ledger::this_device()
            .unwrap_or_else(|_| app.state::<AppContext>().device_id.clone());
        let name = devices::get_device(&app.state::<Arc<Database>>(), &this_pc)?.name;
        name.map(|name| backup::crypto::seal_text(&name, &key))
            .transpose()
    })
    .await
}

/// The PC names `seal_pc_name` sealed, cleaned like a rename. Each is None
/// where none came or the account's backup key on this PC does not open it.
#[tauri::command]
pub async fn open_pc_names(
    account_id: String,
    sealed: Vec<Option<String>>,
) -> Result<Vec<Option<String>>, VaultimeError> {
    on_blocking_pool(move || {
        let key = secure_storage::find_cloud_backup_key(&account_id)?;
        Ok(open_names(sealed, key.as_ref()))
    })
    .await
}

fn open_names(
    sealed: Vec<Option<String>>,
    key: Option<&[u8; BACKUP_KEY_BYTES]>,
) -> Vec<Option<String>> {
    sealed
        .into_iter()
        .map(|text| {
            let name = backup::crypto::open_text(&text?, key?)?;
            Some(devices::clean_name(&name)).filter(|name| !name.is_empty())
        })
        .collect()
}

/// Checks that a Vaultime server answers at the address the player typed.
#[tauri::command]
pub async fn check_cloud_server(address: String) -> Result<ServerInfo, VaultimeError> {
    on_blocking_pool(move || backup::server::check(&address)).await
}

/// Sends one JSON request of the page to the player's cloud server.
#[tauri::command]
pub async fn cloud_request(
    server: String,
    method: String,
    path: String,
    body: Option<serde_json::Value>,
    access_token: Option<String>,
) -> Result<Reply, VaultimeError> {
    on_blocking_pool(move || {
        backup::server::request(
            &server,
            &method,
            &path,
            body.as_ref(),
            access_token.as_deref(),
        )
    })
    .await
}

/// The newest cloud backup of every other PC of the account.
#[tauri::command]
pub async fn list_cloud_pcs(
    app: AppHandle,
    api_base_url: String,
    access_token: String,
    account_id: String,
) -> Result<Vec<CloudPc>, VaultimeError> {
    on_blocking_pool(move || {
        backup::cloud_merge::list_cloud_pcs(
            &app.state::<Arc<Database>>(),
            &api_base_url,
            &access_token,
            &account_id,
        )
    })
    .await
}

/// What merging another PC's cloud backup would bring in.
#[tauri::command]
pub async fn preview_cloud_merge(
    app: AppHandle,
    api_base_url: String,
    access_token: String,
    account_id: String,
    backup_id: String,
    trust_new_keys: bool,
) -> Result<MergePreview, VaultimeError> {
    on_blocking_pool(move || {
        backup::cloud_merge::preview_cloud_merge(
            &app.state::<Arc<Database>>(),
            &app.state::<AppContext>(),
            &api_base_url,
            &access_token,
            &account_id,
            &backup_id,
            trust_new_keys,
        )
    })
    .await
}

/// Merges another PC's cloud backup with the player's choice for each game.
#[tauri::command]
pub async fn merge_cloud_backup(
    app: AppHandle,
    api_base_url: String,
    access_token: String,
    account_id: String,
    backup_id: String,
    choices: Vec<GameChoice>,
    trust_new_keys: bool,
) -> Result<MergeSummary, VaultimeError> {
    on_blocking_pool(move || {
        backup::cloud_merge::merge_cloud_backup(
            &app.state::<Arc<Database>>(),
            &app.state::<AssetManager>(),
            &app.state::<AppContext>(),
            &api_base_url,
            &access_token,
            &account_id,
            &backup_id,
            &choices,
            trust_new_keys,
        )
    })
    .await
}

/// Lets go of a cloud backup downloaded for a merge that did not happen. It
/// waits for a merge that holds the downloaded copies.
#[tauri::command]
pub async fn discard_cloud_merge(app: AppHandle, backup_id: String) -> Result<(), VaultimeError> {
    on_blocking_pool(move || {
        backup::cloud_merge::discard_cloud_merge(&app.state::<AppContext>(), &backup_id);
        Ok(())
    })
    .await
}

/// Merges the newer backups of the PCs merged here before, as far as they
/// need no choice of the player.
#[tauri::command]
pub async fn merge_due_cloud_backups(
    app: AppHandle,
    api_base_url: String,
    access_token: String,
    account_id: String,
) -> Result<Vec<AutoMerge>, VaultimeError> {
    on_blocking_pool(move || {
        let done = backup::cloud_merge::merge_due_cloud_backups(
            &app.state::<Arc<Database>>(),
            &app.state::<AssetManager>(),
            &app.state::<AppContext>(),
            &api_base_url,
            &access_token,
            &account_id,
        )?;
        if done
            .iter()
            .any(|merge| merge.sessions_added + merge.sessions_grown > 0)
        {
            let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
        }
        Ok(done)
    })
    .await
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
            &api_base_url,
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
                &api_base_url,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::crypto::seal_text;

    #[test]
    fn sealed_pc_names_open_with_the_backup_key_only_and_come_cleaned() {
        let key = [7_u8; BACKUP_KEY_BYTES];
        let sealed = vec![
            Some(seal_text("Laptop", &key).unwrap()),
            Some(seal_text(" Deck\u{7}", &key).unwrap()),
            Some(seal_text("\u{1b}", &key).unwrap()),
            Some(seal_text("Laptop", &[8_u8; BACKUP_KEY_BYTES]).unwrap()),
            Some("not sealed".into()),
            None,
        ];
        assert_eq!(
            open_names(sealed.clone(), Some(&key)),
            [
                Some("Laptop".into()),
                Some("Deck".into()),
                None,
                None,
                None,
                None
            ]
        );
        assert_eq!(open_names(sealed, None), vec![None; 6]);
    }
}
