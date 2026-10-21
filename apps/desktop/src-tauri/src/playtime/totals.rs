// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Play time per day, week, month or year, added up from the slices, and
//! clock time with a game running, from the stretches beside them.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::constants::PLAY_SLICE_MS;
use crate::db::connection::Database;
use crate::db::models::Session;
use crate::db::repo::games;
use crate::db::repo::map_db;
use crate::db::repo::sessions::row_to_session;
use crate::error::Result;
use crate::integrity::format_timestamp;
use crate::playtime::slices::{
    STEPS_ASIDE, Slice, Span, TimedEvent, apportion, aside_stretches, placing_events, slice_of,
    slices_from, spans_of, sum_of, wall_ms,
};
use crate::tracking::live::{LiveCounters, LiveSessions};

/// What one total covers, in local time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bucket {
    Day,
    Month,
    Year,
    /// An hour of the week, the same hour of every week added up.
    HourOfWeek,
}

/// A game's time in one bucket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlayTotal {
    /// "2026-10-02" for a day, "2026-10" for a month, "2026" for a year, and
    /// the weekday from Monday as 0 and the hour for an hour of the week,
    /// "4-21".
    pub bucket: String,
    pub game_id: String,
    pub runtime_ms: i64,
    pub active_ms: i64,
    pub idle_ms: i64,
}

/// The first moment of the local day `date`, in milliseconds. Where the
/// clocks jump forward at midnight, the day starts at the first hour that
/// exists.
pub fn day_start_ms<Tz: TimeZone>(tz: &Tz, date: NaiveDate) -> i64 {
    (0..2)
        .filter_map(|hour| date.and_hms_opt(hour, 0, 0))
        .find_map(|start| tz.from_local_datetime(&start).earliest())
        .map_or_else(
            || {
                date.and_time(chrono::NaiveTime::MIN)
                    .and_utc()
                    .timestamp_millis()
            },
            |start| start.timestamp_millis(),
        )
}

fn bucket_of<Tz: TimeZone>(tz: &Tz, start_ms: i64, bucket: Bucket) -> Option<String> {
    let local = DateTime::<Utc>::from_timestamp_millis(start_ms)?.with_timezone(tz);
    let date = local.date_naive();
    Some(match bucket {
        Bucket::Day => date.format("%Y-%m-%d").to_string(),
        Bucket::Month => date.format("%Y-%m").to_string(),
        Bucket::Year => date.format("%Y").to_string(),
        Bucket::HourOfWeek => format!("{}-{}", date.weekday().num_days_from_monday(), local.hour()),
    })
}

/// Open sessions, or those of `game_id` and the games linked to it.
pub(crate) fn open_sessions(
    conn: &rusqlite::Connection,
    game_id: Option<&str>,
) -> Result<Vec<Session>> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT * FROM sessions
             WHERE ended_at_wall IS NULL AND (?1 IS NULL OR game_id = ?1
                 OR game_id IN (SELECT game_id FROM game_links WHERE linked_game_id = ?1))",
        )
        .map_err(map_db)?;
    stmt.query_map([game_id], row_to_session)
        .map_err(map_db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_db)
}

/// The stretches of an open session, placed up to the tracker's latest tick,
/// or up to its last event when the tracker does not run it, like a session
/// left over from a crash.
pub(crate) fn open_session_spans(
    conn: &rusqlite::Connection,
    live: &LiveSessions,
    session: &Session,
    now_ms: i64,
) -> Result<Vec<Span>> {
    let events = placing_events(conn, &session.id)?;
    let latest = live
        .get(&session.id)
        .map(|counters| (now_ms, counters))
        .or_else(|| {
            Some((
                wall_ms(events.last()?.event_time_wall())?,
                LiveCounters {
                    runtime_ms: session.runtime_ms,
                    active_ms: session.active_ms,
                    idle_ms: session.idle_ms,
                },
            ))
        });
    let aside = aside_stretches(conn, session, now_ms)?;
    Ok(spans_of(session, &events, latest, &aside))
}

/// The moment `open_session_spans` places an open session up to, which none
/// of its stretches goes past: the latest tick for one the tracker runs, else
/// its last event or, without one, its start. None when nothing is placed.
/// A session another PC left open long ago ends long ago.
pub(crate) fn placed_until(
    conn: &rusqlite::Connection,
    live: &LiveSessions,
    session: &Session,
    now_ms: i64,
) -> Result<Option<i64>> {
    if live.get(&session.id).is_some() {
        return Ok(Some(now_ms));
    }
    let last: Option<String> = conn
        .prepare_cached(
            "SELECT event_time_wall FROM session_events
             WHERE session_id = ?1 ORDER BY sequence DESC LIMIT 1",
        )
        .and_then(|mut stmt| stmt.query_row([&session.id], |row| row.get(0)).optional())
        .map_err(map_db)?;
    Ok(last
        .as_deref()
        .and_then(wall_ms)
        .or_else(|| wall_ms(&session.started_at_wall)))
}

/// The slices of an open session, as `open_session_spans` places it.
fn open_session_slices(
    conn: &rusqlite::Connection,
    live: &LiveSessions,
    session: &Session,
    now_ms: i64,
) -> Result<Vec<Slice>> {
    Ok(slices_from(&open_session_spans(
        conn, live, session, now_ms,
    )?))
}

/// What was played between two moments, running sessions up to the latest
/// tick.
struct RangePlay {
    /// Each slice with the game that recorded it.
    slices: Vec<(String, Slice)>,
    /// Each stretch with the game it counts as. Empty unless asked for.
    spans: Vec<(String, Span)>,
}

/// The slices of the quarter hours from `from_ms` up to `to_ms`, and with
/// `with_spans` the stretches with time in them, of every game or of
/// `game_id` and the games linked to it. One snapshot for all reads, so a
/// session that ends in between counts once, as running or as stored.
#[expect(clippy::too_many_arguments)]
fn range_play(
    db: &Database,
    live: &LiveSessions,
    links: &HashMap<String, String>,
    from_ms: i64,
    to_ms: i64,
    game_id: Option<&str>,
    now_ms: i64,
    with_spans: bool,
) -> Result<RangePlay> {
    db.with_transaction(|conn| {
        let mut stmt = conn
            .prepare_cached(
                "SELECT sessions.game_id, play_slices.slice_start, play_slices.runtime_ms,
                        play_slices.active_ms, play_slices.idle_ms
                 FROM play_slices JOIN sessions ON sessions.id = play_slices.session_id
                 WHERE play_slices.slice_start >= ?1 AND play_slices.slice_start < ?2
                   AND (?3 IS NULL OR sessions.game_id = ?3 OR sessions.game_id IN
                        (SELECT game_id FROM game_links WHERE linked_game_id = ?3))",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map(params![from_ms, to_ms, game_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Slice {
                        start_ms: row.get(1)?,
                        runtime_ms: row.get(2)?,
                        active_ms: row.get(3)?,
                        idle_ms: row.get(4)?,
                    },
                ))
            })
            .map_err(map_db)?;
        let mut slices: Vec<(String, Slice)> =
            rows.collect::<rusqlite::Result<_>>().map_err(map_db)?;
        let shown = |game: String| links.get(&game).cloned().unwrap_or(game);
        let mut spans = Vec::new();
        if with_spans {
            let mut stmt = conn
                .prepare_cached(
                    "SELECT sessions.game_id, play_spans.span_start, play_spans.span_end,
                            play_spans.runtime_ms, play_spans.active_ms, play_spans.idle_ms
                     FROM play_spans JOIN sessions ON sessions.id = play_spans.session_id
                     WHERE play_spans.span_end >= ?1 AND play_spans.span_start < ?2
                       AND (?3 IS NULL OR sessions.game_id = ?3 OR sessions.game_id IN
                            (SELECT game_id FROM game_links WHERE linked_game_id = ?3))",
                )
                .map_err(map_db)?;
            let rows = stmt
                .query_map(params![from_ms, to_ms, game_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        Span {
                            start_ms: row.get(1)?,
                            end_ms: row.get(2)?,
                            runtime_ms: row.get(3)?,
                            active_ms: row.get(4)?,
                            idle_ms: row.get(5)?,
                        },
                    ))
                })
                .map_err(map_db)?;
            for row in rows {
                let (game, span) = row.map_err(map_db)?;
                spans.push((shown(game), span));
            }
        }
        for session in open_sessions(conn, game_id)? {
            if placed_until(conn, live, &session, now_ms)?.is_none_or(|until| until < from_ms) {
                continue;
            }
            let placed = open_session_spans(conn, live, &session, now_ms)?;
            slices.extend(
                slices_from(&placed)
                    .into_iter()
                    .filter(|slice| slice.start_ms >= from_ms && slice.start_ms < to_ms)
                    .map(|slice| (session.game_id.clone(), slice)),
            );
            if with_spans {
                let game = shown(session.game_id.clone());
                spans.extend(
                    placed
                        .into_iter()
                        .filter(|span| span.end_ms >= from_ms && span.start_ms < to_ms)
                        .map(|span| (game.clone(), span)),
                );
            }
        }
        Ok(RangePlay { slices, spans })
    })
}

/// Clock time with something running among `pieces`, each a start, an end
/// and the runtime spread evenly between them, the densest piece counting
/// at each moment. A piece without length counts in full.
#[allow(
    clippy::cast_precision_loss,
    reason = "milliseconds of play fit f64 exactly"
)]
fn clock_once(pieces: &[(i64, i64, f64)]) -> f64 {
    let untimed: f64 = pieces
        .iter()
        .filter(|(start, end, _)| end <= start)
        .map(|(_, _, runtime)| runtime)
        .sum();
    let mut bounds: Vec<i64> = pieces
        .iter()
        .filter(|(start, end, _)| end > start)
        .flat_map(|(start, end, _)| [*start, *end])
        .collect();
    bounds.sort_unstable();
    bounds.dedup();
    let mut timed = 0.0;
    for pair in bounds.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let densest = pieces
            .iter()
            .filter(|(start, end, _)| end > start && *start <= from && *end >= to)
            .map(|(start, end, runtime)| runtime / (end - start) as f64)
            .fold(0.0, f64::max);
        timed += densest * (to - from) as f64;
    }
    untimed + timed
}

/// Clock time with a game running in each quarter hour, games side by side
/// counted once. `once` holds each shown game's time per quarter hour and
/// `spans` the stretches it was placed from. Where one game ran, that is its
/// time in the slices exactly. Where several did, the stretches tell how
/// much of their time overlapped on the clock, which is taken out.
#[allow(
    clippy::cast_precision_loss,
    reason = "milliseconds of play fit f64 exactly"
)]
fn played_by_slice(
    once: &BTreeMap<(String, i64), [i64; 3]>,
    spans: &[(String, Span)],
) -> BTreeMap<i64, i64> {
    // The sum, the largest and the count of the games in each quarter hour.
    let mut cells: BTreeMap<i64, (i64, i64, usize)> = BTreeMap::new();
    for ((_, start_ms), [runtime_ms, _, _]) in once {
        if *runtime_ms <= 0 {
            continue;
        }
        let cell = cells.entry(*start_ms).or_default();
        cell.0 += runtime_ms;
        cell.1 = cell.1.max(*runtime_ms);
        cell.2 += 1;
    }
    let shared: BTreeSet<i64> = cells
        .iter()
        .filter(|(_, (_, _, games))| *games > 1)
        .map(|(start_ms, _)| *start_ms)
        .collect();
    let mut pieces: HashMap<i64, Vec<(&str, i64, i64, f64)>> = HashMap::new();
    if !shared.is_empty() {
        for (game, span) in spans {
            if span.runtime_ms <= 0 {
                continue;
            }
            let length = span.length_ms();
            if length == 0 {
                let cell = slice_of(span.start_ms);
                if shared.contains(&cell) {
                    pieces.entry(cell).or_default().push((
                        game,
                        span.start_ms,
                        span.end_ms,
                        span.runtime_ms as f64,
                    ));
                }
                continue;
            }
            for &cell in shared.range(slice_of(span.start_ms)..span.end_ms) {
                let (from, to) = (
                    span.start_ms.max(cell),
                    span.end_ms.min(cell + PLAY_SLICE_MS),
                );
                if to > from {
                    let share = span.runtime_ms as f64 * (to - from) as f64 / length as f64;
                    pieces
                        .entry(cell)
                        .or_default()
                        .push((game, from, to, share));
                }
            }
        }
    }
    cells
        .into_iter()
        .map(|(start_ms, (sum, largest, _))| {
            let Some(cell) = pieces.get(&start_ms) else {
                return (start_ms, sum);
            };
            let all: Vec<(i64, i64, f64)> = cell
                .iter()
                .map(|&(_, from, to, runtime)| (from, to, runtime))
                .collect();
            let games: BTreeSet<&str> = cell.iter().map(|(game, ..)| *game).collect();
            let each: f64 = games
                .iter()
                .map(|game| {
                    let own: Vec<(i64, i64, f64)> = cell
                        .iter()
                        .filter(|(other, ..)| other == game)
                        .map(|&(_, from, to, runtime)| (from, to, runtime))
                        .collect();
                    clock_once(&own)
                })
                .sum();
            let overlap = (each - clock_once(&all)).max(0.0).round() as i64;
            (start_ms, (sum - overlap).clamp(largest, sum))
        })
        .collect()
}

/// Fills in the time set aside of sessions whose game steps aside for
/// others: what the session holds minus what its slices count, and for a
/// closed one when the time that counts ended. Running sessions, already
/// brought up to the latest tick, are placed up to it.
pub fn fill_set_aside(
    db: &Database,
    live: &LiveSessions,
    sessions: &mut [Session],
    now_ms: i64,
) -> Result<()> {
    db.with_transaction(|conn| {
        let stepping: HashSet<String> = {
            let mut stmt = conn
                .prepare_cached(&format!("SELECT id FROM games WHERE {STEPS_ASIDE}"))
                .map_err(map_db)?;
            let rows = stmt.query_map([], |row| row.get(0)).map_err(map_db)?;
            rows.collect::<rusqlite::Result<_>>().map_err(map_db)?
        };
        if stepping.is_empty() {
            return Ok(());
        }
        // Closed sessions read what their slices count, which the running
        // sessions the live bar asks for every few seconds do not need.
        let any_closed = sessions
            .iter()
            .any(|session| session.ended_at_wall.is_some() && stepping.contains(&session.game_id));
        let counted: HashMap<String, [i64; 3]> = if any_closed {
            let mut stmt = conn
                .prepare_cached(&format!(
                    "SELECT play_slices.session_id, SUM(play_slices.runtime_ms),
                            SUM(play_slices.active_ms), SUM(play_slices.idle_ms)
                     FROM play_slices
                     JOIN sessions ON sessions.id = play_slices.session_id
                     JOIN games ON games.id = sessions.game_id
                     WHERE {STEPS_ASIDE}
                     GROUP BY play_slices.session_id"
                ))
                .map_err(map_db)?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get(0)?, [row.get(1)?, row.get(2)?, row.get(3)?]))
                })
                .map_err(map_db)?;
            rows.collect::<rusqlite::Result<_>>().map_err(map_db)?
        } else {
            HashMap::new()
        };
        let counted_until: HashMap<String, i64> = if any_closed {
            let mut stmt = conn
                .prepare_cached(&format!(
                    "SELECT play_spans.session_id, MAX(play_spans.span_end)
                     FROM play_spans
                     JOIN sessions ON sessions.id = play_spans.session_id
                     JOIN games ON games.id = sessions.game_id
                     WHERE {STEPS_ASIDE} AND play_spans.runtime_ms > 0
                     GROUP BY play_spans.session_id"
                ))
                .map_err(map_db)?;
            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(map_db)?;
            rows.collect::<rusqlite::Result<_>>().map_err(map_db)?
        } else {
            HashMap::new()
        };
        for session in sessions
            .iter_mut()
            .filter(|session| stepping.contains(&session.game_id))
        {
            let kept = if session.ended_at_wall.is_some() {
                session.counted_until = Some(
                    counted_until
                        .get(&session.id)
                        .and_then(|end| DateTime::from_timestamp_millis(*end))
                        .map_or_else(|| session.started_at_wall.clone(), format_timestamp),
                );
                counted.get(&session.id).copied().unwrap_or_default()
            } else {
                sum_of(&open_session_slices(conn, live, session, now_ms)?)
            };
            session.set_aside_ms = session.runtime_ms.saturating_sub(kept[0]).max(0);
            session.set_aside_active_ms = session.active_ms.saturating_sub(kept[1]).max(0);
            session.set_aside_idle_ms = session.idle_ms.saturating_sub(kept[2]).max(0);
        }
        Ok(())
    })
}

/// Each slice as it counts, in the order given, each with the shown game it
/// belongs to. One game cannot be played twice at once, so where two of its
/// copies ran at the same time, like linked installs or two PCs, the quarter
/// hour holds no more than it lasts, shared by what each copy ran. Their
/// active and idle time shrink alike. A slice that holds more than a quarter
/// hour on its own, time whose clock went backwards, has no place on the
/// clock to share and counts in full.
fn counted_once(slices: &[(&str, Slice)]) -> Vec<Slice> {
    let mut cells: HashMap<(&str, i64), Vec<usize>> = HashMap::new();
    for (index, (game, slice)) in slices.iter().enumerate() {
        if slice.runtime_ms <= PLAY_SLICE_MS {
            cells.entry((game, slice.start_ms)).or_default().push(index);
        }
    }
    let mut counted: Vec<Slice> = slices.iter().map(|(_, slice)| *slice).collect();
    for indices in cells.into_values() {
        let whole: i64 = indices
            .iter()
            .map(|&index| slices[index].1.runtime_ms)
            .sum();
        if whole <= PLAY_SLICE_MS {
            continue;
        }
        let measure = |pick: fn(&Slice) -> i64| -> Vec<i64> {
            indices
                .iter()
                .map(|&index| pick(&slices[index].1))
                .collect()
        };
        let shrunk = |parts: &[i64]| {
            let sum: i64 = parts.iter().sum();
            let kept = i128::from(sum) * i128::from(PLAY_SLICE_MS) / i128::from(whole);
            apportion(parts, i64::try_from(kept).unwrap_or(sum))
        };
        let runtime = apportion(&measure(|slice| slice.runtime_ms), PLAY_SLICE_MS);
        let active = shrunk(&measure(|slice| slice.active_ms));
        let idle = shrunk(&measure(|slice| slice.idle_ms));
        for (part, &index) in indices.iter().enumerate() {
            counted[index].runtime_ms = runtime[part];
            counted[index].active_ms = active[part];
            counted[index].idle_ms = idle[part];
        }
    }
    counted
}

/// Brings sessions to the tracker's latest tick and to what counts of them,
/// each shown as the game it counts as, the way every page gets them.
pub fn as_shown(
    db: &Database,
    live: &LiveSessions,
    sessions: &mut [Session],
    now_ms: i64,
) -> Result<()> {
    for session in sessions.iter_mut() {
        live.apply(session);
    }
    show_counted(db, live, sessions, now_ms)?;
    show_linked_games(db, sessions)
}

/// Shows a session of a linked game, another install here or a game of
/// another PC, as one of the game it is linked to, and says which game
/// recorded it. Only the shown game changes, the session's history keeps
/// the game it was recorded with.
fn show_linked_games(db: &Database, sessions: &mut [Session]) -> Result<()> {
    let links = games::links(db)?;
    if links.is_empty() {
        return Ok(());
    }
    for session in sessions {
        if let Some(linked) = links.get(&session.game_id) {
            let recorded = std::mem::replace(&mut session.game_id, linked.clone());
            session.recorded_game_id = Some(recorded);
        }
    }
    Ok(())
}

/// Brings sessions to what pages show, which is what counts: runtime, active
/// and idle time without the time set aside, which `set_aside_*` keeps, and
/// in `counted_*` what each adds to totals, time shared with another copy
/// of its game counted once. Running sessions, already brought up to the
/// latest tick, are placed up to it.
pub fn show_counted(
    db: &Database,
    live: &LiveSessions,
    sessions: &mut [Session],
    now_ms: i64,
) -> Result<()> {
    fill_set_aside(db, live, sessions, now_ms)?;
    fill_copy_overlap(db, live, sessions, now_ms)?;
    for session in sessions {
        session.runtime_ms = (session.runtime_ms - session.set_aside_ms).max(0);
        session.active_ms = (session.active_ms - session.set_aside_active_ms).max(0);
        session.idle_ms = (session.idle_ms - session.set_aside_idle_ms).max(0);
        session.counted_runtime_ms = (session.runtime_ms - session.copy_overlap_ms).max(0);
        session.counted_active_ms = (session.active_ms - session.copy_overlap_active_ms).max(0);
        session.counted_idle_ms = (session.idle_ms - session.copy_overlap_idle_ms).max(0);
    }
    Ok(())
}

/// Each shown game's time per quarter hour, its copies running at the same
/// time counted once.
fn once_at_a_time(
    slices: &[(String, Slice)],
    links: &HashMap<String, String>,
) -> BTreeMap<(String, i64), [i64; 3]> {
    let shown: Vec<(&str, Slice)> = slices
        .iter()
        .map(|(game, slice)| {
            (
                links.get(game).map_or(game.as_str(), String::as_str),
                *slice,
            )
        })
        .collect();
    let mut summed: BTreeMap<(String, i64), [i64; 3]> = BTreeMap::new();
    for ((game, _), slice) in shown.iter().zip(counted_once(&shown)) {
        let total = summed
            .entry(((*game).to_owned(), slice.start_ms))
            .or_default();
        total[0] += slice.runtime_ms;
        total[1] += slice.active_ms;
        total[2] += slice.idle_ms;
    }
    summed
}

/// Stored slices in quarter hours that copies of one shown game fill past
/// their length, each with its session and its shown game.
fn stored_shared_slices(conn: &rusqlite::Connection) -> Result<Vec<(String, String, Slice)>> {
    let mut stmt = conn
        .prepare_cached(
            "WITH placed AS (
                 SELECT play_slices.session_id, play_slices.slice_start, play_slices.runtime_ms,
                        play_slices.active_ms, play_slices.idle_ms,
                        COALESCE(game_links.linked_game_id, sessions.game_id) AS game
                 FROM play_slices
                 JOIN sessions ON sessions.id = play_slices.session_id
                 LEFT JOIN game_links ON game_links.game_id = sessions.game_id
                 WHERE play_slices.runtime_ms <= ?1),
             shared AS (
                 SELECT game, slice_start FROM placed
                 GROUP BY game, slice_start HAVING SUM(runtime_ms) > ?1)
             SELECT placed.session_id, placed.game, placed.slice_start, placed.runtime_ms,
                    placed.active_ms, placed.idle_ms
             FROM placed JOIN shared USING (game, slice_start)",
        )
        .map_err(map_db)?;
    let rows = stmt
        .query_map([PLAY_SLICE_MS], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                Slice {
                    start_ms: row.get(2)?,
                    runtime_ms: row.get(3)?,
                    active_ms: row.get(4)?,
                    idle_ms: row.get(5)?,
                },
            ))
        })
        .map_err(map_db)?;
    rows.collect::<rusqlite::Result<_>>().map_err(map_db)
}

/// The first stored quarter hour of the sessions `ids`.
fn first_stored_slice(conn: &rusqlite::Connection, ids: &[&str]) -> Result<Option<i64>> {
    if ids.is_empty() {
        return Ok(None);
    }
    let ids = serde_json::to_string(ids).unwrap_or_default();
    conn.prepare_cached(
        "SELECT MIN(slice_start) FROM play_slices
         WHERE session_id IN (SELECT value FROM json_each(?1))",
    )
    .and_then(|mut stmt| stmt.query_row([ids], |row| row.get(0)))
    .map_err(map_db)
}

/// Stored slices of the games `game_ids` from `from_ms` on, each with its
/// session.
fn stored_slices_since(
    conn: &rusqlite::Connection,
    game_ids: &[String],
    from_ms: i64,
) -> Result<Vec<(String, Slice)>> {
    let ids = serde_json::to_string(game_ids).unwrap_or_default();
    let mut stmt = conn
        .prepare_cached(
            "SELECT play_slices.session_id, play_slices.slice_start, play_slices.runtime_ms,
                    play_slices.active_ms, play_slices.idle_ms
             FROM play_slices JOIN sessions ON sessions.id = play_slices.session_id
             WHERE play_slices.slice_start >= ?1
               AND sessions.game_id IN (SELECT value FROM json_each(?2))",
        )
        .map_err(map_db)?;
    let rows = stmt
        .query_map(params![from_ms, ids], |row| {
            Ok((
                row.get(0)?,
                Slice {
                    start_ms: row.get(1)?,
                    runtime_ms: row.get(2)?,
                    active_ms: row.get(3)?,
                    idle_ms: row.get(4)?,
                },
            ))
        })
        .map_err(map_db)?;
    rows.collect::<rusqlite::Result<_>>().map_err(map_db)
}

/// Running sessions that another session of the same shown game ran beside,
/// each with the games that count as that game, the shown one first.
fn running_beside_copies(conn: &rusqlite::Connection) -> Result<Vec<(Session, Vec<String>)>> {
    let open = {
        let mut stmt = conn
            .prepare_cached("SELECT * FROM sessions WHERE ended_at_wall IS NULL")
            .map_err(map_db)?;
        stmt.query_map([], row_to_session)
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)?
    };
    let mut beside = Vec::new();
    for session in open {
        let set = games::linked_set(conn, &session.game_id)?;
        let ids = serde_json::to_string(&set).unwrap_or_default();
        let other: bool = conn
            .prepare_cached(
                "SELECT EXISTS (SELECT 1 FROM sessions
                     WHERE id != ?1 AND game_id IN (SELECT value FROM json_each(?2))
                       AND (ended_at_wall IS NULL OR ended_at_wall > ?3))",
            )
            .and_then(|mut stmt| {
                stmt.query_row(params![session.id, ids, session.started_at_wall], |row| {
                    row.get(0)
                })
            })
            .map_err(map_db)?;
        if other {
            beside.push((session, set));
        }
    }
    Ok(beside)
}

/// Fills in the time each session shares with another copy of its game in
/// the same quarter hours, which the totals count once. Running sessions,
/// already brought up to the latest tick, are placed up to it.
pub fn fill_copy_overlap(
    db: &Database,
    live: &LiveSessions,
    sessions: &mut [Session],
    now_ms: i64,
) -> Result<()> {
    if sessions.is_empty() {
        return Ok(());
    }
    let any_closed = sessions
        .iter()
        .any(|session| session.ended_at_wall.is_some());
    let slices: Vec<(String, String, Slice)> = db.with_transaction(|conn| {
        let beside = running_beside_copies(conn)?;
        // Only quarter hours with time of the given sessions matter, so
        // nothing before the first of them is read or placed.
        let asked: HashSet<&str> = sessions.iter().map(|session| session.id.as_str()).collect();
        let mut placed: HashMap<String, Vec<Slice>> = HashMap::new();
        for (running, _) in &beside {
            if asked.contains(running.id.as_str()) {
                placed.insert(
                    running.id.clone(),
                    open_session_slices(conn, live, running, now_ms)?,
                );
            }
        }
        let closed: Vec<&str> = sessions
            .iter()
            .filter(|session| session.ended_at_wall.is_some())
            .map(|session| session.id.as_str())
            .collect();
        let Some(from_ms) = placed
            .values()
            .flatten()
            .map(|slice| slice.start_ms)
            .chain(first_stored_slice(conn, &closed)?)
            .min()
        else {
            return Ok(Vec::new());
        };
        let mut slices = Vec::new();
        let mut seen: HashSet<(String, i64)> = HashSet::new();
        let mut add = |session: String, game: String, slice: Slice| {
            if seen.insert((session.clone(), slice.start_ms)) {
                slices.push((session, game, slice));
            }
        };
        if any_closed {
            for (session, game, slice) in stored_shared_slices(conn)? {
                add(session, game, slice);
            }
        }
        for (running, set) in beside {
            let game = set[0].clone();
            let start_ms = slice_of(wall_ms(&running.started_at_wall).unwrap_or(0));
            for (session, slice) in stored_slices_since(conn, &set, start_ms.max(from_ms))? {
                add(session, game.clone(), slice);
            }
            let own = match placed.remove(&running.id) {
                Some(own) => own,
                None if placed_until(conn, live, &running, now_ms)?
                    .is_some_and(|until| until >= from_ms) =>
                {
                    open_session_slices(conn, live, &running, now_ms)?
                }
                None => Vec::new(),
            };
            for slice in own {
                add(running.id.clone(), game.clone(), slice);
            }
        }
        Ok(slices)
    })?;
    if slices.is_empty() {
        return Ok(());
    }
    let cells: Vec<(&str, Slice)> = slices
        .iter()
        .map(|(_, game, slice)| (game.as_str(), *slice))
        .collect();
    let mut shared: HashMap<&str, [i64; 3]> = HashMap::new();
    for ((session, _, slice), counted) in slices.iter().zip(counted_once(&cells)) {
        let lost = [
            slice.runtime_ms - counted.runtime_ms,
            slice.active_ms - counted.active_ms,
            slice.idle_ms - counted.idle_ms,
        ];
        if lost != [0; 3] {
            let sum = shared.entry(session.as_str()).or_default();
            for (total, part) in sum.iter_mut().zip(lost) {
                *total += part;
            }
        }
    }
    for session in sessions.iter_mut() {
        if let Some([runtime_ms, active_ms, idle_ms]) = shared.get(session.id.as_str()) {
            session.copy_overlap_ms = *runtime_ms;
            session.copy_overlap_active_ms = *active_ms;
            session.copy_overlap_idle_ms = *idle_ms;
        }
    }
    Ok(())
}

/// Each game's time per bucket in `once`, oldest bucket first.
fn bucketed<Tz: TimeZone>(
    tz: &Tz,
    once: &BTreeMap<(String, i64), [i64; 3]>,
    bucket: Bucket,
) -> Vec<PlayTotal> {
    let mut totals: BTreeMap<(String, String), [i64; 3]> = BTreeMap::new();
    for ((game, start_ms), [runtime_ms, active_ms, idle_ms]) in once {
        let Some(key) = bucket_of(tz, *start_ms, bucket) else {
            continue;
        };
        let total = totals.entry((key, game.clone())).or_default();
        total[0] += runtime_ms;
        total[1] += active_ms;
        total[2] += idle_ms;
    }
    totals
        .into_iter()
        .map(
            |((bucket, game_id), [runtime_ms, active_ms, idle_ms])| PlayTotal {
                bucket,
                game_id,
                runtime_ms,
                active_ms,
                idle_ms,
            },
        )
        .collect()
}

/// Each game's time per bucket, from the local day `from` up to but not
/// including `to`, oldest bucket first. Running sessions count up to the
/// tracker's latest tick, at `now_ms`.
#[expect(clippy::too_many_arguments)]
pub fn play_totals<Tz: TimeZone>(
    db: &Database,
    live: &LiveSessions,
    tz: &Tz,
    from: NaiveDate,
    to: NaiveDate,
    bucket: Bucket,
    game_id: Option<&str>,
    now_ms: i64,
) -> Result<Vec<PlayTotal>> {
    // A linked game, another install here or a game of another PC, counts
    // as the game it is linked to.
    let links = games::links(db)?;
    let range = range_play(
        db,
        live,
        &links,
        day_start_ms(tz, from),
        day_start_ms(tz, to),
        game_id,
        now_ms,
        false,
    )?;
    Ok(bucketed(tz, &once_at_a_time(&range.slices, &links), bucket))
}

/// The runtime of `span` that falls between `from` and `to`, spread evenly.
/// One without length counts where it ended.
fn runtime_within(span: &Span, from: i64, to: i64) -> i128 {
    let length = span.length_ms();
    if length <= 0 {
        return if span.end_ms >= from && span.end_ms < to {
            i128::from(span.runtime_ms)
        } else {
            0
        };
    }
    let overlap = span.end_ms.min(to) - span.start_ms.max(from);
    if overlap <= 0 {
        return 0;
    }
    i128::from(span.runtime_ms) * i128::from(overlap) / i128::from(length)
}

/// The runtime `game_id` counts before `moment_ms`, the game shown with the
/// games linked to it, as the totals count it. The quarter hour the moment
/// falls in counts the share of its time placed before the moment. Running
/// sessions count up to the tracker's latest tick, at `now_ms`.
pub(crate) fn counted_until(
    db: &Database,
    live: &LiveSessions,
    links: &HashMap<String, String>,
    game_id: &str,
    moment_ms: i64,
    now_ms: i64,
) -> Result<i64> {
    let quarter = slice_of(moment_ms);
    let end = quarter + PLAY_SLICE_MS;
    let range = range_play(db, live, links, i64::MIN, end, Some(game_id), now_ms, true)?;
    let (mut whole, mut cut) = (0, 0);
    for ((_, start_ms), [runtime_ms, _, _]) in once_at_a_time(&range.slices, links) {
        if start_ms < quarter {
            whole += runtime_ms;
        } else {
            cut += runtime_ms;
        }
    }
    let (before, within) = range
        .spans
        .iter()
        .fold((0, 0), |(before, within), (_, span)| {
            (
                before + runtime_within(span, quarter, moment_ms),
                within + runtime_within(span, quarter, end),
            )
        });
    let part = if within > 0 {
        i64::try_from(i128::from(cut) * before.min(within) / within).unwrap_or(0)
    } else {
        0
    };
    Ok(whole + part)
}

/// Each game's time per local day, and the clock time with a game running on
/// each, games side by side counted once.
#[derive(Debug, Clone, Default)]
pub struct DayTotals {
    /// By day, oldest first, as `play_totals` gives them.
    pub games: Vec<PlayTotal>,
    /// Clock time with a game running, by day, days without play left out.
    pub played: BTreeMap<String, i64>,
}

impl DayTotals {
    /// Days with time played, oldest first.
    pub fn days_played(&self) -> BTreeSet<&str> {
        self.games
            .iter()
            .filter(|total| total.runtime_ms > 0)
            .map(|total| total.bucket.as_str())
            .collect()
    }

    /// Clock time with a game running over all the days.
    pub fn played_ms(&self) -> i64 {
        self.played.values().sum()
    }
}

/// `DayTotals` from the local day `from` up to but not including `to`.
/// Running sessions count up to the tracker's latest tick, at `now_ms`.
pub fn day_totals<Tz: TimeZone>(
    db: &Database,
    live: &LiveSessions,
    tz: &Tz,
    from: NaiveDate,
    to: NaiveDate,
    now_ms: i64,
) -> Result<DayTotals> {
    let links = games::links(db)?;
    let range = range_play(
        db,
        live,
        &links,
        day_start_ms(tz, from),
        day_start_ms(tz, to),
        None,
        now_ms,
        true,
    )?;
    let once = once_at_a_time(&range.slices, &links);
    let mut played: BTreeMap<String, i64> = BTreeMap::new();
    for (start_ms, ms) in played_by_slice(&once, &range.spans) {
        if let Some(day) = bucket_of(tz, start_ms, Bucket::Day)
            && ms > 0
        {
            *played.entry(day).or_default() += ms;
        }
    }
    Ok(DayTotals {
        games: bucketed(tz, &once, Bucket::Day),
        played,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, TimeDelta};
    use chrono_tz::{America::New_York, Asia::Kathmandu, Europe::Berlin};

    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::{devices, games, sessions};
    use crate::integrity;
    use crate::playtime::slices::rebuild_session_and_around;
    use crate::playtime::testing;

    const MINUTE: i64 = 60_000;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn utc(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn a_session_of_a_linked_install_shows_as_the_game_it_counts_as() {
        let db = testing::database();
        let (steam, gog) = (
            testing::game(&db, "Steam copy"),
            testing::game(&db, "GOG copy"),
        );
        testing::played(&db, &gog, "2026-09-28T16:00:00Z", 60);
        games::link_game(&db, &gog, Some(&steam)).unwrap();
        let mut all = sessions::list_all_sessions(&db).unwrap();
        as_shown(
            &db,
            &LiveSessions::default(),
            &mut all,
            utc("2026-09-30T12:00:00Z").timestamp_millis(),
        )
        .unwrap();
        assert_eq!(all[0].game_id, steam);
        assert_eq!(all[0].recorded_game_id.as_deref(), Some(gog.as_str()));
        assert_eq!(all[0].counted_runtime_ms, 60 * MINUTE);
    }

    struct Library {
        db: Database,
        game_id: String,
    }

    fn library() -> Library {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Totals Game".into(),
                executable_path: Some("game.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        Library {
            db,
            game_id: game.id,
        }
    }

    /// A tracked session from `start` that ran all active for `minutes`,
    /// closed and sliced.
    fn played(library: &Library, start: &str, minutes: i64) {
        played_game(&library.db, &library.game_id, start, minutes);
    }

    /// A tracked session of `game_id`, closed and sliced along with the
    /// sessions that step aside around it. Returns its id.
    fn played_game(db: &Database, game_id: &str, start: &str, minutes: i64) -> String {
        let session = sessions::create_session_at(db, game_id, "device", utc(start)).unwrap();
        let runtime = minutes * MINUTE;
        db.with_conn(|conn| {
            let end = utc(start) + TimeDelta::milliseconds(runtime);
            integrity::append_session_event(
                conn,
                &session.id,
                "ended",
                &integrity::format_timestamp(end),
                Some(runtime),
                &serde_json::json!({
                    "runtime_ms": runtime,
                    "active_ms": runtime,
                    "idle_ms": 0,
                    "integrity_status": "local",
                    "closed_cleanly": true,
                })
                .to_string(),
            )?;
            conn.execute(
                "UPDATE sessions SET ended_at_wall = ?1, runtime_ms = ?2,
                         elapsed_monotonic_ms = ?2, active_ms = ?2, closed_cleanly = 1
                     WHERE id = ?3",
                params![integrity::format_timestamp(end), runtime, session.id],
            )
            .map_err(map_db)?;
            rebuild_session_and_around(conn, &session.id)
        })
        .unwrap();
        session.id
    }

    fn by_bucket(totals: &[PlayTotal]) -> Vec<(String, i64)> {
        totals
            .iter()
            .map(|total| (total.bucket.clone(), total.runtime_ms / MINUTE))
            .collect()
    }

    #[test]
    fn splits_a_session_at_local_midnight() {
        let library = library();
        // 23:00 to 02:00 in Berlin, summer time (UTC+2).
        played(&library, "2026-07-10T21:00:00Z", 180);
        let live = LiveSessions::default();

        let days = play_totals(
            &library.db,
            &live,
            &Berlin,
            date("2026-07-01"),
            date("2026-08-01"),
            Bucket::Day,
            None,
            0,
        )
        .unwrap();
        assert_eq!(
            by_bucket(&days),
            vec![("2026-07-10".into(), 60), ("2026-07-11".into(), 120)]
        );
    }

    #[test]
    fn days_follow_daylight_saving() {
        let library = library();
        // New York's clocks go back at 02:00 on 2026-11-01, a 25 hour day.
        // Played 00:30 to 03:30 local, four hours on the clock.
        played(&library, "2026-11-01T04:30:00Z", 240);
        let live = LiveSessions::default();

        let days = play_totals(
            &library.db,
            &live,
            &New_York,
            date("2026-10-31"),
            date("2026-11-03"),
            Bucket::Day,
            None,
            0,
        )
        .unwrap();
        assert_eq!(by_bucket(&days), vec![("2026-11-01".into(), 240)]);
    }

    #[test]
    fn a_quarter_hour_offset_still_splits_at_midnight() {
        let library = library();
        // Kathmandu is UTC+5:45. 23:45 to 00:45 local.
        played(&library, "2026-03-04T18:00:00Z", 60);
        let live = LiveSessions::default();

        let days = play_totals(
            &library.db,
            &live,
            &Kathmandu,
            date("2026-03-01"),
            date("2026-03-10"),
            Bucket::Day,
            None,
            0,
        )
        .unwrap();
        assert_eq!(
            by_bucket(&days),
            vec![("2026-03-04".into(), 15), ("2026-03-05".into(), 45)]
        );
    }

    #[test]
    fn groups_months_years_and_hours_of_the_week() {
        let library = library();
        let utc_zone = FixedOffset::east_opt(0).unwrap();
        // A Friday evening and the Monday after.
        played(&library, "2026-10-02T20:00:00Z", 60);
        played(&library, "2026-10-05T20:00:00Z", 30);
        let live = LiveSessions::default();
        let totals = |bucket| {
            by_bucket(
                &play_totals(
                    &library.db,
                    &live,
                    &utc_zone,
                    date("2026-01-01"),
                    date("2027-01-01"),
                    bucket,
                    None,
                    0,
                )
                .unwrap(),
            )
        };

        assert_eq!(totals(Bucket::Month), vec![("2026-10".into(), 90)]);
        assert_eq!(totals(Bucket::Year), vec![("2026".into(), 90)]);
        assert_eq!(
            totals(Bucket::HourOfWeek),
            vec![("0-20".into(), 30), ("4-20".into(), 60)]
        );
    }

    #[test]
    fn a_running_session_counts_up_to_the_latest_tick() {
        let library = library();
        let start = "2026-10-02T20:00:00Z";
        let session =
            sessions::create_session_at(&library.db, &library.game_id, "device", utc(start))
                .unwrap();
        let live = LiveSessions::default();
        live.update(
            &session.id,
            LiveCounters {
                runtime_ms: 40 * MINUTE,
                active_ms: 40 * MINUTE,
                idle_ms: 0,
            },
        );
        let now = (utc(start) + TimeDelta::minutes(40)).timestamp_millis();

        let days = play_totals(
            &library.db,
            &live,
            &FixedOffset::east_opt(0).unwrap(),
            date("2026-10-02"),
            date("2026-10-03"),
            Bucket::Day,
            Some(&library.game_id),
            now,
        )
        .unwrap();
        assert_eq!(by_bucket(&days), vec![("2026-10-02".into(), 40)]);
    }

    fn game(db: &Database, title: &str) -> String {
        games::create_game(
            db,
            &CreateGame {
                title: title.into(),
                executable_path: Some(format!("{title}.exe")),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap()
        .id
    }

    fn day_minutes(db: &Database, game_id: &str) -> i64 {
        let totals = play_totals(
            db,
            &LiveSessions::default(),
            &FixedOffset::east_opt(0).unwrap(),
            date("2026-10-02"),
            date("2026-10-03"),
            Bucket::Day,
            Some(game_id),
            0,
        )
        .unwrap();
        totals.iter().map(|total| total.runtime_ms).sum::<i64>() / MINUTE
    }

    #[test]
    fn a_game_that_steps_aside_counts_only_while_nothing_else_runs() {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        games::set_steps_aside(&db, &client, true).unwrap();
        // The client from 18:00 to 21:00, a match inside it from 19:00 to 20:00.
        let client_session = played_game(&db, &client, "2026-10-02T18:00:00Z", 180);
        played_game(&db, &match_game, "2026-10-02T19:00:00Z", 60);

        assert_eq!(day_minutes(&db, &client), 120);
        assert_eq!(day_minutes(&db, &match_game), 60);
        let mut listed = sessions::list_all_sessions(&db).unwrap();
        fill_set_aside(&db, &LiveSessions::default(), &mut listed, 0).unwrap();
        let client_row = listed
            .iter()
            .find(|session| session.id == client_session)
            .unwrap();
        assert_eq!(client_row.set_aside_ms, 60 * MINUTE);
        assert_eq!(client_row.set_aside_active_ms, 60 * MINUTE);
        // The record itself keeps all three hours.
        assert_eq!(client_row.runtime_ms, 180 * MINUTE);

        // Switched off, it counts in full again.
        games::set_steps_aside(&db, &client, false).unwrap();
        crate::playtime::slices::rebuild_all(&db).unwrap();
        assert_eq!(day_minutes(&db, &client), 180);
    }

    #[test]
    fn a_game_that_steps_aside_counts_until_another_game_takes_over() {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        games::set_steps_aside(&db, &client, true).unwrap();
        // The client from 18:00 to 21:00, a match from 20:00 that outlasts it.
        let client_session = played_game(&db, &client, "2026-10-02T18:00:00Z", 180);
        let match_session = played_game(&db, &match_game, "2026-10-02T20:00:00Z", 120);
        // Opened again inside the match, the client counts nothing.
        let covered = played_game(&db, &client, "2026-10-02T21:00:00Z", 30);

        let mut listed = sessions::list_all_sessions(&db).unwrap();
        fill_set_aside(&db, &LiveSessions::default(), &mut listed, 0).unwrap();
        let until = |id: &str| {
            listed
                .iter()
                .find(|session| session.id == id)
                .unwrap()
                .counted_until
                .clone()
        };
        assert_eq!(
            until(&client_session).as_deref(),
            Some("2026-10-02T20:00:00.000Z")
        );
        assert_eq!(until(&covered).as_deref(), Some("2026-10-02T21:00:00.000Z"));
        // Only games that step aside get it.
        assert_eq!(until(&match_session), None);
    }

    #[test]
    fn deleting_a_game_gives_back_the_time_set_aside_for_it() {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        games::set_steps_aside(&db, &client, true).unwrap();
        played_game(&db, &client, "2026-10-02T18:00:00Z", 180);
        played_game(&db, &match_game, "2026-10-02T19:00:00Z", 60);
        assert_eq!(day_minutes(&db, &client), 120);

        let spans = crate::playtime::slices::spans_beside(&db, &match_game).unwrap();
        games::delete_game(&db, &match_game).unwrap();
        crate::playtime::slices::rebuild_aside_around_spans(&db, &spans).unwrap();
        assert_eq!(day_minutes(&db, &client), 180);
        // A game that steps aside sets no time aside for others.
        assert!(
            crate::playtime::slices::spans_beside(&db, &client)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_linked_install_counts_for_the_game_shown_and_once_at_a_time() {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let (steam, gog) = (game(&db, "Steam copy"), game(&db, "GOG copy"));
        played_game(&db, &gog, "2026-10-02T15:00:00Z", 60);
        assert_eq!(day_minutes(&db, &steam), 0);

        games::link_game(&db, &gog, Some(&steam)).unwrap();
        assert_eq!(day_minutes(&db, &steam), 60);
        // Both copies left running at once count as one game playing.
        played_game(&db, &steam, "2026-10-02T18:00:00Z", 60);
        played_game(&db, &gog, "2026-10-02T18:30:00Z", 60);
        assert_eq!(day_minutes(&db, &steam), 60 + 90);
        let all = play_totals(
            &db,
            &LiveSessions::default(),
            &FixedOffset::east_opt(0).unwrap(),
            date("2026-10-02"),
            date("2026-10-03"),
            Bucket::Day,
            None,
            0,
        )
        .unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].game_id, steam);
        assert_eq!(all[0].active_ms, all[0].runtime_ms);

        // Apart again, each has its own.
        games::link_game(&db, &gog, None).unwrap();
        assert_eq!(day_minutes(&db, &steam), 60);
        assert_eq!(day_minutes(&db, &gog), 120);
    }

    /// Each listed session's runtime, active and idle time without what it
    /// shares with another copy of its game.
    fn counted_once_by_session(db: &Database, live: &LiveSessions, now_ms: i64) -> Vec<[i64; 3]> {
        let mut listed = sessions::list_all_sessions(db).unwrap();
        for session in &mut listed {
            live.apply(session);
        }
        fill_copy_overlap(db, live, &mut listed, now_ms).unwrap();
        listed
            .iter()
            .map(|session| {
                [
                    session.runtime_ms - session.copy_overlap_ms,
                    session.active_ms - session.copy_overlap_active_ms,
                    session.idle_ms - session.copy_overlap_idle_ms,
                ]
            })
            .collect()
    }

    #[test]
    fn sessions_of_linked_installs_add_up_to_the_totals() {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let (steam, gog, other) = (
            game(&db, "Steam copy"),
            game(&db, "GOG copy"),
            game(&db, "Other"),
        );
        games::link_game(&db, &gog, Some(&steam)).unwrap();
        played_game(&db, &steam, "2026-10-02T18:00:00Z", 60);
        played_game(&db, &gog, "2026-10-02T18:30:00Z", 60);
        // A copy that starts and ends inside a quarter hour of the other.
        played_game(&db, &gog, "2026-10-02T21:05:00Z", 5);
        played_game(&db, &steam, "2026-10-02T21:00:00Z", 15);
        // Another game beside it is no copy and keeps all of its time.
        played_game(&db, &other, "2026-10-02T18:00:00Z", 90);

        let once = counted_once_by_session(&db, &LiveSessions::default(), 0);
        let sum = |index: usize| once.iter().map(|measures| measures[index]).sum::<i64>();
        let totals = play_totals(
            &db,
            &LiveSessions::default(),
            &FixedOffset::east_opt(0).unwrap(),
            date("2026-10-02"),
            date("2026-10-03"),
            Bucket::Day,
            None,
            0,
        )
        .unwrap();
        let total = |pick: fn(&PlayTotal) -> i64| totals.iter().map(pick).sum::<i64>();
        assert_eq!(sum(0), total(|total| total.runtime_ms));
        assert_eq!(sum(1), total(|total| total.active_ms));
        assert_eq!(sum(2), total(|total| total.idle_ms));
        assert_eq!(day_minutes(&db, &steam), 90 + 15);
        assert_eq!(day_minutes(&db, &other), 90);
        assert!(once.contains(&[90 * MINUTE, 90 * MINUTE, 0]));

        // Unlinked, nothing is shared.
        games::link_game(&db, &gog, None).unwrap();
        let apart = counted_once_by_session(&db, &LiveSessions::default(), 0);
        assert_eq!(
            apart.iter().map(|measures| measures[0]).sum::<i64>(),
            (60 + 60 + 5 + 15 + 90) * MINUTE
        );
    }

    #[test]
    fn a_running_copy_shares_its_time_with_the_copy_beside_it() {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let (steam, gog) = (game(&db, "Steam copy"), game(&db, "GOG copy"));
        games::link_game(&db, &gog, Some(&steam)).unwrap();
        played_game(&db, &steam, "2026-10-02T18:00:00Z", 60);
        let start = "2026-10-02T18:30:00Z";
        let running = sessions::create_session_at(&db, &gog, "device", utc(start)).unwrap();
        let live = LiveSessions::default();
        live.update(
            &running.id,
            LiveCounters {
                runtime_ms: 60 * MINUTE,
                active_ms: 60 * MINUTE,
                idle_ms: 0,
            },
        );
        let now = (utc(start) + TimeDelta::minutes(60)).timestamp_millis();

        let mut active = vec![running.clone()];
        live.apply(&mut active[0]);
        fill_copy_overlap(&db, &live, &mut active, now).unwrap();
        // Half of the half hour both ran is the running copy's.
        assert_eq!(active[0].copy_overlap_ms, 15 * MINUTE);
        let once = counted_once_by_session(&db, &live, now);
        assert_eq!(
            once.iter().map(|measures| measures[0]).sum::<i64>(),
            90 * MINUTE
        );
    }

    #[test]
    fn games_that_step_aside_do_not_set_each_other_aside() {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let (one, other) = (game(&db, "Launcher"), game(&db, "Store"));
        games::set_steps_aside(&db, &one, true).unwrap();
        games::set_steps_aside(&db, &other, true).unwrap();
        played_game(&db, &one, "2026-10-02T18:00:00Z", 60);
        played_game(&db, &other, "2026-10-02T18:30:00Z", 60);
        assert_eq!(day_minutes(&db, &one), 60);
        assert_eq!(day_minutes(&db, &other), 60);
    }

    /// Clock time with a game running on 2 October 2026 in UTC, in minutes,
    /// and each game's runtime added up.
    fn played_that_day(db: &Database) -> (i64, i64) {
        let totals = day_totals(
            db,
            &LiveSessions::default(),
            &FixedOffset::east_opt(0).unwrap(),
            date("2026-10-02"),
            date("2026-10-03"),
            0,
        )
        .unwrap();
        let games: i64 = totals.games.iter().map(|total| total.runtime_ms).sum();
        (totals.played_ms() / MINUTE, games / MINUTE)
    }

    #[test]
    fn games_side_by_side_count_once_in_the_time_played() {
        let db = testing::database();
        let (a, b) = (game(&db, "A"), game(&db, "B"));
        testing::played(&db, &a, "2026-10-02T18:00:00Z", 120);
        testing::played(&db, &b, "2026-10-02T19:00:00Z", 120);
        assert_eq!(played_that_day(&db), (180, 240));
    }

    #[test]
    fn games_taking_turns_in_one_quarter_hour_count_in_full() {
        let db = testing::database();
        let (a, b) = (game(&db, "A"), game(&db, "B"));
        testing::played(&db, &a, "2026-10-02T18:00:00Z", 5);
        testing::played(&db, &b, "2026-10-02T18:10:00Z", 4);
        assert_eq!(played_that_day(&db), (9, 9));
    }

    #[test]
    fn one_game_alone_plays_exactly_its_time() {
        let db = testing::database();
        let a = game(&db, "A");
        testing::recorded(
            &db,
            &a,
            "device",
            "2026-10-02T18:07:00Z",
            &[
                testing::Mark::Check(20, 20, 5),
                testing::Mark::Check(61, 60, 40),
            ],
        );
        let totals = day_totals(
            &db,
            &LiveSessions::default(),
            &FixedOffset::east_opt(0).unwrap(),
            date("2026-10-02"),
            date("2026-10-03"),
            0,
        )
        .unwrap();
        assert_eq!(totals.played_ms(), 60 * MINUTE);
        assert_eq!(totals.games[0].runtime_ms, 60 * MINUTE);
    }

    #[test]
    fn a_sleep_both_games_slept_through_is_left_out() {
        let db = testing::database();
        let (a, b) = (game(&db, "A"), game(&db, "B"));
        // Both on the clock from 10:00 to 12:00, the PC asleep from 10:30 to 11:30.
        let marks = [
            testing::Mark::Check(30, 30, 30),
            testing::Mark::Gap(90),
            testing::Mark::Check(120, 60, 60),
        ];
        testing::recorded(&db, &a, "device", "2026-10-02T10:00:00Z", &marks);
        testing::recorded(&db, &b, "device", "2026-10-02T10:00:00Z", &marks);
        assert_eq!(played_that_day(&db), (60, 120));
    }

    #[test]
    fn time_set_aside_counts_nowhere_in_the_time_played() {
        let db = testing::database();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        games::set_steps_aside(&db, &client, true).unwrap();
        testing::played(&db, &client, "2026-10-02T18:00:00Z", 180);
        testing::played(&db, &match_game, "2026-10-02T19:00:00Z", 60);
        assert_eq!(played_that_day(&db), (180, 180));
    }

    #[test]
    fn copies_of_one_game_count_once_in_the_time_played() {
        let db = testing::database();
        let (steam, gog) = (game(&db, "Steam copy"), game(&db, "GOG copy"));
        games::link_game(&db, &gog, Some(&steam)).unwrap();
        testing::played(&db, &steam, "2026-10-02T18:00:00Z", 60);
        testing::played(&db, &gog, "2026-10-02T18:30:00Z", 60);
        assert_eq!(played_that_day(&db), (90, 90));
    }

    #[test]
    fn pages_get_what_counts_of_each_session() {
        let db = testing::database();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        let copy = game(&db, "Match copy");
        games::set_steps_aside(&db, &client, true).unwrap();
        games::link_game(&db, &copy, Some(&match_game)).unwrap();
        let launcher = testing::played(&db, &client, "2026-10-02T18:00:00Z", 180);
        testing::played(&db, &match_game, "2026-10-02T19:00:00Z", 60);
        testing::played(&db, &copy, "2026-10-02T19:30:00Z", 60);

        let mut listed = sessions::list_all_sessions(&db).unwrap();
        show_counted(&db, &LiveSessions::default(), &mut listed, 0).unwrap();
        let client_row = listed.iter().find(|row| row.id == launcher).unwrap();
        // The match and its copy ran from 19:00 to 20:30.
        assert_eq!(client_row.runtime_ms, 90 * MINUTE);
        assert_eq!(client_row.set_aside_ms, 90 * MINUTE);
        assert_eq!(client_row.counted_runtime_ms, 90 * MINUTE);
        let counted: i64 = listed.iter().map(|row| row.counted_runtime_ms).sum();
        assert_eq!(counted, played_that_day(&db).1 * MINUTE);
        assert_eq!(played_that_day(&db), (180, 180));
    }
}
