// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! A timer for play away from this PC, on a console or a handheld. It keeps
//! only the game and when it started, so it survives a restart, and the
//! player saves the play as a Manual session when it stops.

use serde::{Deserialize, Serialize};

use crate::constants::AWAY_TIMER_SETTING;
use crate::db::connection::Database;
use crate::db::repo::{games, settings};
use crate::error::{Result, VaultimeError};
use crate::integrity;

/// The game timed and when the timer started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwayTimer {
    pub game_id: String,
    pub started_at: String,
}

/// The running timer, if any. One whose game was deleted since is dropped.
pub fn current(db: &Database) -> Result<Option<AwayTimer>> {
    let Some(value) =
        settings::get_setting(db, AWAY_TIMER_SETTING)?.filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let timer = serde_json::from_str::<AwayTimer>(&value).ok();
    let Some(timer) = timer else {
        clear(db)?;
        return Ok(None);
    };
    match games::get_game(db, &timer.game_id) {
        Ok(_) => Ok(Some(timer)),
        Err(VaultimeError::NotFound(_)) => {
            clear(db)?;
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Starts timing `game_id` now. One timer runs at a time, and a game of
/// another PC gets its play on that PC.
pub fn start(db: &Database, game_id: &str, device_id: &str) -> Result<AwayTimer> {
    if current(db)?.is_some() {
        return Err(VaultimeError::Invalid("a timer is running already".into()));
    }
    let game = games::get_game(db, game_id)?;
    if game
        .origin_device_id
        .is_some_and(|origin| origin != device_id)
    {
        return Err(VaultimeError::Invalid(
            "play of another PC's game is timed on that PC".into(),
        ));
    }
    let timer = AwayTimer {
        game_id: game_id.to_owned(),
        started_at: integrity::now_timestamp(),
    };
    let value =
        serde_json::to_string(&timer).map_err(|error| VaultimeError::Invalid(error.to_string()))?;
    settings::set_setting(db, AWAY_TIMER_SETTING, &value)?;
    Ok(timer)
}

/// Stops the timer, once its play was saved or set aside.
pub fn clear(db: &Database) -> Result<()> {
    settings::set_setting(db, AWAY_TIMER_SETTING, "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playtime::testing::{database, game};

    #[test]
    fn a_timer_runs_until_it_is_cleared() {
        let db = database();
        let hades = game(&db, "Hades");
        assert_eq!(current(&db).unwrap(), None);
        let timer = start(&db, &hades, "device").unwrap();
        assert_eq!(current(&db).unwrap(), Some(timer));
        assert!(start(&db, &hades, "device").is_err(), "one timer at a time");
        clear(&db).unwrap();
        assert_eq!(current(&db).unwrap(), None);
    }

    #[test]
    fn a_deleted_game_takes_its_timer_along() {
        let db = database();
        let hades = game(&db, "Hades");
        start(&db, &hades, "device").unwrap();
        games::delete_game(&db, &hades).unwrap();
        assert_eq!(current(&db).unwrap(), None);
        assert_eq!(
            settings::get_setting(&db, AWAY_TIMER_SETTING).unwrap(),
            Some(String::new())
        );
    }

    #[test]
    fn a_game_of_another_pc_is_timed_there() {
        let db = database();
        let hades = game(&db, "Hades");
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE games SET origin_device_id = 'laptop' WHERE id = ?1",
                [&hades],
            )
            .map(|_| ())
            .map_err(crate::db::repo::map_db)
        })
        .unwrap();
        assert!(start(&db, &hades, "device").is_err());
        assert!(start(&db, &hades, "laptop").is_ok());
    }
}
