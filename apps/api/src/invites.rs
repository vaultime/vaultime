// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use argon2::password_hash::rand_core::{OsRng, RngCore};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};

use crate::auth::hash_invite_code;
use crate::error::{AppError, AppResult};

const DEFAULT_PREFIX: &str = "VTLINV";

#[derive(Debug, Clone)]
pub struct GeneratedInvite {
    pub code: String,
    pub lookup_key: String,
    pub salt: String,
    pub code_hash: String,
    pub max_redemptions: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub fn generate_invite(
    prefix: Option<&str>,
    max_redemptions: i32,
    expires_at: Option<DateTime<Utc>>,
    note: Option<String>,
) -> AppResult<GeneratedInvite> {
    if max_redemptions <= 0 {
        return Err(AppError::bad_request(
            "max_redemptions must be a positive integer",
        ));
    }

    let prefix = normalize_prefix(prefix)?;
    let body = generate_body_token();
    let code = format!("{prefix}-{}", chunk_token(&body));
    let lookup_key = body[..12].to_string();
    let salt = random_hex(16);
    let code_hash = hash_invite_code(&code, &salt)?;
    let note = note.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    Ok(GeneratedInvite {
        code,
        lookup_key,
        salt,
        code_hash,
        max_redemptions,
        expires_at,
        note,
        created_at: Utc::now(),
    })
}

fn normalize_prefix(raw: Option<&str>) -> AppResult<String> {
    let value = raw.unwrap_or(DEFAULT_PREFIX).trim().to_ascii_uppercase();
    if value.is_empty() {
        return Err(AppError::bad_request("invite prefix cannot be empty"));
    }
    if !value.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(AppError::bad_request(
            "invite prefix must be alphanumeric only",
        ));
    }

    Ok(value)
}

fn generate_body_token() -> String {
    loop {
        let mut bytes = [0_u8; 18];
        OsRng.fill_bytes(&mut bytes);
        let token = URL_SAFE_NO_PAD
            .encode(bytes)
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .map(|ch| ch.to_ascii_uppercase())
            .collect::<String>();

        if token.len() >= 24 {
            return token[..24].to_string();
        }
    }
}

fn chunk_token(token: &str) -> String {
    token
        .as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("-")
}

fn random_hex(len: usize) -> String {
    let mut bytes = vec![0_u8; len];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::generate_invite;

    #[test]
    fn generates_default_invites() {
        let invite = generate_invite(None, 1, None, None).unwrap();
        assert!(invite.code.starts_with("VTLINV-"));
        assert_eq!(invite.lookup_key.len(), 12);
        assert_eq!(invite.salt.len(), 32);
        assert_eq!(invite.code_hash.len(), 128);
    }
}
