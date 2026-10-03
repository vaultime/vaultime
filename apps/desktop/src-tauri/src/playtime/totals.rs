// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Play time per day, week, month or year, added up from the slices.

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::connection::Database;
use crate::db::repo::map_db;
use crate::db::repo::sessions::row_to_session;
use crate::error::Result;
use crate::integrity;
use crate::playtime::slices::{Slice, slices_of};
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
    {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM sessions
                 WHERE ended_at_wall IS NULL AND (?1 IS NULL OR game_id = ?1)",
            )
            .map_err(map_db)?;
        let open = stmt
            .query_map([game_id], row_to_session)
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)?;
        let mut placed = Vec::new();
        for session in open {
            let events = integrity::load_session_events(conn, &session.id)?;
            let latest = live
                .get(&session.id)
                .map(|counters| (now_ms, counters))
                .or_else(|| {
                    let last = events.last()?;
                    let at = DateTime::parse_from_rfc3339(&last.event_time_wall).ok()?;
                    Some((
                        at.timestamp_millis(),
                        LiveCounters {
                            runtime_ms: session.runtime_ms,
                            active_ms: session.active_ms,
                            idle_ms: session.idle_ms,
                        },
                    ))
                });
            placed.extend(
                slices_of(&session, &events, latest)
                    .into_iter()
                    .map(|slice| (session.game_id.clone(), slice)),
            );
        }
        Ok(placed)
    }
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
                   AND (?3 IS NULL OR sessions.game_id = ?3)",
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
    for (game, slice) in slices {
        let Some(key) = bucket_of(tz, slice.start_ms, bucket) else {
            continue;
        };
        let total = totals.entry((key, game)).or_default();
        total[0] += slice.runtime_ms;
        total[1] += slice.active_ms;
        total[2] += slice.idle_ms;
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
    use crate::playtime::slices::rebuild_session;

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
        let session =
            sessions::create_session_at(&library.db, &library.game_id, "device", utc(start))
                .unwrap();
        let runtime = minutes * MINUTE;
        library
            .db
            .with_conn(|conn| {
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
                rebuild_session(conn, &session.id)
            })
            .unwrap();
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
}
