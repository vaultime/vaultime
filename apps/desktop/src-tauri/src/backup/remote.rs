// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cloud backups. A cloud backup is an encrypted archive with the database
//! and the manifest. Artwork is uploaded apart from it, encrypted once per
//! image, and every backup that shows the same image refers to the same
//! upload, so artwork takes its space on the server only once.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use reqwest::StatusCode;
use reqwest::blocking::{Body, Client, Response};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use super::crypto::{self, ArtworkNamer};
use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::{
    BACKUP_ASSET_DIR, LocalBackupSummary, cleanup_staging_dir, export_local_backup, hash_file,
    import_local_backup,
};
use crate::constants::{ARCHIVE_FILE_MODE, BACKUP_KEY_BYTES, MAX_ARTWORK_IDS_PER_REQUEST};
use crate::db::connection::Database;
use crate::error::{Result, VaultimeError};
use crate::secure_storage;

const CLOUD_STAGING_DIR: &str = ".cloud-staging";
const ARCHIVE_FORMAT: &str = "zip";
/// Maps every artwork file of a backup to the blob that holds it.
const ARTWORK_INDEX_FILE: &str = "artwork.json";
/// Error code of the API when a backup refers to artwork it does not hold.
const MISSING_ARTWORK: &str = "missing_artwork";

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
    pub encryption: String,
    pub archive_checksum: String,
    pub archive_size_bytes: u64,
    /// Size of the artwork the backup restores. It is stored apart from the
    /// archive, once for all backups. Zero for backups made before that.
    #[serde(default)]
    pub artwork_bytes: u64,
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
    /// Artwork files that were new to the server.
    pub artwork_uploaded: usize,
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
    blob_ids: &'a [String],
}

#[derive(Debug, Serialize)]
struct MissingArtworkRequest<'a> {
    ids: &'a [String],
}

#[derive(Debug, Deserialize)]
struct MissingArtworkResponse {
    missing: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    #[serde(default)]
    code: Option<String>,
    message: String,
}

/// One artwork file of a backup and the blob it is stored as.
struct ArtworkFile {
    /// Path inside the backup folder, with forward slashes.
    path: String,
    blob_id: String,
    bytes: u64,
}

/// The API a signed in PC talks to.
struct Api<'a> {
    client: Client,
    base_url: String,
    access_token: &'a str,
}

#[expect(clippy::too_many_arguments)]
pub fn upload_remote_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    api_base_url: &str,
    access_token: &str,
    account_id: &str,
    client_device_id: Option<&str>,
    label: Option<&str>,
) -> Result<RemoteBackupUploadResult> {
    let backup_key = secure_storage::load_cloud_backup_key(account_id)?;
    upload_with_key(
        db,
        asset_manager,
        app_context,
        api_base_url,
        access_token,
        &backup_key,
        client_device_id,
        label,
    )
}

#[expect(clippy::too_many_arguments)]
fn upload_with_key(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    api_base_url: &str,
    access_token: &str,
    backup_key: &[u8; BACKUP_KEY_BYTES],
    client_device_id: Option<&str>,
    label: Option<&str>,
) -> Result<RemoteBackupUploadResult> {
    let staging_dir = create_staging_dir(&app_context.app_dir, "upload")?;

    let result = (|| {
        let api = Api::new(api_base_url, access_token)?;
        let export_root = staging_dir.join("export");
        fs::create_dir_all(&export_root).map_err(map_backup_io)?;

        let local_summary = export_local_backup(db, asset_manager, app_context, &export_root)?;
        let backup_dir = PathBuf::from(&local_summary.backup_path);

        let artwork = index_artwork(&backup_dir, &ArtworkNamer::new(backup_key)?)?;
        write_artwork_index(&backup_dir, &artwork)?;
        let mut artwork_uploaded =
            upload_missing_artwork(&api, &backup_dir, &artwork, backup_key, &staging_dir)?;

        let archive_path = staging_dir.join(format!("{}.zip", local_summary.backup_id));
        create_archive(&backup_dir, &archive_path, |path| {
            !path.starts_with(&format!("{BACKUP_ASSET_DIR}/"))
        })?;
        let encrypted_path = staging_dir.join(format!("{}.enc", local_summary.backup_id));
        crypto::encrypt_file(&archive_path, &encrypted_path, backup_key)?;

        let (archive_size_bytes, archive_checksum) = hash_file(&encrypted_path)?;
        let payload_summary = payload_summary_from_local(
            &local_summary,
            archive_checksum,
            archive_size_bytes,
            artwork.iter().map(|file| file.bytes).sum(),
        );
        let blob_ids = unique_blob_ids(&artwork);
        let request = UploadRequest {
            client_device_id,
            label,
            payload_summary: &payload_summary,
            blob_ids: &blob_ids,
            archive_path: &encrypted_path,
        };

        // Artwork no backup refers to may be cleaned up on the server right
        // after the check, so a refused backup uploads the missing files again.
        let backup = if let Some(backup) = create_and_upload_backup(&api, &request)? {
            backup
        } else {
            artwork_uploaded +=
                upload_missing_artwork(&api, &backup_dir, &artwork, backup_key, &staging_dir)?;
            create_and_upload_backup(&api, &request)?.ok_or_else(|| {
                VaultimeError::Cloud("the server keeps missing artwork of this backup".into())
            })?
        };

        Ok(RemoteBackupUploadResult {
            backup,
            payload_summary,
            artwork_uploaded,
        })
    })();

    cleanup_staging_dir(&staging_dir);
    result
}

pub fn restore_remote_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    api_base_url: &str,
    access_token: &str,
    account_id: &str,
    backup_id: &str,
) -> Result<RemoteBackupRestoreResult> {
    let backup_key = secure_storage::load_cloud_backup_key(account_id)?;
    restore_with_key(
        db,
        asset_manager,
        app_context,
        api_base_url,
        access_token,
        &backup_key,
        backup_id,
    )
}

fn restore_with_key(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    api_base_url: &str,
    access_token: &str,
    backup_key: &[u8; BACKUP_KEY_BYTES],
    backup_id: &str,
) -> Result<RemoteBackupRestoreResult> {
    let staging_dir = create_staging_dir(&app_context.app_dir, "restore")?;

    let result = (|| {
        let api = Api::new(api_base_url, access_token)?;
        let backup = send_json::<RemoteBackupRecord>(
            api.client
                .get(api.url(&format!("/v1/backups/{backup_id}")))
                .bearer_auth(api.access_token),
        )?;

        if backup.status != "complete" {
            return Err(VaultimeError::Cloud(
                "remote backup is not ready for restore".into(),
            ));
        }

        let archive_path = staging_dir.join(format!("{backup_id}.enc"));
        api.download(
            &format!("/v1/backups/{backup_id}/download?attachment=false"),
            &archive_path,
        )?;

        let (_, actual_checksum) = hash_file(&archive_path)?;
        if actual_checksum != backup.checksum {
            return Err(VaultimeError::Backup(
                "downloaded remote backup failed checksum verification".into(),
            ));
        }

        let decrypted_archive_path = staging_dir.join(format!("{backup_id}.zip"));
        crypto::decrypt_file(&archive_path, &decrypted_archive_path, backup_key)?;

        let extracted_dir = staging_dir.join("extracted");
        extract_archive(&decrypted_archive_path, &extracted_dir)?;
        restore_artwork(&api, &extracted_dir, backup_key, &staging_dir)?;

        // The manifest check in the import confirms every artwork file too.
        let local_summary = import_local_backup(db, asset_manager, app_context, &extracted_dir)?;

        Ok(RemoteBackupRestoreResult {
            backup,
            restored_summary: restore_summary_from_local(&local_summary),
        })
    })();

    cleanup_staging_dir(&staging_dir);
    result
}

impl<'a> Api<'a> {
    fn new(base_url: &str, access_token: &'a str) -> Result<Self> {
        let client = Client::builder().build().map_err(|error| {
            VaultimeError::Cloud(format!("failed to build HTTP client: {error}"))
        })?;
        Ok(Self {
            client,
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            access_token,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    /// Streams a response body into `destination`.
    fn download(&self, path: &str, destination: &Path) -> Result<()> {
        let mut response = ensure_success(
            self.client
                .get(self.url(path))
                .bearer_auth(self.access_token)
                .send()
                .map_err(map_cloud_http)?,
        )?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(map_backup_io)?;
        }
        let mut file = File::create(destination).map_err(map_backup_io)?;
        std::io::copy(&mut response, &mut file).map_err(map_backup_io)?;
        file.flush().map_err(map_backup_io)
    }
}

/// Names every artwork file of a backup folder by its content.
fn index_artwork(backup_dir: &Path, namer: &ArtworkNamer) -> Result<Vec<ArtworkFile>> {
    let asset_dir = backup_dir.join(BACKUP_ASSET_DIR);
    let mut artwork = Vec::new();
    if !asset_dir.exists() {
        return Ok(artwork);
    }
    for entry in WalkDir::new(&asset_dir).min_depth(1) {
        let entry = entry.map_err(|error| {
            VaultimeError::Backup(format!("failed to read the artwork of a backup: {error}"))
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        artwork.push(ArtworkFile {
            path: relative_path(backup_dir, entry.path())?,
            blob_id: namer.id(entry.path())?,
            bytes: entry
                .metadata()
                .map_err(|error| {
                    VaultimeError::Backup(format!("failed to read artwork size: {error}"))
                })?
                .len(),
        });
    }
    artwork.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(artwork)
}

fn write_artwork_index(backup_dir: &Path, artwork: &[ArtworkFile]) -> Result<()> {
    let index: BTreeMap<&str, &str> = artwork
        .iter()
        .map(|file| (file.path.as_str(), file.blob_id.as_str()))
        .collect();
    let json = serde_json::to_vec_pretty(&index).map_err(|error| {
        VaultimeError::Backup(format!("failed to write artwork index: {error}"))
    })?;
    fs::write(backup_dir.join(ARTWORK_INDEX_FILE), json).map_err(map_backup_io)
}

fn unique_blob_ids(artwork: &[ArtworkFile]) -> Vec<String> {
    let mut seen = HashSet::new();
    artwork
        .iter()
        .filter(|file| seen.insert(file.blob_id.as_str()))
        .map(|file| file.blob_id.clone())
        .collect()
}

/// Uploads the artwork the server does not hold yet. Returns how many files
/// were new.
fn upload_missing_artwork(
    api: &Api,
    backup_dir: &Path,
    artwork: &[ArtworkFile],
    key: &[u8; BACKUP_KEY_BYTES],
    staging_dir: &Path,
) -> Result<usize> {
    let ids = unique_blob_ids(artwork);
    let mut missing = Vec::new();
    for batch in ids.chunks(MAX_ARTWORK_IDS_PER_REQUEST) {
        let response = send_json::<MissingArtworkResponse>(
            api.client
                .post(api.url("/v1/blobs/missing"))
                .bearer_auth(api.access_token)
                .json(&MissingArtworkRequest { ids: batch }),
        )?;
        missing.extend(response.missing);
    }

    let by_id: HashMap<&str, &ArtworkFile> = artwork
        .iter()
        .map(|file| (file.blob_id.as_str(), file))
        .collect();
    let sealed = staging_dir.join("artwork.enc");
    for id in &missing {
        let file = by_id.get(id.as_str()).ok_or_else(|| {
            VaultimeError::Cloud(format!("the server asked for unknown artwork {id}"))
        })?;
        crypto::encrypt_file(&backup_dir.join(&file.path), &sealed, key)?;
        let body = File::open(&sealed).map_err(map_backup_io)?;
        ensure_success(
            api.client
                .put(api.url(&format!("/v1/blobs/{id}")))
                .bearer_auth(api.access_token)
                .header(CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(body))
                .send()
                .map_err(map_cloud_http)?,
        )?;
    }
    let _ = fs::remove_file(&sealed);
    Ok(missing.len())
}

/// Puts the artwork a backup lists into the extracted backup folder. Backups
/// made before artwork was stored apart carry it in the archive instead.
fn restore_artwork(
    api: &Api,
    extracted_dir: &Path,
    key: &[u8; BACKUP_KEY_BYTES],
    staging_dir: &Path,
) -> Result<()> {
    let index_path = extracted_dir.join(ARTWORK_INDEX_FILE);
    if !index_path.exists() {
        return Ok(());
    }
    let index: BTreeMap<String, String> = serde_json::from_slice(
        &fs::read(&index_path).map_err(map_backup_io)?,
    )
    .map_err(|error| VaultimeError::Backup(format!("the artwork index is damaged: {error}")))?;
    fs::remove_file(&index_path).map_err(map_backup_io)?;

    let sealed = staging_dir.join("artwork.enc");
    let mut restored: HashMap<&str, PathBuf> = HashMap::new();
    for (path, blob_id) in &index {
        let target = artwork_target(extracted_dir, path)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(map_backup_io)?;
        }
        if let Some(earlier) = restored.get(blob_id.as_str()) {
            fs::copy(earlier, &target).map_err(map_backup_io)?;
            continue;
        }
        if blob_id.len() != 64 || !blob_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(VaultimeError::Backup(format!(
                "the artwork index names an invalid id for {path}"
            )));
        }
        api.download(&format!("/v1/blobs/{blob_id}"), &sealed)?;
        crypto::decrypt_file(&sealed, &target, key)?;
        restored.insert(blob_id, target);
    }
    let _ = fs::remove_file(&sealed);
    Ok(())
}

/// Where an artwork file of the index goes. Only plain paths inside the
/// artwork folder are accepted.
fn artwork_target(extracted_dir: &Path, path: &str) -> Result<PathBuf> {
    let relative = Path::new(path);
    let plain = relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    if !plain || !relative.starts_with(BACKUP_ASSET_DIR) {
        return Err(VaultimeError::Backup(format!(
            "the artwork index names an unsafe path: {path}"
        )));
    }
    Ok(extracted_dir.join(relative))
}

struct UploadRequest<'a> {
    client_device_id: Option<&'a str>,
    label: Option<&'a str>,
    payload_summary: &'a RemoteBackupPayloadSummary,
    blob_ids: &'a [String],
    archive_path: &'a Path,
}

/// Creates the backup and uploads its archive. `None` when the server lacks
/// some of its artwork.
fn create_and_upload_backup(
    api: &Api,
    upload: &UploadRequest,
) -> Result<Option<RemoteBackupRecord>> {
    let metadata_json = serde_json::to_value(upload.payload_summary).map_err(|error| {
        VaultimeError::Cloud(format!("failed to serialize backup metadata: {error}"))
    })?;
    let request = CreateBackupRequest {
        label: upload
            .label
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        checksum: &upload.payload_summary.archive_checksum,
        backup_created_at: &upload.payload_summary.created_at,
        metadata_json,
        client_device_id: upload
            .client_device_id
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        blob_ids: upload.blob_ids,
    };

    let response = api
        .client
        .post(api.url("/v1/backups"))
        .bearer_auth(api.access_token)
        .json(&request)
        .send()
        .map_err(map_cloud_http)?;
    if response.status() == StatusCode::CONFLICT {
        let (code, message) = api_error(response);
        if code.as_deref() == Some(MISSING_ARTWORK) {
            return Ok(None);
        }
        return Err(VaultimeError::Cloud(message));
    }
    let created = ensure_success(response)?
        .json::<RemoteBackupRecord>()
        .map_err(|error| VaultimeError::Cloud(format!("failed to parse API response: {error}")))?;

    let archive_file = File::open(upload.archive_path).map_err(map_backup_io)?;
    send_json::<RemoteBackupRecord>(
        api.client
            .put(api.url(&format!("/v1/backups/{}/content", created.id)))
            .bearer_auth(api.access_token)
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(archive_file)),
    )
    .map(Some)
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
    Err(VaultimeError::Cloud(api_error(response).1))
}

/// Error code and message of a failed API response.
fn api_error(response: Response) -> (Option<String>, String) {
    let status = response.status();
    let body = response.text().unwrap_or_default();
    let (code, message) = match serde_json::from_str::<ApiErrorEnvelope>(&body) {
        Ok(payload) => (payload.error.code, payload.error.message),
        Err(_) => (None, body),
    };
    let message = message.trim();
    if message.is_empty() {
        return (
            code,
            format!("cloud API request failed with status {}", status.as_u16()),
        );
    }
    (code, message.to_string())
}

fn payload_summary_from_local(
    local_summary: &LocalBackupSummary,
    archive_checksum: String,
    archive_size_bytes: u64,
    artwork_bytes: u64,
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
        encryption: crypto::SCHEME.to_string(),
        archive_checksum,
        archive_size_bytes,
        artwork_bytes,
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

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    Ok(path
        .strip_prefix(root)
        .map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to normalize backup path {}: {error}",
                path.display()
            ))
        })?
        .to_string_lossy()
        .replace('\\', "/"))
}

/// Zips the files of `source_dir` whose relative path `include` accepts.
fn create_archive(
    source_dir: &Path,
    destination_path: &Path,
    include: impl Fn(&str) -> bool,
) -> Result<()> {
    let archive_file = File::create(destination_path).map_err(map_backup_io)?;
    let mut archive = zip::ZipWriter::new(archive_file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(ARCHIVE_FILE_MODE);

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

        let relative_path = relative_path(source_dir, entry.path())?;
        if !include(&relative_path) {
            continue;
        }

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

fn map_backup_io(error: std::io::Error) -> VaultimeError {
    VaultimeError::Backup(error.to_string())
}

fn map_zip_error(error: zip::result::ZipError) -> VaultimeError {
    VaultimeError::Backup(format!("archive error: {error}"))
}

fn map_cloud_http(error: reqwest::Error) -> VaultimeError {
    VaultimeError::Cloud(format!("HTTP request failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artwork_paths_stay_inside_the_artwork_folder() {
        let root = Path::new("extracted");
        assert!(artwork_target(root, "asset-cache/game/cover.jpg").is_ok());
        assert!(artwork_target(root, "asset-cache/../vaultime.db").is_err());
        assert!(artwork_target(root, "vaultime.db").is_err());
        assert!(artwork_target(root, "/etc/passwd").is_err());
        assert!(artwork_target(root, "asset-cache/game/../../x").is_err());
    }

    /// A PC with its own data folder, database and artwork.
    fn pc(name: &str) -> (PathBuf, AppContext, Database, AssetManager) {
        let root =
            std::env::temp_dir().join(format!("vaultime-e2e-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("asset-cache")).unwrap();
        let context = AppContext {
            app_dir: root.clone(),
            device_id: format!("{name}-device"),
            app_version: "0.1.0".into(),
        };
        let db = Database::open(&root.join("vaultime.db")).unwrap();
        let assets = AssetManager::new(root.join("asset-cache"));
        (root, context, db, assets)
    }

    fn artwork(root: &Path, path: &str) -> PathBuf {
        root.join("asset-cache").join(path)
    }

    fn storage(api: &str, token: &str) -> Value {
        Client::new()
            .get(format!("{api}/v1/storage"))
            .bearer_auth(token)
            .send()
            .unwrap()
            .json()
            .unwrap()
    }

    /// Runs against a real API that keeps two backups, allows back to back
    /// backups and cleans up unused artwork at once:
    /// `VAULTIME_MAX_COMPLETE_BACKUPS_PER_ACCOUNT=2`,
    /// `VAULTIME_MIN_BACKUP_INTERVAL_SECONDS=0` and
    /// `VAULTIME_STALE_PENDING_BACKUP_SECONDS=0`.
    #[test]
    #[ignore = "needs a running API, set VAULTIME_E2E_API and VAULTIME_E2E_TOKEN"]
    fn artwork_is_uploaded_once_and_restores() {
        let api = std::env::var("VAULTIME_E2E_API").unwrap();
        let token = std::env::var("VAULTIME_E2E_TOKEN").unwrap();
        let key = [3_u8; BACKUP_KEY_BYTES];
        let (root, context, db, assets) = pc("source");
        fs::create_dir_all(artwork(&root, "one")).unwrap();
        fs::create_dir_all(artwork(&root, "two")).unwrap();
        fs::write(artwork(&root, "one/cover.jpg"), b"cover-a").unwrap();
        fs::write(artwork(&root, "one/banner.jpg"), b"shared").unwrap();
        fs::write(artwork(&root, "two/banner.jpg"), b"shared").unwrap();
        let upload = || upload_with_key(&db, &assets, &context, &api, &token, &key, None, None);

        let first = upload().unwrap();
        assert_eq!(first.artwork_uploaded, 2, "the shared image goes up once");
        assert_eq!(first.payload_summary.artwork_bytes, 19);
        assert_eq!(upload().unwrap().artwork_uploaded, 0, "nothing new");

        fs::write(artwork(&root, "one/cover.jpg"), b"cover-b").unwrap();
        let third = upload().unwrap();
        assert_eq!(third.artwork_uploaded, 1);
        // Keeping two backups, the first is gone and the second still needs cover-a.
        let with_both = storage(&api, &token)["artwork_bytes"].as_i64().unwrap();
        assert_eq!(upload().unwrap().artwork_uploaded, 0);
        let cleaned = storage(&api, &token)["artwork_bytes"].as_i64().unwrap();
        assert!(
            cleaned < with_both,
            "cover-a was cleaned up: {with_both} -> {cleaned}"
        );

        let (other_root, other_context, other_db, other_assets) = pc("restore");
        restore_with_key(
            &other_db,
            &other_assets,
            &other_context,
            &api,
            &token,
            &key,
            &third.backup.id,
        )
        .unwrap();
        for (path, bytes) in [
            ("one/cover.jpg", &b"cover-b"[..]),
            ("one/banner.jpg", b"shared"),
            ("two/banner.jpg", b"shared"),
        ] {
            assert_eq!(
                fs::read(artwork(&other_root, path)).unwrap(),
                bytes,
                "{path}"
            );
        }

        let wrong_key = restore_with_key(
            &other_db,
            &other_assets,
            &other_context,
            &api,
            &token,
            &[4_u8; BACKUP_KEY_BYTES],
            &third.backup.id,
        );
        assert!(wrong_key.is_err());

        drop((db, other_db));
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(other_root).unwrap();
    }

    #[test]
    fn same_artwork_is_listed_once() {
        let file = |path: &str, id: &str| ArtworkFile {
            path: path.into(),
            blob_id: id.into(),
            bytes: 1,
        };
        let ids = unique_blob_ids(&[file("a", "1"), file("b", "2"), file("c", "1")]);
        assert_eq!(ids, vec!["1".to_string(), "2".to_string()]);
    }
}
