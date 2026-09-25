// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Sync client — uploads local session events to Supabase and tracks progress.

use std::sync::Arc;

use log::{info, warn};

use crate::db::connection::Database;
use crate::db::repo::session_events;
use crate::error::{Result, VaultimeError};

use super::auth::AuthManager;
use super::config;

/// Maximum number of events to upload in a single sync batch.
const SYNC_BATCH_SIZE: u32 = 100;

/// Result of a single sync pass.
#[derive(Debug, serde::Serialize)]
pub struct SyncResult {
    pub uploaded: usize,
    pub remaining: usize,
}

/// Run one sync pass: query unsynced events, upload them, mark as synced.
///
/// Returns the number of events uploaded and how many remain.
pub async fn sync_events(
    db: &Arc<Database>,
    auth: &AuthManager,
    device_id: &str,
) -> Result<SyncResult> {
    let access_token = auth
        .access_token()
        .ok_or_else(|| VaultimeError::Cloud("not signed in — cannot sync".into()))?;

    // 1. Query unsynced events.
    let events = session_events::list_unsynced_events(db, SYNC_BATCH_SIZE)?;

    if events.is_empty() {
        return Ok(SyncResult {
            uploaded: 0,
            remaining: 0,
        });
    }

    // 2. Build the Supabase insert payload.
    let rows: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "device_id": device_id,
                "session_id": e.session_id,
                "sequence": e.sequence,
                "event_type": e.event_type,
                "event_time_wall": e.event_time_wall,
                "payload_json": e.payload_json,
                "hash_self": e.hash_self,
            })
        })
        .collect();

    let url = format!("{}/rest/v1/cloud_sync_events", config::supabase_url());

    let http = reqwest::Client::new();
    let resp = http
        .post(&url)
        .header("apikey", config::supabase_anon_key())
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .header("Prefer", "resolution=merge-duplicates")
        .json(&rows)
        .send()
        .await
        .map_err(|e| VaultimeError::Cloud(format!("sync upload failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        warn!("sync upload error: {status} {text}");
        return Err(VaultimeError::Cloud(format!(
            "sync upload failed: HTTP {status}"
        )));
    }

    // 3. Mark uploaded events as synced locally.
    let synced_ids: Vec<String> = events.iter().map(|e| e.id.clone()).collect();
    let count = synced_ids.len();
    session_events::mark_events_synced(db, &synced_ids)?;

    // 4. Check how many remain.
    let remaining = session_events::list_unsynced_events(db, 1)?;

    info!("synced {count} events to cloud, {} remaining", remaining.len());

    Ok(SyncResult {
        uploaded: count,
        remaining: remaining.len(),
    })
}
