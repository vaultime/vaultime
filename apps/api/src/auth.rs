// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{Error as PasswordHashError, PasswordHasher, PasswordVerifier};
use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::AppState;
use crate::config::Config;
use crate::error::{AppError, AppResult};

const ACCESS_TOKEN_TTL_MINUTES: i64 = 15;
const REFRESH_TOKEN_TTL_DAYS: i64 = 30;
const MIN_PASSWORD_LENGTH: usize = 10;

// The VPS and Node invite generators use the same scrypt settings. Changing them breaks every
// stored invite hash.
const INVITE_SCRYPT_LOG_N: u8 = 14;
const INVITE_SCRYPT_R: u32 = 8;
const INVITE_SCRYPT_P: u32 = 1;
const INVITE_HASH_BYTES: usize = 64;

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
        let mut parts = raw.trim().split('-').filter(|part| !part.is_empty());
        let prefix = parts
            .next()
            .ok_or_else(|| AppError::bad_request("invite code is missing a prefix"))?
            .to_ascii_uppercase();

        let body = parts
            .flat_map(str::chars)
            .filter(char::is_ascii_alphanumeric)
            .map(|ch| ch.to_ascii_uppercase())
            .collect::<String>();

        if body.len() < 12 {
            return Err(AppError::bad_request("invite code is too short"));
        }

        Ok(Self {
            normalized_code: format!("{prefix}-{}", chunk_code(&body)),
            lookup_key: body[..12].to_string(),
        })
    }
}

/// Splits an ASCII code body into dash separated groups of four.
pub fn chunk_code(body: &str) -> String {
    body.as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn normalize_email(raw: &str) -> AppResult<String> {
    let email = raw.trim().to_ascii_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::bad_request("a valid email address is required"));
    }

    Ok(email)
}

pub fn hash_password(password: &str) -> AppResult<String> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(AppError::bad_request(format!(
            "password must be at least {MIN_PASSWORD_LENGTH} characters long"
        )));
    }

    Ok(Argon2::default()
        .hash_password(password.as_bytes())?
        .to_string())
}

/// The Argon2 parameters come from the stored PHC string, so hashes created with older defaults
/// keep verifying.
pub fn verify_password(password: &str, stored_hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(stored_hash)
        .map_err(|error| AppError::internal(format!("stored password hash is invalid: {error}")))?;

    match Argon2::default().verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(PasswordHashError::PasswordInvalid) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub fn hash_refresh_token(token: &str, pepper: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pepper.as_bytes());
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn generate_refresh_token() -> AppResult<String> {
    Ok(URL_SAFE_NO_PAD.encode(random_bytes::<32>()?))
}

pub fn random_bytes<const N: usize>() -> AppResult<[u8; N]> {
    let mut bytes = [0_u8; N];
    getrandom::fill(&mut bytes)?;
    Ok(bytes)
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
        exp: unix_seconds(expires_at),
        iat: unix_seconds(issued_at),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.access_token_secret.as_bytes()),
    )
    .map_err(|error| AppError::internal(format!("failed to sign access token: {error}")))?;

    Ok((token, expires_at))
}

/// Checks the HS256 signature and expiry and returns the account id from `sub`.
pub fn decode_access_token(config: &Config, token: &str) -> AppResult<Uuid> {
    let rejected = || AppError::unauthorized("access token is invalid or expired");

    let decoded = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(config.access_token_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|error| {
        tracing::debug!(error = %error, "rejected access token");
        rejected()
    })?;

    Uuid::parse_str(&decoded.claims.sub).map_err(|_| rejected())
}

pub fn refresh_token_expiry() -> DateTime<Utc> {
    Utc::now() + Duration::days(REFRESH_TOKEN_TTL_DAYS)
}

pub fn hash_invite_code(normalized_code: &str, salt: &str) -> AppResult<String> {
    let params = scrypt::Params::new(INVITE_SCRYPT_LOG_N, INVITE_SCRYPT_R, INVITE_SCRYPT_P)
        .map_err(|error| AppError::internal(format!("invalid scrypt params: {error}")))?;

    let mut out = [0_u8; INVITE_HASH_BYTES];
    scrypt::scrypt(
        normalized_code.as_bytes(),
        salt.as_bytes(),
        &params,
        &mut out,
    )
    .map_err(|error| AppError::internal(format!("failed to hash invite: {error}")))?;

    Ok(hex::encode(out))
}

pub fn verify_invite_hash(
    normalized_code: &str,
    salt: &str,
    expected_hash: &str,
) -> AppResult<bool> {
    Ok(hash_invite_code(normalized_code, salt)? == expected_hash)
}

fn unix_seconds(value: DateTime<Utc>) -> usize {
    usize::try_from(value.timestamp()).unwrap_or_default()
}

/// A valid token is not enough. The account must still exist and be active, so revoking access
/// takes effect before the token expires. The role is read from the database for the same reason.
impl FromRequestParts<AppState> for AuthenticatedAccount {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let auth_value = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or_else(|| AppError::unauthorized("missing Authorization header"))?
            .to_str()
            .map_err(|_| AppError::unauthorized("invalid Authorization header"))?;

        let token = auth_value
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::unauthorized("expected Bearer token"))?;

        let account_id = decode_access_token(&state.config, token)?;

        let (role, access_state) = sqlx::query_as::<_, (String, String)>(
            "SELECT role, access_state FROM cloud_accounts WHERE id = $1",
        )
        .bind(account_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::unauthorized("access token is invalid or expired"))?;

        if access_state != "active" {
            return Err(AppError::forbidden("account does not have cloud access"));
        }

        Ok(Self { account_id, role })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Computed with argon2 0.5, scrypt 0.11, sha2 0.10 and jsonwebtoken 9 before the upgrade. The
    // invite and refresh vectors also match Python hashlib and Node crypto.
    const INVITE_CODE: &str = "VTLINV-A3Q1-S5F8-S1DT-R1DJ-2RIC-NXAO";
    const INVITE_SALT: &str = "00112233445566778899aabbccddeeff";
    const INVITE_HASH: &str = "68f6d364d82e0da5f6a1015d7b2a1c350d5cbe76a4d44b19822bf1e2888a22d4e3d65e7dac381a8c4e56bd4cda419578f8ec7ad87802c369bbdd8ff7b765dae5";
    const REFRESH_HASH: &str = "a7cd44fe97cd3560610062a1b4f308a6c24e4e7fe06c7ec0d42421475929140d";
    const PASSWORD: &str = "correct horse battery";
    const PASSWORD_HASHES: [&str; 2] = [
        "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0c2FsdA$iPOQ5f2O21FsjnBvo1AiFcDuSXciCnKUrXFO+yfgNoM",
        "$argon2id$v=19$m=19456,t=2,p=1$IEwE8AixcRI3RoMdp+C9IA$mrpXCTXBgs8ZvrjuLgA7Fok6qdUhdewblGqYoW6KzYs",
    ];
    const ACCESS_SECRET: &str = "test-access-secret";
    const PINNED_ACCOUNT_ID: &str = "7f1c1f5e-3c1a-4f7e-9d3b-2a4c5e6f7a8b";
    const ACCESS_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiI3ZjFjMWY1ZS0zYzFhLTRmN2UtOWQzYi0yYTRjNWU2ZjdhOGIiLCJlbWFpbCI6InBsYXllckBleGFtcGxlLmNvbSIsInJvbGUiOiJ1c2VyIiwiZXhwIjo0MTAyNDQ0ODAwLCJpYXQiOjE3NjcyMjU2MDB9.5lxWbB-mcBDjNSvhnlPg_YmiovXxYfa4B_YbjS-anJg";

    fn test_config() -> Config {
        Config {
            bind: "127.0.0.1:0".parse().unwrap(),
            public_base_url: "http://localhost".into(),
            database_url: "postgres://localhost/vaultime".into(),
            backup_root: std::env::temp_dir(),
            access_token_secret: ACCESS_SECRET.into(),
            refresh_token_pepper: "test-pepper".into(),
            max_backup_bytes: 1024,
            max_pending_backups_per_account: 1,
            max_complete_backups_per_account: 30,
            min_backup_interval_seconds: 900,
            stale_pending_backup_seconds: 3600,
        }
    }

    fn pinned_claims() -> AccessClaims {
        AccessClaims {
            sub: PINNED_ACCOUNT_ID.into(),
            email: "player@example.com".into(),
            role: "user".into(),
            exp: 4_102_444_800,
            iat: 1_767_225_600,
        }
    }

    #[test]
    fn parses_invite_codes_case_insensitively() {
        let parsed = ParsedInviteCode::parse("vtlinv-a3q1-s5f8-s1dt-r1dj-2ric-nxao").unwrap();
        assert_eq!(parsed.lookup_key, "A3Q1S5F8S1DT");
        assert_eq!(parsed.normalized_code, INVITE_CODE);
    }

    #[test]
    fn invite_hash_matches_pinned_vector() {
        assert_eq!(
            hash_invite_code(INVITE_CODE, INVITE_SALT).unwrap(),
            INVITE_HASH
        );
        assert!(verify_invite_hash(INVITE_CODE, INVITE_SALT, INVITE_HASH).unwrap());
        assert!(
            !verify_invite_hash(
                "VTLINV-A3Q1-S5F8-S1DT-R1DJ-2RIC-NXAP",
                INVITE_SALT,
                INVITE_HASH
            )
            .unwrap()
        );
    }

    #[test]
    fn refresh_token_hash_matches_pinned_vector() {
        assert_eq!(
            hash_refresh_token("test-refresh-token", "test-pepper"),
            REFRESH_HASH
        );
    }

    #[test]
    fn verifies_existing_password_hashes() {
        for stored in PASSWORD_HASHES {
            assert!(verify_password(PASSWORD, stored).unwrap());
            assert!(!verify_password("wrong horse battery", stored).unwrap());
        }
    }

    #[test]
    fn new_password_hashes_round_trip() {
        let stored = hash_password(PASSWORD).unwrap();
        assert!(stored.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(verify_password(PASSWORD, &stored).unwrap());
    }

    #[test]
    fn rejects_short_passwords() {
        assert!(matches!(
            hash_password("too-short"),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn decodes_pinned_access_token() {
        let account_id = decode_access_token(&test_config(), ACCESS_TOKEN).unwrap();
        assert_eq!(account_id.to_string(), PINNED_ACCOUNT_ID);

        let decoded = decode::<AccessClaims>(
            ACCESS_TOKEN,
            &DecodingKey::from_secret(ACCESS_SECRET.as_bytes()),
            &Validation::default(),
        )
        .unwrap();
        let expected = pinned_claims();
        assert_eq!(decoded.header.alg, jsonwebtoken::Algorithm::HS256);
        assert_eq!(decoded.claims.email, expected.email);
        assert_eq!(decoded.claims.role, expected.role);
        assert_eq!(decoded.claims.exp, expected.exp);
        assert_eq!(decoded.claims.iat, expected.iat);
    }

    #[test]
    fn encodes_pinned_access_token() {
        let token = encode(
            &Header::default(),
            &pinned_claims(),
            &EncodingKey::from_secret(ACCESS_SECRET.as_bytes()),
        )
        .unwrap();
        assert_eq!(token, ACCESS_TOKEN);
    }

    #[test]
    fn issued_access_tokens_use_hs256() {
        let config = test_config();
        let account_id = Uuid::new_v4();
        let (token, _) =
            create_access_token(&config, account_id, "player@example.com", "admin").unwrap();
        let header = jsonwebtoken::decode_header(&token).unwrap();
        assert_eq!(header.alg, jsonwebtoken::Algorithm::HS256);
        assert_eq!(decode_access_token(&config, &token).unwrap(), account_id);
    }

    #[test]
    fn rejects_tokens_signed_with_another_secret() {
        let mut config = test_config();
        config.access_token_secret = "another-secret".into();
        assert!(matches!(
            decode_access_token(&config, ACCESS_TOKEN),
            Err(AppError::Unauthorized(_))
        ));
    }

    #[test]
    fn generates_distinct_refresh_tokens() {
        let first = generate_refresh_token().unwrap();
        assert_eq!(first.len(), 43);
        assert_ne!(first, generate_refresh_token().unwrap());
    }
}
