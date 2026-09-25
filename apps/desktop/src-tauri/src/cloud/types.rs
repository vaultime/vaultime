// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Cloud-related types shared between auth, sync, and the frontend.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

/// Credentials submitted by the user for sign-up or sign-in.
#[derive(Debug, Deserialize)]
pub struct AuthCredentials {
    pub email: String,
    pub password: String,
}

/// The authenticated user profile returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudUser {
    pub id: String,
    pub email: String,
    pub created_at: Option<String>,
}

/// Tokens returned by a successful Supabase auth exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

/// Full session state visible to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct CloudSession {
    pub user: CloudUser,
    pub device_registered: bool,
    pub expires_at: i64,
}

// ---------------------------------------------------------------------------
// Supabase auth API shapes (deserialized from JSON responses)
// ---------------------------------------------------------------------------

/// Shape of the Supabase `/auth/v1/token` and `/auth/v1/signup` response.
///
/// When email confirmation is enabled, sign-up returns a user object
/// without tokens — `access_token` and `refresh_token` will be `None`.
#[derive(Debug, Deserialize)]
pub struct SupabaseAuthResponse {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub expires_in: Option<i64>,
    pub user: Option<SupabaseUser>,
    /// Present on sign-up when the user object is at the top level
    /// (no wrapping token response).
    pub id: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SupabaseUser {
    pub id: String,
    pub email: Option<String>,
    pub created_at: Option<String>,
}

/// Error body returned by Supabase auth endpoints.
#[derive(Debug, Deserialize)]
pub struct SupabaseAuthError {
    #[serde(alias = "msg")]
    pub message: Option<String>,
    pub error_description: Option<String>,
}

// ---------------------------------------------------------------------------
// Sync contract (defined now, implemented in Milestone 11)
// ---------------------------------------------------------------------------

/// Metadata about a cloud-registered device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudDevice {
    pub id: String,
    pub user_id: String,
    pub device_name: String,
    pub platform: String,
    pub app_version: String,
    pub registered_at: String,
    pub last_sync_at: Option<String>,
}

/// High-level sync status reported to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct SyncStatus {
    pub connected: bool,
    pub last_sync_at: Option<String>,
    pub pending_events: u64,
}
