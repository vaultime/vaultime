// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Cleanup that runs on a timer, so an account that never uploads again still
//! loses its stale uploads and unused artwork.

use std::time::Duration;

use uuid::Uuid;

use super::backups::prune_stale_pending_backups;
use super::blobs::collect_unreferenced_blobs;
use crate::AppState;
use crate::constants::{BETA_APPLICATION_RETENTION_DAYS, MAINTENANCE_INTERVAL_SECS};
use crate::error::AppResult;

pub fn spawn_maintenance(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(MAINTENANCE_INTERVAL_SECS));
        loop {
            interval.tick().await;
            if let Err(error) = run(&state).await {
                tracing::error!(error = %error, "maintenance failed");
            }
        }
    });
}

async fn run(state: &AppState) -> AppResult<()> {
    let accounts = sqlx::query_scalar::<_, Uuid>(
        "SELECT account_id FROM cloud_backups WHERE status = 'pending'
         UNION
         SELECT account_id FROM cloud_blobs",
    )
    .fetch_all(&state.db)
    .await?;
    for account_id in accounts {
        prune_stale_pending_backups(state, account_id).await?;
        collect_unreferenced_blobs(state, account_id).await?;
    }

    // Revoked tokens stay until they expire, so a copy that comes back is recognized.
    sqlx::query("DELETE FROM cloud_refresh_tokens WHERE expires_at < NOW()")
        .execute(&state.db)
        .await?;
    sqlx::query(
        "DELETE FROM beta_applications WHERE created_at < NOW() - make_interval(days => $1)",
    )
    .bind(BETA_APPLICATION_RETENTION_DAYS)
    .execute(&state.db)
    .await?;
    Ok(())
}
