// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::constants::{
    DEFAULT_MAX_ACCOUNT_BYTES, DEFAULT_MAX_BACKUP_BYTES, DEFAULT_MAX_COMPLETE_BACKUPS_PER_ACCOUNT,
    DEFAULT_MAX_PENDING_BACKUPS_PER_ACCOUNT, DEFAULT_MIN_BACKUP_INTERVAL_SECS,
    DEFAULT_STALE_PENDING_BACKUP_SECS,
};
use crate::error::AppError;

/// Backup limits apply to every account alike. They only exist to stop abuse.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub public_base_url: String,
    pub database_url: String,
    pub backup_root: PathBuf,
    pub access_token_secret: String,
    pub refresh_token_pepper: String,
    pub max_backup_bytes: i64,
    /// Backups and artwork of one account together.
    pub max_account_bytes: i64,
    pub max_pending_backups_per_account: i64,
    /// Older complete backups beyond this count are rotated out after each new upload.
    pub max_complete_backups_per_account: i64,
    pub min_backup_interval_secs: i64,
    pub stale_pending_backup_secs: i64,
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        Ok(Self {
            bind: env_var("VAULTIME_API_BIND")?.parse().map_err(|error| {
                AppError::configuration(format!("invalid VAULTIME_API_BIND value: {error}"))
            })?,
            public_base_url: env_var("VAULTIME_PUBLIC_BASE_URL")?,
            database_url: env_var("VAULTIME_DATABASE_URL")?,
            backup_root: PathBuf::from(env_var("VAULTIME_BACKUP_ROOT")?),
            access_token_secret: env_var("VAULTIME_ACCESS_TOKEN_SECRET")?,
            refresh_token_pepper: env_var("VAULTIME_REFRESH_TOKEN_PEPPER")?,
            max_backup_bytes: env_i64("VAULTIME_MAX_BACKUP_BYTES", DEFAULT_MAX_BACKUP_BYTES, 1)?,
            max_account_bytes: env_i64("VAULTIME_MAX_ACCOUNT_BYTES", DEFAULT_MAX_ACCOUNT_BYTES, 1)?,
            max_pending_backups_per_account: env_i64(
                "VAULTIME_MAX_PENDING_BACKUPS_PER_ACCOUNT",
                DEFAULT_MAX_PENDING_BACKUPS_PER_ACCOUNT,
                1,
            )?,
            max_complete_backups_per_account: env_i64(
                "VAULTIME_MAX_COMPLETE_BACKUPS_PER_ACCOUNT",
                DEFAULT_MAX_COMPLETE_BACKUPS_PER_ACCOUNT,
                1,
            )?,
            min_backup_interval_secs: env_i64(
                "VAULTIME_MIN_BACKUP_INTERVAL_SECONDS",
                DEFAULT_MIN_BACKUP_INTERVAL_SECS,
                0,
            )?,
            stale_pending_backup_secs: env_i64(
                "VAULTIME_STALE_PENDING_BACKUP_SECONDS",
                DEFAULT_STALE_PENDING_BACKUP_SECS,
                0,
            )?,
        })
    }
}

fn env_var(name: &str) -> Result<String, AppError> {
    env::var(name).map_err(|_| AppError::configuration(format!("missing env var {name}")))
}

fn env_i64(name: &str, default: i64, min: i64) -> Result<i64, AppError> {
    let value = match env::var(name) {
        Ok(value) => value
            .parse::<i64>()
            .map_err(|error| AppError::configuration(format!("invalid {name} value: {error}")))?,
        Err(env::VarError::NotPresent) => default,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(AppError::configuration(format!("invalid env var {name}")));
        }
    };

    if value < min {
        return Err(AppError::configuration(format!(
            "{name} must be at least {min}"
        )));
    }

    Ok(value)
}
