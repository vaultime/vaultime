// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Cloud backup pipeline — uploads local snapshot folders into Supabase Storage
//! and restores them back into the local database on demand.

use std::fs;
use std::path::{Path, PathBuf};

use log::{info, warn};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::{self, LocalBackupSummary};
use crate::db::connection::Database;
use crate::db::repo::{session_events, sessions, settings};
use crate::error::{Result, VaultimeError};

use super::auth::AuthManager;
use super::config;
use super::types::{
    CloudBackupRecord, CloudBackupRestorePreview, CloudBackupSummary, CloudBackupUploadResult,
};

const STORAGE_ROOT: &str = "snapshots";
const BACKUP_STAGING_DIR: &str = ".cloud-backup-staging";
const RESTORE_STAGING_PREFIX: &str = ".cloud-restore-";

#[derive(Debug, Deserialize)]
struct RemoteBackupManifest {
    backup_id: String,
    backup_version: u32,
    created_at: String,
    app_version: String,
    source_device_id: String,
    schema_migrations: Vec<String>,
    overall_checksum: String,
    games_count: i64,
    sessions_count: i64,
    assets_count: i64,
    files: Vec<RemoteBackupFileEntry>,
}

#[derive(Debug, Deserialize)]
struct RemoteBackupFileEntry {
    path: String,
}

pub async fn list_backups(auth: &AuthManager) -> Result<Vec<CloudBackupRecord>> {
    let access_token = auth
        .access_token()
        .ok_or_else(|| VaultimeError::Cloud("not signed in".into()))?;
    let url = format!(
        "{}/rest/v1/cloud_backups?select=id,device_id,created_at,checksum,storage_path,size_bytes,label&order=created_at.desc",
        config::supabase_url()
    );

    let response = reqwest::Client::new()
        .get(&url)
        .header("apikey", config::supabase_anon_key())
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|error| VaultimeError::Cloud(format!("failed to list cloud backups: {error}")))?;

    parse_json_response(response, "list cloud backups").await
}

pub async fn create_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    auth: &AuthManager,
    label: Option<&str>,
) -> Result<CloudBackupUploadResult> {
    let access_token = auth
        .access_token()
        .ok_or_else(|| VaultimeError::Cloud("not signed in".into()))?;
    let staging_root = app_context.app_dir.join(BACKUP_STAGING_DIR);
    fs::create_dir_all(&staging_root).map_err(|error| {
        VaultimeError::Cloud(format!(
            "failed to create cloud backup staging directory {}: {error}",
            staging_root.display()
        ))
    })?;

    let summary = backup::export_local_backup(db, asset_manager, app_context, &staging_root)?;
    let backup_dir = PathBuf::from(&summary.backup_path);
    let storage_prefix = build_storage_prefix(app_context, &summary.backup_id);
    let upload_result = upload_backup_directory(&access_token, &backup_dir, &storage_prefix).await;
    let _ = cleanup_path(&backup_dir);

    let (uploaded_files, size_bytes) = upload_result?;
    let record = insert_backup_metadata(
        &access_token,
        &summary,
        &app_context.device_id,
        &storage_prefix,
        size_bytes,
        label,
    )
    .await?;

    let _ = settings::set_setting(db, "cloud_last_backup_at", &record.created_at);
    let _ = settings::set_setting(db, "cloud_last_backup_id", &record.id);

    Ok(CloudBackupUploadResult {
        backup: record,
        summary: cloud_summary_from_local(&summary),
        uploaded_files,
    })
}

pub async fn get_restore_preview(
    db: &Database,
    auth: &AuthManager,
    backup_id: &str,
) -> Result<CloudBackupRestorePreview> {
    let access_token = auth
        .access_token()
        .ok_or_else(|| VaultimeError::Cloud("not signed in".into()))?;
    let backup = get_backup_record(&access_token, backup_id).await?;
    let manifest = download_manifest(&access_token, &backup).await?;
    let has_active_sessions = !sessions::get_active_sessions(db)?.is_empty();
    let unsynced_events = session_events::count_unsynced_events(db)?;
    let newer_local_sessions = sessions::count_sessions_started_after(db, &manifest.created_at)?;
    let requires_force = unsynced_events > 0 || newer_local_sessions > 0;

    Ok(CloudBackupRestorePreview {
        backup,
        summary: cloud_summary_from_manifest(&manifest),
        has_active_sessions,
        unsynced_events,
        newer_local_sessions,
        requires_force,
    })
}

pub async fn restore_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    auth: &AuthManager,
    backup_id: &str,
    force: bool,
) -> Result<(CloudBackupRecord, LocalBackupSummary)> {
    let access_token = auth
        .access_token()
        .ok_or_else(|| VaultimeError::Cloud("not signed in".into()))?;
    let preview = get_restore_preview(db, auth, backup_id).await?;

    if preview.has_active_sessions {
        return Err(VaultimeError::Cloud(
            "close all live sessions before restoring a cloud backup".into(),
        ));
    }

    if preview.requires_force && !force {
        return Err(VaultimeError::Cloud(format!(
            "cloud restore would overwrite newer local history ({} unsynced events, {} newer local sessions); retry with force enabled",
            preview.unsynced_events, preview.newer_local_sessions
        )));
    }

    let manifest = download_manifest(&access_token, &preview.backup).await?;
    let staging_dir = app_context
        .app_dir
        .join(format!("{RESTORE_STAGING_PREFIX}{backup_id}"));
    if staging_dir.exists() {
        cleanup_path(&staging_dir)?;
    }
    fs::create_dir_all(&staging_dir).map_err(|error| {
        VaultimeError::Cloud(format!(
            "failed to create cloud restore staging directory {}: {error}",
            staging_dir.display()
        ))
    })?;

    let download_result =
        download_backup_directory(&access_token, &preview.backup, &manifest, &staging_dir).await;
    if let Err(error) = download_result {
        let _ = cleanup_path(&staging_dir);
        return Err(error);
    }

    let import_result = backup::import_local_backup(db, asset_manager, app_context, &staging_dir);
    let _ = cleanup_path(&staging_dir);

    import_result.map(|summary| (preview.backup, summary))
}

fn build_storage_prefix(app_context: &AppContext, backup_id: &str) -> String {
    format!("{STORAGE_ROOT}/{}/{backup_id}", app_context.device_id)
}

async fn upload_backup_directory(
    access_token: &str,
    backup_dir: &Path,
    storage_prefix: &str,
) -> Result<(usize, i64)> {
    let mut uploaded_files = 0usize;
    let mut total_bytes = 0i64;

    for entry in WalkDir::new(backup_dir).min_depth(1) {
        let entry = entry.map_err(|error| {
            VaultimeError::Cloud(format!(
                "failed to walk backup directory {}: {error}",
                backup_dir.display()
            ))
        })?;

        if !entry.file_type().is_file() {
            continue;
        }

        let relative_path = entry
            .path()
            .strip_prefix(backup_dir)
            .map_err(|error| {
                VaultimeError::Cloud(format!(
                    "failed to normalize backup file path {}: {error}",
                    entry.path().display()
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let remote_path = format!("{storage_prefix}/{relative_path}");
        let bytes = fs::read(entry.path()).map_err(|error| {
            VaultimeError::Cloud(format!(
                "failed to read {} for upload: {error}",
                entry.path().display()
            ))
        })?;

        upload_storage_object(
            access_token,
            &remote_path,
            bytes,
            content_type_for_path(&relative_path),
        )
        .await?;

        uploaded_files += 1;
        total_bytes += entry.metadata().map_or(0, |metadata| metadata.len() as i64);
    }

    Ok((uploaded_files, total_bytes))
}

async fn insert_backup_metadata(
    access_token: &str,
    summary: &LocalBackupSummary,
    device_id: &str,
    storage_path: &str,
    size_bytes: i64,
    label: Option<&str>,
) -> Result<CloudBackupRecord> {
    let url = format!("{}/rest/v1/cloud_backups", config::supabase_url());
    let body = serde_json::json!({
        "id": summary.backup_id,
        "device_id": device_id,
        "checksum": summary.overall_checksum,
        "storage_path": storage_path,
        "size_bytes": size_bytes,
        "label": label.unwrap_or("Cloud snapshot"),
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("apikey", config::supabase_anon_key())
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .header(
            "Prefer",
            "return=representation,resolution=merge-duplicates",
        )
        .json(&body)
        .send()
        .await
        .map_err(|error| {
            VaultimeError::Cloud(format!("failed to record cloud backup metadata: {error}"))
        })?;

    let mut rows: Vec<CloudBackupRecord> =
        parse_json_response(response, "record cloud backup metadata").await?;
    rows.pop()
        .ok_or_else(|| VaultimeError::Cloud("cloud backup metadata response was empty".into()))
}

async fn get_backup_record(access_token: &str, backup_id: &str) -> Result<CloudBackupRecord> {
    let url = format!(
        "{}/rest/v1/cloud_backups?select=id,device_id,created_at,checksum,storage_path,size_bytes,label&id=eq.{}",
        config::supabase_url(),
        backup_id
    );
    let response = reqwest::Client::new()
        .get(&url)
        .header("apikey", config::supabase_anon_key())
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|error| VaultimeError::Cloud(format!("failed to fetch cloud backup: {error}")))?;
    let mut rows: Vec<CloudBackupRecord> =
        parse_json_response(response, "fetch cloud backup").await?;
    rows.pop()
        .ok_or_else(|| VaultimeError::Cloud(format!("cloud backup {backup_id} was not found")))
}

async fn download_manifest(
    access_token: &str,
    backup: &CloudBackupRecord,
) -> Result<RemoteBackupManifest> {
    let path = format!("{}/manifest.json", backup.storage_path);
    let bytes = download_storage_object(access_token, &path).await?;

    serde_json::from_slice::<RemoteBackupManifest>(&bytes).map_err(|error| {
        VaultimeError::Cloud(format!(
            "failed to parse cloud backup manifest for {}: {error}",
            backup.id
        ))
    })
}

async fn download_backup_directory(
    access_token: &str,
    backup: &CloudBackupRecord,
    manifest: &RemoteBackupManifest,
    staging_dir: &Path,
) -> Result<()> {
    for file in &manifest.files {
        let local_path = resolve_restore_path(staging_dir, &file.path)?;
        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                VaultimeError::Cloud(format!(
                    "failed to create restore directory {}: {error}",
                    parent.display()
                ))
            })?;
        }

        let remote_path = format!("{}/{path}", backup.storage_path, path = file.path);
        let bytes = download_storage_object(access_token, &remote_path).await?;
        fs::write(&local_path, bytes).map_err(|error| {
            VaultimeError::Cloud(format!(
                "failed to write restored backup file {}: {error}",
                local_path.display()
            ))
        })?;
    }

    Ok(())
}

async fn upload_storage_object(
    access_token: &str,
    remote_path: &str,
    bytes: Vec<u8>,
    content_type: &str,
) -> Result<()> {
    let url = format!(
        "{}/storage/v1/object/{}/{}",
        config::supabase_url(),
        config::supabase_backup_bucket(),
        remote_path
    );
    let response = reqwest::Client::new()
        .post(&url)
        .header("apikey", config::supabase_anon_key())
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", content_type)
        .header("x-upsert", "true")
        .body(bytes)
        .send()
        .await
        .map_err(|error| {
            VaultimeError::Cloud(format!("failed to upload cloud backup object: {error}"))
        })?;

    ensure_success(response, "upload cloud backup object")
        .await
        .map(|_| ())
}

async fn download_storage_object(access_token: &str, remote_path: &str) -> Result<Vec<u8>> {
    let url = format!(
        "{}/storage/v1/object/authenticated/{}/{}",
        config::supabase_url(),
        config::supabase_backup_bucket(),
        remote_path
    );
    let response = reqwest::Client::new()
        .get(&url)
        .header("apikey", config::supabase_anon_key())
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|error| {
            VaultimeError::Cloud(format!("failed to download cloud backup object: {error}"))
        })?;

    let response = ensure_success(response, "download cloud backup object").await?;
    response
        .bytes()
        .await
        .map(|body| body.to_vec())
        .map_err(|error| {
            VaultimeError::Cloud(format!("failed to read cloud backup object body: {error}"))
        })
}

async fn ensure_success(response: reqwest::Response, action: &str) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    warn!("{action} failed: {status} {body}");
    Err(VaultimeError::Cloud(format!(
        "{action} failed: HTTP {status}"
    )))
}

async fn parse_json_response<T>(response: reqwest::Response, action: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let response = ensure_success(response, action).await?;
    let body = response.text().await.map_err(|error| {
        VaultimeError::Cloud(format!("failed to read {action} response: {error}"))
    })?;

    serde_json::from_str(&body).map_err(|error| {
        VaultimeError::Cloud(format!("failed to parse {action} response: {error}"))
    })
}

fn cloud_summary_from_local(summary: &LocalBackupSummary) -> CloudBackupSummary {
    CloudBackupSummary {
        backup_id: summary.backup_id.clone(),
        backup_version: summary.backup_version,
        created_at: summary.created_at.clone(),
        app_version: summary.app_version.clone(),
        source_device_id: summary.source_device_id.clone(),
        schema_migrations: summary.schema_migrations.clone(),
        overall_checksum: summary.overall_checksum.clone(),
        games_count: summary.games_count,
        sessions_count: summary.sessions_count,
        assets_count: summary.assets_count,
        asset_file_count: summary.asset_file_count,
    }
}

fn cloud_summary_from_manifest(manifest: &RemoteBackupManifest) -> CloudBackupSummary {
    CloudBackupSummary {
        backup_id: manifest.backup_id.clone(),
        backup_version: manifest.backup_version,
        created_at: manifest.created_at.clone(),
        app_version: manifest.app_version.clone(),
        source_device_id: manifest.source_device_id.clone(),
        schema_migrations: manifest.schema_migrations.clone(),
        overall_checksum: manifest.overall_checksum.clone(),
        games_count: manifest.games_count,
        sessions_count: manifest.sessions_count,
        assets_count: manifest.assets_count,
        asset_file_count: manifest
            .files
            .iter()
            .filter(|file| file.path.starts_with("asset-cache/"))
            .count(),
    }
}

fn resolve_restore_path(root: &Path, relative_path: &str) -> Result<PathBuf> {
    let path = Path::new(relative_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(VaultimeError::Cloud(format!(
            "cloud backup manifest contains an unsafe path: {relative_path}"
        )));
    }

    Ok(root.join(path))
}

fn content_type_for_path(path: &str) -> &'static str {
    if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".png") {
        "image/png"
    } else {
        "application/octet-stream"
    }
}

fn cleanup_path(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_dir_all(path).map_err(|error| {
        VaultimeError::Cloud(format!("failed to remove {}: {error}", path.display()))
    })?;
    info!("removed cloud staging path {}", path.display());
    Ok(())
}
