// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthenticatedAccount;
use crate::constants::{
    MAX_DEVICE_FIELD_CHARS, MAX_DEVICE_PUBLIC_KEY_CHARS, MAX_DEVICES_PER_ACCOUNT,
};
use crate::error::{AppError, AppResult};
use crate::models::{DeviceResponse, RegisterDeviceRequest};

pub async fn register(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    Json(payload): Json<RegisterDeviceRequest>,
) -> AppResult<Json<DeviceResponse>> {
    let client_device_id = trim_required(&payload.client_device_id, "client_device_id")?;
    let device_name = trim_required(&payload.device_name, "device_name")?;
    let platform = trim_required(&payload.platform, "platform")?;
    let app_version = trim_required(&payload.app_version, "app_version")?;
    if payload
        .device_public_key
        .as_deref()
        .is_some_and(|key| key.chars().count() > MAX_DEVICE_PUBLIC_KEY_CHARS)
    {
        return Err(AppError::bad_request("device_public_key is too long"));
    }

    // One registration at a time per account, so two at once cannot both
    // take the last free place.
    let mut tx = state.db.begin().await?;
    lock_account(&mut tx, "devices", auth.account_id).await?;
    let (known, devices) = sqlx::query_as::<_, (bool, i64)>(
        "SELECT
             EXISTS(SELECT 1 FROM cloud_devices WHERE account_id = $1 AND client_device_id = $2),
             (SELECT COUNT(*) FROM cloud_devices WHERE account_id = $1)",
    )
    .bind(auth.account_id)
    .bind(&client_device_id)
    .fetch_one(&mut *tx)
    .await?;
    if !known && devices >= MAX_DEVICES_PER_ACCOUNT {
        return Err(AppError::conflict(format!(
            "an account can register up to {MAX_DEVICES_PER_ACCOUNT} devices"
        )));
    }

    let row = sqlx::query_as::<_, DeviceResponse>(
        r#"
        INSERT INTO cloud_devices (
            account_id,
            client_device_id,
            device_name,
            platform,
            app_version,
            device_public_key
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (account_id, client_device_id)
        DO UPDATE
        SET device_name = EXCLUDED.device_name,
            platform = EXCLUDED.platform,
            app_version = EXCLUDED.app_version,
            device_public_key = EXCLUDED.device_public_key,
            last_seen_at = NOW()
        RETURNING
            id,
            client_device_id,
            device_name,
            platform,
            app_version,
            registered_at,
            last_seen_at
        "#,
    )
    .bind(auth.account_id)
    .bind(client_device_id)
    .bind(device_name)
    .bind(platform)
    .bind(app_version)
    .bind(payload.device_public_key)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Json(row))
}

/// The devices of the account, the one seen last first.
pub async fn list(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<DeviceResponse>>> {
    let rows = sqlx::query_as::<_, DeviceResponse>(
        "SELECT id, client_device_id, device_name, platform, app_version, registered_at, last_seen_at
         FROM cloud_devices
         WHERE account_id = $1
         ORDER BY last_seen_at DESC, id",
    )
    .bind(auth.account_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

/// Removes a device of the account. Its backups stay, without a device, and
/// a device that is still used registers again when it next signs in.
pub async fn remove(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    Path(device_id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let removed = sqlx::query("DELETE FROM cloud_devices WHERE id = $1 AND account_id = $2")
        .bind(device_id)
        .bind(auth.account_id)
        .execute(&state.db)
        .await?
        .rows_affected();
    if removed == 0 {
        return Err(AppError::not_found("device not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Waits for and holds, until `tx` ends, the lock of `what` for the account,
/// so counts read in `tx` stay true until it commits.
pub(super) async fn lock_account(
    tx: &mut Transaction<'_, Postgres>,
    what: &str,
    account_id: Uuid,
) -> AppResult<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("{what}:{account_id}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn trim_required(value: &str, field_name: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request(format!("{field_name} is required")));
    }
    if trimmed.chars().count() > MAX_DEVICE_FIELD_CHARS {
        return Err(AppError::bad_request(format!("{field_name} is too long")));
    }

    Ok(trimmed.to_string())
}
