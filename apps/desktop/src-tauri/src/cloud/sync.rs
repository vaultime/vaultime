// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Sync client — uploads local session events to Supabase and tracks progress.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use log::{info, warn};
use serde::Deserialize;

use crate::db::connection::Database;
use crate::db::repo::session_events::{self, SyncAck};
use crate::db::repo::{sessions, settings};
use crate::error::{Result, VaultimeError};

use super::auth::AuthManager;
use super::config;

/// Maximum number of events to upload in a single sync batch.
const SYNC_BATCH_SIZE: u32 = 100;

/// Result of a single sync pass.
#[derive(Debug, serde::Serialize)]
pub struct SyncResult {
    pub uploaded: usize,
    pub remaining: u64,
    pub verified_sessions: usize,
    pub conflicted_events: usize,
    pub last_sync_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SyncResponseRow {
    id: String,
    session_id: String,
    hash_self: Option<String>,
    server_received_at: Option<String>,
}

/// Run one sync pass: query unsynced events, upload them, mark as synced.
///
/// Returns the number of events uploaded and how many remain.
#[allow(clippy::too_many_lines)]
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
            verified_sessions: 0,
            conflicted_events: 0,
            last_sync_at: settings::get_setting(db, "cloud_last_sync_at")?
                .filter(|value| !value.is_empty()),
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
                "payload_json": serde_json::from_str::<serde_json::Value>(&e.payload_json)
                    .unwrap_or_else(|_| serde_json::json!({ "_raw": e.payload_json })),
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
        .header(
            "Prefer",
            "resolution=merge-duplicates,return=representation",
        )
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

    let response_rows: Vec<SyncResponseRow> = resp
        .json()
        .await
        .map_err(|error| VaultimeError::Cloud(format!("invalid sync response: {error}")))?;
    let event_by_id: HashMap<&str, &crate::db::models::SessionEvent> = events
        .iter()
        .map(|event| (event.id.as_str(), event))
        .collect();
    let mut acknowledgements = Vec::new();
    let mut acknowledged_sessions = HashSet::new();
    let mut conflicts = 0usize;
    let mut last_sync_at = None::<String>;

    for row in response_rows {
        let Some(local_event) = event_by_id.get(row.id.as_str()) else {
            continue;
        };

        if row.session_id != local_event.session_id || row.hash_self != local_event.hash_self {
            conflicts += 1;
            continue;
        }

        if row.server_received_at > last_sync_at {
            last_sync_at.clone_from(&row.server_received_at);
        }

        acknowledgements.push(SyncAck {
            event_id: row.id,
            server_ack_at: row.server_received_at,
        });
        acknowledged_sessions.insert(local_event.session_id.clone());
    }

    // 3. Mark uploaded events as synced locally.
    session_events::mark_events_synced(db, &acknowledgements)?;
    let verified_sessions = sessions::refresh_cloud_verified_sessions(
        db,
        &acknowledged_sessions.into_iter().collect::<Vec<_>>(),
    )?;
    let uploaded = acknowledgements.len();

    if let Some(timestamp) = last_sync_at.as_deref() {
        let _ = settings::set_setting(db, "cloud_last_sync_at", timestamp);
    }

    // 4. Check how many remain.
    let remaining = session_events::count_unsynced_events(db)?;

    info!(
        "synced {uploaded} events to cloud, {remaining} remaining, {verified_sessions} verified sessions, {conflicts} conflicts"
    );

    Ok(SyncResult {
        uploaded,
        remaining,
        verified_sessions,
        conflicted_events: conflicts,
        last_sync_at,
    })
}
