// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

mod auth;
mod config;
mod constants;
mod error;
mod limits;
mod models;
mod routes;

use std::io::IsTerminal;
use std::sync::Arc;

use config::Config;
use constants::DB_MAX_CONNECTIONS;
use error::AppError;
use limits::Limits;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub limits: Arc<Limits>,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vaultime_api=info,tower_http=info")),
        )
        // Color codes only clutter docker logs and journals.
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    let config = Arc::new(Config::from_env()?);

    // Signals count from the start. As PID 1 in a container the server
    // would ignore them otherwise while it waits for its database.
    let mut shutdown = Box::pin(shutdown_signal());
    let (state, listener) = tokio::select! {
        started = start(config) => started?,
        () = &mut shutdown => return Ok(()),
    };
    limits::spawn_pruning(Arc::clone(&state.limits));
    routes::spawn_maintenance(state.clone());

    info!(
        "vaultime-api listening on {}, sign-ups {}",
        listener.local_addr()?,
        if state.config.open_signups {
            "open"
        } else {
            "closed"
        }
    );

    axum::serve(listener, routes::service(state))
        .with_graceful_shutdown(shutdown)
        .await?;

    Ok(())
}

/// Connects to the database, brings its schema up to date and takes the port.
async fn start(config: Arc<Config>) -> Result<(AppState, TcpListener), AppError> {
    tokio::fs::create_dir_all(&config.backup_root).await?;

    let db = PgPoolOptions::new()
        .max_connections(DB_MAX_CONNECTIONS)
        .connect(&config.database_url)
        .await?;

    // A release rolled back after a newer one ran keeps working, since
    // migrations only add. The one exception is 0008, which removed what
    // the hosted beta needed.
    let mut migrator = sqlx::migrate!();
    migrator.set_ignore_missing(true);
    migrator.run(&db).await?;

    let listener = TcpListener::bind(config.bind).await?;
    let state = AppState {
        config,
        db,
        limits: Arc::new(Limits::new()),
    };
    Ok((state, listener))
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
impl AppState {
    /// The pool connects only once a request uses it.
    pub fn for_tests(config: Config) -> Self {
        Self {
            db: PgPoolOptions::new()
                .connect_lazy(&config.database_url)
                .expect("the test database address parses"),
            config: Arc::new(config),
            limits: Arc::new(Limits::new()),
        }
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
        (
            6,
            "ef36b8b07c2fe5d257df7f8e5d26ec58b72ef092710f2705879457402069d05768100781b02ad1676fc9056e8b848092",
        ),
        (
            7,
            "71b69b8279874626c131101c87ffb17e0e54090a3277f213fe0f24a834892e61789e6e3280fe817ac329a013dffb0757",
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
