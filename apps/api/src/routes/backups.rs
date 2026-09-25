// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use axum::Json;
use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use futures_util::TryStreamExt;
use sha2::{Digest, Sha256};
use tokio::fs::{self, File};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthenticatedAccount;
use crate::error::{AppError, AppResult};
use crate::models::{BackupRecordResponse, CreateBackupRequest, DownloadQuery};

const MAX_BACKUP_BYTES: i64 = 512 * 1024 * 1024;
const MAX_PENDING_BACKUPS_PER_ACCOUNT: i64 = 1;
const MAX_COMPLETE_BACKUPS_PER_ACCOUNT: i64 = 30;
const MIN_BACKUP_INTERVAL_SECONDS: i64 = 15 * 60;
const STALE_PENDING_BACKUP_SECONDS: i64 = 60 * 60;

pub async fn list_backups(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<BackupRecordResponse>>> {
    let rows = sqlx::query_as::<_, BackupRecordResponse>(
        r#"
        SELECT
            b.id,
            b.label,
            b.storage_key,
            b.checksum,
            b.size_bytes,
            b.backup_created_at,
            b.uploaded_at,
            b.status,
            d.client_device_id,
            b.metadata_json
        FROM cloud_backups b
        LEFT JOIN cloud_devices d ON d.id = b.device_id
        WHERE b.account_id = $1
        ORDER BY b.uploaded_at DESC
        "#,
    )
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
    prune_stale_pending_backups(&state, auth.account_id).await?;
    enforce_backup_limits(&state, auth.account_id).await?;

    let checksum = payload.checksum.trim();
    if checksum.is_empty() {
        return Err(AppError::bad_request("checksum is required"));
    }
    if !is_sha256_hex(checksum) {
        return Err(AppError::bad_request(
            "checksum must be a 64-character SHA-256 hex digest",
        ));
    }

    let client_device_id = match payload.client_device_id.as_deref().map(str::trim) {
        Some("") | None => None,
        Some(value) => Some(value.to_string()),
    };

    let device_id = match client_device_id.as_deref() {
        Some("") | None => None,
        Some(client_device_id) => {
            let device_id = sqlx::query_scalar::<_, Uuid>(
                r#"
                SELECT id
                FROM cloud_devices
                WHERE account_id = $1 AND client_device_id = $2
                "#,
            )
            .bind(auth.account_id)
            .bind(client_device_id)
            .fetch_optional(&state.db)
            .await?;

            Some(device_id.ok_or_else(|| {
                AppError::bad_request("client_device_id is not registered for this account")
            })?)
        }
    };

    let backup_id = Uuid::new_v4();
    let storage_key = format!("{}/{backup_id}.vaultime.enc", auth.account_id);
    let label = payload
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let metadata_json = payload.metadata_json.unwrap_or_default();

    let row = sqlx::query_as::<_, BackupRecordResponse>(
        r#"
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
        RETURNING
            id,
            label,
            storage_key,
            checksum,
            size_bytes,
            backup_created_at,
            uploaded_at,
            status,
            $9::TEXT AS client_device_id,
            metadata_json
        "#,
    )
    .bind(backup_id)
    .bind(auth.account_id)
    .bind(device_id)
    .bind(label)
    .bind(storage_key)
    .bind(checksum)
    .bind(payload.backup_created_at)
    .bind(metadata_json)
    .bind(client_device_id.as_deref())
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

    let final_path = backup_path(&state.config.backup_root, &backup.storage_key);
    let parent = final_path
        .parent()
        .ok_or_else(|| AppError::internal("invalid backup path"))?;
    fs::create_dir_all(parent).await?;

    let temp_path = temporary_backup_path(parent, backup_id);
    let file = File::create(&temp_path).await?;
    let mut writer = BufWriter::new(file);
    let mut body_stream = request
        .into_body()
        .into_data_stream()
        .map_err(std::io::Error::other);
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_i64;

    while let Some(chunk) = body_stream.try_next().await? {
        size_bytes += i64::try_from(chunk.len())
            .map_err(|_| AppError::internal("backup payload length overflow"))?;
        if size_bytes > MAX_BACKUP_BYTES {
            let _ = fs::remove_file(&temp_path).await;
            delete_backup(&state, auth.account_id, backup_id).await?;
            return Err(AppError::bad_request(format!(
                "backup exceeds the current {} MiB size limit",
                MAX_BACKUP_BYTES / (1024 * 1024)
            )));
        }
        hasher.update(&chunk);
        writer.write_all(&chunk).await?;
    }
    writer.flush().await?;
    drop(writer);

    let actual_checksum = hex::encode(hasher.finalize());
    if actual_checksum != backup.checksum {
        let _ = fs::remove_file(&temp_path).await;
        delete_backup(&state, auth.account_id, backup_id).await?;
        return Err(AppError::bad_request(
            "uploaded backup checksum does not match declared checksum",
        ));
    }

    fs::rename(&temp_path, &final_path).await?;

    let row = sqlx::query_as::<_, BackupRecordResponse>(
        r#"
        UPDATE cloud_backups
        SET size_bytes = $1,
            status = 'complete',
            uploaded_at = NOW()
        WHERE id = $2 AND account_id = $3
        RETURNING
            id,
            label,
            storage_key,
            checksum,
            size_bytes,
            backup_created_at,
            uploaded_at,
            status,
            NULL::TEXT AS client_device_id,
            metadata_json
        "#,
    )
    .bind(size_bytes)
    .bind(backup_id)
    .bind(auth.account_id)
    .fetch_one(&state.db)
    .await?;

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
    if !Path::new(&file_path).exists() {
        return Err(AppError::not_found("backup file is missing on disk"));
    }

    let file = File::open(file_path).await?;
    let stream = ReaderStream::new(file);
    let mut response = Response::new(Body::from_stream(stream));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&backup.size_bytes.to_string())
            .map_err(|error| AppError::internal(format!("invalid content length: {error}")))?,
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));

    if query.attachment.unwrap_or(true) {
        response.headers_mut().insert(
            CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename=\"vaultime-backup-{}.enc\"",
                backup.id
            ))
            .map_err(|error| AppError::internal(format!("invalid filename header: {error}")))?,
        );
    }

    Ok(response)
}

async fn find_backup(
    state: &AppState,
    account_id: Uuid,
    backup_id: Uuid,
) -> AppResult<BackupRecordResponse> {
    sqlx::query_as::<_, BackupRecordResponse>(
        r#"
        SELECT
            b.id,
            b.label,
            b.storage_key,
            b.checksum,
            b.size_bytes,
            b.backup_created_at,
            b.uploaded_at,
            b.status,
            d.client_device_id,
            b.metadata_json
        FROM cloud_backups b
        LEFT JOIN cloud_devices d ON d.id = b.device_id
        WHERE b.id = $1 AND b.account_id = $2
        "#,
    )
    .bind(backup_id)
    .bind(account_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("backup not found"))
}

fn backup_path(root: &Path, storage_key: &str) -> PathBuf {
    root.join(storage_key)
}

fn temporary_backup_path(parent: &Path, backup_id: Uuid) -> PathBuf {
    parent.join(format!("{backup_id}.part"))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

async fn prune_stale_pending_backups(state: &AppState, account_id: Uuid) -> AppResult<()> {
    sqlx::query(
        r#"
        DELETE FROM cloud_backups
        WHERE account_id = $1
          AND status = 'pending'
          AND uploaded_at < NOW() - make_interval(secs => $2)
        "#,
    )
    .bind(account_id)
    .bind(STALE_PENDING_BACKUP_SECONDS)
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn enforce_backup_limits(state: &AppState, account_id: Uuid) -> AppResult<()> {
    let pending_backups = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM cloud_backups
        WHERE account_id = $1 AND status = 'pending'
        "#,
    )
    .bind(account_id)
    .fetch_one(&state.db)
    .await?;

    if pending_backups >= MAX_PENDING_BACKUPS_PER_ACCOUNT {
        return Err(AppError::conflict(
            "a backup upload is already pending for this account",
        ));
    }

    let complete_backups = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM cloud_backups
        WHERE account_id = $1 AND status = 'complete'
        "#,
    )
    .bind(account_id)
    .fetch_one(&state.db)
    .await?;

    if complete_backups >= MAX_COMPLETE_BACKUPS_PER_ACCOUNT {
        return Err(AppError::conflict(format!(
            "backup quota reached; keep at most {} remote backups per account for now",
            MAX_COMPLETE_BACKUPS_PER_ACCOUNT
        )));
    }

    let last_uploaded_at = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>(
        r#"
        SELECT uploaded_at
        FROM cloud_backups
        WHERE account_id = $1 AND status = 'complete'
        ORDER BY uploaded_at DESC
        LIMIT 1
        "#,
    )
    .bind(account_id)
    .fetch_optional(&state.db)
    .await?;

    if let Some(last_uploaded_at) = last_uploaded_at {
        let earliest_next_backup =
            last_uploaded_at + chrono::Duration::seconds(MIN_BACKUP_INTERVAL_SECONDS);
        if earliest_next_backup > chrono::Utc::now() {
            return Err(AppError::conflict(format!(
                "wait at least {} minutes between remote backups",
                MIN_BACKUP_INTERVAL_SECONDS / 60
            )));
        }
    }

    Ok(())
}

async fn delete_backup(state: &AppState, account_id: Uuid, backup_id: Uuid) -> AppResult<()> {
    sqlx::query(
        r#"
        DELETE FROM cloud_backups
        WHERE id = $1 AND account_id = $2
        "#,
    )
    .bind(backup_id)
    .bind(account_id)
    .execute(&state.db)
    .await?;

    Ok(())
}
