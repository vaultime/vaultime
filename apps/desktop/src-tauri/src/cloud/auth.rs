// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Supabase auth client — sign-up, sign-in, token refresh, sign-out.
//!
//! Tokens are persisted to a JSON file inside the app data directory so the
//! user stays signed in across restarts.  A future iteration can move this
//! to OS secure storage (keychain / credential manager).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{info, warn};

use crate::error::{Result, VaultimeError};

use super::config;
use super::types::{
    AuthTokens, CloudSession, CloudUser, SupabaseAuthError, SupabaseAuthResponse,
};

// ---------------------------------------------------------------------------
// Token file helpers
// ---------------------------------------------------------------------------

fn token_path(app_dir: &Path) -> PathBuf {
    app_dir.join("cloud_tokens.json")
}

fn read_stored_tokens(app_dir: &Path) -> Option<AuthTokens> {
    let path = token_path(app_dir);
    let data = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_stored_tokens(app_dir: &Path, tokens: &AuthTokens) -> Result<()> {
    let path = token_path(app_dir);
    let json = serde_json::to_string_pretty(tokens)
        .map_err(|e| VaultimeError::Cloud(format!("failed to serialize tokens: {e}")))?;
    fs::write(&path, json)
        .map_err(|e| VaultimeError::Cloud(format!("failed to write tokens: {e}")))?;
    Ok(())
}

fn remove_stored_tokens(app_dir: &Path) {
    let path = token_path(app_dir);
    let _ = fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// AuthManager — managed Tauri state
// ---------------------------------------------------------------------------

/// Holds the current cloud session and an HTTP client.
pub struct AuthManager {
    app_dir: PathBuf,
    http: reqwest::Client,
    session: Mutex<Option<CloudSession>>,
}

impl AuthManager {
    /// Create a new `AuthManager` and attempt to restore a persisted session.
    pub fn new(app_dir: PathBuf) -> Self {
        let http = reqwest::Client::new();
        let session = Mutex::new(None);
        let manager = Self { app_dir, http, session };

        // Try to restore from disk.
        if let Some(tokens) = read_stored_tokens(&manager.app_dir) {
            info!("found stored cloud tokens, will validate on first use");
            // We store a minimal session so the frontend knows we have tokens.
            // Actual validation happens when the user navigates to the Cloud page.
            let user = CloudUser {
                id: String::new(),
                email: String::new(),
                created_at: None,
            };
            *manager.session.lock().unwrap() = Some(CloudSession {
                user,
                device_registered: false,
                expires_at: tokens.expires_at,
            });
        }

        manager
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Sign up with email + password.
    pub async fn sign_up(&self, email: &str, password: &str) -> Result<CloudSession> {
        let url = format!("{}/auth/v1/signup", config::supabase_url());
        let body = serde_json::json!({ "email": email, "password": password });

        let resp = self
            .http
            .post(&url)
            .header("apikey", config::supabase_anon_key())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| VaultimeError::Cloud(format!("sign-up request failed: {e}")))?;

        self.handle_auth_response(resp).await
    }

    /// Sign in with email + password.
    pub async fn sign_in(&self, email: &str, password: &str) -> Result<CloudSession> {
        let url = format!(
            "{}/auth/v1/token?grant_type=password",
            config::supabase_url()
        );
        let body = serde_json::json!({ "email": email, "password": password });

        let resp = self
            .http
            .post(&url)
            .header("apikey", config::supabase_anon_key())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| VaultimeError::Cloud(format!("sign-in request failed: {e}")))?;

        self.handle_auth_response(resp).await
    }

    /// Refresh the current access token using the stored refresh token.
    pub async fn refresh_token(&self) -> Result<CloudSession> {
        let tokens = read_stored_tokens(&self.app_dir)
            .ok_or_else(|| VaultimeError::Cloud("no stored tokens to refresh".into()))?;

        let url = format!(
            "{}/auth/v1/token?grant_type=refresh_token",
            config::supabase_url()
        );
        let body = serde_json::json!({ "refresh_token": tokens.refresh_token });

        let resp = self
            .http
            .post(&url)
            .header("apikey", config::supabase_anon_key())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| VaultimeError::Cloud(format!("token refresh failed: {e}")))?;

        self.handle_auth_response(resp).await
    }

    /// Sign out — clears local tokens and revokes the session server-side.
    pub async fn sign_out(&self) -> Result<()> {
        if let Some(tokens) = read_stored_tokens(&self.app_dir) {
            let url = format!("{}/auth/v1/logout", config::supabase_url());
            let _ = self
                .http
                .post(&url)
                .header("apikey", config::supabase_anon_key())
                .header("Authorization", format!("Bearer {}", tokens.access_token))
                .send()
                .await;
        }

        remove_stored_tokens(&self.app_dir);
        *self.session.lock().unwrap() = None;
        info!("signed out of cloud");
        Ok(())
    }

    /// Get the current session if one exists, or `None`.
    pub fn current_session(&self) -> Option<CloudSession> {
        self.session.lock().unwrap().clone()
    }

    /// Returns `true` when the cloud layer has real Supabase credentials.
    pub fn is_configured(&self) -> bool {
        config::is_cloud_configured()
    }

    /// Returns the stored access token, if any.
    pub fn access_token(&self) -> Option<String> {
        read_stored_tokens(&self.app_dir).map(|t| t.access_token)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    async fn handle_auth_response(
        &self,
        resp: reqwest::Response,
    ) -> Result<CloudSession> {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| VaultimeError::Cloud(format!("failed to read response: {e}")))?;

        if !status.is_success() {
            let detail = serde_json::from_str::<SupabaseAuthError>(&body)
                .ok()
                .and_then(|e| e.message.or(e.error_description))
                .unwrap_or_else(|| format!("HTTP {status}"));
            warn!("cloud auth error: {detail}");
            return Err(VaultimeError::Cloud(detail));
        }

        let auth: SupabaseAuthResponse = serde_json::from_str(&body)
            .map_err(|e| VaultimeError::Cloud(format!("invalid auth response: {e}")))?;

        let expires_at = auth
            .expires_at
            .or_else(|| {
                auth.expires_in.map(|secs| {
                    chrono::Utc::now().timestamp() + secs
                })
            })
            .unwrap_or(0);

        let tokens = AuthTokens {
            access_token: auth.access_token,
            refresh_token: auth.refresh_token,
            expires_at,
        };

        write_stored_tokens(&self.app_dir, &tokens)?;

        let user = CloudUser {
            id: auth.user.id,
            email: auth.user.email.unwrap_or_default(),
            created_at: auth.user.created_at,
        };

        let session = CloudSession {
            user,
            device_registered: false,
            expires_at,
        };

        *self.session.lock().unwrap() = Some(session.clone());
        info!("cloud auth succeeded");

        Ok(session)
    }
}
