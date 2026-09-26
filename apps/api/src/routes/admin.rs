// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::AppState;
use crate::auth::AuthenticatedAccount;
use crate::constants::DEFAULT_INVITE_MAX_REDEMPTIONS;
use crate::error::{AppError, AppResult};
use crate::invites::generate_invite;
use crate::models::{AdminInviteResponse, CreateAdminInviteRequest};

pub async fn create_invite(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    Json(payload): Json<CreateAdminInviteRequest>,
) -> AppResult<(StatusCode, Json<AdminInviteResponse>)> {
    require_admin(&auth)?;

    let invite = generate_invite(
        payload.prefix.as_deref(),
        payload
            .max_redemptions
            .unwrap_or(DEFAULT_INVITE_MAX_REDEMPTIONS),
        payload.expires_at,
        payload.note,
    )?;

    sqlx::query(
        r#"
        INSERT INTO cloud_invites (
            lookup_key,
            salt,
            code_hash,
            max_redemptions,
            expires_at,
            note,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(&invite.lookup_key)
    .bind(&invite.salt)
    .bind(&invite.code_hash)
    .bind(invite.max_redemptions)
    .bind(invite.expires_at)
    .bind(&invite.note)
    .bind(invite.created_at)
    .execute(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(AdminInviteResponse {
            code: invite.code,
            lookup_key: invite.lookup_key,
            salt: invite.salt,
            code_hash: invite.code_hash,
            max_redemptions: invite.max_redemptions,
            expires_at: invite.expires_at,
            note: invite.note,
            created_at: invite.created_at,
        }),
    ))
}

fn require_admin(auth: &AuthenticatedAccount) -> AppResult<()> {
    if auth.role != "admin" {
        return Err(AppError::forbidden("admin access is required"));
    }

    Ok(())
}
