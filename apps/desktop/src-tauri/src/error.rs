// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Error type of the desktop core. It reaches the frontend as its message.

use std::fmt;

#[derive(Debug)]
pub enum VaultimeError {
    Database(String),
    /// Local and remote backups, including archive and encryption errors.
    Backup(String),
    Tracking(String),
    Integrity(String),
    Asset(String),
    /// Secure storage and the cloud backup API.
    Cloud(String),
    /// Input the core refuses, with a message for the user.
    Invalid(String),
}

impl fmt::Display for VaultimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(msg) => write!(f, "database error: {msg}"),
            Self::Backup(msg) => write!(f, "backup error: {msg}"),
            Self::Tracking(msg) => write!(f, "tracking error: {msg}"),
            Self::Integrity(msg) => write!(f, "integrity error: {msg}"),
            Self::Asset(msg) => write!(f, "asset error: {msg}"),
            Self::Cloud(msg) => write!(f, "cloud error: {msg}"),
            Self::Invalid(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for VaultimeError {}

impl VaultimeError {
    /// The message for the page: without the category the logs carry, and
    /// starting with a capital letter.
    fn user_message(&self) -> String {
        let (Self::Database(message)
        | Self::Backup(message)
        | Self::Tracking(message)
        | Self::Integrity(message)
        | Self::Asset(message)
        | Self::Cloud(message)
        | Self::Invalid(message)) = self;
        let mut chars = message.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(chars).collect()
        })
    }
}

impl serde::Serialize for VaultimeError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.user_message())
    }
}

pub type Result<T> = std::result::Result<T, VaultimeError>;
