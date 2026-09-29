// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    /// A backup refers to artwork the server does not hold. The client
    /// uploads it and tries again.
    #[error("{0}")]
    MissingArtwork(String),
    /// The account has no room left for the upload.
    #[error("{0}")]
    StorageFull(String),
    #[error("{0}")]
    TooManyRequests(String),
    #[error("{0}")]
    Configuration(String),
    /// The message is only logged. Clients get a generic one.
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn missing_artwork(message: impl Into<String>) -> Self {
        Self::MissingArtwork(message.into())
    }

    pub fn storage_full(message: impl Into<String>) -> Self {
        Self::StorageFull(message.into())
    }

    pub fn too_many_requests() -> Self {
        Self::TooManyRequests("too many attempts, try again in a few minutes".into())
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Unauthorized(message) => (StatusCode::UNAUTHORIZED, "unauthorized", message),
            Self::Forbidden(message) => (StatusCode::FORBIDDEN, "forbidden", message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            Self::MissingArtwork(message) => (StatusCode::CONFLICT, "missing_artwork", message),
            Self::StorageFull(message) => (StatusCode::PAYLOAD_TOO_LARGE, "storage_full", message),
            Self::TooManyRequests(message) => {
                (StatusCode::TOO_MANY_REQUESTS, "too_many_requests", message)
            }
            Self::Configuration(detail) | Self::Internal(detail) => {
                tracing::error!(error = %detail, "request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_string(),
                )
            }
        };

        (
            status,
            Json(ErrorBody {
                error: ErrorDetails { code, message },
            }),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(database_error) = &error
            && database_error.is_unique_violation()
        {
            tracing::warn!(error = %error, "unique constraint violation");
            return Self::conflict("resource already exists");
        }

        Self::internal(format!("database error: {error}"))
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(error: sqlx::migrate::MigrateError) -> Self {
        Self::internal(format!("database migration error: {error}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::internal(format!("i/o error: {error}"))
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(error: argon2::password_hash::Error) -> Self {
        Self::internal(format!("password hash error: {error}"))
    }
}

impl From<getrandom::Error> for AppError {
    fn from(error: getrandom::Error) -> Self {
        Self::internal(format!("system random source failed: {error}"))
    }
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: ErrorDetails,
}

#[derive(Debug, Serialize)]
struct ErrorDetails {
    code: &'static str,
    message: String,
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;

    use super::*;

    async fn body_of(error: AppError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn internal_errors_hide_details() {
        let (status, body) = body_of(AppError::internal("database error: secret detail")).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"]["code"], "internal_error");
        assert_eq!(body["error"]["message"], "internal server error");
    }

    #[tokio::test]
    async fn client_errors_keep_their_message() {
        let (status, body) =
            body_of(AppError::conflict("a backup upload is already pending")).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "conflict");
        assert_eq!(
            body["error"]["message"],
            "a backup upload is already pending"
        );
    }
}
