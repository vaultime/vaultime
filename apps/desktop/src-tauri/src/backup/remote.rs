// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use reqwest::blocking::{Body, Client, Response};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::{
    LocalBackupSummary, cleanup_staging_dir, export_local_backup, hash_file, import_local_backup,
};
use crate::constants::{
    ARCHIVE_FILE_MODE, BACKUP_KEY_BYTES, ENCRYPTION_CHUNK_BYTES, NONCE_BYTES, NONCE_PREFIX_BYTES,
};
use crate::db::connection::Database;
use crate::error::{Result, VaultimeError};
use crate::secure_storage;

const CLOUD_STAGING_DIR: &str = ".cloud-staging";
const ARCHIVE_FORMAT: &str = "zip";
const ENCRYPTION_SCHEME: &str = "chacha20poly1305-chunked-v1";
const ENCRYPTED_MAGIC: &[u8; 8] = b"VTENC01\n";
const ENCRYPTED_AAD: &[u8] = b"vaultime-cloud-backup";

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
    let staging_dir = create_staging_dir(&app_context.app_dir, "upload")?;

    let result = (|| {
        let export_root = staging_dir.join("export");
        fs::create_dir_all(&export_root).map_err(map_backup_io)?;

        let local_summary = export_local_backup(db, asset_manager, app_context, &export_root)?;
        let archive_path = staging_dir.join(format!("{}.zip", local_summary.backup_id));
        create_archive(Path::new(&local_summary.backup_path), &archive_path)?;
        let encrypted_path = staging_dir.join(format!("{}.enc", local_summary.backup_id));
        let backup_key = secure_storage::load_cloud_backup_key(account_id)?;
        encrypt_archive(&archive_path, &encrypted_path, &backup_key)?;

        let (archive_size_bytes, archive_checksum) = hash_file(&encrypted_path)?;
        let payload_summary =
            payload_summary_from_local(&local_summary, archive_checksum, archive_size_bytes);
        let backup = create_and_upload_backup(
            api_base_url,
            access_token,
            client_device_id,
            label,
            &payload_summary,
            &encrypted_path,
        )?;

        Ok(RemoteBackupUploadResult {
            backup,
            payload_summary,
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

        let archive_path = staging_dir.join(format!("{backup_id}.enc"));
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

        let decrypted_archive_path = staging_dir.join(format!("{backup_id}.zip"));
        let backup_key = secure_storage::load_cloud_backup_key(account_id)?;
        decrypt_archive(&archive_path, &decrypted_archive_path, &backup_key)?;

        let extracted_dir = staging_dir.join("extracted");
        extract_archive(&decrypted_archive_path, &extracted_dir)?;

        let local_summary = import_local_backup(db, asset_manager, app_context, &extracted_dir)?;

        Ok(RemoteBackupRestoreResult {
            backup,
            restored_summary: restore_summary_from_local(&local_summary),
        })
    })();

    cleanup_staging_dir(&staging_dir);
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
        .map_or_else(|_| body.trim().to_string(), |payload| payload.error.message)
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
        encryption: ENCRYPTION_SCHEME.to_string(),
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

fn create_archive(source_dir: &Path, destination_path: &Path) -> Result<()> {
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

fn encrypt_archive(
    input_path: &Path,
    output_path: &Path,
    key: &[u8; BACKUP_KEY_BYTES],
) -> Result<()> {
    let cipher = ChaCha20Poly1305::new(key.into());
    let mut input = File::open(input_path).map_err(map_backup_io)?;
    let mut output = File::create(output_path).map_err(map_backup_io)?;
    let nonce_seed = uuid::Uuid::new_v4();
    let nonce_prefix = &nonce_seed.as_bytes()[..NONCE_PREFIX_BYTES];

    output.write_all(ENCRYPTED_MAGIC).map_err(map_backup_io)?;
    output.write_all(nonce_prefix).map_err(map_backup_io)?;

    let mut current = vec![0_u8; ENCRYPTION_CHUNK_BYTES];
    let mut next = vec![0_u8; ENCRYPTION_CHUNK_BYTES];
    let mut current_len = input.read(&mut current).map_err(map_backup_io)?;
    let mut chunk_index = 0_u64;

    if current_len == 0 {
        let ciphertext = encrypt_chunk(&cipher, nonce_prefix, chunk_index, &[])?;
        write_encrypted_chunk(&mut output, true, 0, &ciphertext)?;
        output.flush().map_err(map_backup_io)?;
        return Ok(());
    }

    loop {
        let next_len = input.read(&mut next).map_err(map_backup_io)?;
        let is_last = next_len == 0;
        let ciphertext =
            encrypt_chunk(&cipher, nonce_prefix, chunk_index, &current[..current_len])?;
        write_encrypted_chunk(
            &mut output,
            is_last,
            u32::try_from(current_len)
                .map_err(|_| VaultimeError::Backup("backup chunk length overflow".into()))?,
            &ciphertext,
        )?;

        if is_last {
            break;
        }

        std::mem::swap(&mut current, &mut next);
        current_len = next_len;
        chunk_index += 1;
    }

    output.flush().map_err(map_backup_io)?;
    Ok(())
}

fn decrypt_archive(
    input_path: &Path,
    output_path: &Path,
    key: &[u8; BACKUP_KEY_BYTES],
) -> Result<()> {
    let cipher = ChaCha20Poly1305::new(key.into());
    let mut input = File::open(input_path).map_err(map_backup_io)?;
    let mut output = File::create(output_path).map_err(map_backup_io)?;
    let mut magic = [0_u8; ENCRYPTED_MAGIC.len()];
    input.read_exact(&mut magic).map_err(map_backup_io)?;
    if &magic != ENCRYPTED_MAGIC {
        return Err(VaultimeError::Backup(
            "remote backup has an unexpected encryption header".into(),
        ));
    }

    let mut nonce_prefix = [0_u8; NONCE_PREFIX_BYTES];
    input.read_exact(&mut nonce_prefix).map_err(map_backup_io)?;
    let mut chunk_index = 0_u64;

    loop {
        let mut flags = [0_u8; 1];
        input.read_exact(&mut flags).map_err(map_backup_io)?;
        let is_last = (flags[0] & 0x1) == 0x1;

        let plaintext_len = read_u32(&mut input)?;
        let ciphertext_len = read_u32(&mut input)?;
        let mut ciphertext = vec![0_u8; ciphertext_len as usize];
        input.read_exact(&mut ciphertext).map_err(map_backup_io)?;

        let plaintext = decrypt_chunk(&cipher, &nonce_prefix, chunk_index, &ciphertext)?;
        if plaintext.len()
            != usize::try_from(plaintext_len)
                .map_err(|_| VaultimeError::Backup("invalid plaintext length".into()))?
        {
            return Err(VaultimeError::Backup(
                "decrypted backup chunk length mismatch".into(),
            ));
        }

        output.write_all(&plaintext).map_err(map_backup_io)?;

        if is_last {
            break;
        }

        chunk_index += 1;
    }

    output.flush().map_err(map_backup_io)?;
    Ok(())
}

fn encrypt_chunk(
    cipher: &ChaCha20Poly1305,
    nonce_prefix: &[u8],
    chunk_index: u64,
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    cipher
        .encrypt(
            &Nonce::from(chunk_nonce(nonce_prefix, chunk_index)),
            Payload {
                msg: plaintext,
                aad: ENCRYPTED_AAD,
            },
        )
        .map_err(|error| VaultimeError::Backup(format!("failed to encrypt backup chunk: {error}")))
}

fn decrypt_chunk(
    cipher: &ChaCha20Poly1305,
    nonce_prefix: &[u8],
    chunk_index: u64,
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    cipher
        .decrypt(
            &Nonce::from(chunk_nonce(nonce_prefix, chunk_index)),
            Payload {
                msg: ciphertext,
                aad: ENCRYPTED_AAD,
            },
        )
        .map_err(|error| VaultimeError::Backup(format!("failed to decrypt backup chunk: {error}")))
}

fn chunk_nonce(nonce_prefix: &[u8], chunk_index: u64) -> [u8; NONCE_BYTES] {
    let mut nonce = [0_u8; NONCE_BYTES];
    nonce[..NONCE_PREFIX_BYTES].copy_from_slice(&nonce_prefix[..NONCE_PREFIX_BYTES]);
    nonce[NONCE_PREFIX_BYTES..].copy_from_slice(&chunk_index.to_be_bytes());
    nonce
}

fn write_encrypted_chunk(
    output: &mut File,
    is_last: bool,
    plaintext_len: u32,
    ciphertext: &[u8],
) -> Result<()> {
    output
        .write_all(&[u8::from(is_last)])
        .map_err(map_backup_io)?;
    output
        .write_all(&plaintext_len.to_be_bytes())
        .map_err(map_backup_io)?;
    output
        .write_all(
            &u32::try_from(ciphertext.len())
                .map_err(|_| VaultimeError::Backup("encrypted chunk length overflow".into()))?
                .to_be_bytes(),
        )
        .map_err(map_backup_io)?;
    output.write_all(ciphertext).map_err(map_backup_io)?;
    Ok(())
}

fn read_u32(input: &mut File) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    input.read_exact(&mut bytes).map_err(map_backup_io)?;
    Ok(u32::from_be_bytes(bytes))
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

#[cfg(test)]
mod tests {
    use super::{decrypt_archive, encrypt_archive, encrypt_chunk};
    use chacha20poly1305::{ChaCha20Poly1305, KeyInit};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn test_dir() -> PathBuf {
        std::env::temp_dir().join(format!("vaultime-remote-backup-test-{}", Uuid::new_v4()))
    }

    // Existing cloud backups must stay decryptable across crate upgrades.
    #[test]
    fn chunk_encryption_output_is_stable() {
        let cipher = ChaCha20Poly1305::new_from_slice(&[7_u8; 32]).expect("key");
        let ciphertext = encrypt_chunk(&cipher, &[1, 2, 3, 4], 3, b"vaultime").expect("encrypt");
        assert_eq!(
            crate::hex::encode(&ciphertext),
            "8b520c0a28beb3c892b7e36e387f57ca4d0da19da09e9629"
        );
    }

    #[test]
    fn encrypted_archives_round_trip_across_multiple_chunks() {
        let dir = test_dir();
        fs::create_dir_all(&dir).expect("test dir");

        let plaintext_path = dir.join("plain.zip");
        let encrypted_path = dir.join("plain.enc");
        let restored_path = dir.join("plain-restored.zip");
        let mut plaintext = Vec::with_capacity(700_000);
        while plaintext.len() < 700_000 {
            plaintext.extend_from_slice(b"vaultime-cloud-backup-round-trip");
        }
        plaintext.truncate(700_000);
        fs::write(&plaintext_path, &plaintext).expect("write plaintext");

        let key = [7_u8; 32];
        encrypt_archive(&plaintext_path, &encrypted_path, &key).expect("encrypt");
        decrypt_archive(&encrypted_path, &restored_path, &key).expect("decrypt");

        let restored = fs::read(&restored_path).expect("read restored");
        assert_eq!(restored, plaintext);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tampered_archives_fail_authentication() {
        let dir = test_dir();
        fs::create_dir_all(&dir).expect("test dir");

        let plaintext_path = dir.join("plain.zip");
        let encrypted_path = dir.join("plain.enc");
        let restored_path = dir.join("plain-restored.zip");
        fs::write(&plaintext_path, b"vaultime-backup").expect("write plaintext");

        let key = [9_u8; 32];
        encrypt_archive(&plaintext_path, &encrypted_path, &key).expect("encrypt");

        let mut encrypted = fs::read(&encrypted_path).expect("read encrypted");
        let last_index = encrypted.len() - 1;
        encrypted[last_index] ^= 0x5a;
        fs::write(&encrypted_path, encrypted).expect("rewrite encrypted");

        let error =
            decrypt_archive(&encrypted_path, &restored_path, &key).expect_err("tamper fail");
        assert!(
            error.to_string().contains("failed to decrypt backup chunk"),
            "unexpected error: {error}"
        );

        fs::remove_dir_all(&dir).ok();
    }
}
