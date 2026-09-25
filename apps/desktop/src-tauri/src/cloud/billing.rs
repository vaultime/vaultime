// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Billing — fetch subscription state, create Stripe checkout/portal URLs.
//!
//! The desktop app cannot host webhooks, so Stripe events flow through
//! Supabase Edge Functions that update a `subscriptions` table.  This
//! module reads that table and calls the Edge Functions to obtain
//! checkout and customer-portal URLs which the app opens in the system
//! browser.

use log::warn;

use crate::error::{Result, VaultimeError};

use super::auth::AuthManager;
use super::config;
use super::types::{
    EdgeFunctionUrlResponse, Subscription, SubscriptionStatus, SubscriptionTier,
    SupabaseSubscriptionRow,
};

/// Fetch the current user's subscription from the Supabase `subscriptions`
/// table.  Returns a free-tier default when no row exists.
pub async fn get_subscription(auth: &AuthManager) -> Result<Subscription> {
    if !config::is_billing_enabled() {
        // Billing not configured — grant premium access to all authed users.
        return Ok(Subscription {
            tier: SubscriptionTier::Pro,
            status: SubscriptionStatus::Active,
            current_period_end: None,
            cancel_at_period_end: false,
        });
    }

    let access_token = auth
        .access_token()
        .ok_or_else(|| VaultimeError::Cloud("not signed in".into()))?;

    let url = format!(
        "{}/rest/v1/subscriptions?select=tier,status,current_period_end,cancel_at_period_end&limit=1",
        config::supabase_url(),
    );

    let http = reqwest::Client::new();
    let resp = http
        .get(&url)
        .header("apikey", config::supabase_anon_key())
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| VaultimeError::Cloud(format!("subscription fetch failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        warn!("subscription query error: {status} {text}");
        return Ok(Subscription::free_default());
    }

    let rows: Vec<SupabaseSubscriptionRow> = resp
        .json()
        .await
        .map_err(|e| VaultimeError::Cloud(format!("invalid subscription response: {e}")))?;

    let Some(row) = rows.into_iter().next() else {
        return Ok(Subscription::free_default());
    };

    Ok(Subscription {
        tier: parse_tier(row.tier.as_deref()),
        status: parse_status(row.status.as_deref()),
        current_period_end: row.current_period_end,
        cancel_at_period_end: row.cancel_at_period_end.unwrap_or(false),
    })
}

/// Call the `create-checkout-session` Edge Function and return a URL that
/// the app can open in the system browser.
pub async fn create_checkout_url(auth: &AuthManager) -> Result<String> {
    call_edge_function(auth, "create-checkout-session").await
}

/// Call the `create-portal-session` Edge Function and return a URL for the
/// Stripe Customer Portal.
pub async fn create_portal_url(auth: &AuthManager) -> Result<String> {
    call_edge_function(auth, "create-portal-session").await
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn call_edge_function(auth: &AuthManager, function_name: &str) -> Result<String> {
    let access_token = auth
        .access_token()
        .ok_or_else(|| VaultimeError::Cloud("not signed in".into()))?;

    let url = format!(
        "{}/functions/v1/{function_name}",
        config::supabase_url(),
    );

    let http = reqwest::Client::new();
    let resp = http
        .post(&url)
        .header("apikey", config::supabase_anon_key())
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .map_err(|e| VaultimeError::Cloud(format!("{function_name} request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        warn!("{function_name} error: {status} {text}");
        return Err(VaultimeError::Cloud(format!(
            "{function_name} failed: HTTP {status}"
        )));
    }

    let body: EdgeFunctionUrlResponse = resp
        .json()
        .await
        .map_err(|e| VaultimeError::Cloud(format!("invalid {function_name} response: {e}")))?;

    if let Some(error) = body.error {
        return Err(VaultimeError::Cloud(error));
    }

    body.url.ok_or_else(|| {
        VaultimeError::Cloud(format!("{function_name} returned no URL"))
    })
}

fn parse_tier(value: Option<&str>) -> SubscriptionTier {
    match value {
        Some("pro") => SubscriptionTier::Pro,
        _ => SubscriptionTier::Free,
    }
}

fn parse_status(value: Option<&str>) -> SubscriptionStatus {
    match value {
        Some("active") => SubscriptionStatus::Active,
        Some("past_due") => SubscriptionStatus::PastDue,
        Some("canceled") => SubscriptionStatus::Canceled,
        Some("expired") => SubscriptionStatus::Expired,
        _ => SubscriptionStatus::None,
    }
}
