// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

mod auth;
mod backups;
mod blobs;
mod devices;
mod ledger_keys;
mod maintenance;

pub use maintenance::spawn_maintenance;

use std::net::SocketAddr;

use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use axum::http::{Method, header};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router, extract::State};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::AppState;
use crate::models::HealthResponse;

/// The routes as a service that hands each request its peer address, which
/// the rate limits count.
pub fn service(state: AppState) -> IntoMakeServiceWithConnectInfo<Router, SocketAddr> {
    router(state).into_make_service_with_connect_info::<SocketAddr>()
}

fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    Router::new()
        .route("/healthz", get(health))
        .route("/v1/auth/signup", post(auth::sign_up))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/refresh", post(auth::refresh))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/password", post(auth::change_password))
        .route("/v1/devices", get(devices::list))
        .route("/v1/devices/register", post(devices::register))
        .route("/v1/devices/{device_id}", delete(devices::remove))
        .route(
            "/v1/ledger-keys",
            get(ledger_keys::list).post(ledger_keys::register),
        )
        .route(
            "/v1/backups",
            get(backups::list_backups).post(backups::create_backup),
        )
        .route(
            "/v1/backups/{backup_id}",
            get(backups::get_backup).delete(backups::delete_backup),
        )
        .route(
            "/v1/backups/{backup_id}/content",
            put(backups::upload_backup_content),
        )
        .route(
            "/v1/backups/{backup_id}/download",
            get(backups::download_backup),
        )
        .route("/v1/blobs/missing", post(blobs::missing_blobs))
        .route(
            "/v1/blobs/{blob_id}",
            get(blobs::download_blob).put(blobs::upload_blob),
        )
        .route("/v1/storage", get(blobs::storage))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "vaultime-api",
        signups_open: state.config.open_signups,
    })
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use super::*;
    use crate::config::Config;

    /// Sends one request and returns the whole answer.
    async fn send(address: SocketAddr, request_line: &str, body: &str) -> String {
        let mut stream = TcpStream::connect(address).await.unwrap();
        let request = format!(
            "{request_line} HTTP/1.1\r\nhost: localhost\r\ncontent-type: application/json\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).await.unwrap();
        answer
    }

    #[tokio::test]
    async fn closed_sign_ups_answer_403_without_the_database() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let config = Config {
            open_signups: false,
            ..Config::for_tests()
        };
        let app = service(AppState::for_tests(config));
        tokio::spawn(async move { axum::serve(listener, app).await });

        let health = send(address, "GET /healthz", "").await;
        assert!(
            health.ends_with(r#"{"status":"ok","service":"vaultime-api","signups_open":false}"#),
            "{health}"
        );
        let body = r#"{"email":"player@example.com","password":"correct horse battery"}"#;
        let sign_up = send(address, "POST /v1/auth/signup", body).await;
        assert!(sign_up.starts_with("HTTP/1.1 403 "), "{sign_up}");
        assert!(
            sign_up.ends_with(
                r#"{"error":{"code":"forbidden","message":"this server takes no new accounts"}}"#
            ),
            "{sign_up}"
        );
    }
}
