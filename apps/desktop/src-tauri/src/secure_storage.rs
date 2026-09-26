// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use argon2::Argon2;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use keyring::Entry;

use crate::constants::BACKUP_KEY_BYTES;
use crate::error::{Result, VaultimeError};

const KEYRING_SERVICE: &str = "de.codfish.vaultime";
const CLOUD_SESSION_ACCOUNT: &str = "cloud-session";
const CLOUD_BACKUP_KEY_ACCOUNT_PREFIX: &str = "cloud-backup-key:";
const LEGACY_CLOUD_BACKUP_KEY_ACCOUNT: &str = "cloud-backup-key";

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

pub fn has_cloud_backup_key(account_id: &str) -> Result<bool> {
    Ok(load_cloud_backup_key_encoded(account_id)?.is_some())
}

pub fn store_cloud_backup_key(account_id: &str, passphrase: &str) -> Result<()> {
    let key = derive_cloud_backup_key(account_id, passphrase)?;
    let encoded = STANDARD.encode(key);
    cloud_backup_key_entry(account_id)?
        .set_password(&encoded)
        .map_err(map_keyring_error)?;
    let _ = clear_keyring_entry(legacy_cloud_backup_key_entry()?);
    Ok(())
}

pub fn load_cloud_backup_key(account_id: &str) -> Result<[u8; BACKUP_KEY_BYTES]> {
    let encoded = load_cloud_backup_key_encoded(account_id)?.ok_or_else(|| {
        VaultimeError::Cloud(
            "No backup passphrase is configured for this account on this device.".into(),
        )
    })?;
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|error| VaultimeError::Cloud(format!("invalid stored backup key: {error}")))?;

    let bytes: [u8; BACKUP_KEY_BYTES] = decoded
        .try_into()
        .map_err(|_| VaultimeError::Cloud("stored backup key has an unexpected length".into()))?;

    Ok(bytes)
}

pub fn clear_cloud_backup_key(account_id: &str) -> Result<()> {
    clear_keyring_entry(cloud_backup_key_entry(account_id)?)?;
    clear_keyring_entry(legacy_cloud_backup_key_entry()?)?;
    Ok(())
}

fn derive_cloud_backup_key(account_id: &str, passphrase: &str) -> Result<[u8; BACKUP_KEY_BYTES]> {
    let salt = format!("vaultime-cloud-backup-passphrase:{account_id}");
    let mut output = [0_u8; BACKUP_KEY_BYTES];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt.as_bytes(), &mut output)
        .map_err(|error| VaultimeError::Cloud(format!("failed to derive backup key: {error}")))?;
    Ok(output)
}

fn cloud_session_entry() -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, CLOUD_SESSION_ACCOUNT)
        .map_err(|error| VaultimeError::Cloud(format!("failed to access secure storage: {error}")))
}

fn cloud_backup_key_entry(account_id: &str) -> Result<Entry> {
    Entry::new(
        KEYRING_SERVICE,
        &format!("{CLOUD_BACKUP_KEY_ACCOUNT_PREFIX}{account_id}"),
    )
    .map_err(|error| VaultimeError::Cloud(format!("failed to access secure storage: {error}")))
}

fn legacy_cloud_backup_key_entry() -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, LEGACY_CLOUD_BACKUP_KEY_ACCOUNT)
        .map_err(|error| VaultimeError::Cloud(format!("failed to access secure storage: {error}")))
}

fn load_cloud_backup_key_encoded(account_id: &str) -> Result<Option<String>> {
    if let Some(encoded) = load_password(cloud_backup_key_entry(account_id)?)? {
        return Ok(Some(encoded));
    }

    load_password(legacy_cloud_backup_key_entry()?)
}

fn load_password(entry: Entry) -> Result<Option<String>> {
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(map_keyring_error(error)),
    }
}

fn clear_keyring_entry(entry: Entry) -> Result<()> {
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(map_keyring_error(error)),
    }
}

fn map_keyring_error(error: keyring::Error) -> VaultimeError {
    VaultimeError::Cloud(format!("secure storage error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{derive_cloud_backup_key, load_password};
    use keyring::Entry;

    // Writes to the real OS credential store, so it only runs on request:
    // cargo test -- --ignored os_credential_store
    #[test]
    #[ignore = "touches the OS credential store"]
    fn os_credential_store_round_trip() {
        let entry = Entry::new("de.codfish.vaultime.test", "round-trip").unwrap();
        entry.set_password("secret-value").unwrap();

        let reopened = Entry::new("de.codfish.vaultime.test", "round-trip").unwrap();
        assert_eq!(reopened.get_password().unwrap(), "secret-value");

        reopened.delete_credential().unwrap();
        let gone = Entry::new("de.codfish.vaultime.test", "round-trip").unwrap();
        assert_eq!(load_password(gone).unwrap(), None);
    }

    // Existing cloud backups must stay decryptable across crate upgrades.
    #[test]
    fn backup_key_derivation_is_stable() {
        let key = derive_cloud_backup_key("account-1", "correct horse battery staple").unwrap();
        assert_eq!(
            crate::hex::encode(&key),
            "b1b95eb48a327cdd6aeb2d20d560f2798763232685640fbe390ccbf5d8ecdcc9"
        );
    }
}
