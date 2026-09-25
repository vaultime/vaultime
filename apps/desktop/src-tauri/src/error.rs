// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Structured error types for Vaultime.

use std::fmt;

/// Top-level error type for Vaultime operations.
#[derive(Debug)]
pub enum VaultimeError {
    /// Database-related errors.
    Database(String),
    /// Tracking engine errors.
    Tracking(String),
    /// Integrity validation errors.
    Integrity(String),
    /// Asset/image pipeline errors.
    Asset(String),
    /// Platform-specific errors.
    Platform(String),
    /// Cloud sync errors.
    Cloud(String),
}

impl fmt::Display for VaultimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(msg) => write!(f, "database error: {msg}"),
            Self::Tracking(msg) => write!(f, "tracking error: {msg}"),
            Self::Integrity(msg) => write!(f, "integrity error: {msg}"),
            Self::Asset(msg) => write!(f, "asset error: {msg}"),
            Self::Platform(msg) => write!(f, "platform error: {msg}"),
            Self::Cloud(msg) => write!(f, "cloud error: {msg}"),
        }
    }
}

impl std::error::Error for VaultimeError {}

impl serde::Serialize for VaultimeError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Convenience type alias.
pub type Result<T> = std::result::Result<T, VaultimeError>;
