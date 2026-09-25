// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

mod admin;
mod auth;
mod backups;
mod devices;

use axum::http::{Method, header};
use axum::routing::{get, post, put};
use axum::{Json, Router, extract::State};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::AppState;
use crate::models::HealthResponse;

pub fn router(state: AppState) -> Router {
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
        .route("/v1/admin/invites", post(admin::create_invite))
        .route("/v1/devices/register", post(devices::register))
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
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "vaultime-api",
        public_base_url: state.config.public_base_url.clone(),
    })
}
