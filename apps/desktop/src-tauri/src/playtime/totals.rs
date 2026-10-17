// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Play time per day, week, month or year, added up from the slices.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::constants::PLAY_SLICE_MS;
use crate::db::connection::Database;
use crate::db::models::Session;
use crate::db::repo::games;
use crate::db::repo::map_db;
use crate::db::repo::sessions::row_to_session;
use crate::error::Result;
use crate::playtime::slices::{
    STEPS_ASIDE, Slice, TimedEvent, apportion, aside_stretches, placing_events, slice_of,
    slices_of, sum_of, wall_ms,
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

/// The slices of running sessions, placed up to the tracker's latest tick.
/// A session the tracker does not run, left over from a crash, counts up to
/// its last event.
fn running_slices(
    conn: &rusqlite::Connection,
    live: &LiveSessions,
    game_id: Option<&str>,
    now_ms: i64,
) -> Result<Vec<(String, Slice)>> {
    let mut stmt = conn
        .prepare(
            "SELECT * FROM sessions
             WHERE ended_at_wall IS NULL AND (?1 IS NULL OR game_id = ?1
                 OR game_id IN (SELECT game_id FROM game_links WHERE linked_game_id = ?1))",
        )
        .map_err(map_db)?;
    let open = stmt
        .query_map([game_id], row_to_session)
        .map_err(map_db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_db)?;
    let mut placed = Vec::new();
    for session in open {
        placed.extend(
            open_session_slices(conn, live, &session, now_ms)?
                .into_iter()
                .map(|slice| (session.game_id.clone(), slice)),
        );
    }
    Ok(placed)
}

/// The slices of an open session, placed up to the tracker's latest tick,
/// or up to its last event when the tracker does not run it.
fn open_session_slices(
    conn: &rusqlite::Connection,
    live: &LiveSessions,
    session: &Session,
    now_ms: i64,
) -> Result<Vec<Slice>> {
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
    Ok(slices_of(session, &events, latest, &aside))
}

/// Fills in the time set aside of sessions whose game steps aside for
/// others: what the session holds minus what its slices count. Running
/// sessions, already brought up to the latest tick, are placed up to it.
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
        for session in sessions
            .iter_mut()
            .filter(|session| stepping.contains(&session.game_id))
        {
            let kept = if session.ended_at_wall.is_some() {
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
    let any_closed = sessions
        .iter()
        .any(|session| session.ended_at_wall.is_some());
    let slices: Vec<(String, String, Slice)> = db.with_transaction(|conn| {
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
        for (running, set) in running_beside_copies(conn)? {
            let game = set[0].clone();
            let from_ms = slice_of(wall_ms(&running.started_at_wall).unwrap_or(0));
            for (session, slice) in stored_slices_since(conn, &set, from_ms)? {
                add(session, game.clone(), slice);
            }
            for slice in open_session_slices(conn, live, &running, now_ms)? {
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
    let (from_ms, to_ms) = (day_start_ms(tz, from), day_start_ms(tz, to));
    // One snapshot for both reads, so a session that ends in between counts
    // once, as running or as stored.
    let slices: Vec<(String, Slice)> = db.with_transaction(|conn| {
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
        slices.extend(
            running_slices(conn, live, game_id, now_ms)?
                .into_iter()
                .filter(|(_, slice)| slice.start_ms >= from_ms && slice.start_ms < to_ms),
        );
        Ok(slices)
    })?;

    let mut totals: BTreeMap<(String, String), [i64; 3]> = BTreeMap::new();
    // A linked game, another install here or a game of another PC, counts
    // as the game it is linked to.
    let links = games::links(db)?;
    for ((game, start_ms), [runtime_ms, active_ms, idle_ms]) in once_at_a_time(&slices, &links) {
        let Some(key) = bucket_of(tz, start_ms, bucket) else {
            continue;
        };
        let total = totals.entry((key, game)).or_default();
        total[0] += runtime_ms;
        total[1] += active_ms;
        total[2] += idle_ms;
    }
    Ok(totals
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
        .collect())
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

    const MINUTE: i64 = 60_000;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn utc(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
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
}
