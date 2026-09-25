// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use std::fmt::Write;

use argon2::Argon2;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use scrypt::{Params, scrypt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::AppState;
use crate::config::Config;
use crate::error::{AppError, AppResult};

const ACCESS_TOKEN_TTL_MINUTES: i64 = 15;
const REFRESH_TOKEN_TTL_DAYS: i64 = 30;

#[derive(Debug, Clone)]
pub struct AuthenticatedAccount {
    pub account_id: Uuid,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AccessClaims {
    sub: String,
    email: String,
    role: String,
    exp: usize,
    iat: usize,
}

pub struct ParsedInviteCode {
    pub normalized_code: String,
    pub lookup_key: String,
}

impl ParsedInviteCode {
    pub fn parse(raw: &str) -> AppResult<Self> {
        let trimmed = raw.trim();
        let mut parts = trimmed.split('-').filter(|part| !part.is_empty());
        let prefix = parts
            .next()
            .ok_or_else(|| AppError::bad_request("invite code is missing a prefix"))?;
        let prefix = prefix.to_ascii_uppercase();

        let body = parts
            .flat_map(|part| part.chars())
            .filter(|ch| ch.is_ascii_alphanumeric())
            .map(|ch| ch.to_ascii_uppercase())
            .collect::<String>();

        if body.len() < 12 {
            return Err(AppError::bad_request("invite code is too short"));
        }

        let normalized_chunks = body
            .as_bytes()
            .chunks(4)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("-");

        Ok(Self {
            normalized_code: format!("{prefix}-{normalized_chunks}"),
            lookup_key: body[..12].to_string(),
        })
    }
}

pub fn normalize_email(raw: &str) -> AppResult<String> {
    let email = raw.trim().to_ascii_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::bad_request("a valid email address is required"));
    }

    Ok(email)
}

pub fn hash_password(password: &str) -> AppResult<String> {
    if password.len() < 10 {
        return Err(AppError::bad_request(
            "password must be at least 10 characters long",
        ));
    }

    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

pub fn verify_password(password: &str, password_hash: &str) -> AppResult<bool> {
    let parsed_hash = PasswordHash::new(password_hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub fn hash_refresh_token(token: &str, pepper: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pepper.as_bytes());
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn generate_refresh_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn create_access_token(
    config: &Config,
    account_id: Uuid,
    email: &str,
    role: &str,
) -> AppResult<(String, DateTime<Utc>)> {
    let issued_at = Utc::now();
    let expires_at = issued_at + Duration::minutes(ACCESS_TOKEN_TTL_MINUTES);
    let claims = AccessClaims {
        sub: account_id.to_string(),
        email: email.to_string(),
        role: role.to_string(),
        exp: expires_at.timestamp() as usize,
        iat: issued_at.timestamp() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.access_token_secret.as_bytes()),
    )?;

    Ok((token, expires_at))
}

pub fn decode_access_token(config: &Config, token: &str) -> AppResult<AuthenticatedAccount> {
    let decoded = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(config.access_token_secret.as_bytes()),
        &Validation::default(),
    )?;

    let account_id = Uuid::parse_str(&decoded.claims.sub)
        .map_err(|error| AppError::unauthorized(format!("invalid token subject: {error}")))?;

    Ok(AuthenticatedAccount {
        account_id,
        role: decoded.claims.role,
    })
}

pub fn refresh_token_expiry() -> DateTime<Utc> {
    Utc::now() + Duration::days(REFRESH_TOKEN_TTL_DAYS)
}

pub fn hash_invite_code(normalized_code: &str, salt: &str) -> AppResult<String> {
    let params = Params::new(14, 8, 1, 64)
        .map_err(|error| AppError::internal(format!("invalid scrypt params: {error}")))?;

    let mut out = [0_u8; 64];
    scrypt(
        normalized_code.as_bytes(),
        salt.as_bytes(),
        &params,
        &mut out,
    )
    .map_err(|error| AppError::internal(format!("failed to hash invite: {error}")))?;

    let mut actual = String::with_capacity(out.len() * 2);
    for byte in out {
        let _ = write!(&mut actual, "{byte:02x}");
    }

    Ok(actual)
}

pub fn verify_invite_hash(
    normalized_code: &str,
    salt: &str,
    expected_hash: &str,
) -> AppResult<bool> {
    Ok(hash_invite_code(normalized_code, salt)? == expected_hash)
}

impl FromRequestParts<AppState> for AuthenticatedAccount {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or_else(|| AppError::unauthorized("missing Authorization header"))?;

        let auth_value = auth_header
            .to_str()
            .map_err(|_| AppError::unauthorized("invalid Authorization header"))?;

        let token = auth_value
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::unauthorized("expected Bearer token"))?;

        decode_access_token(&state.config, token)
    }
}

#[cfg(test)]
mod tests {
    use super::ParsedInviteCode;

    #[test]
    fn parses_invite_codes_case_insensitively() {
        let parsed = ParsedInviteCode::parse("vtlinv-a3q1-s5f8-s1dt-r1dj-2ric-nxao").unwrap();
        assert_eq!(parsed.lookup_key, "A3Q1S5F8S1DT");
        assert_eq!(
            parsed.normalized_code,
            "VTLINV-A3Q1-S5F8-S1DT-R1DJ-2RIC-NXAO"
        );
    }
}
