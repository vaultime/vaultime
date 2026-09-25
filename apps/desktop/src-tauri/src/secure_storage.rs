// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use argon2::Argon2;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use keyring::Entry;

use crate::error::{Result, VaultimeError};

const KEYRING_SERVICE: &str = "de.codfish.vaultime";
const CLOUD_SESSION_ACCOUNT: &str = "cloud-session";
const CLOUD_BACKUP_KEY_ACCOUNT: &str = "cloud-backup-key";

pub fn store_cloud_session(session_json: &str) -> Result<()> {
    cloud_session_entry()?
        .set_password(session_json)
        .map_err(map_keyring_error)
}

pub fn load_cloud_session() -> Result<Option<String>> {
    match cloud_session_entry()?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(map_keyring_error(error)),
    }
}

pub fn clear_cloud_session() -> Result<()> {
    match cloud_session_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(map_keyring_error(error)),
    }
}

pub fn store_cloud_backup_key(account_id: &str, email: &str, password: &str) -> Result<()> {
    let key = derive_cloud_backup_key(account_id, email, password)?;
    let encoded = STANDARD.encode(key);
    cloud_backup_key_entry()?
        .set_password(&encoded)
        .map_err(map_keyring_error)
}

pub fn load_cloud_backup_key() -> Result<[u8; 32]> {
    let encoded = cloud_backup_key_entry()?
        .get_password()
        .map_err(map_keyring_error)?;
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|error| VaultimeError::Cloud(format!("invalid stored backup key: {error}")))?;

    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| VaultimeError::Cloud("stored backup key has an unexpected length".into()))?;

    Ok(bytes)
}

pub fn clear_cloud_backup_key() -> Result<()> {
    match cloud_backup_key_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(map_keyring_error(error)),
    }
}

fn derive_cloud_backup_key(account_id: &str, email: &str, password: &str) -> Result<[u8; 32]> {
    let normalized_email = email.trim().to_ascii_lowercase();
    let salt = format!("vaultime-cloud-backup:{account_id}:{normalized_email}");
    let mut output = [0_u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt.as_bytes(), &mut output)
        .map_err(|error| VaultimeError::Cloud(format!("failed to derive backup key: {error}")))?;
    Ok(output)
}

fn cloud_session_entry() -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, CLOUD_SESSION_ACCOUNT)
        .map_err(|error| VaultimeError::Cloud(format!("failed to access secure storage: {error}")))
}

fn cloud_backup_key_entry() -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, CLOUD_BACKUP_KEY_ACCOUNT)
        .map_err(|error| VaultimeError::Cloud(format!("failed to access secure storage: {error}")))
}

fn map_keyring_error(error: keyring::Error) -> VaultimeError {
    VaultimeError::Cloud(format!("secure storage error: {error}"))
}
