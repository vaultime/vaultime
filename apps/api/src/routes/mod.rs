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
use axum::routing::{delete, get, post, put};
use axum::{Json, Router, extract::State};
use tower_http::trace::TraceLayer;

use crate::AppState;
use crate::models::HealthResponse;

/// The routes as a service that hands each request its peer address, which
/// the rate limits count.
pub fn service(state: AppState) -> IntoMakeServiceWithConnectInfo<Router, SocketAddr> {
    router(state).into_make_service_with_connect_info::<SocketAddr>()
}

// No CORS headers: only the app calls the server, from its core, so no web
// page can make the browsers of its visitors sign in or sign up.
fn router(state: AppState) -> Router {
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
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "vaultime-api",
        signups_open: state.config.open_signups,
        signup_code_required: state.config.signup_code_digest.is_some(),
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
            health.ends_with(
                r#"{"status":"ok","service":"vaultime-api","signups_open":false,"signup_code_required":false}"#
            ),
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

    #[tokio::test]
    async fn a_sign_up_without_the_code_answers_403_without_the_database() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let config = Config {
            open_signups: true,
            signup_code_digest: Some(crate::auth::signup_code_digest("river-lantern-42")),
            ..Config::for_tests()
        };
        let app = service(AppState::for_tests(config));
        tokio::spawn(async move { axum::serve(listener, app).await });

        let health = send(address, "GET /healthz", "").await;
        assert!(
            health.ends_with(r#""signups_open":true,"signup_code_required":true}"#),
            "{health}"
        );
        for (body, message) in [
            (
                r#"{"email":"player@example.com","password":"correct horse battery"}"#,
                "this server needs a sign-up code, which Vaultime asks for from version 0.7.1 on",
            ),
            (
                r#"{"email":"player@example.com","password":"correct horse battery","signup_code":"guess-1234"}"#,
                "the sign-up code is not right",
            ),
        ] {
            let sign_up = send(address, "POST /v1/auth/signup", body).await;
            assert!(sign_up.starts_with("HTTP/1.1 403 "), "{sign_up}");
            assert!(
                sign_up.ends_with(&format!(
                    r#"{{"error":{{"code":"forbidden","message":"{message}"}}}}"#
                )),
                "{sign_up}"
            );
        }
        // The right code gets on to the email, still before the database.
        let body = r#"{"email":"no address","password":"correct horse battery","signup_code":" river-lantern-42 "}"#;
        let sign_up = send(address, "POST /v1/auth/signup", body).await;
        assert!(sign_up.starts_with("HTTP/1.1 400 "), "{sign_up}");
        assert!(
            sign_up.contains("a valid email address is required"),
            "{sign_up}"
        );
    }

    #[tokio::test]
    async fn closed_sign_ups_stay_closed_with_a_code_set() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let config = Config {
            open_signups: false,
            signup_code_digest: Some(crate::auth::signup_code_digest("river-lantern-42")),
            ..Config::for_tests()
        };
        let app = service(AppState::for_tests(config));
        tokio::spawn(async move { axum::serve(listener, app).await });

        let body = r#"{"email":"player@example.com","password":"correct horse battery","signup_code":"river-lantern-42"}"#;
        let sign_up = send(address, "POST /v1/auth/signup", body).await;
        assert!(sign_up.starts_with("HTTP/1.1 403 "), "{sign_up}");
        assert!(
            sign_up.contains("this server takes no new accounts"),
            "{sign_up}"
        );
    }
}
