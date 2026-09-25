// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

mod auth;
mod config;
mod error;
mod invites;
mod models;
mod routes;

use std::sync::Arc;

use config::Config;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
}

#[tokio::main]
async fn main() -> Result<(), error::AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vaultime_api=info,tower_http=info")),
        )
        .init();

    let config = Arc::new(Config::from_env()?);
    tokio::fs::create_dir_all(&config.backup_root).await?;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    let state = AppState {
        config: Arc::clone(&config),
        db,
    };

    let app = routes::router(state);
    let listener = tokio::net::TcpListener::bind(config.bind).await?;

    info!(
        "vaultime-api listening on {} ({})",
        listener.local_addr()?,
        config.public_base_url
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
