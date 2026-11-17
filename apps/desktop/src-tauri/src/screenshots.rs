// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Steam screenshots shown under the sessions they were taken in. They are
//! read from Steam's folder each time and never copied, so backups never
//! hold them.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use base64::Engine;
use serde::Serialize;

use crate::constants::{
    SCREENSHOTS_PER_SESSION, STEAM_SCREENSHOT_MAX_BYTES, STEAM_THUMBNAIL_MAX_BYTES,
};
use crate::db::connection::Database;
use crate::db::repo::{games, map_db};
use crate::discovery::steam;
use crate::discovery::steam_screenshots::{self, SteamScreenshot};
use crate::error::{Result, VaultimeError};

/// A screenshot taken during a session.
#[derive(Debug, Clone, Serialize)]
pub struct Screenshot {
    pub app_id: String,
    /// File name in Steam's folder for the app, which `picture` takes.
    pub file: String,
    /// Unix milliseconds.
    pub taken_at_ms: i64,
    pub width: u32,
    pub height: u32,
    /// Steam's small copy as a data URL, when it kept one.
    pub thumbnail: Option<String>,
}

/// The screenshots of one session, oldest first.
#[derive(Debug, Clone, Serialize)]
pub struct SessionScreenshots {
    pub session_id: String,
    /// Every screenshot taken in the session, of which `shots` holds the first.
    pub total: usize,
    pub shots: Vec<Screenshot>,
}

/// When a session of this PC ran, a running one up to now.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionRange {
    session_id: String,
    game_id: String,
    start_ms: i64,
    end_ms: i64,
}

/// The Steam screenshots taken during each of the sessions among `ids` that
/// this PC recorded. Sessions without any are left out.
pub fn for_sessions(
    db: &Database,
    device_id: &str,
    ids: &[String],
    now_ms: i64,
) -> Result<Vec<SessionScreenshots>> {
    let Some(root) = steam::find_steam_root() else {
        return Ok(Vec::new());
    };
    let ranges = session_ranges(db, device_id, ids, now_ms)?;
    let mut app_of = HashMap::new();
    for range in &ranges {
        if app_of.contains_key(&range.game_id) {
            continue;
        }
        let app_id = games::get_game(db, &range.game_id)?
            .install_folder
            .and_then(|folder| steam::app_id_for_install_folder(Path::new(&folder)));
        app_of.insert(range.game_id.clone(), app_id);
    }
    let app_of: HashMap<String, String> = app_of
        .into_iter()
        .filter_map(|(game_id, app_id)| Some((game_id, app_id?)))
        .collect();
    Ok(
        taken_during(&ranges, &app_of, steam_screenshots::list(&root))
            .into_iter()
            .map(|(session_id, shots)| SessionScreenshots {
                session_id,
                total: shots.len(),
                shots: shots
                    .into_iter()
                    .take(SCREENSHOTS_PER_SESSION)
                    .map(|shot| Screenshot {
                        thumbnail: steam_screenshots::picture(
                            &root,
                            &shot.app_id,
                            &shot.file,
                            true,
                        )
                        .and_then(|path| data_url(&path, STEAM_THUMBNAIL_MAX_BYTES)),
                        taken_at_ms: shot.taken_at * 1000,
                        width: shot.width,
                        height: shot.height,
                        app_id: shot.app_id,
                        file: shot.file,
                    })
                    .collect(),
            })
            .collect(),
    )
}

/// One Steam screenshot in full, as a data URL.
pub fn picture(app_id: &str, file: &str) -> Result<String> {
    steam::find_steam_root()
        .and_then(|root| steam_screenshots::picture(&root, app_id, file, false))
        .and_then(|path| data_url(&path, STEAM_SCREENSHOT_MAX_BYTES))
        .ok_or_else(|| VaultimeError::NotFound(format!("screenshot {file} of Steam app {app_id}")))
}

fn session_ranges(
    db: &Database,
    device_id: &str,
    ids: &[String],
    now_ms: i64,
) -> Result<Vec<SessionRange>> {
    let ids =
        serde_json::to_string(ids).map_err(|error| VaultimeError::Invalid(error.to_string()))?;
    let rows: Vec<(String, String, String, Option<String>)> = db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, game_id, started_at_wall, ended_at_wall FROM sessions
                 WHERE device_id = ?1 AND id IN (SELECT value FROM json_each(?2))",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([device_id, ids.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(map_db)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })?;
    let ms = |text: &str| {
        chrono::DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|time| time.timestamp_millis())
    };
    Ok(rows
        .into_iter()
        .filter_map(|(session_id, game_id, started, ended)| {
            Some(SessionRange {
                session_id,
                game_id,
                start_ms: ms(&started)?,
                end_ms: ended.as_deref().map_or(Some(now_ms), ms)?,
            })
        })
        .collect())
}

/// Each screenshot goes to the session of its game that ran when it was
/// taken. Screenshots come oldest first, and sessions in the order given.
fn taken_during(
    ranges: &[SessionRange],
    app_of: &HashMap<String, String>,
    mut listed: Vec<SteamScreenshot>,
) -> Vec<(String, Vec<SteamScreenshot>)> {
    listed.sort_by(|a, b| {
        a.taken_at
            .cmp(&b.taken_at)
            .then_with(|| a.file.cmp(&b.file))
    });
    ranges
        .iter()
        .filter_map(|range| {
            let app_id = app_of.get(&range.game_id)?;
            let shots: Vec<_> = listed
                .iter()
                .filter(|shot| {
                    &shot.app_id == app_id
                        && (range.start_ms..=range.end_ms).contains(&(shot.taken_at * 1000))
                })
                .cloned()
                .collect();
            (!shots.is_empty()).then(|| (range.session_id.clone(), shots))
        })
        .collect()
}

fn data_url(path: &Path, max_bytes: u64) -> Option<String> {
    if fs::metadata(path).ok()?.len() > max_bytes {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let png = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
    let mime = if png { "image/png" } else { "image/jpeg" };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::{devices, sessions};

    fn shot(app_id: &str, file: &str, taken_at: i64) -> SteamScreenshot {
        SteamScreenshot {
            app_id: app_id.into(),
            file: file.into(),
            taken_at,
            width: 1920,
            height: 1080,
        }
    }

    fn range(session_id: &str, game_id: &str, start_secs: i64, end_secs: i64) -> SessionRange {
        SessionRange {
            session_id: session_id.into(),
            game_id: game_id.into(),
            start_ms: start_secs * 1000,
            end_ms: end_secs * 1000,
        }
    }

    #[test]
    fn a_screenshot_goes_to_the_session_of_its_game_that_ran_when_it_was_taken() {
        let ranges = [
            range("evening", "game", 1_000, 2_000),
            range("other game", "other", 1_000, 2_000),
            range("not steam", "local", 1_000, 2_000),
            range("night", "game", 3_000, 4_000),
        ];
        let app_of = HashMap::from([
            ("game".to_owned(), "10".to_owned()),
            ("other".to_owned(), "20".to_owned()),
        ]);
        let listed = vec![
            shot("10", "b.jpg", 2_000),
            shot("10", "a.jpg", 1_000),
            shot("10", "between.jpg", 2_500),
            shot("20", "c.jpg", 1_500),
            shot("10", "d.jpg", 3_500),
        ];
        let files =
            |shots: &[SteamScreenshot]| shots.iter().map(|s| s.file.clone()).collect::<Vec<_>>();
        let matched = taken_during(&ranges, &app_of, listed);
        let found: Vec<_> = matched
            .iter()
            .map(|(session, shots)| (session.as_str(), files(shots)))
            .collect();
        assert_eq!(
            found,
            vec![
                ("evening", vec!["a.jpg".to_owned(), "b.jpg".to_owned()]),
                ("other game", vec!["c.jpg".to_owned()]),
                ("night", vec!["d.jpg".to_owned()]),
            ]
        );
    }

    #[test]
    fn ranges_cover_only_sessions_of_this_pc_and_run_a_live_one_up_to_now() {
        let db = Database::open_in_memory().unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Kessler Line".into(),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        let start = chrono::DateTime::parse_from_rfc3339("2026-11-14T13:00:00Z")
            .unwrap()
            .to_utc();
        for device in ["this-pc", "other-pc"] {
            devices::ensure_device(&db, device, "windows", "1.0.0").unwrap();
        }
        let here = sessions::create_session_at(&db, &game.id, "this-pc", start).unwrap();
        let elsewhere = sessions::create_session_at(&db, &game.id, "other-pc", start).unwrap();
        let ids = [here.id.clone(), elsewhere.id];
        let now_ms = start.timestamp_millis() + 60_000;
        assert_eq!(
            session_ranges(&db, "this-pc", &ids, now_ms).unwrap(),
            vec![SessionRange {
                session_id: here.id,
                game_id: game.id,
                start_ms: start.timestamp_millis(),
                end_ms: now_ms,
            }]
        );
    }
}
