// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::auth::AuthenticatedAccount;
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
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row))
}

fn trim_required(value: &str, field_name: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request(format!("{field_name} is required")));
    }

    Ok(trimmed.to_string())
}
