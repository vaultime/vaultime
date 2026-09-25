// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Cloud configuration — Supabase project URL and anon key.
//!
//! In production these come from compile-time env vars so secrets are not
//! checked into the repository.  During development you can set them in a
//! `.env` file or export them in the shell before running `cargo tauri dev`.

/// Supabase project URL (e.g. `https://<project>.supabase.co`).
pub fn supabase_url() -> String {
    option_env!("VAULTIME_SUPABASE_URL")
        .unwrap_or("https://PLACEHOLDER.supabase.co")
        .to_string()
}

/// Supabase anonymous (public) API key.
pub fn supabase_anon_key() -> String {
    option_env!("VAULTIME_SUPABASE_ANON_KEY")
        .unwrap_or("PLACEHOLDER")
        .to_string()
}

/// Supabase Storage bucket used for private cloud backups.
pub fn supabase_backup_bucket() -> String {
    option_env!("VAULTIME_SUPABASE_BACKUP_BUCKET")
        .unwrap_or("vaultime-backups")
        .to_string()
}

/// Returns `true` when the cloud layer has real credentials configured.
pub fn is_cloud_configured() -> bool {
    option_env!("VAULTIME_SUPABASE_URL").is_some()
        && option_env!("VAULTIME_SUPABASE_ANON_KEY").is_some()
}

/// Returns `true` when the Stripe billing integration is configured.
///
/// When this is `false` the app skips subscription checks and treats
/// every authenticated user as having premium access — useful during
/// development or before the Stripe product is set up.
pub fn is_billing_enabled() -> bool {
    option_env!("VAULTIME_BILLING_ENABLED").is_some_and(|v| v == "1" || v == "true")
}
