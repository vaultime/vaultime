// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{
    ParsedInviteCode, create_access_token, generate_refresh_token, hash_password,
    hash_refresh_token, normalize_email, refresh_token_expiry, verify_invite_hash, verify_password,
};
use crate::error::{AppError, AppResult};
use crate::models::{
    AccountPasswordRow, AuthResponse, AuthUserResponse, InviteRow, LoginRequest, LogoutRequest,
    LogoutResponse, RefreshRequest, RefreshTokenAccountRow, SignUpRequest,
};

pub async fn sign_up(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SignUpRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    let email = normalize_email(&payload.email)?;
    let parsed_invite = ParsedInviteCode::parse(&payload.invite_code)?;
    let password_hash = hash_password(&payload.password)?;
    let user_agent = user_agent(&headers);

    let mut tx = state.db.begin().await?;

    let existing =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cloud_accounts WHERE email = $1")
            .bind(&email)
            .fetch_one(&mut *tx)
            .await?;
    if existing > 0 {
        return Err(AppError::conflict(
            "an account with that email already exists",
        ));
    }

    let invite = sqlx::query_as::<_, InviteRow>(
        r#"
        SELECT id, salt, code_hash, max_redemptions, redeemed_count, expires_at, revoked_at
        FROM cloud_invites
        WHERE lookup_key = $1
        FOR UPDATE
        "#,
    )
    .bind(&parsed_invite.lookup_key)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::bad_request("invite code is invalid"))?;

    if invite.revoked_at.is_some() {
        return Err(AppError::forbidden("invite has been revoked"));
    }
    if invite
        .expires_at
        .is_some_and(|expires_at| expires_at < Utc::now())
    {
        return Err(AppError::forbidden("invite has expired"));
    }
    if invite.redeemed_count >= invite.max_redemptions {
        return Err(AppError::forbidden("invite has no redemptions remaining"));
    }
    if !verify_invite_hash(
        &parsed_invite.normalized_code,
        &invite.salt,
        &invite.code_hash,
    )? {
        return Err(AppError::bad_request("invite code is invalid"));
    }

    let account_row = sqlx::query(
        r#"
        INSERT INTO cloud_accounts (email, invited_by_invite_id, access_granted_at, last_login_at)
        VALUES ($1, $2, NOW(), NOW())
        RETURNING id, email, role
        "#,
    )
    .bind(&email)
    .bind(invite.id)
    .fetch_one(&mut *tx)
    .await?;
    let account_id: Uuid = account_row.get("id");
    let account_email: String = account_row.get("email");
    let account_role: String = account_row.get("role");

    sqlx::query(
        r#"
        INSERT INTO cloud_account_passwords (account_id, password_hash)
        VALUES ($1, $2)
        "#,
    )
    .bind(account_id)
    .bind(password_hash)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO cloud_invite_redemptions (invite_id, account_id)
        VALUES ($1, $2)
        "#,
    )
    .bind(invite.id)
    .bind(account_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE cloud_invites
        SET redeemed_count = redeemed_count + 1
        WHERE id = $1
        "#,
    )
    .bind(invite.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let session = issue_session(
        &state,
        account_id,
        &account_email,
        &account_role,
        user_agent.as_deref(),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let email = normalize_email(&payload.email)?;
    let user_agent = user_agent(&headers);

    let row = sqlx::query_as::<_, AccountPasswordRow>(
        r#"
        SELECT a.id, a.email, a.role, a.access_state, p.password_hash
        FROM cloud_accounts a
        JOIN cloud_account_passwords p ON p.account_id = a.id
        WHERE a.email = $1
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::unauthorized("invalid email or password"))?;

    if row.access_state != "active" {
        return Err(AppError::forbidden("account does not have cloud access"));
    }
    if !verify_password(&payload.password, &row.password_hash)? {
        return Err(AppError::unauthorized("invalid email or password"));
    }

    sqlx::query("UPDATE cloud_accounts SET last_login_at = NOW() WHERE id = $1")
        .bind(row.id)
        .execute(&state.db)
        .await?;

    Ok(Json(
        issue_session(&state, row.id, &row.email, &row.role, user_agent.as_deref()).await?,
    ))
}

pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RefreshRequest>,
) -> AppResult<Json<AuthResponse>> {
    let hashed = hash_refresh_token(&payload.refresh_token, &state.config.refresh_token_pepper);
    let user_agent = user_agent(&headers);
    let next_refresh_token = generate_refresh_token();
    let next_hash = hash_refresh_token(&next_refresh_token, &state.config.refresh_token_pepper);
    let refresh_expires_at = refresh_token_expiry();

    let mut tx = state.db.begin().await?;

    let row = sqlx::query_as::<_, RefreshTokenAccountRow>(
        r#"
        SELECT t.id, t.account_id, a.email, a.role, a.access_state
        FROM cloud_refresh_tokens t
        JOIN cloud_accounts a ON a.id = t.account_id
        WHERE t.token_hash = $1
          AND t.revoked_at IS NULL
          AND t.expires_at > NOW()
        FOR UPDATE
        "#,
    )
    .bind(&hashed)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::unauthorized("refresh token is invalid or expired"))?;

    if row.access_state != "active" {
        return Err(AppError::forbidden("account does not have cloud access"));
    }

    sqlx::query(
        r#"
        UPDATE cloud_refresh_tokens
        SET revoked_at = NOW(), last_used_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(row.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO cloud_refresh_tokens (account_id, token_hash, user_agent, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(row.account_id)
    .bind(next_hash)
    .bind(user_agent)
    .bind(refresh_expires_at)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let (access_token, access_expires_at) =
        create_access_token(&state.config, row.account_id, &row.email, &row.role)?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: next_refresh_token,
        expires_at: access_expires_at,
        refresh_expires_at,
        user: AuthUserResponse {
            id: row.account_id,
            email: row.email,
            role: row.role,
        },
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<LogoutRequest>,
) -> AppResult<Json<LogoutResponse>> {
    let hashed = hash_refresh_token(&payload.refresh_token, &state.config.refresh_token_pepper);

    sqlx::query(
        r#"
        UPDATE cloud_refresh_tokens
        SET revoked_at = COALESCE(revoked_at, NOW()), last_used_at = NOW()
        WHERE token_hash = $1
        "#,
    )
    .bind(hashed)
    .execute(&state.db)
    .await?;

    Ok(Json(LogoutResponse { success: true }))
}

async fn issue_session(
    state: &AppState,
    account_id: Uuid,
    email: &str,
    role: &str,
    user_agent: Option<&str>,
) -> AppResult<AuthResponse> {
    let refresh_token = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&refresh_token, &state.config.refresh_token_pepper);
    let refresh_expires_at = refresh_token_expiry();

    sqlx::query(
        r#"
        INSERT INTO cloud_refresh_tokens (account_id, token_hash, user_agent, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(account_id)
    .bind(refresh_hash)
    .bind(user_agent)
    .bind(refresh_expires_at)
    .execute(&state.db)
    .await?;

    let (access_token, expires_at) = create_access_token(&state.config, account_id, email, role)?;

    Ok(AuthResponse {
        access_token,
        refresh_token,
        expires_at,
        refresh_expires_at,
        user: AuthUserResponse {
            id: account_id,
            email: email.to_string(),
            role: role.to_string(),
        },
    })
}

fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}
