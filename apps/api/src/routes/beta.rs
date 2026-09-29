// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Applications for cloud beta access. The website posts a plain HTML form, so
//! answers are redirects to its pages. Admins read and remove applications.

use axum::Json;
use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::Redirect;
use serde::Deserialize;
use uuid::Uuid;

use super::admin::require_admin;
use crate::AppState;
use crate::auth::{AuthenticatedAccount, normalize_email};
use crate::constants::{
    BETA_APPLICATION_RETENTION_DAYS, BETA_APPLICATIONS_PER_HOUR, BETA_EMAIL_MAX_CHARS,
    BETA_NOTE_MAX_CHARS,
};
use crate::error::AppResult;
use crate::models::BetaApplicationResponse;

/// Website page after an application went in.
const APPLIED_PAGE: &str = "/applied.html";
/// Website page when an application could not be taken.
const FAILED_PAGE: &str = "/apply-failed.html";
const PLATFORMS: &[&str] = &["windows", "linux", "both"];

#[derive(Debug, Deserialize)]
pub struct BetaApplicationForm {
    #[serde(default)]
    email: String,
    #[serde(default)]
    platform: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    consent: Option<String>,
    /// Hidden from people, so only bots fill it in.
    #[serde(default)]
    website: String,
}

pub async fn apply(
    State(state): State<AppState>,
    Form(form): Form<BetaApplicationForm>,
) -> Redirect {
    match store_application(&state, form).await {
        Ok(()) => Redirect::to(APPLIED_PAGE),
        Err(error) => {
            tracing::warn!(error = %error, "beta application not taken");
            Redirect::to(FAILED_PAGE)
        }
    }
}

/// Stores an application. A second application for the same address and one
/// from a bot look like success, so the form tells nothing about others.
async fn store_application(state: &AppState, form: BetaApplicationForm) -> Result<(), String> {
    if !form.website.trim().is_empty() {
        return Ok(());
    }
    let application = validate(&form)?;

    sqlx::query(
        "DELETE FROM beta_applications WHERE created_at < NOW() - make_interval(days => $1)",
    )
    .bind(BETA_APPLICATION_RETENTION_DAYS)
    .execute(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    let last_hour: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM beta_applications WHERE created_at > NOW() - INTERVAL '1 hour'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|error| error.to_string())?;
    if last_hour >= BETA_APPLICATIONS_PER_HOUR {
        return Err("too many applications in the last hour".into());
    }

    sqlx::query(
        "INSERT INTO beta_applications (email, platform, note) VALUES ($1, $2, $3)
         ON CONFLICT (email) DO NOTHING",
    )
    .bind(&application.email)
    .bind(&application.platform)
    .bind(&application.note)
    .execute(&state.db)
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, PartialEq)]
struct Application {
    email: String,
    platform: String,
    note: Option<String>,
}

fn validate(form: &BetaApplicationForm) -> Result<Application, String> {
    if form.consent.as_deref() != Some("yes") {
        return Err("the privacy policy was not accepted".into());
    }
    let email = normalize_email(&form.email).map_err(|error| error.to_string())?;
    let (local, domain) = email.split_once('@').unwrap_or_default();
    if email.chars().count() > BETA_EMAIL_MAX_CHARS
        || local.is_empty()
        || !domain.contains('.')
        || email.chars().any(char::is_whitespace)
    {
        return Err("the email address is not valid".into());
    }
    let platform = form.platform.trim().to_ascii_lowercase();
    if !PLATFORMS.contains(&platform.as_str()) {
        return Err("the platform is not known".into());
    }
    let note = form.note.trim();
    if note.chars().count() > BETA_NOTE_MAX_CHARS {
        return Err("the note is too long".into());
    }
    Ok(Application {
        email,
        platform,
        note: (!note.is_empty()).then(|| note.to_owned()),
    })
}

pub async fn list_applications(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<BetaApplicationResponse>>> {
    require_admin(&auth)?;
    let rows = sqlx::query_as::<_, BetaApplicationResponse>(
        "SELECT id, email::TEXT AS email, platform, note, created_at
         FROM beta_applications ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

pub async fn delete_application(
    auth: AuthenticatedAccount,
    State(state): State<AppState>,
    Path(application_id): Path<Uuid>,
) -> AppResult<StatusCode> {
    require_admin(&auth)?;
    sqlx::query("DELETE FROM beta_applications WHERE id = $1")
        .bind(application_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(email: &str, platform: &str, note: &str, consent: bool) -> BetaApplicationForm {
        BetaApplicationForm {
            email: email.into(),
            platform: platform.into(),
            note: note.into(),
            consent: consent.then(|| "yes".into()),
            website: String::new(),
        }
    }

    #[test]
    fn takes_a_complete_application() {
        assert_eq!(
            validate(&form(" Player@Example.com ", "Linux", "  ", true)),
            Ok(Application {
                email: "player@example.com".into(),
                platform: "linux".into(),
                note: None,
            })
        );
    }

    #[test]
    fn turns_away_incomplete_applications() {
        assert!(validate(&form("player@example.com", "windows", "", false)).is_err());
        assert!(validate(&form("player", "windows", "", true)).is_err());
        assert!(validate(&form("player@localhost", "windows", "", true)).is_err());
        assert!(validate(&form("pla yer@example.com", "windows", "", true)).is_err());
        assert!(validate(&form("player@example.com", "amiga", "", true)).is_err());
        let long_note = "x".repeat(BETA_NOTE_MAX_CHARS + 1);
        assert!(validate(&form("player@example.com", "both", &long_note, true)).is_err());
    }
}
