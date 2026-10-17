// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Whether a game looks like a launcher left open while other games ran, so
//! its page can suggest the switch that sets that time aside.

use std::collections::{HashMap, HashSet};

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::constants::{
    STEPS_ASIDE_HINT_MIN_MS, STEPS_ASIDE_HINT_MIN_OPENED_FIRST_SHARE, STEPS_ASIDE_HINT_MIN_SHARE,
};
use crate::db::connection::Database;
use crate::db::repo::{games, map_db};
use crate::error::Result;
use crate::playtime::slices::{STEPS_ASIDE, steps_aside, wall_ms};

/// Why a game's page suggests stepping aside.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepsAsideHint {
    /// How long other games ran beside it.
    pub beside_ms: i64,
    /// Which share of its time on the clock that was.
    pub share: f64,
}

/// A session on the clock.
#[derive(Debug, Clone)]
struct Run {
    /// The game it counts as.
    game_id: String,
    device_id: String,
    start_ms: i64,
    end_ms: i64,
    /// Whether its game steps aside.
    stepping: bool,
}

/// How long other games ran beside a game, which share of its time on the
/// clock that was, and which share of that came from games started while
/// it was already open.
struct Measured {
    beside_ms: i64,
    share: f64,
    opened_first_share: f64,
}

/// Every session that counts as play on the clock, open ones up to `now_ms`
/// on this PC and up to their last event on another.
fn runs(db: &Database, this_device: &str, now_ms: i64) -> Result<Vec<Run>> {
    let links = games::links(db)?;
    db.with_conn(|conn| {
        let stepping: HashSet<String> = {
            let mut stmt = conn
                .prepare_cached(&format!("SELECT id FROM games WHERE {STEPS_ASIDE}"))
                .map_err(map_db)?;
            let rows = stmt.query_map([], |row| row.get(0)).map_err(map_db)?;
            rows.collect::<rusqlite::Result<_>>().map_err(map_db)?
        };
        let mut last_event = conn
            .prepare_cached(
                "SELECT event_time_wall FROM session_events
                 WHERE session_id = ?1 ORDER BY sequence DESC LIMIT 1",
            )
            .map_err(map_db)?;
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, game_id, device_id, started_at_wall, ended_at_wall FROM sessions
                 WHERE ended_at_wall IS NULL OR runtime_ms > 0",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)?;
        let mut runs = Vec::with_capacity(rows.len());
        for (id, game_id, device_id, start, end) in rows {
            let Some(start_ms) = wall_ms(&start) else {
                continue;
            };
            let end_ms = match end {
                Some(end) => wall_ms(&end).unwrap_or(start_ms),
                None if device_id == this_device => now_ms,
                None => last_event
                    .query_row([&id], |row| row.get::<_, String>(0))
                    .optional()
                    .map_err(map_db)?
                    .as_deref()
                    .and_then(wall_ms)
                    .unwrap_or(start_ms),
            };
            runs.push(Run {
                stepping: stepping.contains(&game_id),
                game_id: links.get(&game_id).cloned().unwrap_or(game_id),
                device_id,
                start_ms,
                end_ms,
            });
        }
        Ok(runs)
    })
}

/// How `game_id` ran beside other games among `runs`. Only games that do not
/// step aside count, and only on the same PC.
#[allow(
    clippy::cast_precision_loss,
    reason = "milliseconds of play fit f64 exactly"
)]
fn beside_others(game_id: &str, runs: &[Run]) -> Measured {
    let own: Vec<&Run> = runs.iter().filter(|run| run.game_id == game_id).collect();
    let own_ms: i64 = own
        .iter()
        .map(|run| (run.end_ms - run.start_ms).max(0))
        .sum();
    // Where other games ran on each PC, by start.
    let mut others: HashMap<&str, Vec<(i64, i64)>> = HashMap::new();
    for run in runs
        .iter()
        .filter(|run| run.game_id != game_id && !run.stepping && run.end_ms > run.start_ms)
    {
        others
            .entry(run.device_id.as_str())
            .or_default()
            .push((run.start_ms, run.end_ms));
    }
    for list in others.values_mut() {
        list.sort_unstable();
    }
    // The same stretches joined, so overlaps count once.
    let covered: HashMap<&str, Vec<(i64, i64)>> = others
        .iter()
        .map(|(device, list)| {
            let mut joined: Vec<(i64, i64)> = Vec::new();
            for &(start, end) in list {
                match joined.last_mut() {
                    Some(last) if start <= last.1 => last.1 = last.1.max(end),
                    _ => joined.push((start, end)),
                }
            }
            (*device, joined)
        })
        .collect();
    let mut beside_ms = 0;
    let mut opened_first_ms = 0;
    for run in &own {
        let (start, end) = (run.start_ms, run.end_ms);
        if end <= start {
            continue;
        }
        let device = run.device_id.as_str();
        let joined = covered.get(device).map_or(&[][..], Vec::as_slice);
        let first = joined.partition_point(|&(_, to)| to <= start);
        for &(from, to) in joined[first..].iter().take_while(|(from, _)| *from < end) {
            beside_ms += (to.min(end) - from.max(start)).max(0);
        }
        // Games that started while it was open.
        let later = others.get(device).map_or(&[][..], Vec::as_slice);
        let first = later.partition_point(|&(from, _)| from < start);
        let mut cursor = start;
        for &(from, to) in later[first..].iter().take_while(|(from, _)| *from < end) {
            let to = to.min(end);
            opened_first_ms += (to - cursor.max(from)).max(0);
            cursor = cursor.max(to);
        }
    }
    Measured {
        beside_ms,
        share: if own_ms > 0 {
            beside_ms as f64 / own_ms as f64
        } else {
            0.0
        },
        opened_first_share: if beside_ms > 0 {
            (opened_first_ms as f64 / beside_ms as f64).min(1.0)
        } else {
            0.0
        },
    }
}

/// Why `game_id`'s page should suggest that it steps aside, or None when it
/// does already or does not look like a launcher left open.
pub fn steps_aside_hint(
    db: &Database,
    game_id: &str,
    this_device: &str,
    now_ms: i64,
) -> Result<Option<StepsAsideHint>> {
    if db.with_conn(|conn| steps_aside(conn, game_id))? {
        return Ok(None);
    }
    let beside = beside_others(game_id, &runs(db, this_device, now_ms)?);
    let suggest = beside.share >= STEPS_ASIDE_HINT_MIN_SHARE
        && beside.beside_ms >= STEPS_ASIDE_HINT_MIN_MS
        && beside.opened_first_share >= STEPS_ASIDE_HINT_MIN_OPENED_FIRST_SHARE;
    Ok(suggest.then_some(StepsAsideHint {
        beside_ms: beside.beside_ms,
        share: beside.share,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60_000;

    fn run(game: &str, start_minute: i64, minutes: i64) -> Run {
        Run {
            game_id: game.into(),
            device_id: "pc".into(),
            start_ms: start_minute * MINUTE,
            end_ms: (start_minute + minutes) * MINUTE,
            stepping: false,
        }
    }

    #[test]
    fn measures_how_long_other_games_ran_beside_a_game() {
        let runs = [run("client", 0, 120), run("match", 30, 90)];
        let beside = beside_others("client", &runs);
        assert_eq!(beside.beside_ms, 90 * MINUTE);
        assert!((beside.share - 0.75).abs() < f64::EPSILON);
        assert!((beside.opened_first_share - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tells_a_launcher_from_the_game_played_inside_it() {
        let runs = [run("client", 0, 180), run("match", 60, 60)];
        // The match ran beside the client all its time, but the client was there first.
        let inside = beside_others("match", &runs);
        assert!((inside.share - 1.0).abs() < f64::EPSILON);
        assert!(inside.opened_first_share.abs() < f64::EPSILON);
        assert!((beside_others("client", &runs).opened_first_share - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn looks_only_at_games_on_the_same_pc_that_do_not_step_aside() {
        let mut elsewhere = run("match", 30, 60);
        elsewhere.device_id = "laptop".into();
        let mut store = run("store", 30, 60);
        store.stepping = true;
        let beside = beside_others("client", &[run("client", 0, 120), elsewhere, store]);
        assert_eq!(beside.beside_ms, 0);
    }
}
