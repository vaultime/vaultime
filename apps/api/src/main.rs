// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

mod auth;
mod config;
mod constants;
mod error;
mod invites;
mod limits;
mod models;
mod routes;

use std::sync::Arc;

use config::Config;
use constants::DB_MAX_CONNECTIONS;
use limits::Limits;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub limits: Arc<Limits>,
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
        .max_connections(DB_MAX_CONNECTIONS)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!().run(&db).await?;

    let state = AppState {
        config: Arc::clone(&config),
        db,
        limits: Arc::new(Limits::new()),
    };
    routes::spawn_maintenance(state.clone());

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

#[cfg(test)]
mod tests {
    /// Checksums the live database recorded for each migration. sqlx refuses
    /// to start once a shipped migration changes, line endings included.
    const APPLIED: &[(i64, &str)] = &[
        (
            1,
            "e92b3daac42b4fe2baa7a858e9ce1b332068fcc7850eea3c171deead15c8f0be073c4a3a538176dd732707d13216eb83",
        ),
        (
            2,
            "674f0f38eec20b32c67f5f1cb7b8a2aa4af007d5ccae79c64568ef6a9c7b8633435e95201305fa7891a19adaa8c6532d",
        ),
        (
            3,
            "2135971a75776eaeeaf9c191428d840d11d552c609e48ad8b3813f6076af35d683c197d5790d59a67093c33d6492ca55",
        ),
        (
            4,
            "e390d4a743d6ffe0257bfb2aa11992788ab85f4580462c63f93249b2ab4b3d8ee60f7d39e9e7b3955cfbc9e5f5992c68",
        ),
        (
            5,
            "08520b6eba886012668100ec200a50dae20008915215da28bc9c1cb7747a6239c0b10a1447d0f33db883978782dfc366",
        ),
    ];

    #[test]
    fn shipped_migrations_keep_their_bytes() {
        let migrator = sqlx::migrate!();
        for (version, checksum) in APPLIED {
            let migration = migrator
                .iter()
                .find(|migration| migration.version == *version)
                .expect("the migration exists");
            assert_eq!(
                hex::encode(&migration.checksum),
                *checksum,
                "migration {version} changed, the live database would refuse it"
            );
        }
    }
}
