// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};

use crate::auth::{chunk_code, hash_invite_code, random_bytes};
use crate::constants::{
    INVITE_BODY_LENGTH, INVITE_BODY_RANDOM_BYTES, INVITE_LOOKUP_KEY_LENGTH, INVITE_SALT_BYTES,
};
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
    let body = generate_body_token()?;
    let code = format!("{prefix}-{}", chunk_code(&body));
    let lookup_key = body[..INVITE_LOOKUP_KEY_LENGTH].to_string();
    let salt = hex::encode(random_bytes::<INVITE_SALT_BYTES>()?);
    let code_hash = hash_invite_code(&code, &salt)?;
    let note = note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

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

/// Same scheme as the invite generator scripts: base64url of random bytes, alphanumerics only,
/// uppercased and cut to the body length.
fn generate_body_token() -> AppResult<String> {
    loop {
        let token = URL_SAFE_NO_PAD
            .encode(random_bytes::<INVITE_BODY_RANDOM_BYTES>()?)
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|ch| ch.to_ascii_uppercase())
            .collect::<String>();

        if token.len() >= INVITE_BODY_LENGTH {
            return Ok(token[..INVITE_BODY_LENGTH].to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::generate_invite;
    use crate::auth::{ParsedInviteCode, verify_invite_hash};

    #[test]
    fn generates_default_invites() {
        let invite = generate_invite(None, 1, None, None).unwrap();
        assert!(invite.code.starts_with("VTLINV-"));
        assert_eq!(invite.lookup_key.len(), 12);
        assert_eq!(invite.salt.len(), 32);
        assert_eq!(invite.code_hash.len(), 128);
    }

    #[test]
    fn generated_invites_redeem() {
        let invite = generate_invite(Some("beta"), 2, None, Some("  ".into())).unwrap();
        assert!(invite.note.is_none());

        let parsed = ParsedInviteCode::parse(&invite.code.to_lowercase()).unwrap();
        assert_eq!(parsed.normalized_code, invite.code);
        assert_eq!(parsed.lookup_key, invite.lookup_key);
        assert!(
            verify_invite_hash(&parsed.normalized_code, &invite.salt, &invite.code_hash).unwrap()
        );
    }

    #[test]
    fn rejects_invalid_invite_settings() {
        assert!(generate_invite(None, 0, None, None).is_err());
        assert!(generate_invite(Some("bad-prefix"), 1, None, None).is_err());
    }
}
