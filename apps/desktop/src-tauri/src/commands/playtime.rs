// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Play time worked out for the pages, and playtime from before Vaultime.

use std::sync::Arc;

use tauri::State;

use crate::AppContext;
use crate::constants::{RECENT_PLAY_MAX_DAYS, STEAM_SOURCE};
use crate::db::connection::Database;
use crate::db::models::EarlierPlaytime;
use crate::db::repo::earlier_playtime;
use crate::earlier;
use crate::error::VaultimeError;
use crate::playtime;
use crate::playtime::beside::StepsAsideHint;
use crate::playtime::journal::JournalWeek;
use crate::playtime::summaries::{RecentPlay, YearPlay};
use crate::playtime::totals::{Bucket, PlayTotal};
use crate::tracking::live::LiveSessions;

fn parse_day(text: &str) -> Result<chrono::NaiveDate, VaultimeError> {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map_err(|_| VaultimeError::Invalid(format!("not a date: {text}")))
}

/// Each game's play time per day, week, month, year or hour of the week,
/// from the local day `from` up to but not including `to`, both "2026-10-02".
#[tauri::command(async)]
pub fn get_play_totals(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    from: String,
    to: String,
    bucket: Bucket,
    game_id: Option<String>,
) -> Result<Vec<PlayTotal>, VaultimeError> {
    playtime::totals::play_totals(
        &db,
        &live,
        &chrono::Local,
        parse_day(&from)?,
        parse_day(&to)?,
        bucket,
        game_id.as_deref(),
        chrono::Utc::now().timestamp_millis(),
    )
}

/// The journal's week from the local day `from`, a Monday, "2026-10-05".
#[tauri::command(async)]
pub fn get_journal_week(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    app_context: State<'_, AppContext>,
    from: String,
) -> Result<JournalWeek, VaultimeError> {
    playtime::journal::journal_week(
        &db,
        &live,
        &chrono::Local,
        parse_day(&from)?,
        &app_context.device_id,
        chrono::Utc::now().timestamp_millis(),
    )
}

/// The year `year` for the stats page.
#[tauri::command(async)]
pub fn get_year_play(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    year: i32,
) -> Result<YearPlay, VaultimeError> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    playtime::summaries::year_play(
        &db,
        &live,
        &chrono::Local,
        year,
        playtime::summaries::today_of(&chrono::Local, now_ms),
        now_ms,
    )
}

/// Every day with play up to today, for the stats page's all time view.
#[tauri::command(async)]
pub fn get_all_time_play(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
) -> Result<YearPlay, VaultimeError> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    playtime::summaries::all_time_play(
        &db,
        &live,
        &chrono::Local,
        playtime::summaries::today_of(&chrono::Local, now_ms),
        now_ms,
    )
}

/// The last `days` local days of play, today included, for the library.
#[tauri::command(async)]
pub fn get_recent_play(
    db: State<'_, Arc<Database>>,
    live: State<'_, Arc<LiveSessions>>,
    days: u32,
) -> Result<RecentPlay, VaultimeError> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    playtime::summaries::recent_play(
        &db,
        &live,
        &chrono::Local,
        days.min(RECENT_PLAY_MAX_DAYS),
        playtime::summaries::today_of(&chrono::Local, now_ms),
        now_ms,
    )
}

/// Why a game's page should suggest that it counts only while no other game
/// runs, or None.
#[tauri::command(async)]
pub fn get_steps_aside_hint(
    db: State<'_, Arc<Database>>,
    app_context: State<'_, AppContext>,
    game_id: String,
) -> Result<Option<StepsAsideHint>, VaultimeError> {
    playtime::beside::steps_aside_hint(
        &db,
        &game_id,
        &app_context.device_id,
        chrono::Utc::now().timestamp_millis(),
    )
}

/// Playtime from before Vaultime, per game the library shows.
#[tauri::command(async)]
pub fn list_earlier_playtime(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<EarlierPlaytime>, VaultimeError> {
    earlier::shown_earlier_playtime(&db)
}

/// What an import from Steam would add, without storing anything.
#[tauri::command(async)]
pub fn preview_steam_playtime(
    db: State<'_, Arc<Database>>,
) -> Result<earlier::SteamPlaytimePreview, VaultimeError> {
    earlier::preview_steam(&db)
}

#[tauri::command(async)]
pub fn import_steam_playtime(
    db: State<'_, Arc<Database>>,
) -> Result<earlier::SteamPlaytimePreview, VaultimeError> {
    earlier::import_steam(&db)
}

/// Removes all playtime imported from Steam. Returns how many games had some.
#[tauri::command]
pub fn remove_steam_playtime(db: State<'_, Arc<Database>>) -> Result<usize, VaultimeError> {
    earlier_playtime::clear_earlier_playtime(&db, STEAM_SOURCE)
}
