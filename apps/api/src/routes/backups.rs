// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use axum::Json;
use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use chrono::{DateTime, Duration, Utc};
use futures_util::TryStreamExt;
use sha2::{Digest, Sha256};
use sqlx::AssertSqlSafe;
use tokio::fs::{self, File};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthenticatedAccount;
use crate::constants::{BYTES_PER_MIB, SECS_PER_MINUTE};
use crate::error::{AppError, AppResult};
use crate::models::{BackupRecordResponse, CreateBackupRequest, DownloadQuery};

/// Turns `b`, a CTE of `cloud_backups` rows, into `BackupRecordResponse` rows.
const BACKUP_RECORD_SELECT: &str = "
    SELECT b.id, b.label, b.storage_key, b.checksum, b.size_bytes, b.backup_created_at,
           b.uploaded_at, b.status, d.client_device_id, b.metadata_json
    FROM b
    LEFT JOIN cloud_devices d ON d.id = b.device_id
";

pub async fn list_backups(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<BackupRecordResponse>>> {
    let rows = sqlx::query_as::<_, BackupRecordResponse>(AssertSqlSafe(format!(
        "WITH b AS (SELECT * FROM cloud_backups WHERE account_id = $1)
         {BACKUP_RECORD_SELECT}
         ORDER BY b.uploaded_at DESC"
    )))
    .bind(auth.account_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

pub async fn create_backup(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    Json(payload): Json<CreateBackupRequest>,
) -> AppResult<(StatusCode, Json<BackupRecordResponse>)> {
    let checksum = payload.checksum.trim();
    if checksum.is_empty() {
        return Err(AppError::bad_request("checksum is required"));
    }
    if !is_sha256_hex(checksum) {
        return Err(AppError::bad_request(
            "checksum must be a 64-character SHA-256 hex digest",
        ));
    }

    prune_stale_pending_backups(&state, auth.account_id).await?;
    enforce_backup_limits(&state, auth.account_id).await?;

    let client_device_id = payload
        .client_device_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let device_id = match client_device_id {
        None => None,
        Some(client_device_id) => Some(
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM cloud_devices WHERE account_id = $1 AND client_device_id = $2",
            )
            .bind(auth.account_id)
            .bind(client_device_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| {
                AppError::bad_request("client_device_id is not registered for this account")
            })?,
        ),
    };

    let backup_id = Uuid::new_v4();
    let storage_key = format!("{}/{backup_id}.vaultime.enc", auth.account_id);
    let label = payload
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let metadata_json = payload.metadata_json.unwrap_or_default();

    let row = sqlx::query_as::<_, BackupRecordResponse>(AssertSqlSafe(format!(
        "WITH b AS (
             INSERT INTO cloud_backups (
                 id,
                 account_id,
                 device_id,
                 label,
                 storage_key,
                 checksum,
                 size_bytes,
                 backup_created_at,
                 status,
                 metadata_json
             )
             VALUES ($1, $2, $3, $4, $5, $6, 0, $7, 'pending', $8)
             RETURNING *
         )
         {BACKUP_RECORD_SELECT}"
    )))
    .bind(backup_id)
    .bind(auth.account_id)
    .bind(device_id)
    .bind(label)
    .bind(storage_key)
    .bind(checksum)
    .bind(payload.backup_created_at)
    .bind(metadata_json)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn upload_backup_content(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    AxumPath(backup_id): AxumPath<Uuid>,
    request: Request,
) -> AppResult<Json<BackupRecordResponse>> {
    let backup = find_backup(&state, auth.account_id, backup_id).await?;
    if backup.status == "complete" {
        return Err(AppError::conflict(
            "backup content has already been uploaded",
        ));
    }

    let root = &state.config.backup_root;
    let final_path = backup_path(root, &backup.storage_key);
    let temp_path = temporary_backup_path(&final_path, backup_id);
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let max_backup_bytes = state.config.max_backup_bytes;
    let received = receive_body(request.into_body(), &temp_path, max_backup_bytes).await;
    let (size_bytes, actual_checksum) = match received {
        Ok(Some(received)) => received,
        Ok(None) => {
            remove_backup(&state, auth.account_id, backup_id, &backup.storage_key).await?;
            return Err(AppError::bad_request(format!(
                "backup exceeds the current {} MiB size limit",
                max_backup_bytes / BYTES_PER_MIB
            )));
        }
        Err(error) => {
            if let Err(cleanup_error) = remove_file_if_present(&temp_path).await {
                tracing::error!(%backup_id, error = %cleanup_error, "failed to remove partial upload");
            }
            return Err(error);
        }
    };

    if !actual_checksum.eq_ignore_ascii_case(&backup.checksum) {
        remove_backup(&state, auth.account_id, backup_id, &backup.storage_key).await?;
        return Err(AppError::bad_request(
            "uploaded backup checksum does not match declared checksum",
        ));
    }

    fs::rename(&temp_path, &final_path).await?;

    let row = sqlx::query_as::<_, BackupRecordResponse>(AssertSqlSafe(format!(
        "WITH b AS (
             UPDATE cloud_backups
             SET size_bytes = $1,
                 status = 'complete',
                 uploaded_at = NOW()
             WHERE id = $2 AND account_id = $3
             RETURNING *
         )
         {BACKUP_RECORD_SELECT}"
    )))
    .bind(size_bytes)
    .bind(backup_id)
    .bind(auth.account_id)
    .fetch_optional(&state.db)
    .await?;

    // The row can vanish while the body streams in, for example through a parallel delete.
    let Some(row) = row else {
        remove_file_if_present(&final_path).await?;
        return Err(AppError::not_found("backup not found"));
    };

    if let Err(error) = rotate_complete_backups(&state, auth.account_id, backup_id).await {
        tracing::error!(account_id = %auth.account_id, error = %error, "backup rotation failed");
    }

    Ok(Json(row))
}

pub async fn get_backup(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    AxumPath(backup_id): AxumPath<Uuid>,
) -> AppResult<Json<BackupRecordResponse>> {
    Ok(Json(find_backup(&state, auth.account_id, backup_id).await?))
}

pub async fn download_backup(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    AxumPath(backup_id): AxumPath<Uuid>,
    Query(query): Query<DownloadQuery>,
) -> AppResult<Response> {
    let backup = find_backup(&state, auth.account_id, backup_id).await?;
    if backup.status != "complete" {
        return Err(AppError::conflict(
            "backup content is not ready for download",
        ));
    }

    let file_path = backup_path(&state.config.backup_root, &backup.storage_key);
    let file = match File::open(&file_path).await {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(AppError::not_found("backup file is missing on disk"));
        }
        Err(error) => return Err(error.into()),
    };

    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(CONTENT_LENGTH, HeaderValue::from(backup.size_bytes));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));

    if query.attachment.unwrap_or(true) {
        let disposition = format!("attachment; filename=\"vaultime-backup-{}.enc\"", backup.id);
        headers.insert(
            CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposition)
                .map_err(|error| AppError::internal(format!("invalid filename header: {error}")))?,
        );
    }

    Ok(response)
}

pub async fn delete_backup(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    AxumPath(backup_id): AxumPath<Uuid>,
) -> AppResult<StatusCode> {
    let backup = find_backup(&state, auth.account_id, backup_id).await?;
    remove_backup(&state, auth.account_id, backup.id, &backup.storage_key).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn find_backup(
    state: &AppState,
    account_id: Uuid,
    backup_id: Uuid,
) -> AppResult<BackupRecordResponse> {
    sqlx::query_as::<_, BackupRecordResponse>(AssertSqlSafe(format!(
        "WITH b AS (SELECT * FROM cloud_backups WHERE id = $1 AND account_id = $2)
         {BACKUP_RECORD_SELECT}"
    )))
    .bind(backup_id)
    .bind(account_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("backup not found"))
}

/// Streams the body to `path` and returns its size and SHA-256 hex digest. Returns `None` as soon
/// as the body grows past `max_bytes`.
async fn receive_body(body: Body, path: &Path, max_bytes: i64) -> AppResult<Option<(i64, String)>> {
    let mut writer = BufWriter::new(File::create(path).await?);
    let mut stream = body.into_data_stream();
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_i64;

    while let Some(chunk) = stream.try_next().await.map_err(|error| {
        tracing::debug!(error = %error, "backup upload body failed");
        AppError::bad_request("backup upload was interrupted")
    })? {
        size_bytes = size_bytes.saturating_add(i64::try_from(chunk.len()).unwrap_or(i64::MAX));
        if size_bytes > max_bytes {
            return Ok(None);
        }
        hasher.update(&chunk);
        writer.write_all(&chunk).await?;
    }
    writer.flush().await?;

    Ok(Some((size_bytes, hex::encode(hasher.finalize()))))
}

async fn enforce_backup_limits(state: &AppState, account_id: Uuid) -> AppResult<()> {
    let (pending_backups, last_complete_at) = sqlx::query_as::<_, (i64, Option<DateTime<Utc>>)>(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE status = 'pending'),
            MAX(uploaded_at) FILTER (WHERE status = 'complete')
        FROM cloud_backups
        WHERE account_id = $1
        "#,
    )
    .bind(account_id)
    .fetch_one(&state.db)
    .await?;

    if pending_backups >= state.config.max_pending_backups_per_account {
        return Err(AppError::conflict(
            "a backup upload is already pending for this account",
        ));
    }

    let min_interval_seconds = state.config.min_backup_interval_seconds;
    if let Some(last_complete_at) = last_complete_at
        && last_complete_at + Duration::seconds(min_interval_seconds) > Utc::now()
    {
        return Err(AppError::conflict(format!(
            "wait at least {} minutes between remote backups",
            min_interval_seconds / SECS_PER_MINUTE
        )));
    }

    Ok(())
}

async fn prune_stale_pending_backups(state: &AppState, account_id: Uuid) -> AppResult<()> {
    let stale = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        DELETE FROM cloud_backups
        WHERE account_id = $1
          AND status = 'pending'
          AND uploaded_at < NOW() - make_interval(secs => $2)
        RETURNING id, storage_key
        "#,
    )
    .bind(account_id)
    .bind(state.config.stale_pending_backup_seconds)
    .fetch_all(&state.db)
    .await?;

    for (backup_id, storage_key) in stale {
        if let Err(error) =
            remove_backup_files(&state.config.backup_root, &storage_key, backup_id).await
        {
            tracing::error!(%backup_id, error = %error, "failed to remove stale backup upload");
        }
    }

    Ok(())
}

/// Keeps the newest `max_complete_backups_per_account` complete backups, counting `keep_id`, and
/// deletes the rest oldest first. Runs only after a new backup is stored and verified, so an
/// account never loses an old backup for a failed upload.
async fn rotate_complete_backups(
    state: &AppState,
    account_id: Uuid,
    keep_id: Uuid,
) -> AppResult<()> {
    let expired = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, storage_key
        FROM cloud_backups
        WHERE account_id = $1 AND status = 'complete' AND id <> $2
        ORDER BY uploaded_at DESC, id DESC
        OFFSET $3
        "#,
    )
    .bind(account_id)
    .bind(keep_id)
    .bind(state.config.max_complete_backups_per_account - 1)
    .fetch_all(&state.db)
    .await?;

    for (backup_id, storage_key) in expired {
        remove_backup(state, account_id, backup_id, &storage_key).await?;
        tracing::info!(%account_id, %backup_id, "rotated out old backup");
    }

    Ok(())
}

/// Removes files before the row. If file removal fails the row stays, so the delete can be
/// retried and nothing is left orphaned on disk.
async fn remove_backup(
    state: &AppState,
    account_id: Uuid,
    backup_id: Uuid,
    storage_key: &str,
) -> AppResult<()> {
    remove_backup_files(&state.config.backup_root, storage_key, backup_id).await?;

    sqlx::query("DELETE FROM cloud_backups WHERE id = $1 AND account_id = $2")
        .bind(backup_id)
        .bind(account_id)
        .execute(&state.db)
        .await?;

    Ok(())
}

async fn remove_backup_files(
    root: &Path,
    storage_key: &str,
    backup_id: Uuid,
) -> std::io::Result<()> {
    let final_path = backup_path(root, storage_key);
    remove_file_if_present(&temporary_backup_path(&final_path, backup_id)).await?;
    remove_file_if_present(&final_path).await
}

async fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path).await {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn backup_path(root: &Path, storage_key: &str) -> PathBuf {
    root.join(storage_key)
}

fn temporary_backup_path(final_path: &Path, backup_id: Uuid) -> PathBuf {
    final_path.with_file_name(format!("{backup_id}.part"))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaultime-api-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn validates_sha256_hex() {
        assert!(is_sha256_hex(&"a".repeat(64)));
        assert!(is_sha256_hex(&"F".repeat(64)));
        assert!(!is_sha256_hex(&"a".repeat(63)));
        assert!(!is_sha256_hex(&"g".repeat(64)));
    }

    #[test]
    fn temporary_path_sits_next_to_the_backup() {
        let backup_id = Uuid::new_v4();
        let final_path = backup_path(Path::new("/srv/backups"), "account/file.vaultime.enc");
        assert_eq!(
            temporary_backup_path(&final_path, backup_id),
            Path::new("/srv/backups/account").join(format!("{backup_id}.part"))
        );
    }

    #[tokio::test]
    async fn removes_final_and_partial_files() {
        let root = scratch_dir("remove");
        let backup_id = Uuid::new_v4();
        let storage_key = format!("account/{backup_id}.vaultime.enc");
        let final_path = backup_path(&root, &storage_key);
        let temp_path = temporary_backup_path(&final_path, backup_id);
        std::fs::create_dir_all(final_path.parent().unwrap()).unwrap();
        std::fs::write(&final_path, b"backup").unwrap();
        std::fs::write(&temp_path, b"partial").unwrap();

        remove_backup_files(&root, &storage_key, backup_id)
            .await
            .unwrap();
        assert!(!final_path.exists());
        assert!(!temp_path.exists());

        // Missing files are not an error, so deletes can be retried.
        remove_backup_files(&root, &storage_key, backup_id)
            .await
            .unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn receives_bodies_within_the_limit() {
        let root = scratch_dir("receive");
        let path = root.join("upload.part");

        let received = receive_body(Body::from("vaultime"), &path, 1024)
            .await
            .unwrap();
        let expected = hex::encode(Sha256::digest(b"vaultime"));
        assert_eq!(received, Some((8, expected)));
        assert_eq!(std::fs::read(&path).unwrap(), b"vaultime");

        assert_eq!(
            receive_body(Body::from("vaultime"), &path, 4)
                .await
                .unwrap(),
            None
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
