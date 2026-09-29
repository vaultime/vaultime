// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sessions as a CSV or JSON file for spreadsheets and other tools. A session
//! that still runs is left out until it ends.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Local, SecondsFormat};
use serde::{Deserialize, Serialize};

use crate::constants::{EXPORT_FORMAT_NAME, EXPORT_FORMAT_VERSION};
use crate::db::connection::Database;
use crate::db::repo::{annotations, earlier_playtime, games, sessions};
use crate::error::{Result, VaultimeError};
use crate::integrity;

/// Lets spreadsheet apps read the file as UTF-8.
const UTF8_BOM: &str = "\u{feff}";
/// Line ending of CSV, as RFC 4180 sets it.
const CSV_LINE_END: &str = "\r\n";
const CSV_COLUMNS: [&str; 10] = [
    "game",
    "started_at",
    "ended_at",
    "runtime_seconds",
    "active_seconds",
    "idle_seconds",
    "trust",
    "note",
    "session_id",
    "game_id",
];

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
}

#[derive(Debug, Serialize)]
struct ExportedSession {
    id: String,
    game_id: String,
    game: String,
    started_at: String,
    ended_at: String,
    runtime_seconds: u64,
    active_seconds: u64,
    idle_seconds: u64,
    /// The trust label, such as `local` or `edited`.
    trust: String,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExportedGame {
    id: String,
    title: String,
    hidden: bool,
    status: Option<String>,
    /// Playtime from before Vaultime, counted by a launcher.
    earlier_seconds: u64,
    earlier_source: Option<String>,
}

#[derive(Debug, Serialize)]
struct JsonExport {
    format: &'static str,
    version: u32,
    exported_at: String,
    app_version: String,
    games: Vec<ExportedGame>,
    sessions: Vec<ExportedSession>,
}

/// Writes every finished session to `path`, oldest first, and returns how
/// many it wrote.
pub fn export_sessions(
    db: &Database,
    path: &Path,
    format: ExportFormat,
    app_version: &str,
) -> Result<usize> {
    let sessions = exported_sessions(db)?;
    let count = sessions.len();
    let contents = match format {
        ExportFormat::Csv => to_csv(&sessions),
        ExportFormat::Json => {
            let export = JsonExport {
                format: EXPORT_FORMAT_NAME,
                version: EXPORT_FORMAT_VERSION,
                exported_at: local_time(&integrity::now_timestamp()),
                app_version: app_version.to_string(),
                games: exported_games(db)?,
                sessions,
            };
            serde_json::to_string_pretty(&export)
                .map_err(|e| VaultimeError::Invalid(format!("could not build the export: {e}")))?
        }
    };
    fs::write(path, contents)
        .map_err(|e| VaultimeError::Invalid(format!("could not save the export: {e}")))?;
    Ok(count)
}

fn exported_sessions(db: &Database) -> Result<Vec<ExportedSession>> {
    let titles: HashMap<String, String> = games::list_all_games(db)?
        .into_iter()
        .map(|game| (game.id, game.title))
        .collect();
    let notes: HashMap<String, String> = annotations::list_session_notes(db)?
        .into_iter()
        .map(|note| (note.session_id, note.note))
        .collect();
    let mut exported: Vec<ExportedSession> = sessions::list_all_sessions(db)?
        .into_iter()
        .filter_map(|session| {
            let ended_at = session.ended_at_wall.as_deref().map(local_time)?;
            Some(ExportedSession {
                game: titles.get(&session.game_id).cloned().unwrap_or_default(),
                started_at: local_time(&session.started_at_wall),
                ended_at,
                runtime_seconds: seconds(session.runtime_ms),
                active_seconds: seconds(session.active_ms),
                idle_seconds: seconds(session.idle_ms),
                trust: session.integrity_status,
                note: notes.get(&session.id).cloned(),
                game_id: session.game_id,
                id: session.id,
            })
        })
        .collect();
    exported.reverse();
    Ok(exported)
}

fn exported_games(db: &Database) -> Result<Vec<ExportedGame>> {
    // Changes come oldest first, so the last one per game is its status.
    let mut statuses = HashMap::new();
    for change in annotations::list_status_changes(db)? {
        statuses.insert(change.game_id, change.status);
    }
    let earlier: HashMap<String, (i64, String)> = earlier_playtime::list_earlier_playtime(db)?
        .into_iter()
        .map(|entry| (entry.game_id, (entry.earlier_ms, entry.source)))
        .collect();
    Ok(games::list_all_games(db)?
        .into_iter()
        .map(|game| {
            let earlier = earlier.get(&game.id).filter(|(ms, _)| *ms > 0);
            ExportedGame {
                status: statuses
                    .get(&game.id)
                    .filter(|status| status.as_str() != annotations::STATUS_NONE)
                    .cloned(),
                earlier_seconds: earlier.map_or(0, |(ms, _)| seconds(*ms)),
                earlier_source: earlier.map(|(_, source)| source.clone()),
                hidden: game.is_hidden,
                title: game.title,
                id: game.id,
            }
        })
        .collect())
}

/// Local time with its offset, which spreadsheets and people read more easily
/// than UTC. A value that does not parse stays as stored.
fn local_time(stored: &str) -> String {
    DateTime::parse_from_rfc3339(stored).map_or_else(
        |_| stored.to_string(),
        |time| {
            time.with_timezone(&Local)
                .to_rfc3339_opts(SecondsFormat::Secs, false)
        },
    )
}

fn seconds(ms: i64) -> u64 {
    Duration::from_millis(u64::try_from(ms).unwrap_or(0)).as_secs()
}

fn to_csv(sessions: &[ExportedSession]) -> String {
    let mut csv = String::from(UTF8_BOM);
    csv.push_str(&CSV_COLUMNS.join(","));
    csv.push_str(CSV_LINE_END);
    for session in sessions {
        let row = [
            session.game.clone(),
            session.started_at.clone(),
            session.ended_at.clone(),
            session.runtime_seconds.to_string(),
            session.active_seconds.to_string(),
            session.idle_seconds.to_string(),
            session.trust.clone(),
            session.note.clone().unwrap_or_default(),
            session.id.clone(),
            session.game_id.clone(),
        ];
        let fields: Vec<String> = row.iter().map(|field| csv_field(field)).collect();
        csv.push_str(&fields.join(","));
        csv.push_str(CSV_LINE_END);
    }
    csv
}

/// Quotes a field when it needs it. Spreadsheets run a cell that starts like
/// a formula, so such text gets a leading apostrophe and stays text.
fn csv_field(value: &str) -> String {
    let value = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::{corrections, devices};
    use chrono::Utc;

    const DEVICE: &str = "test-device";
    const HOUR_MS: i64 = 3_600_000;

    fn setup() -> (Database, String, std::path::PathBuf) {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, DEVICE, "windows", "0.1.0").unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "=Hades, \"II\"".into(),
                executable_path: Some("C:/Games/Hades II/Hades2.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("vaultime-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        (db, game.id, dir)
    }

    fn add(db: &Database, game_id: &str, hours_ago: i64, reason: &str) -> String {
        let start = (Utc::now() - chrono::Duration::hours(hours_ago))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        corrections::add_manual_session(db, game_id, DEVICE, &start, HOUR_MS, reason)
            .unwrap()
            .id
    }

    #[test]
    fn csv_has_finished_sessions_oldest_first() {
        let (db, game_id, dir) = setup();
        let older = add(&db, &game_id, 30, "");
        let newer = add(&db, &game_id, 5, "");
        annotations::set_session_note(&db, &newer, "Beat the boss, finally").unwrap();
        sessions::create_session(&db, &game_id, DEVICE).unwrap();

        let path = dir.join("sessions.csv");
        let count = export_sessions(&db, &path, ExportFormat::Csv, "0.1.0").unwrap();
        assert_eq!(count, 2);

        let csv = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = csv
            .trim_start_matches(UTF8_BOM)
            .split(CSV_LINE_END)
            .collect();
        assert_eq!(lines[0], CSV_COLUMNS.join(","));
        assert!(lines[1].starts_with("\"'=Hades, \"\"II\"\"\","));
        assert!(lines[1].contains(&older));
        assert!(lines[1].contains(",3600,3600,0,manual,,"));
        assert!(lines[2].contains(&newer));
        assert!(lines[2].contains(",\"Beat the boss, finally\","));
        assert_eq!(lines[3], "");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn json_carries_games_with_status_and_earlier_playtime() {
        let (db, game_id, dir) = setup();
        add(&db, &game_id, 5, "On the Steam Deck");
        annotations::set_game_status(&db, &game_id, "finished").unwrap();

        let path = dir.join("sessions.json");
        export_sessions(&db, &path, ExportFormat::Json, "0.1.0").unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

        assert_eq!(json["format"], EXPORT_FORMAT_NAME);
        assert_eq!(json["version"], EXPORT_FORMAT_VERSION);
        assert_eq!(json["games"][0]["status"], "finished");
        assert_eq!(json["games"][0]["earlier_seconds"], 0);
        assert!(json["games"][0]["earlier_source"].is_null());
        assert_eq!(json["sessions"][0]["runtime_seconds"], 3600);
        assert_eq!(json["sessions"][0]["trust"], "manual");
        assert!(json["sessions"][0]["note"].is_null());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn times_keep_their_offset() {
        let local = local_time("2026-09-29T18:12:05.250Z");
        let parsed = DateTime::parse_from_rfc3339(&local).unwrap();
        assert_eq!(
            parsed.with_timezone(&Utc),
            DateTime::parse_from_rfc3339("2026-09-29T18:12:05Z").unwrap()
        );
        assert_eq!(local_time("not a time"), "not a time");
    }

    #[test]
    fn fields_are_quoted_and_formulas_defused() {
        assert_eq!(csv_field("Celeste"), "Celeste");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("two\nlines"), "\"two\nlines\"");
        assert_eq!(csv_field("=1+1"), "'=1+1");
        assert_eq!(csv_field("@SUM(A1)"), "'@SUM(A1)");
        assert_eq!(csv_field("-5"), "'-5");
    }
}
