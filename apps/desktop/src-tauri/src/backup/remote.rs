// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use reqwest::blocking::{Body, Client, Response};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::{LocalBackupSummary, export_local_backup, import_local_backup};
use crate::db::connection::Database;
use crate::error::{Result, VaultimeError};

const CLOUD_STAGING_DIR: &str = ".cloud-staging";
const ARCHIVE_FORMAT: &str = "zip";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteBackupPayloadSummary {
    pub local_backup_id: String,
    pub backup_version: u32,
    pub created_at: String,
    pub app_version: String,
    pub source_device_id: String,
    pub overall_checksum: String,
    pub games_count: i64,
    pub sessions_count: i64,
    pub assets_count: i64,
    pub asset_file_count: usize,
    pub archive_format: String,
    pub archive_checksum: String,
    pub archive_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteBackupRestoreSummary {
    pub created_at: String,
    pub source_device_id: String,
    pub overall_checksum: String,
    pub games_count: i64,
    pub sessions_count: i64,
    pub assets_count: i64,
    pub asset_file_count: usize,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteBackupRecord {
    pub id: String,
    pub label: Option<String>,
    pub storage_key: String,
    pub checksum: String,
    pub size_bytes: i64,
    pub backup_created_at: String,
    pub uploaded_at: String,
    pub status: String,
    pub client_device_id: Option<String>,
    pub metadata_json: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteBackupUploadResult {
    pub backup: RemoteBackupRecord,
    pub payload_summary: RemoteBackupPayloadSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteBackupRestoreResult {
    pub backup: RemoteBackupRecord,
    pub restored_summary: RemoteBackupRestoreSummary,
}

#[derive(Debug, Serialize)]
struct CreateBackupRequest<'a> {
    label: Option<&'a str>,
    checksum: &'a str,
    backup_created_at: &'a str,
    metadata_json: Value,
    client_device_id: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    message: String,
}

pub fn upload_remote_backup(
    db: &Database,
    app_context: &AppContext,
    api_base_url: &str,
    access_token: &str,
    client_device_id: Option<&str>,
    label: Option<&str>,
) -> Result<RemoteBackupUploadResult> {
    let staging_dir = create_staging_dir(&app_context.app_dir, "upload")?;

    let result = (|| {
        let export_root = staging_dir.join("export");
        fs::create_dir_all(&export_root).map_err(map_backup_io)?;

        let asset_manager = AssetManager::new(app_context.asset_cache_dir.clone());
        let local_summary = export_local_backup(db, &asset_manager, app_context, &export_root)?;
        let archive_path = staging_dir.join(format!("{}.zip", local_summary.backup_id));
        create_archive(Path::new(&local_summary.backup_path), &archive_path)?;

        let (archive_size_bytes, archive_checksum) = hash_file(&archive_path)?;
        let payload_summary =
            payload_summary_from_local(&local_summary, archive_checksum, archive_size_bytes);
        let backup = create_and_upload_backup(
            api_base_url,
            access_token,
            client_device_id,
            label,
            &payload_summary,
            &archive_path,
        )?;

        Ok(RemoteBackupUploadResult {
            backup,
            payload_summary,
        })
    })();

    let _ = cleanup_staging_dir(&staging_dir);
    result
}

pub fn restore_remote_backup(
    db: &Database,
    app_context: &AppContext,
    api_base_url: &str,
    access_token: &str,
    backup_id: &str,
) -> Result<RemoteBackupRestoreResult> {
    let staging_dir = create_staging_dir(&app_context.app_dir, "restore")?;

    let result = (|| {
        let client = build_client()?;
        let api_base_url = normalized_api_base(api_base_url);
        let backup = send_json::<RemoteBackupRecord>(
            client
                .get(format!("{api_base_url}/v1/backups/{backup_id}"))
                .bearer_auth(access_token),
        )?;

        if backup.status != "complete" {
            return Err(VaultimeError::Cloud(
                "remote backup is not ready for restore".into(),
            ));
        }

        let archive_path = staging_dir.join(format!("{backup_id}.zip"));
        download_backup_archive(
            &client,
            &api_base_url,
            access_token,
            backup_id,
            &archive_path,
        )?;

        let (_, actual_checksum) = hash_file(&archive_path)?;
        if actual_checksum != backup.checksum {
            return Err(VaultimeError::Backup(
                "downloaded remote backup failed checksum verification".into(),
            ));
        }

        let extracted_dir = staging_dir.join("extracted");
        extract_archive(&archive_path, &extracted_dir)?;

        let asset_manager = AssetManager::new(app_context.asset_cache_dir.clone());
        let local_summary = import_local_backup(db, &asset_manager, app_context, &extracted_dir)?;

        Ok(RemoteBackupRestoreResult {
            backup,
            restored_summary: restore_summary_from_local(&local_summary),
        })
    })();

    let _ = cleanup_staging_dir(&staging_dir);
    result
}

fn create_and_upload_backup(
    api_base_url: &str,
    access_token: &str,
    client_device_id: Option<&str>,
    label: Option<&str>,
    payload_summary: &RemoteBackupPayloadSummary,
    archive_path: &Path,
) -> Result<RemoteBackupRecord> {
    let client = build_client()?;
    let api_base_url = normalized_api_base(api_base_url);
    let metadata_json = serde_json::to_value(payload_summary).map_err(|error| {
        VaultimeError::Cloud(format!("failed to serialize backup metadata: {error}"))
    })?;
    let request = CreateBackupRequest {
        label: label.map(str::trim).filter(|value| !value.is_empty()),
        checksum: &payload_summary.archive_checksum,
        backup_created_at: &payload_summary.created_at,
        metadata_json,
        client_device_id: client_device_id
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    };

    let created = send_json::<RemoteBackupRecord>(
        client
            .post(format!("{api_base_url}/v1/backups"))
            .bearer_auth(access_token)
            .json(&request),
    )?;

    let archive_file = File::open(archive_path).map_err(map_backup_io)?;
    send_json::<RemoteBackupRecord>(
        client
            .put(format!("{api_base_url}/v1/backups/{}/content", created.id))
            .bearer_auth(access_token)
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(Body::new(archive_file)),
    )
}

fn download_backup_archive(
    client: &Client,
    api_base_url: &str,
    access_token: &str,
    backup_id: &str,
    destination_path: &Path,
) -> Result<()> {
    let mut response = ensure_success(
        client
            .get(format!(
                "{api_base_url}/v1/backups/{backup_id}/download?attachment=false"
            ))
            .bearer_auth(access_token)
            .send()
            .map_err(map_cloud_http)?,
    )?;

    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent).map_err(map_backup_io)?;
    }

    let mut file = File::create(destination_path).map_err(map_backup_io)?;
    std::io::copy(&mut response, &mut file).map_err(map_backup_io)?;
    file.flush().map_err(map_backup_io)?;
    Ok(())
}

fn build_client() -> Result<Client> {
    Client::builder()
        .build()
        .map_err(|error| VaultimeError::Cloud(format!("failed to build HTTP client: {error}")))
}

fn send_json<T>(request: reqwest::blocking::RequestBuilder) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let response = request.send().map_err(map_cloud_http)?;
    let response = ensure_success(response)?;

    response
        .json::<T>()
        .map_err(|error| VaultimeError::Cloud(format!("failed to parse API response: {error}")))
}

fn ensure_success(response: Response) -> Result<Response> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status();
    let body = response.text().unwrap_or_default();
    let message = serde_json::from_str::<ApiErrorEnvelope>(&body)
        .map(|payload| payload.error.message)
        .unwrap_or_else(|_| body.trim().to_string())
        .trim()
        .to_string();

    if message.is_empty() {
        return Err(VaultimeError::Cloud(format!(
            "cloud API request failed with status {}",
            status.as_u16()
        )));
    }

    Err(VaultimeError::Cloud(message))
}

fn payload_summary_from_local(
    local_summary: &LocalBackupSummary,
    archive_checksum: String,
    archive_size_bytes: u64,
) -> RemoteBackupPayloadSummary {
    RemoteBackupPayloadSummary {
        local_backup_id: local_summary.backup_id.clone(),
        backup_version: local_summary.backup_version,
        created_at: local_summary.created_at.clone(),
        app_version: local_summary.app_version.clone(),
        source_device_id: local_summary.source_device_id.clone(),
        overall_checksum: local_summary.overall_checksum.clone(),
        games_count: local_summary.games_count,
        sessions_count: local_summary.sessions_count,
        assets_count: local_summary.assets_count,
        asset_file_count: local_summary.asset_file_count,
        archive_format: ARCHIVE_FORMAT.to_string(),
        archive_checksum,
        archive_size_bytes,
    }
}

fn restore_summary_from_local(local_summary: &LocalBackupSummary) -> RemoteBackupRestoreSummary {
    RemoteBackupRestoreSummary {
        created_at: local_summary.created_at.clone(),
        source_device_id: local_summary.source_device_id.clone(),
        overall_checksum: local_summary.overall_checksum.clone(),
        games_count: local_summary.games_count,
        sessions_count: local_summary.sessions_count,
        assets_count: local_summary.assets_count,
        asset_file_count: local_summary.asset_file_count,
        restart_required: local_summary.restart_required,
    }
}

fn create_staging_dir(app_dir: &Path, operation: &str) -> Result<PathBuf> {
    let staging_dir = app_dir
        .join(CLOUD_STAGING_DIR)
        .join(format!("{operation}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging_dir).map_err(map_backup_io)?;
    Ok(staging_dir)
}

fn cleanup_staging_dir(staging_dir: &Path) -> Result<()> {
    if !staging_dir.exists() {
        return Ok(());
    }

    fs::remove_dir_all(staging_dir).map_err(|error| {
        VaultimeError::Backup(format!(
            "failed to clean staging directory {}: {error}",
            staging_dir.display()
        ))
    })
}

fn create_archive(source_dir: &Path, destination_path: &Path) -> Result<()> {
    let archive_file = File::create(destination_path).map_err(map_backup_io)?;
    let mut archive = zip::ZipWriter::new(archive_file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for entry in WalkDir::new(source_dir).min_depth(1) {
        let entry = entry.map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to walk backup directory {}: {error}",
                source_dir.display()
            ))
        })?;

        if !entry.file_type().is_file() {
            continue;
        }

        let relative_path = entry
            .path()
            .strip_prefix(source_dir)
            .map_err(|error| {
                VaultimeError::Backup(format!(
                    "failed to normalize backup archive path {}: {error}",
                    entry.path().display()
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");

        archive
            .start_file(relative_path, options)
            .map_err(map_zip_error)?;

        let mut file = File::open(entry.path()).map_err(map_backup_io)?;
        std::io::copy(&mut file, &mut archive).map_err(map_backup_io)?;
    }

    archive.finish().map_err(map_zip_error)?;
    Ok(())
}

fn extract_archive(archive_path: &Path, destination_dir: &Path) -> Result<()> {
    fs::create_dir_all(destination_dir).map_err(map_backup_io)?;
    let archive_file = File::open(archive_path).map_err(map_backup_io)?;
    let mut archive = zip::ZipArchive::new(archive_file).map_err(map_zip_error)?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(map_zip_error)?;
        let enclosed_name = entry.enclosed_name().ok_or_else(|| {
            VaultimeError::Backup(format!(
                "backup archive entry contains an unsafe path: {}",
                entry.name()
            ))
        })?;
        let target_path = destination_dir.join(enclosed_name);

        if entry.is_dir() {
            fs::create_dir_all(&target_path).map_err(map_backup_io)?;
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(map_backup_io)?;
        }

        let mut file = File::create(&target_path).map_err(map_backup_io)?;
        std::io::copy(&mut entry, &mut file).map_err(map_backup_io)?;
        file.flush().map_err(map_backup_io)?;
    }

    Ok(())
}

fn hash_file(path: &Path) -> Result<(u64, String)> {
    let mut file = File::open(path).map_err(map_backup_io)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).map_err(map_backup_io)?;
        if read == 0 {
            break;
        }

        bytes += u64::try_from(read)
            .map_err(|_| VaultimeError::Backup("backup file length overflow".into()))?;
        hasher.update(&buffer[..read]);
    }

    Ok((bytes, format!("{:x}", hasher.finalize())))
}

fn normalized_api_base(api_base_url: &str) -> String {
    api_base_url.trim().trim_end_matches('/').to_string()
}

fn map_backup_io(error: std::io::Error) -> VaultimeError {
    VaultimeError::Backup(error.to_string())
}

fn map_zip_error(error: zip::result::ZipError) -> VaultimeError {
    VaultimeError::Backup(format!("archive error: {error}"))
}

fn map_cloud_http(error: reqwest::Error) -> VaultimeError {
    VaultimeError::Cloud(format!("HTTP request failed: {error}"))
}
