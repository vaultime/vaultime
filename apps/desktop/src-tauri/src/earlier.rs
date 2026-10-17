// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Playtime games had before Vaultime, from Steam's own count. Only games in
//! the library get it, matched through their Steam install folder.

use std::collections::HashMap;
use std::path::Path;

use chrono::DateTime;
use log::info;
use serde::Serialize;

use crate::constants::STEAM_SOURCE;
use crate::db::connection::Database;
use crate::db::models::EarlierPlaytime;
use crate::db::repo::sessions::LauncherRuntime;
use crate::db::repo::{earlier_playtime, games, sessions};
use crate::discovery::steam::{self, SteamPlaytime};
use crate::error::Result;
use crate::integrity;

/// A library game and the playtime Steam counted for it.
#[derive(Debug, Clone, Serialize)]
pub struct EarlierCandidate {
    pub game_id: String,
    pub title: String,
    pub launcher_minutes: i64,
    /// Runtime Vaultime tracked already, which Steam counted too.
    pub tracked_before_ms: i64,
    pub earlier_ms: i64,
    pub last_played_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SteamPlaytimePreview {
    /// Whether a Steam install with playtime was found at all.
    pub found: bool,
    pub account: Option<String>,
    /// Most earlier playtime first.
    pub games: Vec<EarlierCandidate>,
}

pub fn preview_steam(db: &Database) -> Result<SteamPlaytimePreview> {
    let Some(steam) = steam::steam_playtime() else {
        return Ok(SteamPlaytimePreview {
            found: false,
            account: None,
            games: Vec::new(),
        });
    };
    candidates(db, &steam, |folder| {
        steam::app_id_for_install_folder(Path::new(folder))
    })
}

/// Stores what `preview_steam` finds, replacing earlier imports of the same games.
pub fn import_steam(db: &Database) -> Result<SteamPlaytimePreview> {
    let preview = preview_steam(db)?;
    let imported_at = integrity::now_timestamp();
    let entries: Vec<EarlierPlaytime> = preview
        .games
        .iter()
        .map(|candidate| EarlierPlaytime {
            game_id: candidate.game_id.clone(),
            source: STEAM_SOURCE.into(),
            launcher_minutes: candidate.launcher_minutes,
            tracked_before_ms: candidate.tracked_before_ms,
            earlier_ms: candidate.earlier_ms,
            last_played_at: candidate.last_played_at.clone(),
            imported_at: imported_at.clone(),
        })
        .collect();
    earlier_playtime::replace_earlier_playtime(db, &entries)?;
    info!("imported Steam playtime for {} game(s)", entries.len());
    Ok(preview)
}

/// The runtime Vaultime tracked that Steam counted too, for the Steam copy
/// `game_id`. Of the games that count as the same game, only copies Steam
/// started count with their tracked time, like the Steam copy on another
/// PC, and a copy from another launcher only with play the player said
/// Steam counted.
fn seen_by_steam(
    game_id: &str,
    runtime: &HashMap<String, LauncherRuntime>,
    launchers: &HashMap<String, Option<String>>,
    links: &HashMap<String, String>,
) -> i64 {
    let shown = links.get(game_id).map_or(game_id, String::as_str);
    std::iter::once(shown)
        .chain(
            links
                .iter()
                .filter(|(_, target)| target.as_str() == shown)
                .map(|(linked, _)| linked.as_str()),
        )
        .filter_map(|id| Some((id, runtime.get(id)?)))
        .map(|(id, counted)| {
            let from_steam =
                id == game_id || launchers.get(id).and_then(Option::as_deref) == Some(STEAM_SOURCE);
            counted.manual_ms + if from_steam { counted.tracked_ms } else { 0 }
        })
        .sum()
}

fn candidates(
    db: &Database,
    steam: &SteamPlaytime,
    app_id_of: impl Fn(&str) -> Option<String>,
) -> Result<SteamPlaytimePreview> {
    let runtime = sessions::launcher_runtime_by_game(db, STEAM_SOURCE)?;
    let launchers: HashMap<String, Option<String>> = games::list_all_games(db)?
        .into_iter()
        .map(|game| (game.id, game.launcher_source))
        .collect();
    let links = games::links(db)?;
    let mut games = Vec::new();
    for game in games::list_local_games(db)? {
        let Some(app) = game
            .install_folder
            .as_deref()
            .and_then(&app_id_of)
            .and_then(|app_id| steam.apps.get(&app_id))
        else {
            continue;
        };
        let tracked_before_ms = seen_by_steam(&game.id, &runtime, &launchers, &links);
        games.push(EarlierCandidate {
            game_id: game.id,
            title: game.title,
            launcher_minutes: app.minutes,
            tracked_before_ms,
            earlier_ms: earlier_playtime::earlier_ms(app.minutes, tracked_before_ms),
            last_played_at: app
                .last_played
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
                .map(|time| time.to_rfc3339()),
        });
    }
    games.sort_by_key(|candidate| std::cmp::Reverse(candidate.earlier_ms));
    Ok(SteamPlaytimePreview {
        found: true,
        account: steam.account.clone(),
        games,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{SecondsFormat, Utc};

    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::{corrections, devices};
    use crate::discovery::steam::AppPlaytime;

    #[test]
    fn matches_library_games_by_their_steam_folder() {
        let db = Database::open_in_memory().unwrap();
        let create = |title: &str, folder: &str| {
            games::create_game(
                &db,
                &CreateGame {
                    title: title.into(),
                    executable_path: Some(format!("{folder}/game.exe")),
                    install_folder: Some(folder.into()),
                    launcher_source: Some("steam".into()),
                },
            )
            .unwrap()
        };
        create("Counter-Strike 2", "C:/Steam/steamapps/common/cs2");
        create("Not on Steam", "C:/Games/other");
        let steam = SteamPlaytime {
            account: Some("Player".into()),
            apps: HashMap::from([(
                "730".to_string(),
                AppPlaytime {
                    minutes: 600,
                    last_played: Some(1_727_600_000),
                },
            )]),
        };

        let preview = candidates(&db, &steam, |folder| {
            folder.ends_with("cs2").then(|| "730".to_string())
        })
        .unwrap();
        assert_eq!(preview.account.as_deref(), Some("Player"));
        assert_eq!(preview.games.len(), 1);
        assert_eq!(preview.games[0].title, "Counter-Strike 2");
        assert_eq!(preview.games[0].earlier_ms, 600 * 60_000);
        assert_eq!(
            preview.games[0].last_played_at.as_deref(),
            Some("2024-09-29T08:53:20+00:00")
        );
    }

    #[test]
    fn subtracts_only_the_time_steam_saw() {
        const DEVICE: &str = "test-device";
        const HOUR_MS: i64 = 3_600_000;
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, DEVICE, "windows", "0.1.0").unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Counter-Strike 2".into(),
                executable_path: Some("C:/Steam/steamapps/common/cs2/cs2.exe".into()),
                install_folder: Some("C:/Steam/steamapps/common/cs2".into()),
                launcher_source: Some(STEAM_SOURCE.into()),
            },
        )
        .unwrap();

        // Tracked for three hours and then taken out: Steam still counted it.
        let tracked = sessions::create_session(&db, &game.id, DEVICE).unwrap();
        sessions::end_session(&db, &tracked.id, 3 * HOUR_MS, 3 * HOUR_MS, 0, "local").unwrap();
        corrections::discard_session(&db, &tracked.id, "Left it running").unwrap();
        // An hour on a console that Steam never saw, two on a Steam Deck.
        let start =
            (Utc::now() - chrono::Duration::hours(10)).to_rfc3339_opts(SecondsFormat::Millis, true);
        corrections::add_manual_session(&db, &game.id, DEVICE, &start, HOUR_MS, "", None).unwrap();
        let later =
            (Utc::now() - chrono::Duration::hours(5)).to_rfc3339_opts(SecondsFormat::Millis, true);
        corrections::add_manual_session(
            &db,
            &game.id,
            DEVICE,
            &later,
            2 * HOUR_MS,
            "",
            Some(STEAM_SOURCE),
        )
        .unwrap();

        let steam = SteamPlaytime {
            account: None,
            apps: HashMap::from([(
                "730".to_string(),
                AppPlaytime {
                    minutes: 600,
                    last_played: None,
                },
            )]),
        };
        let preview = candidates(&db, &steam, |_| Some("730".to_string())).unwrap();
        assert_eq!(preview.games[0].tracked_before_ms, 5 * HOUR_MS);
        assert_eq!(preview.games[0].earlier_ms, 5 * HOUR_MS);
    }

    #[test]
    fn a_copy_from_another_launcher_is_not_what_steam_saw() {
        const DEVICE: &str = "test-device";
        const HOUR_MS: i64 = 3_600_000;
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, DEVICE, "windows", "0.1.0").unwrap();
        let create = |launcher: &str, folder: &str| {
            games::create_game(
                &db,
                &CreateGame {
                    title: "Cyberpunk 2077".into(),
                    executable_path: Some(format!("{folder}/Cyberpunk2077.exe")),
                    install_folder: Some(folder.into()),
                    launcher_source: Some(launcher.into()),
                },
            )
            .unwrap()
        };
        let steam_copy = create(STEAM_SOURCE, "C:/Steam/steamapps/common/Cyberpunk 2077");
        let gog_copy = create("gog", "C:/GOG Games/Cyberpunk 2077");
        // The Steam copy counts as the GOG copy, the one the library shows.
        games::link_game(&db, &steam_copy.id, Some(&gog_copy.id)).unwrap();
        let played = |game_id: &str, hours: i64| {
            let session = sessions::create_session(&db, game_id, DEVICE).unwrap();
            let ms = hours * HOUR_MS;
            sessions::end_session(&db, &session.id, ms, ms, 0, "local").unwrap();
        };
        played(&steam_copy.id, 3);
        played(&gog_copy.id, 2);
        let steam = SteamPlaytime {
            account: None,
            apps: HashMap::from([(
                "1091500".to_string(),
                AppPlaytime {
                    minutes: 600,
                    last_played: None,
                },
            )]),
        };

        let preview = candidates(&db, &steam, |folder| {
            folder.contains("steamapps").then(|| "1091500".to_string())
        })
        .unwrap();
        assert_eq!(preview.games.len(), 1);
        assert_eq!(preview.games[0].game_id, steam_copy.id);
        assert_eq!(preview.games[0].tracked_before_ms, 3 * HOUR_MS);
        assert_eq!(preview.games[0].earlier_ms, 7 * HOUR_MS);

        let entries: Vec<EarlierPlaytime> = preview
            .games
            .iter()
            .map(|candidate| EarlierPlaytime {
                game_id: candidate.game_id.clone(),
                source: STEAM_SOURCE.into(),
                launcher_minutes: candidate.launcher_minutes,
                tracked_before_ms: candidate.tracked_before_ms,
                earlier_ms: candidate.earlier_ms,
                last_played_at: None,
                imported_at: "2026-10-17T10:00:00Z".into(),
            })
            .collect();
        earlier_playtime::replace_earlier_playtime(&db, &entries).unwrap();
        // The library shows it for the game the Steam copy counts as.
        let shown = earlier_playtime::list_shown_earlier_playtime(&db).unwrap();
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].game_id, gog_copy.id);
        assert_eq!(shown[0].earlier_ms, 7 * HOUR_MS);
    }
}
