// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SignUpRequest {
    pub email: String,
    pub password: String,
    pub invite_code: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub user: AuthUserResponse,
}

#[derive(Debug, Serialize)]
pub struct AuthUserResponse {
    pub id: Uuid,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterDeviceRequest {
    pub client_device_id: String,
    pub device_name: String,
    pub platform: String,
    pub app_version: String,
    pub device_public_key: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct DeviceResponse {
    pub id: Uuid,
    pub client_device_id: String,
    pub device_name: String,
    pub platform: String,
    pub app_version: String,
    pub registered_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBackupRequest {
    pub label: Option<String>,
    pub checksum: String,
    pub backup_created_at: DateTime<Utc>,
    pub metadata_json: Option<Value>,
    pub client_device_id: Option<String>,
    /// Artwork blobs the backup needs. Older clients put artwork into the
    /// backup itself and send none.
    #[serde(default)]
    pub blob_ids: Vec<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BetaApplicationResponse {
    pub id: Uuid,
    pub email: String,
    pub platform: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct MissingBlobsRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MissingBlobsResponse {
    pub missing: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StorageResponse {
    pub backup_bytes: i64,
    pub artwork_bytes: i64,
    pub limit_bytes: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct BackupRecordResponse {
    pub id: Uuid,
    pub label: Option<String>,
    pub storage_key: String,
    pub checksum: String,
    pub size_bytes: i64,
    pub backup_created_at: DateTime<Utc>,
    pub uploaded_at: DateTime<Utc>,
    pub status: String,
    pub client_device_id: Option<String>,
    pub metadata_json: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub public_base_url: String,
}

#[derive(Debug, FromRow)]
pub struct InviteRow {
    pub id: Uuid,
    pub salt: String,
    pub code_hash: String,
    pub max_redemptions: i32,
    pub redeemed_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
pub struct AccountPasswordRow {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub access_state: String,
    pub password_hash: String,
}

#[derive(Debug, FromRow)]
pub struct RefreshTokenAccountRow {
    pub id: Uuid,
    pub account_id: Uuid,
    pub email: String,
    pub role: String,
    pub access_state: String,
}

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub attachment: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateAdminInviteRequest {
    pub prefix: Option<String>,
    pub max_redemptions: Option<i32>,
    pub expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminInviteResponse {
    pub code: String,
    pub lookup_key: String,
    pub salt: String,
    pub code_hash: String,
    pub max_redemptions: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}
