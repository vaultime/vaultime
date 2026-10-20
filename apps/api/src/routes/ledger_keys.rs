// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The public keys each PC's ledger signs with. The app registers its own
//! with every upload, and a PC that merges another one's backup counts that
//! PC's keys only when they are here. A registered key stays.

use std::collections::BTreeSet;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::AppState;
use crate::auth::AuthenticatedAccount;
use crate::constants::{
    LEDGER_KEY_HEX_CHARS, MAX_DEVICE_FIELD_CHARS, MAX_DEVICES_PER_ACCOUNT, MAX_LEDGER_KEYS_PER_PC,
};
use crate::error::{AppError, AppResult};
use crate::models::{LedgerKeysResponse, RegisterLedgerKeysRequest};

pub async fn register(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    Json(payload): Json<RegisterLedgerKeysRequest>,
) -> AppResult<StatusCode> {
    let pc = payload.ledger_device_id.trim();
    if pc.is_empty() || pc.chars().count() > MAX_DEVICE_FIELD_CHARS {
        return Err(AppError::bad_request(
            "ledger_device_id is missing or too long",
        ));
    }
    let keys: BTreeSet<String> = payload
        .public_keys
        .iter()
        .map(|key| key.trim().to_ascii_lowercase())
        .collect();
    if keys.is_empty() || keys.iter().any(|key| !is_public_key(key)) {
        return Err(AppError::bad_request(
            "public_keys must hold Ed25519 public keys as hex",
        ));
    }
    let keys: Vec<String> = keys.into_iter().collect();

    // One registration at a time per account, so the limits below hold.
    let _upload = state.limits.uploads.lock(auth.account_id).await?;
    let (known_pc, pcs, kept, already) = sqlx::query_as::<_, (bool, i64, i64, i64)>(
        "SELECT
             EXISTS(SELECT 1 FROM cloud_ledger_keys WHERE account_id = $1 AND ledger_device_id = $2),
             (SELECT COUNT(DISTINCT ledger_device_id) FROM cloud_ledger_keys WHERE account_id = $1),
             (SELECT COUNT(*) FROM cloud_ledger_keys WHERE account_id = $1 AND ledger_device_id = $2),
             (SELECT COUNT(*) FROM cloud_ledger_keys
              WHERE account_id = $1 AND ledger_device_id = $2 AND public_key = ANY($3))",
    )
    .bind(auth.account_id)
    .bind(pc)
    .bind(&keys)
    .fetch_one(&state.db)
    .await?;
    if !known_pc && pcs >= MAX_DEVICES_PER_ACCOUNT {
        return Err(AppError::conflict(format!(
            "an account can register keys for up to {MAX_DEVICES_PER_ACCOUNT} PCs"
        )));
    }
    let new = i64::try_from(keys.len()).unwrap_or(i64::MAX) - already;
    if kept + new > MAX_LEDGER_KEYS_PER_PC {
        return Err(AppError::conflict(format!(
            "a PC can register up to {MAX_LEDGER_KEYS_PER_PC} keys"
        )));
    }

    sqlx::query(
        "INSERT INTO cloud_ledger_keys (account_id, ledger_device_id, public_key)
         SELECT $1, $2, key FROM UNNEST($3::TEXT[]) AS key
         ON CONFLICT DO NOTHING",
    )
    .bind(auth.account_id)
    .bind(pc)
    .bind(&keys)
    .execute(&state.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Every PC of the account with the keys it registered, the oldest first.
pub async fn list(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<LedgerKeysResponse>>> {
    let rows = sqlx::query_as::<_, LedgerKeysResponse>(
        "SELECT ledger_device_id,
                ARRAY_AGG(public_key ORDER BY first_seen_at, public_key) AS public_keys
         FROM cloud_ledger_keys
         WHERE account_id = $1
         GROUP BY ledger_device_id
         ORDER BY ledger_device_id",
    )
    .bind(auth.account_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

fn is_public_key(key: &str) -> bool {
    key.len() == LEDGER_KEY_HEX_CHARS && key.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_ed25519_public_keys_as_hex_only() {
        assert!(is_public_key(&"ab".repeat(32)));
        assert!(!is_public_key(&"ab".repeat(31)));
        assert!(!is_public_key(&"zz".repeat(32)));
        assert!(!is_public_key(""));
    }
}
