// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Changes to the library that also change where play counts: linking a
//! game, letting it step aside and deleting it. Each works out the slices
//! it touches once more.

use log::warn;

use crate::assets::{self, AssetManager};
use crate::constants::PLAY_SLICES_VERSION_SETTING;
use crate::db::connection::Database;
use crate::db::models::Game;
use crate::db::repo::{games, settings};
use crate::error::Result;
use crate::playtime::slices;

/// Makes a game count as a game of this PC, or as itself again, and works
/// out once more where the time of every game it now counts with fell, since
/// whether they step aside follows the game shown.
pub fn link_game(db: &Database, game_id: &str, linked_game_id: Option<&str>) -> Result<()> {
    games::link_game(db, game_id, linked_game_id)?;
    let shown = db.with_conn(|conn| games::shown_id(conn, game_id))?;
    slices::rebuild_for_game(db, &shown)
}

/// Makes a game count only while no other game runs, like a launcher, or
/// always again, and works out where all play fell once more.
pub fn set_steps_aside(db: &Database, game_id: &str, steps_aside: bool) -> Result<Game> {
    let game = games::set_steps_aside(db, game_id, steps_aside)?;
    slices::rebuild_for_game(db, game_id)?;
    Ok(game)
}

/// Deletes a game with its sessions and artwork. Games that stepped aside
/// for it count that time again.
pub fn delete_game(db: &Database, asset_manager: &AssetManager, game_id: &str) -> Result<bool> {
    let spans = slices::spans_beside(db, game_id)?;
    let deleted = assets::delete_game(db, asset_manager, game_id)?;
    if let Err(error) = slices::rebuild_aside_around_spans(db, &spans) {
        // The game is gone either way, so the next start builds every slice.
        warn!("slices beside a deleted game not rebuilt: {error}");
        if let Err(error) = settings::set_setting(db, PLAY_SLICES_VERSION_SETTING, "0") {
            warn!("slices not marked for a rebuild: {error}");
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use chrono_tz::Europe::Berlin;

    use super::*;
    use crate::playtime::testing::{MINUTE, database, game, played, utc};
    use crate::playtime::totals::{Bucket, play_totals};
    use crate::tracking::live::LiveSessions;

    fn runtime_of(db: &Database, game_id: &str) -> i64 {
        let day = |text| NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap();
        play_totals(
            db,
            &LiveSessions::default(),
            &Berlin,
            day("2026-09-28"),
            day("2026-09-29"),
            Bucket::Day,
            Some(game_id),
            utc("2026-09-30T12:00:00Z").timestamp_millis(),
        )
        .unwrap()
        .iter()
        .map(|total| total.runtime_ms)
        .sum()
    }

    #[test]
    fn a_launcher_counts_its_time_again_once_the_game_beside_it_is_deleted() {
        let db = database();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        played(&db, &client, "2026-09-28T16:00:00Z", 180);
        played(&db, &match_game, "2026-09-28T16:30:00Z", 150);
        set_steps_aside(&db, &client, true).unwrap();
        assert_eq!(runtime_of(&db, &client), 30 * MINUTE);

        let cache = std::env::temp_dir().join(format!("vaultime-library-{}", uuid::Uuid::new_v4()));
        assert!(delete_game(&db, &AssetManager::new(cache), &match_game).unwrap());
        assert_eq!(runtime_of(&db, &client), 180 * MINUTE);
        assert!(games::get_game(&db, &match_game).is_err());
    }

    #[test]
    fn a_linked_install_counts_once_beside_the_game_it_counts_as() {
        let db = database();
        let (steam, gog) = (game(&db, "Steam copy"), game(&db, "GOG copy"));
        played(&db, &steam, "2026-09-28T16:00:00Z", 60);
        played(&db, &gog, "2026-09-28T16:30:00Z", 60);
        link_game(&db, &gog, Some(&steam)).unwrap();
        // 16:00 to 17:30, the half hour both ran counted once.
        assert_eq!(runtime_of(&db, &steam), 90 * MINUTE);
        link_game(&db, &gog, None).unwrap();
        assert_eq!(runtime_of(&db, &steam), 60 * MINUTE);
    }
}
