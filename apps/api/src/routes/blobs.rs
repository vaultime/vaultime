// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Artwork blobs: encrypted images stored once per account and shared by every
//! backup that refers to them. Blob ids are keyed hashes of the image, made on
//! the client, so the same image always has the same id and the server cannot
//! tell what it shows.

use std::collections::HashSet;
use std::io::ErrorKind;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path as AxumPath, Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use tokio::fs::{self, File};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use super::backups::{is_sha256_hex, receive_body, remove_file_if_present};
use crate::AppState;
use crate::auth::AuthenticatedAccount;
use crate::constants::{
    BYTES_PER_GIB, BYTES_PER_MIB, MAX_BLOB_IDS_PER_REQUEST, UPLOAD_LOCK_WAIT_SECS,
};
use crate::error::{AppError, AppResult};
use crate::models::{MissingBlobsRequest, MissingBlobsResponse, StorageResponse};

pub async fn missing_blobs(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    Json(payload): Json<MissingBlobsRequest>,
) -> AppResult<Json<MissingBlobsResponse>> {
    let ids = validate_blob_ids(&payload.ids)?;
    let present: HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT id FROM cloud_blobs WHERE account_id = $1 AND id = ANY($2)",
    )
    .bind(auth.account_id)
    .bind(&ids)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .collect();

    Ok(Json(MissingBlobsResponse {
        missing: ids.into_iter().filter(|id| !present.contains(id)).collect(),
    }))
}

pub async fn upload_blob(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    AxumPath(blob_id): AxumPath<String>,
    request: Request,
) -> AppResult<StatusCode> {
    let blob_id = validate_blob_id(&blob_id)?;
    let Some(_upload) = state
        .limits
        .uploads
        .lock_within(
            auth.account_id,
            std::time::Duration::from_secs(UPLOAD_LOCK_WAIT_SECS),
        )
        .await
    else {
        return Err(AppError::conflict(
            "another upload of this account is still running",
        ));
    };
    if blob_exists(&state, auth.account_id, &blob_id).await? {
        return Ok(StatusCode::OK);
    }

    // The declared length turns a hopeless upload away before it starts.
    let room = upload_room(&state, auth.account_id).await?;
    let declared = request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok()?.parse::<i64>().ok());
    let max_blob_bytes = state.config.max_backup_bytes;
    if declared.is_some_and(|bytes| bytes > max_blob_bytes) {
        return Err(too_large(max_blob_bytes));
    }
    if room <= 0 || declared.is_some_and(|bytes| bytes > room) {
        return Err(storage_full(state.config.max_account_bytes));
    }

    let storage_key = format!("{}/blobs/{blob_id}.{}", auth.account_id, Uuid::new_v4());
    let final_path = state.config.backup_root.join(&storage_key);
    let temp_path = final_path.with_file_name(format!(
        "{}.part",
        final_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let limit = room.min(max_blob_bytes);
    let received = receive_body(request.into_body(), &temp_path, limit).await;
    let size_bytes = match received {
        Ok(Some((size_bytes, _))) => size_bytes,
        Ok(None) => {
            remove_file_if_present(&temp_path).await?;
            return Err(if limit < max_blob_bytes {
                storage_full(state.config.max_account_bytes)
            } else {
                too_large(max_blob_bytes)
            });
        }
        Err(error) => {
            if let Err(cleanup_error) = remove_file_if_present(&temp_path).await {
                tracing::error!(error = %cleanup_error, "failed to remove partial artwork upload");
            }
            return Err(error);
        }
    };
    fs::rename(&temp_path, &final_path).await?;

    let inserted = sqlx::query(
        "INSERT INTO cloud_blobs (account_id, id, storage_key, size_bytes)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (account_id, id) DO NOTHING",
    )
    .bind(auth.account_id)
    .bind(&blob_id)
    .bind(&storage_key)
    .bind(size_bytes)
    .execute(&state.db)
    .await?
    .rows_affected();

    // Another upload of the same image won the race, its copy stays.
    if inserted == 0 {
        remove_file_if_present(&final_path).await?;
        return Ok(StatusCode::OK);
    }
    Ok(StatusCode::CREATED)
}

pub async fn download_blob(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    AxumPath(blob_id): AxumPath<String>,
) -> AppResult<Response> {
    let blob_id = validate_blob_id(&blob_id)?;
    let (storage_key, size_bytes) = sqlx::query_as::<_, (String, i64)>(
        "SELECT storage_key, size_bytes FROM cloud_blobs WHERE account_id = $1 AND id = $2",
    )
    .bind(auth.account_id)
    .bind(&blob_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("artwork not found"))?;

    let file = match File::open(state.config.backup_root.join(&storage_key)).await {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(AppError::not_found("artwork file is missing on disk"));
        }
        Err(error) => return Err(error.into()),
    };

    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(CONTENT_LENGTH, HeaderValue::from(size_bytes));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

pub async fn storage(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
) -> AppResult<Json<StorageResponse>> {
    let (backup_bytes, artwork_bytes) = usage_parts(&state, auth.account_id).await?;
    Ok(Json(StorageResponse {
        backup_bytes,
        artwork_bytes,
        limit_bytes: state.config.max_account_bytes,
    }))
}

/// Refers a new backup to its artwork, inside the transaction that creates
/// it. Fails with `missing_artwork` when the account lacks any of the blobs.
pub(super) async fn attach_blobs(
    connection: &mut sqlx::PgConnection,
    account_id: Uuid,
    backup_id: Uuid,
    blob_ids: &[String],
) -> AppResult<()> {
    if blob_ids.is_empty() {
        return Ok(());
    }
    let attached = sqlx::query(
        "INSERT INTO cloud_backup_blobs (backup_id, account_id, blob_id)
         SELECT $1, account_id, id FROM cloud_blobs WHERE account_id = $2 AND id = ANY($3)",
    )
    .bind(backup_id)
    .bind(account_id)
    .bind(blob_ids)
    .execute(connection)
    .await?
    .rows_affected();

    let missing = blob_ids.len() - usize::try_from(attached).unwrap_or(blob_ids.len());
    if missing > 0 {
        return Err(AppError::missing_artwork(format!(
            "{missing} artwork files of this backup are not on the server"
        )));
    }
    Ok(())
}

/// Deletes artwork no backup refers to once it is older than the grace period,
/// which protects artwork uploaded for a backup that is still being put
/// together. Rows go before files: a backup that starts to refer to a blob in
/// the meantime keeps its row, and a blob uploaded again gets a new file name,
/// so removing an old file never touches artwork a backup needs.
pub(super) async fn collect_unreferenced_blobs(
    state: &AppState,
    account_id: Uuid,
) -> AppResult<()> {
    let removed = sqlx::query_scalar::<_, String>(
        "DELETE FROM cloud_blobs b
         WHERE b.account_id = $1
           AND b.uploaded_at < NOW() - make_interval(secs => $2)
           AND NOT EXISTS (
               SELECT 1 FROM cloud_backup_blobs r
               WHERE r.account_id = b.account_id AND r.blob_id = b.id
           )
         RETURNING b.storage_key",
    )
    .bind(account_id)
    .bind(state.config.stale_pending_backup_secs)
    .fetch_all(&state.db)
    .await?;

    for storage_key in &removed {
        if let Err(error) =
            remove_file_if_present(&state.config.backup_root.join(storage_key)).await
        {
            tracing::error!(%account_id, error = %error, "failed to remove unused artwork");
        }
    }
    if !removed.is_empty() {
        tracing::info!(%account_id, count = removed.len(), "removed unused artwork");
    }
    Ok(())
}

/// Bytes an account stores, backups and artwork together.
pub(super) async fn account_usage(state: &AppState, account_id: Uuid) -> AppResult<i64> {
    let (backup_bytes, artwork_bytes) = usage_parts(state, account_id).await?;
    Ok(backup_bytes.saturating_add(artwork_bytes))
}

/// What an upload may still add. While a new backup comes in, an account may
/// go past its storage by up to one backup, since the oldest backups make
/// room only once the new one is stored. A failed upload so never costs an
/// old backup.
pub(super) async fn upload_room(state: &AppState, account_id: Uuid) -> AppResult<i64> {
    Ok(state
        .config
        .max_account_bytes
        .saturating_add(state.config.max_backup_bytes)
        .saturating_sub(account_usage(state, account_id).await?))
}

/// Storage the account keeps: its backups and the artwork they refer to.
/// Artwork no backup needs any more goes with the next cleanup.
pub(super) async fn kept_usage(state: &AppState, account_id: Uuid) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT
             COALESCE((SELECT SUM(size_bytes) FROM cloud_backups WHERE account_id = $1), 0)::BIGINT
           + COALESCE((SELECT SUM(b.size_bytes) FROM cloud_blobs b
                       WHERE b.account_id = $1 AND EXISTS (
                           SELECT 1 FROM cloud_backup_blobs r
                           WHERE r.account_id = b.account_id AND r.blob_id = b.id
                       )), 0)::BIGINT",
    )
    .bind(account_id)
    .fetch_one(&state.db)
    .await?)
}

async fn usage_parts(state: &AppState, account_id: Uuid) -> AppResult<(i64, i64)> {
    Ok(sqlx::query_as::<_, (i64, i64)>(
        "SELECT
             COALESCE((SELECT SUM(size_bytes) FROM cloud_backups WHERE account_id = $1), 0)::BIGINT,
             COALESCE((SELECT SUM(size_bytes) FROM cloud_blobs WHERE account_id = $1), 0)::BIGINT",
    )
    .bind(account_id)
    .fetch_one(&state.db)
    .await?)
}

async fn blob_exists(state: &AppState, account_id: Uuid, blob_id: &str) -> AppResult<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM cloud_blobs WHERE account_id = $1 AND id = $2)",
    )
    .bind(account_id)
    .bind(blob_id)
    .fetch_one(&state.db)
    .await?)
}

/// Blob ids in lower case, without duplicates, in their first order.
pub(super) fn validate_blob_ids(ids: &[String]) -> AppResult<Vec<String>> {
    if ids.len() > MAX_BLOB_IDS_PER_REQUEST {
        return Err(AppError::bad_request(format!(
            "at most {MAX_BLOB_IDS_PER_REQUEST} artwork ids per request"
        )));
    }
    let mut seen = HashSet::new();
    let mut valid = Vec::with_capacity(ids.len());
    for id in ids {
        let id = validate_blob_id(id)?;
        if seen.insert(id.clone()) {
            valid.push(id);
        }
    }
    Ok(valid)
}

fn validate_blob_id(id: &str) -> AppResult<String> {
    if is_sha256_hex(id) {
        Ok(id.to_ascii_lowercase())
    } else {
        Err(AppError::bad_request(
            "artwork ids are 64-character hex digests",
        ))
    }
}

fn too_large(max_bytes: i64) -> AppError {
    AppError::bad_request(format!(
        "artwork exceeds the {} MiB size limit",
        max_bytes / BYTES_PER_MIB
    ))
}

pub(super) fn storage_full(max_account_bytes: i64) -> AppError {
    let limit = if max_account_bytes % BYTES_PER_GIB == 0 {
        format!("{} GiB", max_account_bytes / BYTES_PER_GIB)
    } else {
        format!("{} MiB", max_account_bytes / BYTES_PER_MIB)
    };
    AppError::storage_full(format!(
        "this account's {limit} of cloud storage is full. Delete older backups to make room."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_ids_are_lowercased_and_deduplicated() {
        let upper = "A".repeat(64);
        let lower = "a".repeat(64);
        let other = "b".repeat(64);
        assert_eq!(
            validate_blob_ids(&[upper, other.clone(), lower.clone()]).unwrap(),
            vec![lower, other]
        );
    }

    #[test]
    fn rejects_ids_that_could_leave_the_folder() {
        assert!(validate_blob_id("../../etc/passwd").is_err());
        assert!(validate_blob_id(&format!("{}/x", "a".repeat(62))).is_err());
        assert!(validate_blob_ids(&vec!["a".repeat(64); MAX_BLOB_IDS_PER_REQUEST + 1]).is_err());
    }
}
