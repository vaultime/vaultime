// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where each session's time fell on the clock, a quarter hour at a time.
//!
//! Slices are worked out from the session events, which stay the record.
//! Between two checkpoints a session's time is all active or all idle, so
//! spreading each stretch over the quarter hours it covers places both
//! exactly. Sessions from before checkpoints, and sessions added by hand,
//! spread the same way over the points they have.

use std::borrow::Cow;
use std::collections::BTreeMap;

use chrono::DateTime;
use rusqlite::{Connection, OptionalExtension, params};

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

/// The fields of an event payload that place time, read without building
/// the whole payload, since every checkpoint is read on each rebuild.
#[derive(Default, serde::Deserialize)]
struct Placing<'a> {
    runtime_ms: Option<i64>,
    active_ms: Option<i64>,
    idle_ms: Option<i64>,
    #[serde(borrow)]
    started_at_wall: Option<Cow<'a, str>>,
    #[serde(borrow)]
    ended_at_wall: Option<Cow<'a, str>>,
}

impl Placing<'_> {
    fn counters(&self) -> Option<[i64; MEASURES]> {
        Some([self.runtime_ms?, self.active_ms?, self.idle_ms?])
    }
}

/// The counters at every event that records them, in the order they were
/// written. A tracking gap is a point where the counters stood still, so the
/// time after a sleep stays after it. A correction is left out, the end of
/// the session row cuts the points instead.
fn points(events: &[SessionEvent]) -> Vec<Point> {
    let mut points = Vec::new();
    for event in events {
        let payload: Placing = serde_json::from_str(&event.payload_json).unwrap_or_default();
        let at = wall_ms(&event.event_time_wall);
        let counters = payload.counters();
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
                if let (Some(wall_ms), Some(counters)) = (
                    payload.ended_at_wall.as_deref().and_then(wall_ms).or(at),
                    counters,
                ) {
                    points.push(Point { wall_ms, counters });
                }
            }
            "added_manually" => {
                if let (Some(start), Some(end), Some(counters)) = (
                    payload.started_at_wall.as_deref().and_then(wall_ms),
                    payload.ended_at_wall.as_deref().and_then(wall_ms),
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

/// A session's runtime, active and idle time at `wall_ms`, worked out from
/// the points around it. Between two checkpoints the time is all of one
/// kind, so this is exact for tracked sessions. None for a session without
/// checkpoints, like one added by hand, and before a session's start.
pub(crate) fn counters_at(events: &[SessionEvent], wall_ms: i64) -> Option<[i64; MEASURES]> {
    if !events.iter().any(|event| event.event_type == "heartbeat") {
        return None;
    }
    let points = points(events);
    let after = points.iter().position(|point| point.wall_ms > wall_ms);
    match after {
        Some(0) => None,
        None => points.last().map(|point| point.counters),
        Some(index) => {
            let (from, to) = (points[index - 1], points[index]);
            let span = i128::from(to.wall_ms - from.wall_ms);
            let into = i128::from(wall_ms - from.wall_ms);
            let mut counters = from.counters;
            for (value, (start, end)) in counters
                .iter_mut()
                .zip(from.counters.iter().zip(to.counters))
            {
                let grown = i128::from(end - start) * into / span.max(1);
                *value = start + i64::try_from(grown).unwrap_or(0);
            }
            Some(counters)
        }
    }
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

/// Splits the clock from `from` to `to` where it enters and leaves the
/// stretches of `aside`, sorted and apart. Each piece says whether it lies
/// in one. A moment without length belongs to the stretch it ends in.
fn split_by(from: i64, to: i64, aside: &[(i64, i64)]) -> Vec<(i64, i64, bool)> {
    if to <= from {
        let inside = aside.iter().any(|&(start, end)| start < to && to <= end);
        return vec![(from, to, inside)];
    }
    let mut pieces = Vec::new();
    let mut cursor = from;
    for &(start, end) in aside {
        if end <= cursor {
            continue;
        }
        if start >= to {
            break;
        }
        if start > cursor {
            pieces.push((cursor, start, false));
        }
        let stop = end.min(to);
        pieces.push((start.max(cursor), stop, true));
        cursor = stop;
    }
    if cursor < to {
        pieces.push((cursor, to, false));
    }
    pieces
}

/// Where `session`'s time fell, from its events. For a running session,
/// `live` holds the time of the latest tick and the counters at it. Time in
/// the stretches of `aside`, when other games ran beside a game that steps
/// aside for them, is left out. The slices and the time left out add up to
/// the session's totals exactly.
pub fn slices_of(
    session: &Session,
    events: &[SessionEvent],
    live: Option<(i64, LiveCounters)>,
    aside: &[(i64, i64)],
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
    let mut set_aside = [0_i64; MEASURES];
    for pair in points.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let mut amounts = [0; MEASURES];
        for (index, amount) in amounts.iter_mut().enumerate() {
            *amount = (to.counters[index] - from.counters[index]).max(0);
        }
        if amounts.iter().all(|amount| *amount == 0) {
            continue;
        }
        // Between two points the time is all of one kind, so it splits by
        // how long each piece lasted.
        let pieces = split_by(from.wall_ms, to.wall_ms, aside);
        let lengths: Vec<i64> = pieces
            .iter()
            .map(|&(start, end, _)| (end - start).max(1))
            .collect();
        let parts: Vec<Vec<i64>> = amounts
            .iter()
            .map(|amount| apportion(&lengths, *amount))
            .collect();
        for (slot, &(piece_start, piece_end, beside)) in pieces.iter().enumerate() {
            let piece = [parts[0][slot], parts[1][slot], parts[2][slot]];
            if beside {
                for (total, part) in set_aside.iter_mut().zip(piece) {
                    *total += part;
                }
            } else {
                spread(&mut placed, piece_start, piece_end, piece);
            }
        }
    }
    // Time the points cannot place, like a record cut short, spreads over
    // the whole session.
    for (index, total) in totals.iter().enumerate() {
        if *total > 0 && set_aside[index] == 0 && placed.values().all(|values| values[index] <= 0) {
            let mut amounts = [0; MEASURES];
            amounts[index] = *total;
            spread(&mut placed, start, end, amounts);
        }
    }

    let starts: Vec<i64> = placed.keys().copied().collect();
    let per_measure: Vec<Vec<i64>> = (0..MEASURES)
        .map(|index| {
            let weights: Vec<i64> = placed.values().map(|values| values[index]).collect();
            // What counts keeps its share of the session's total, so a
            // correction that cut the record short scales both alike.
            let counted: i64 = weights.iter().sum();
            let whole = i128::from(counted) + i128::from(set_aside[index]);
            let target = if whole == 0 {
                0
            } else {
                i64::try_from(i128::from(totals[index]) * i128::from(counted) / whole).unwrap_or(0)
            };
            apportion(&weights, target)
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

/// Games whose switch makes them step aside, read from their metadata.
pub(crate) const STEPS_ASIDE: &str =
    "COALESCE(json_extract(games.metadata_json, '$.steps_aside'), 0) = 1";

/// Whether a game counts only while no other game runs.
pub fn steps_aside(conn: &Connection, game_id: &str) -> Result<bool> {
    conn.query_row(
        &format!("SELECT {STEPS_ASIDE} FROM games WHERE id = ?1"),
        [game_id],
        |row| row.get(0),
    )
    .optional()
    .map(Option::unwrap_or_default)
    .map_err(map_db)
}

/// The stretches of `session`'s time in which a game that does not step
/// aside ran beside it, sorted and joined, up to `now_ms` for running ones.
/// Empty unless the session's game steps aside.
pub fn aside_stretches(
    conn: &Connection,
    session: &Session,
    now_ms: i64,
) -> Result<Vec<(i64, i64)>> {
    if !steps_aside(conn, &session.game_id)? {
        return Ok(Vec::new());
    }
    let Some(start) = wall_ms(&session.started_at_wall) else {
        return Ok(Vec::new());
    };
    let end = session
        .ended_at_wall
        .as_deref()
        .and_then(wall_ms)
        .unwrap_or(now_ms);
    let mut stmt = conn
        .prepare_cached(&format!(
            "SELECT sessions.started_at_wall, sessions.ended_at_wall
             FROM sessions JOIN games ON games.id = sessions.game_id
             WHERE sessions.game_id != ?1 AND NOT ({STEPS_ASIDE})
               AND (sessions.ended_at_wall IS NULL OR sessions.runtime_ms > 0)
               AND sessions.started_at_wall < ?3
               AND (sessions.ended_at_wall IS NULL OR sessions.ended_at_wall > ?2)
             ORDER BY sessions.started_at_wall"
        ))
        .map_err(map_db)?;
    let format = |ms: i64| {
        chrono::DateTime::from_timestamp_millis(ms)
            .map_or_else(String::new, integrity::format_timestamp)
    };
    let rows = stmt
        .query_map(
            params![session.game_id, format(start), format(end)],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .map_err(map_db)?;
    let mut stretches: Vec<(i64, i64)> = Vec::new();
    for row in rows {
        let (other_start, other_end) = row.map_err(map_db)?;
        let Some(other_start) = wall_ms(&other_start) else {
            continue;
        };
        let other_end = other_end.as_deref().and_then(wall_ms).unwrap_or(now_ms);
        let (from, to) = (other_start.max(start), other_end.min(end));
        if to <= from {
            continue;
        }
        match stretches.last_mut() {
            Some(last) if from <= last.1 => last.1 = last.1.max(to),
            _ => stretches.push((from, to)),
        }
    }
    Ok(stretches)
}

/// Builds again the slices of sessions of games that step aside and ran
/// at some point between `from` and `to`, after a session there changed.
pub fn rebuild_aside_around(conn: &Connection, from: &str, to: &str) -> Result<()> {
    let ids: Vec<String> = {
        let mut stmt = conn
            .prepare_cached(&format!(
                "SELECT sessions.id FROM sessions JOIN games ON games.id = sessions.game_id
                 WHERE {STEPS_ASIDE} AND sessions.ended_at_wall IS NOT NULL
                   AND sessions.started_at_wall < ?2 AND sessions.ended_at_wall > ?1"
            ))
            .map_err(map_db)?;
        let rows = stmt
            .query_map([from, to], |row| row.get(0))
            .map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)?
    };
    for id in &ids {
        rebuild_session(conn, id)?;
    }
    Ok(())
}

/// Works out the slices of a session again, and of the sessions that step
/// aside around it. An open session keeps none, its time is placed when it
/// is asked for.
pub fn rebuild_session_and_around(conn: &Connection, session_id: &str) -> Result<()> {
    rebuild_session(conn, session_id)?;
    let span: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT started_at_wall, ended_at_wall FROM sessions WHERE id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_db)?;
    if let Some((start, Some(end))) = span {
        rebuild_aside_around(conn, &start, &end)?;
    }
    Ok(())
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
    let aside = aside_stretches(conn, &session, chrono::Utc::now().timestamp_millis())?;
    let mut insert = conn
        .prepare_cached(
            "INSERT INTO play_slices (session_id, slice_start, runtime_ms, active_ms, idle_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .map_err(map_db)?;
    for slice in slices_of(&session, &events, None, &aside) {
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

/// Builds again the slices that depend on whether `game_id` steps aside:
/// its own sessions, and those of games that step aside around them.
pub fn rebuild_for_game(db: &Database, game_id: &str) -> Result<()> {
    db.with_transaction(|conn| {
        let spans: Vec<(String, String, String)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT id, started_at_wall, ended_at_wall FROM sessions
                     WHERE game_id = ?1 AND ended_at_wall IS NOT NULL",
                )
                .map_err(map_db)?;
            let rows = stmt
                .query_map([game_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .map_err(map_db)?;
            rows.collect::<rusqlite::Result<_>>().map_err(map_db)?
        };
        for (id, start, end) in &spans {
            rebuild_session(conn, id)?;
            rebuild_aside_around(conn, start, end)?;
        }
        Ok(())
    })
}

/// Builds again the slices of every session of a game that steps aside, as
/// after a game they shared time with was deleted.
pub fn rebuild_aside_all(db: &Database) -> Result<()> {
    db.with_transaction(|conn| {
        let ids: Vec<String> = {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT sessions.id FROM sessions JOIN games ON games.id = sessions.game_id
                     WHERE {STEPS_ASIDE} AND sessions.ended_at_wall IS NOT NULL"
                ))
                .map_err(map_db)?;
            let rows = stmt.query_map([], |row| row.get(0)).map_err(map_db)?;
            rows.collect::<rusqlite::Result<_>>().map_err(map_db)?
        };
        for id in &ids {
            rebuild_session(conn, id)?;
        }
        Ok(())
    })
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
    use serde_json::Value;

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
            set_aside_ms: 0,
            set_aside_active_ms: 0,
            set_aside_idle_ms: 0,
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
            &[],
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
    fn counters_at_a_moment_come_from_the_points_around_it() {
        let events = [
            event(1, "started", &at(18, 0, 0), &serde_json::json!({})),
            event(
                2,
                "heartbeat",
                &at(18, 30, 0),
                &counters(30 * MINUTE, 30 * MINUTE, 0),
            ),
            event(
                3,
                "heartbeat",
                &at(19, 0, 0),
                &counters(60 * MINUTE, 30 * MINUTE, 30 * MINUTE),
            ),
            event(
                4,
                "tracking_gap",
                &at(21, 0, 0),
                &serde_json::json!({ "wall_gap_ms": 120 * MINUTE }),
            ),
            event(
                5,
                "ended",
                &at(21, 30, 0),
                &counters(90 * MINUTE, 30 * MINUTE, 60 * MINUTE),
            ),
        ];
        assert_eq!(
            counters_at(&events, ms(&at(18, 15, 0))),
            Some([15 * MINUTE, 15 * MINUTE, 0])
        );
        assert_eq!(
            counters_at(&events, ms(&at(18, 45, 0))),
            Some([45 * MINUTE, 30 * MINUTE, 15 * MINUTE])
        );
        // Asleep from 19:00 to 21:00, so nothing grew in between.
        assert_eq!(
            counters_at(&events, ms(&at(20, 0, 0))),
            Some([60 * MINUTE, 30 * MINUTE, 30 * MINUTE])
        );
        assert_eq!(counters_at(&events, ms(&at(17, 0, 0))), None);
        // Without checkpoints nothing tells where the time fell.
        assert_eq!(
            counters_at(&[events[0].clone(), events[4].clone()], ms(&at(18, 15, 0))),
            None
        );
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
        assert!(slices_of(&discarded, &events, None, &[]).is_empty());
    }
}
