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

/// Returns `true` when the cloud layer has real credentials configured.
pub fn is_cloud_configured() -> bool {
    option_env!("VAULTIME_SUPABASE_URL").is_some()
        && option_env!("VAULTIME_SUPABASE_ANON_KEY").is_some()
}
