// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where each session's time fell on the clock, a quarter hour at a time.
//!
//! Slices are worked out from the session events, which stay the record.
//! Between two checkpoints a session's time is all active or all idle, so
//! spreading each stretch over the quarter hours it covers places both
//! exactly. Sessions from before checkpoints, and sessions added by hand,
//! spread the same way over the points they have.

use std::collections::BTreeMap;

use chrono::DateTime;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::constants::{PLAY_SLICE_MS, PLAY_SLICES_VERSION, PLAY_SLICES_VERSION_SETTING};
use crate::db::connection::Database;
use crate::db::models::{Session, SessionEvent};
use crate::db::repo::map_db;
use crate::db::repo::sessions::row_to_session;
use crate::error::Result;
use crate::integrity;
use crate::tracking::live::LiveCounters;

/// A session's time in one quarter hour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slice {
    /// Start of the quarter hour, milliseconds since 1970 in UTC.
    pub start_ms: i64,
    pub runtime_ms: i64,
    pub active_ms: i64,
    pub idle_ms: i64,
}

/// The counters of a session at one moment.
#[derive(Debug, Clone, Copy)]
struct Point {
    wall_ms: i64,
    counters: [i64; MEASURES],
}

/// Runtime, active and idle time, in that order.
const MEASURES: usize = 3;

fn wall_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.timestamp_millis())
}

fn counters_in(payload: &Value) -> Option<[i64; MEASURES]> {
    let field = |key: &str| payload.get(key).and_then(Value::as_i64);
    Some([field("runtime_ms")?, field("active_ms")?, field("idle_ms")?])
}

/// The counters at every event that records them, in the order they were
/// written. A tracking gap is a point where the counters stood still, so the
/// time after a sleep stays after it. A correction is left out, the end of
/// the session row cuts the points instead.
fn points(events: &[SessionEvent]) -> Vec<Point> {
    let mut points = Vec::new();
    for event in events {
        let payload: Option<Value> = serde_json::from_str(&event.payload_json).ok();
        let at = wall_ms(&event.event_time_wall);
        let in_payload = |key: &str| {
            payload
                .as_ref()
                .and_then(|payload| payload.get(key))
                .and_then(Value::as_str)
                .and_then(wall_ms)
        };
        let counters = payload.as_ref().and_then(counters_in);
        match event.event_type.as_str() {
            "started" => points.extend(at.map(|wall_ms| Point {
                wall_ms,
                counters: [0; MEASURES],
            })),
            "heartbeat" | "ended" => {
                if let (Some(wall_ms), Some(counters)) = (at, counters) {
                    points.push(Point { wall_ms, counters });
                }
            }
            "tracking_gap" => {
                if let (Some(wall_ms), Some(last)) = (at, points.last()) {
                    let counters = last.counters;
                    points.push(Point { wall_ms, counters });
                }
            }
            "recovered" => {
                if let (Some(wall_ms), Some(counters)) =
                    (in_payload("ended_at_wall").or(at), counters)
                {
                    points.push(Point { wall_ms, counters });
                }
            }
            "added_manually" => {
                if let (Some(start), Some(end), Some(counters)) = (
                    in_payload("started_at_wall"),
                    in_payload("ended_at_wall"),
                    counters,
                ) {
                    points.push(Point {
                        wall_ms: start,
                        counters: [0; MEASURES],
                    });
                    points.push(Point {
                        wall_ms: end,
                        counters,
                    });
                }
            }
            _ => {}
        }
    }
    points
}

fn slice_of(wall_ms: i64) -> i64 {
    wall_ms.div_euclid(PLAY_SLICE_MS) * PLAY_SLICE_MS
}

/// Splits `amount` in proportion to `weights`, in whole milliseconds that
/// add up to `amount`. Leftovers go to the largest remainders, the earlier
/// part first on a tie.
fn apportion(weights: &[i64], amount: i64) -> Vec<i64> {
    let sum: i128 = weights.iter().map(|weight| i128::from(*weight)).sum();
    if amount <= 0 || sum <= 0 {
        return vec![0; weights.len()];
    }
    let products: Vec<i128> = weights
        .iter()
        .map(|weight| i128::from(*weight) * i128::from(amount))
        .collect();
    let mut parts: Vec<i64> = products
        .iter()
        .map(|product| i64::try_from(product / sum).unwrap_or(i64::MAX))
        .collect();
    let mut left = amount - parts.iter().sum::<i64>();
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| {
        (products[b] % sum)
            .cmp(&(products[a] % sum))
            .then(a.cmp(&b))
    });
    // Each part lost less than a millisecond, so one round is enough.
    for index in order {
        if left <= 0 {
            break;
        }
        parts[index] += 1;
        left -= 1;
    }
    parts
}

/// Adds `amounts` spread evenly over the clock from `from` to `to`. Time
/// whose clock went backwards lands in the quarter hour it ended in.
fn spread(
    placed: &mut BTreeMap<i64, [i64; MEASURES]>,
    from: i64,
    to: i64,
    amounts: [i64; MEASURES],
) {
    if to <= from {
        let entry = placed.entry(slice_of(to)).or_default();
        for (value, amount) in entry.iter_mut().zip(amounts) {
            *value += amount;
        }
        return;
    }
    let mut overlaps = Vec::new();
    let mut cursor = from;
    while cursor < to {
        let slice = slice_of(cursor);
        let stop = (slice + PLAY_SLICE_MS).min(to);
        overlaps.push((slice, stop - cursor));
        cursor = stop;
    }
    let weights: Vec<i64> = overlaps.iter().map(|(_, overlap)| *overlap).collect();
    for (index, amount) in amounts.into_iter().enumerate() {
        for ((slice, _), part) in overlaps.iter().zip(apportion(&weights, amount)) {
            placed.entry(*slice).or_default()[index] += part;
        }
    }
}

/// Where `session`'s time fell, from its events. For a running session,
/// `live` holds the time of the latest tick and the counters at it. The
/// slices add up to the session's totals exactly.
pub fn slices_of(
    session: &Session,
    events: &[SessionEvent],
    live: Option<(i64, LiveCounters)>,
) -> Vec<Slice> {
    let totals = match live {
        Some((_, counters)) => [counters.runtime_ms, counters.active_ms, counters.idle_ms],
        None => [session.runtime_ms, session.active_ms, session.idle_ms],
    };
    let Some(start) = wall_ms(&session.started_at_wall) else {
        return Vec::new();
    };
    if totals.iter().all(|total| *total <= 0) {
        return Vec::new();
    }
    let end = match live {
        Some((now, _)) => now,
        None => session
            .ended_at_wall
            .as_deref()
            .and_then(wall_ms)
            .unwrap_or(start),
    };

    let mut points: Vec<Point> = points(events)
        .into_iter()
        .filter(|point| point.wall_ms <= end)
        .collect();
    if let Some((now, counters)) = live {
        points.push(Point {
            wall_ms: now,
            counters: [counters.runtime_ms, counters.active_ms, counters.idle_ms],
        });
    }

    let mut placed: BTreeMap<i64, [i64; MEASURES]> = BTreeMap::new();
    for pair in points.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let mut amounts = [0; MEASURES];
        for (index, amount) in amounts.iter_mut().enumerate() {
            *amount = (to.counters[index] - from.counters[index]).max(0);
        }
        if amounts.iter().any(|amount| *amount > 0) {
            spread(&mut placed, from.wall_ms, to.wall_ms, amounts);
        }
    }
    // Time the points cannot place, like a record cut short, spreads over
    // the whole session.
    for (index, total) in totals.iter().enumerate() {
        if *total > 0 && placed.values().all(|values| values[index] <= 0) {
            let mut amounts = [0; MEASURES];
            amounts[index] = *total;
            spread(&mut placed, start, end, amounts);
        }
    }

    let starts: Vec<i64> = placed.keys().copied().collect();
    let per_measure: Vec<Vec<i64>> = (0..MEASURES)
        .map(|index| {
            let weights: Vec<i64> = placed.values().map(|values| values[index]).collect();
            apportion(&weights, totals[index])
        })
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(slot, &start_ms)| Slice {
            start_ms,
            runtime_ms: per_measure[0][slot],
            active_ms: per_measure[1][slot],
            idle_ms: per_measure[2][slot],
        })
        .filter(|slice| slice.runtime_ms != 0 || slice.active_ms != 0 || slice.idle_ms != 0)
        .collect()
}

/// Works out the slices of a session again. An open session keeps none, its
/// time is placed when it is asked for.
pub fn rebuild_session(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM play_slices WHERE session_id = ?1",
        [session_id],
    )
    .map_err(map_db)?;
    let session = conn
        .query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [session_id],
            row_to_session,
        )
        .optional()
        .map_err(map_db)?;
    let Some(session) = session.filter(|session| session.ended_at_wall.is_some()) else {
        return Ok(());
    };
    let events = integrity::load_session_events(conn, session_id)?;
    let mut insert = conn
        .prepare_cached(
            "INSERT INTO play_slices (session_id, slice_start, runtime_ms, active_ms, idle_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .map_err(map_db)?;
    for slice in slices_of(&session, &events, None) {
        insert
            .execute(params![
                session_id,
                slice.start_ms,
                slice.runtime_ms,
                slice.active_ms,
                slice.idle_ms
            ])
            .map_err(map_db)?;
    }
    Ok(())
}

/// Builds the slices of every closed session again. Returns how many
/// sessions it went through.
pub fn rebuild_all(db: &Database) -> Result<usize> {
    db.with_transaction(|conn| {
        conn.execute("DELETE FROM play_slices", [])
            .map_err(map_db)?;
        let ids: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT id FROM sessions WHERE ended_at_wall IS NOT NULL")
                .map_err(map_db)?;
            let rows = stmt.query_map([], |row| row.get(0)).map_err(map_db)?;
            rows.collect::<rusqlite::Result<_>>().map_err(map_db)?
        };
        for id in &ids {
            rebuild_session(conn, id)?;
        }
        conn.execute(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
            params![PLAY_SLICES_VERSION_SETTING, PLAY_SLICES_VERSION.to_string()],
        )
        .map_err(map_db)?;
        Ok(ids.len())
    })
}

/// Builds every slice again when they were made another way or are missing,
/// like after a restore. Returns how many sessions it went through.
pub fn ensure_current(db: &Database) -> Result<usize> {
    let built = crate::db::repo::settings::get_setting(db, PLAY_SLICES_VERSION_SETTING)?;
    if built.as_deref() == Some(PLAY_SLICES_VERSION.to_string().as_str()) {
        return Ok(0);
    }
    rebuild_all(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60_000;

    fn at(hour: u32, minute: u32, second: u32) -> String {
        format!("2026-10-02T{hour:02}:{minute:02}:{second:02}.000Z")
    }

    fn ms(value: &str) -> i64 {
        wall_ms(value).unwrap()
    }

    fn event(sequence: i64, event_type: &str, wall: &str, payload: &Value) -> SessionEvent {
        SessionEvent {
            id: format!("event-{sequence}"),
            session_id: "session".into(),
            sequence,
            event_type: event_type.into(),
            event_time_wall: wall.into(),
            event_time_monotonic: None,
            payload_json: payload.to_string(),
            hash_prev: None,
            hash_self: None,
            signature: None,
        }
    }

    fn counters(runtime: i64, active: i64, idle: i64) -> Value {
        serde_json::json!({ "runtime_ms": runtime, "active_ms": active, "idle_ms": idle })
    }

    fn session(start: &str, end: Option<&str>, totals: [i64; 3]) -> Session {
        Session {
            id: "session".into(),
            game_id: "game".into(),
            device_id: "device".into(),
            started_at_wall: start.into(),
            ended_at_wall: end.map(Into::into),
            elapsed_monotonic_ms: totals[0],
            active_ms: totals[1],
            idle_ms: totals[2],
            runtime_ms: totals[0],
            integrity_status: "local".into(),
            closed_cleanly: true,
        }
    }

    fn sums(slices: &[Slice]) -> [i64; 3] {
        slices.iter().fold([0; 3], |sum, slice| {
            [
                sum[0] + slice.runtime_ms,
                sum[1] + slice.active_ms,
                sum[2] + slice.idle_ms,
            ]
        })
    }

    #[test]
    fn active_and_idle_land_in_the_quarter_hours_they_happened() {
        // Active from 18:10 to 18:20, then idle until 18:40.
        let events = [
            event(1, "started", &at(18, 10, 0), &serde_json::json!({})),
            event(
                2,
                "heartbeat",
                &at(18, 20, 0),
                &counters(10 * MINUTE, 10 * MINUTE, 0),
            ),
            event(
                3,
                "ended",
                &at(18, 40, 0),
                &counters(30 * MINUTE, 10 * MINUTE, 20 * MINUTE),
            ),
        ];
        let slices = slices_of(
            &session(
                &at(18, 10, 0),
                Some(&at(18, 40, 0)),
                [30 * MINUTE, 10 * MINUTE, 20 * MINUTE],
            ),
            &events,
            None,
        );

        assert_eq!(
            slices,
            vec![
                Slice {
                    start_ms: ms(&at(18, 0, 0)),
                    runtime_ms: 5 * MINUTE,
                    active_ms: 5 * MINUTE,
                    idle_ms: 0
                },
                Slice {
                    start_ms: ms(&at(18, 15, 0)),
                    runtime_ms: 15 * MINUTE,
                    active_ms: 5 * MINUTE,
                    idle_ms: 10 * MINUTE
                },
                Slice {
                    start_ms: ms(&at(18, 30, 0)),
                    runtime_ms: 10 * MINUTE,
                    active_ms: 0,
                    idle_ms: 10 * MINUTE
                },
            ]
        );
    }

    #[test]
    fn a_sleep_gap_gets_no_time() {
        // Played 18:00 to 18:10, slept until 20:00, played until 20:05.
        let events = [
            event(1, "started", &at(18, 0, 0), &serde_json::json!({})),
            event(
                2,
                "heartbeat",
                &at(18, 10, 0),
                &counters(10 * MINUTE, 10 * MINUTE, 0),
            ),
            event(
                3,
                "tracking_gap",
                &at(20, 0, 0),
                &serde_json::json!({ "wall_gap_ms": 110 * MINUTE }),
            ),
            event(
                4,
                "ended",
                &at(20, 5, 0),
                &counters(15 * MINUTE, 15 * MINUTE, 0),
            ),
        ];
        let slices = slices_of(
            &session(
                &at(18, 0, 0),
                Some(&at(20, 5, 0)),
                [15 * MINUTE, 15 * MINUTE, 0],
            ),
            &events,
            None,
        );

        assert_eq!(
            slices,
            vec![
                Slice {
                    start_ms: ms(&at(18, 0, 0)),
                    runtime_ms: 10 * MINUTE,
                    active_ms: 10 * MINUTE,
                    idle_ms: 0
                },
                Slice {
                    start_ms: ms(&at(20, 0, 0)),
                    runtime_ms: 5 * MINUTE,
                    active_ms: 5 * MINUTE,
                    idle_ms: 0
                },
            ]
        );
    }

    #[test]
    fn a_corrected_session_ends_where_the_correction_says() {
        // Ran 18:00 to 20:00, corrected to end at 19:00 with an hour.
        let events = [
            event(1, "started", &at(18, 0, 0), &serde_json::json!({})),
            event(
                2,
                "heartbeat",
                &at(19, 0, 0),
                &counters(60 * MINUTE, 60 * MINUTE, 0),
            ),
            event(
                3,
                "ended",
                &at(20, 0, 0),
                &counters(120 * MINUTE, 60 * MINUTE, 60 * MINUTE),
            ),
            event(
                4,
                "corrected",
                &at(21, 0, 0),
                &counters(60 * MINUTE, 60 * MINUTE, 0),
            ),
        ];
        let slices = slices_of(
            &session(
                &at(18, 0, 0),
                Some(&at(19, 0, 0)),
                [60 * MINUTE, 60 * MINUTE, 0],
            ),
            &events,
            None,
        );

        assert_eq!(sums(&slices), [60 * MINUTE, 60 * MINUTE, 0]);
        assert!(
            slices
                .iter()
                .all(|slice| slice.start_ms < ms(&at(19, 0, 0)))
        );
    }

    #[test]
    fn a_session_added_by_hand_spreads_evenly() {
        let events = [event(
            1,
            "added_manually",
            &at(21, 0, 0),
            &serde_json::json!({
                "started_at_wall": at(19, 0, 0),
                "ended_at_wall": at(20, 0, 0),
                "runtime_ms": 60 * MINUTE,
                "active_ms": 60 * MINUTE,
                "idle_ms": 0,
            }),
        )];
        let slices = slices_of(
            &session(
                &at(19, 0, 0),
                Some(&at(20, 0, 0)),
                [60 * MINUTE, 60 * MINUTE, 0],
            ),
            &events,
            None,
        );

        assert_eq!(slices.len(), 4);
        assert!(slices.iter().all(|slice| slice.runtime_ms == 15 * MINUTE));
    }

    #[test]
    fn a_running_session_places_time_up_to_the_latest_tick() {
        let events = [
            event(1, "started", &at(18, 0, 0), &serde_json::json!({})),
            event(
                2,
                "heartbeat",
                &at(18, 10, 0),
                &counters(10 * MINUTE, 10 * MINUTE, 0),
            ),
        ];
        let live = LiveCounters {
            runtime_ms: 20 * MINUTE,
            active_ms: 15 * MINUTE,
            idle_ms: 5 * MINUTE,
        };
        let slices = slices_of(
            &session(&at(18, 0, 0), None, [10 * MINUTE, 10 * MINUTE, 0]),
            &events,
            Some((ms(&at(18, 20, 0)), live)),
        );

        assert_eq!(sums(&slices), [20 * MINUTE, 15 * MINUTE, 5 * MINUTE]);
        assert_eq!(slices.last().unwrap().start_ms, ms(&at(18, 15, 0)));
    }

    #[test]
    fn rounding_keeps_the_totals_exact() {
        // A minute of runtime over three quarter hours does not divide evenly.
        let events = [
            event(1, "started", &at(18, 10, 0), &serde_json::json!({})),
            event(2, "ended", &at(18, 40, 0), &counters(60_001, 60_001, 0)),
        ];
        let slices = slices_of(
            &session(&at(18, 10, 0), Some(&at(18, 40, 0)), [60_001, 60_001, 0]),
            &events,
            None,
        );

        assert_eq!(sums(&slices), [60_001, 60_001, 0]);
    }

    #[test]
    fn ending_a_session_stores_its_slices_and_a_new_version_rebuilds_them() {
        use crate::db::models::CreateGame;
        use crate::db::repo::{devices, games, sessions, settings};

        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Slice Game".into(),
                executable_path: Some("game.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        let session = sessions::create_session(&db, &game.id, "device").unwrap();
        sessions::end_session(&db, &session.id, 90_000, 60_000, 30_000, "local").unwrap();
        let stored = |db: &Database| -> (i64, i64) {
            db.with_conn(|conn| {
                conn.query_row(
                    "SELECT COUNT(*), COALESCE(SUM(runtime_ms), 0) FROM play_slices",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(map_db)
            })
            .unwrap()
        };
        assert_eq!(stored(&db).1, 90_000);

        db.with_conn(|conn| {
            conn.execute("DELETE FROM play_slices", [])
                .map_err(map_db)?;
            Ok(())
        })
        .unwrap();
        settings::set_setting(&db, PLAY_SLICES_VERSION_SETTING, "0").unwrap();
        assert_eq!(ensure_current(&db).unwrap(), 1);
        assert_eq!(stored(&db).1, 90_000);
        // Current now, so the next start does nothing.
        assert_eq!(ensure_current(&db).unwrap(), 0);
    }

    #[test]
    fn a_discarded_session_has_no_slices() {
        let events = [
            event(1, "started", &at(18, 0, 0), &serde_json::json!({})),
            event(
                2,
                "ended",
                &at(19, 0, 0),
                &counters(60 * MINUTE, 60 * MINUTE, 0),
            ),
            event(3, "corrected", &at(20, 0, 0), &counters(0, 0, 0)),
        ];
        let discarded = session(&at(18, 0, 0), Some(&at(18, 0, 0)), [0, 0, 0]);
        assert!(slices_of(&discarded, &events, None).is_empty());
    }
}
