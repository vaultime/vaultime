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
// Billing / Subscription (Milestone 12)
// ---------------------------------------------------------------------------

/// The subscription tier a user is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionTier {
    Free,
    Pro,
}

/// Current status of the user's subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    None,
    Active,
    PastDue,
    Canceled,
    Expired,
}

/// Subscription state returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub tier: SubscriptionTier,
    pub status: SubscriptionStatus,
    pub current_period_end: Option<String>,
    pub cancel_at_period_end: bool,
}

impl Subscription {
    /// Returns a default free-tier subscription for users with no record.
    pub fn free_default() -> Self {
        Self {
            tier: SubscriptionTier::Free,
            status: SubscriptionStatus::None,
            current_period_end: None,
            cancel_at_period_end: false,
        }
    }

    /// Whether the subscription grants access to premium features.
    pub fn has_premium_access(&self) -> bool {
        self.tier == SubscriptionTier::Pro
            && matches!(
                self.status,
                SubscriptionStatus::Active | SubscriptionStatus::PastDue
            )
    }
}

/// Response from the checkout / portal Edge Functions.
#[derive(Debug, Deserialize)]
pub struct EdgeFunctionUrlResponse {
    pub url: Option<String>,
    pub error: Option<String>,
}

/// Row shape returned by querying the Supabase `subscriptions` table.
#[derive(Debug, Deserialize)]
pub struct SupabaseSubscriptionRow {
    pub tier: Option<String>,
    pub status: Option<String>,
    pub current_period_end: Option<String>,
    pub cancel_at_period_end: Option<bool>,
}

// ---------------------------------------------------------------------------
// Sync contract
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
    pub last_backup_at: Option<String>,
    pub pending_events: u64,
}

/// Metadata recorded for a cloud backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudBackupRecord {
    pub id: String,
    pub device_id: String,
    pub created_at: String,
    pub checksum: String,
    pub storage_path: String,
    pub size_bytes: Option<i64>,
    pub label: Option<String>,
}

/// Summary of a backup payload without any local filesystem paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudBackupSummary {
    pub backup_id: String,
    pub backup_version: u32,
    pub created_at: String,
    pub app_version: String,
    pub source_device_id: String,
    pub schema_migrations: Vec<String>,
    pub overall_checksum: String,
    pub games_count: i64,
    pub sessions_count: i64,
    pub assets_count: i64,
    pub asset_file_count: usize,
}

/// Result of creating and uploading a cloud backup snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct CloudBackupUploadResult {
    pub backup: CloudBackupRecord,
    pub summary: CloudBackupSummary,
    pub uploaded_files: usize,
}

/// Preflight information shown before restoring a cloud backup.
#[derive(Debug, Clone, Serialize)]
pub struct CloudBackupRestorePreview {
    pub backup: CloudBackupRecord,
    pub summary: CloudBackupSummary,
    pub has_active_sessions: bool,
    pub unsynced_events: u64,
    pub newer_local_sessions: u64,
    pub requires_force: bool,
}

/// Result returned after a cloud backup restore completes locally.
#[derive(Debug, Clone, Serialize)]
pub struct CloudBackupRestoreResult {
    pub backup: CloudBackupRecord,
    pub restart_required: bool,
}
