// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub public_base_url: String,
    pub database_url: String,
    pub backup_root: PathBuf,
    pub access_token_secret: String,
    pub refresh_token_pepper: String,
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
        })
    }
}

fn env_var(name: &str) -> Result<String, AppError> {
    env::var(name).map_err(|_| AppError::configuration(format!("missing env var {name}")))
}
