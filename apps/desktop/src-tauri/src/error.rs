// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Error type of the desktop core. It reaches the frontend as its kind and
//! its message.

use std::fmt;

use serde::ser::SerializeStruct;

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
    /// The cloud server could not be reached at all.
    Offline(String),
    /// What the page asked about is gone, like a game deleted meanwhile.
    NotFound(String),
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
            Self::Offline(msg) => write!(f, "offline: {msg}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::Invalid(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for VaultimeError {}

impl VaultimeError {
    /// What kind of failure it is, so the page can tell them apart. Same as
    /// `CoreErrorKind` in `lib/types.ts`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Database(_) => "database",
            Self::Backup(_) => "backup",
            Self::Tracking(_) => "tracking",
            Self::Integrity(_) => "integrity",
            Self::Asset(_) => "asset",
            Self::Cloud(_) => "cloud",
            Self::Offline(_) => "offline",
            Self::NotFound(_) => "not_found",
            Self::Invalid(_) => "invalid",
        }
    }

    /// The message for the page: without the category the logs carry, and
    /// starting with a capital letter.
    pub fn user_message(&self) -> String {
        let (Self::Database(message)
        | Self::Backup(message)
        | Self::Tracking(message)
        | Self::Integrity(message)
        | Self::Asset(message)
        | Self::Cloud(message)
        | Self::Offline(message)
        | Self::NotFound(message)
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
        let mut error = serializer.serialize_struct("VaultimeError", 2)?;
        error.serialize_field("kind", self.kind())?;
        error.serialize_field("message", &self.user_message())?;
        error.end()
    }
}

pub type Result<T> = std::result::Result<T, VaultimeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_gets_the_kind_and_a_message_without_the_category() {
        let error = VaultimeError::NotFound("this session is gone".into());
        assert_eq!(error.to_string(), "not found: this session is gone");
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::json!({ "kind": "not_found", "message": "This session is gone" })
        );
    }
}
