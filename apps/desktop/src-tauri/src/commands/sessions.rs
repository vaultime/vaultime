// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sessions: listing, correcting, notes and the export.

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::AppContext;
use crate::db::connection::Database;
use crate::db::models::{Session, SessionEvent, SessionNote};
use crate::db::repo::{annotations, corrections, session_events, sessions};
use crate::error::VaultimeError;
use crate::playtime;
use crate::playtime::games::GameTotals;
use crate::tracking::live::LiveSessions;

/// Every session and each game's totals of them.
#[derive(Debug, Serialize)]
pub struct SessionList {
    pub sessions: Vec<Session>,
    pub games: Vec<GameTotals>,
}

/// Every session, running ones as of the tracker's latest tick, each with
/// the time it set aside and the time it shares with another copy of its
/// game, and each game's totals. Trust labels of the last `recent_days` days
/// count as recent too.
#[tauri::command(async)]
pub fn list_sessions(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    app_context: State<'_, AppContext>,
    recent_days: u32,
) -> Result<SessionList, VaultimeError> {
    let mut all = sessions::list_all_sessions(&db)?;
    let now = chrono::Utc::now();
    playtime::totals::as_shown(&db, &live, &mut all, now.timestamp_millis())?;
    sessions::show_running_elsewhere(&db, &app_context.device_id, &mut all)?;
    let recent_since = now - chrono::TimeDelta::days(i64::from(recent_days));
    Ok(SessionList {
        games: playtime::games::game_totals(&all, recent_since, now),
        sessions: all,
    })
}

/// This PC's running sessions and today's play, for the live bar.
#[derive(Debug, Serialize)]
pub struct ActiveSessions {
    pub sessions: Vec<Session>,
    /// Clock time with a game running today, as the journal counts it.
    pub played_today_ms: i64,
}

/// The running sessions as of the tracker's latest tick, and today's play.
#[tauri::command(async)]
pub fn get_active_sessions(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    app_context: State<'_, AppContext>,
) -> Result<ActiveSessions, VaultimeError> {
    let mut active = sessions::get_active_sessions(&db, &app_context.device_id)?;
    playtime::totals::as_shown(
        &db,
        &live,
        &mut active,
        chrono::Utc::now().timestamp_millis(),
    )?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let played_today_ms = playtime::summaries::played_on(
        &db,
        &live,
        &chrono::Local,
        playtime::summaries::today_of(&chrono::Local, now_ms),
        now_ms,
    )?;
    Ok(ActiveSessions {
        sessions: active,
        played_today_ms,
    })
}

/// The events of a game's sessions and those of the games linked to it,
/// newest first, without the checkpoints, which only place time.
#[tauri::command(async)]
pub fn get_notable_events_for_game(
    db: State<'_, Arc<Database>>,
    game_id: String,
) -> Result<Vec<SessionEvent>, VaultimeError> {
    session_events::list_notable_events_for_game(&db, &game_id)
}

/// Saves every finished session as CSV or JSON and returns how many.
#[tauri::command(async)]
pub fn export_sessions(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    path: String,
    format: crate::export::ExportFormat,
) -> Result<usize, VaultimeError> {
    crate::export::export_sessions(&db, Path::new(&path), format, &app_context.app_version)
}

/// Counts a closed session only up to `ended_at`, with a reason.
#[tauri::command]
pub fn trim_session(
    db: State<'_, Arc<Database>>,
    session_id: String,
    ended_at: String,
    reason: String,
) -> Result<Session, VaultimeError> {
    corrections::trim_session(&db, &session_id, &ended_at, &reason)
}

/// What a closed session would keep when it counted only up to `ended_at`.
#[tauri::command(async)]
pub fn preview_trim(
    db: State<'_, Arc<Database>>,
    session_id: String,
    ended_at: String,
) -> Result<corrections::TrimPreview, VaultimeError> {
    corrections::preview_trim(&db, &session_id, &ended_at)
}

/// Takes all time out of a closed session, with a reason.
#[tauri::command]
pub fn discard_session(
    db: State<'_, Arc<Database>>,
    session_id: String,
    reason: String,
) -> Result<Session, VaultimeError> {
    corrections::discard_session(&db, &session_id, &reason)
}

/// Adds play Vaultime did not see, labeled Manual. `launcher` is `steam`
/// when Steam counted the play too.
#[tauri::command]
pub fn add_manual_session(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    game_id: String,
    started_at: String,
    runtime_ms: i64,
    reason: String,
    launcher: Option<String>,
) -> Result<Session, VaultimeError> {
    corrections::add_manual_session(
        &db,
        &game_id,
        &app_context.device_id,
        &started_at,
        runtime_ms,
        &reason,
        launcher.as_deref(),
    )
}

#[tauri::command(async)]
pub fn list_session_notes(db: State<'_, Arc<Database>>) -> Result<Vec<SessionNote>, VaultimeError> {
    annotations::list_session_notes(&db)
}

/// Sets the note of a session, an empty note removes it.
#[tauri::command]
pub fn set_session_note(
    db: State<'_, Arc<Database>>,
    session_id: String,
    note: String,
) -> Result<Option<SessionNote>, VaultimeError> {
    annotations::set_session_note(&db, &session_id, &note)
}
