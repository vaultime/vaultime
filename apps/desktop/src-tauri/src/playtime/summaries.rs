// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Play summed up for the pages: a year for the stats, the last days for the
//! library and today for the live bar and the tray. Time counts on the day
//! it happened.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{NaiveDate, TimeDelta, TimeZone};
use rusqlite::params;
use serde::Serialize;

use crate::constants::{
    SESSION_LONG_MAX_MS, SESSION_PLAIN_MAX_MS, SESSION_QUICK_MAX_MS, SESSION_SHORT_MAX_MS,
    STREAK_FIRST_LOOK_DAYS,
};
use crate::db::connection::Database;
use crate::db::models::Session;
use crate::db::repo::games;
use crate::db::repo::map_db;
use crate::db::repo::sessions::row_to_session;
use crate::error::Result;
use crate::integrity;
use crate::playtime::journal::{GameTime, local_day};
use crate::playtime::slices::slices_from;
use crate::playtime::totals::{
    Bucket, PlayTotal, day_start_ms, day_totals, fill_set_aside, open_session_spans, open_sessions,
    placed_until, play_totals,
};
use crate::tracking::live::LiveSessions;

/// Days in a row with play.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Streak {
    pub days: i64,
    /// "2026-03-01".
    pub start: String,
    pub end: String,
}

/// A year of play for the stats page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct YearPlay {
    /// Each game's time per day.
    pub days: Vec<PlayTotal>,
    /// Each game's time per hour of the week.
    pub hours: Vec<PlayTotal>,
    /// Clock time with a game running, games side by side counted once.
    pub played_ms: i64,
    /// The same by local day, "2026-10-05", as the journal's days count it.
    /// Days without play are left out.
    pub played: BTreeMap<String, i64>,
    pub days_played: usize,
    /// The longest run of days with play in the year, the earliest on a tie.
    pub longest_streak: Option<Streak>,
    /// Days in a row with play up to today, or up to yesterday while today
    /// has none yet, however far back they go.
    pub current_streak: i64,
    /// Sessions with play that started in the year.
    pub sessions_count: usize,
    /// The same by the game each counts as.
    pub sessions_by_game: BTreeMap<String, usize>,
    /// The same by length: quick looks, short, plain, long and marathons.
    pub session_lengths: [usize; 5],
    /// The longest session that started in the year.
    pub longest_session_id: Option<String>,
}

/// The last few days of play, for the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentPlay {
    /// Clock time with a game running, games side by side counted once.
    pub played_ms: i64,
    pub runtime_ms: i64,
    pub active_ms: i64,
    /// Sessions that started in those days.
    pub sessions_count: usize,
    /// Days with play.
    pub days_count: usize,
    /// Each game's time, the most first.
    pub games: Vec<GameTime>,
    /// The longest session that started in those days.
    pub longest_session_id: Option<String>,
}

fn date_of(key: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(key, "%Y-%m-%d").ok()
}

/// The longest run of days in a row among `days`, the earliest on a tie.
fn longest_run(days: &BTreeSet<NaiveDate>) -> Option<Streak> {
    let mut best: Option<(NaiveDate, NaiveDate, i64)> = None;
    let mut current: Option<(NaiveDate, NaiveDate, i64)> = None;
    for &day in days {
        current = match current {
            Some((start, end, count)) if end + TimeDelta::days(1) == day => {
                Some((start, day, count + 1))
            }
            _ => Some((day, day, 1)),
        };
        if current.map(|(_, _, count)| count) > best.map(|(_, _, count)| count) {
            best = current;
        }
    }
    best.map(|(start, end, days)| Streak {
        days,
        start: start.format("%Y-%m-%d").to_string(),
        end: end.format("%Y-%m-%d").to_string(),
    })
}

/// Days with play of the open sessions from the local day `from` up to but
/// not including `to`. Running sessions keep no slices yet. A day has play
/// when a quarter hour of it does, also where copies of a game share one.
fn open_days_with_play<Tz: TimeZone>(
    db: &Database,
    live: &LiveSessions,
    tz: &Tz,
    from: NaiveDate,
    to: NaiveDate,
    now_ms: i64,
) -> Result<BTreeSet<NaiveDate>> {
    let (from_ms, to_ms) = (day_start_ms(tz, from), day_start_ms(tz, to));
    db.with_transaction(|conn| {
        let mut days = BTreeSet::new();
        for session in open_sessions(conn, None)? {
            if placed_until(conn, live, &session, now_ms)?.is_none_or(|until| until < from_ms) {
                continue;
            }
            let spans = open_session_spans(conn, live, &session, now_ms)?;
            days.extend(
                slices_from(&spans)
                    .into_iter()
                    .filter(|slice| {
                        slice.start_ms >= from_ms && slice.start_ms < to_ms && slice.runtime_ms > 0
                    })
                    .filter_map(|slice| local_day(tz, slice.start_ms)),
            );
        }
        Ok(days)
    })
}

/// Local days with play in the stored slices from `from_ms` on. Quicker than
/// totals, as only the days count.
fn stored_days_with_play<Tz: TimeZone>(
    db: &Database,
    tz: &Tz,
    from_ms: i64,
) -> Result<BTreeSet<NaiveDate>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare_cached(
                "SELECT DISTINCT slice_start FROM play_slices
                 WHERE slice_start >= ?1 AND runtime_ms > 0",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([from_ms], |row| row.get::<_, i64>(0))
            .map_err(map_db)?;
        let mut days = BTreeSet::new();
        for row in rows {
            days.extend(local_day(tz, row.map_err(map_db)?));
        }
        Ok(days)
    })
}

/// Days in a row with play up to `today`, or up to the day before while
/// `today` has none yet, looked for in `first_look` days first and in the
/// whole history when it goes back further.
fn current_streak<Tz: TimeZone>(
    db: &Database,
    live: &LiveSessions,
    tz: &Tz,
    (today, now_ms): (NaiveDate, i64),
    first_look: i64,
) -> Result<i64> {
    let (first_play, first_open) = db.with_conn(|conn| {
        conn.query_row(
            "SELECT MIN(started_at_wall),
                    MIN(CASE WHEN ended_at_wall IS NULL THEN started_at_wall END)
             FROM sessions",
            [],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .map_err(map_db)
    })?;
    let day_of = |wall: Option<String>| {
        wall.as_deref()
            .and_then(crate::playtime::slices::wall_ms)
            .and_then(|ms| local_day(tz, ms))
    };
    let Some(first_day) = day_of(first_play) else {
        return Ok(0);
    };
    let yesterday = today - TimeDelta::days(1);
    let open_from = day_of(first_open).map_or(yesterday, |day| day.min(yesterday));
    let mut look = first_look;
    loop {
        let from = today - TimeDelta::days(look);
        let mut days = stored_days_with_play(db, tz, day_start_ms(tz, from))?;
        // Days before `from` never decide the streak, so a session another
        // PC left open long ago is not placed.
        days.extend(open_days_with_play(
            db,
            live,
            tz,
            open_from.max(from),
            today + TimeDelta::days(1),
            now_ms,
        )?);
        let mut day = if days.contains(&today) {
            today
        } else {
            yesterday
        };
        let mut streak = 0;
        while days.contains(&day) {
            streak += 1;
            day -= TimeDelta::days(1);
        }
        // The streak may go on before what was read, unless the history does not.
        if day >= from || from <= first_day {
            return Ok(streak);
        }
        look = (today - first_day).num_days() + 1;
    }
}

/// The year `year` for the stats page, with `today` and `now_ms` for the
/// running sessions and the current streak.
pub fn year_play<Tz: TimeZone>(
    db: &Database,
    live: &LiveSessions,
    tz: &Tz,
    year: i32,
    today: NaiveDate,
    now_ms: i64,
) -> Result<YearPlay> {
    let first = NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or(today);
    let next = NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap_or(today);
    let totals = day_totals(db, live, tz, first, next, now_ms)?;
    let played_days: BTreeSet<NaiveDate> = totals
        .days_played()
        .into_iter()
        .filter_map(date_of)
        .collect();
    let started = started_sessions(db, live, tz, first, Some(next), now_ms)?;
    let links = games::links(db)?;
    let mut sessions_by_game: BTreeMap<String, usize> = BTreeMap::new();
    let mut session_lengths = [0; 5];
    for session in &started {
        let shown = links.get(&session.game_id).unwrap_or(&session.game_id);
        *sessions_by_game.entry(shown.clone()).or_default() += 1;
        session_lengths[length_class(shown_runtime(session))] += 1;
    }
    Ok(YearPlay {
        hours: play_totals(db, live, tz, first, next, Bucket::HourOfWeek, None, now_ms)?,
        played_ms: totals.played_ms(),
        days_played: played_days.len(),
        longest_streak: longest_run(&played_days),
        current_streak: current_streak(db, live, tz, (today, now_ms), STREAK_FIRST_LOOK_DAYS)?,
        days: totals.games,
        played: totals.played,
        sessions_count: started.len(),
        sessions_by_game,
        session_lengths,
        longest_session_id: longest_of(&started),
    })
}

/// Sessions with play that started from the local day `from` on, up to but
/// not including `to`, running ones as of the tracker's latest tick, each
/// with the time it set aside.
fn started_sessions<Tz: TimeZone>(
    db: &Database,
    live: &LiveSessions,
    tz: &Tz,
    from: NaiveDate,
    to: Option<NaiveDate>,
    now_ms: i64,
) -> Result<Vec<Session>> {
    let wall_of = |day: NaiveDate| {
        chrono::DateTime::from_timestamp_millis(day_start_ms(tz, day))
            .map_or_else(String::new, integrity::format_timestamp)
    };
    let from_wall = wall_of(from);
    // Past every stored time, which all start with a four digit year.
    let to_wall = to.map_or_else(|| "~".to_owned(), wall_of);
    let mut started: Vec<Session> = db.with_conn(|conn| {
        let mut stmt = conn
            .prepare_cached(
                "SELECT * FROM sessions
                 WHERE started_at_wall >= ?1 AND started_at_wall < ?2
                   AND (ended_at_wall IS NULL OR runtime_ms > 0)",
            )
            .map_err(map_db)?;
        stmt.query_map(params![from_wall, to_wall], row_to_session)
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)
    })?;
    for session in &mut started {
        live.apply(session);
    }
    fill_set_aside(db, live, &mut started, now_ms)?;
    Ok(started)
}

/// A session's runtime as the pages show it, without the time set aside.
fn shown_runtime(session: &Session) -> i64 {
    session.runtime_ms - session.set_aside_ms
}

fn longest_of(sessions: &[Session]) -> Option<String> {
    sessions
        .iter()
        .max_by_key(|session| shown_runtime(session))
        .map(|session| session.id.clone())
}

/// Which of `YearPlay::session_lengths` a session of `runtime_ms` counts in.
fn length_class(runtime_ms: i64) -> usize {
    [
        SESSION_QUICK_MAX_MS,
        SESSION_SHORT_MAX_MS,
        SESSION_PLAIN_MAX_MS,
        SESSION_LONG_MAX_MS,
    ]
    .iter()
    .take_while(|&&max| runtime_ms >= max)
    .count()
}

/// The last `days` local days of play, today included.
pub fn recent_play<Tz: TimeZone>(
    db: &Database,
    live: &LiveSessions,
    tz: &Tz,
    days: u32,
    today: NaiveDate,
    now_ms: i64,
) -> Result<RecentPlay> {
    let from = today - TimeDelta::days(i64::from(days.max(1)) - 1);
    let totals = day_totals(db, live, tz, from, today + TimeDelta::days(1), now_ms)?;
    let started = started_sessions(db, live, tz, from, None, now_ms)?;
    let longest = longest_of(&started);
    let games = crate::playtime::journal::game_times(&totals);
    Ok(RecentPlay {
        played_ms: totals.played_ms(),
        runtime_ms: games.iter().map(|game| game.runtime_ms).sum(),
        active_ms: games.iter().map(|game| game.active_ms).sum(),
        sessions_count: started.len(),
        days_count: totals.days_played().len(),
        games,
        longest_session_id: longest,
    })
}

/// Clock time with a game running on the local day `today`, games side by
/// side counted once and time set aside left out, as the journal counts it.
pub fn played_on<Tz: TimeZone>(
    db: &Database,
    live: &LiveSessions,
    tz: &Tz,
    today: NaiveDate,
    now_ms: i64,
) -> Result<i64> {
    Ok(day_totals(db, live, tz, today, today + TimeDelta::days(1), now_ms)?.played_ms())
}

/// The local day `now_ms` falls in.
pub fn today_of<Tz: TimeZone>(tz: &Tz, now_ms: i64) -> NaiveDate {
    local_day(tz, now_ms).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use chrono_tz::Europe::Berlin;

    use super::*;
    use crate::db::repo::games;
    use crate::playtime::testing::{MINUTE, database, game, played, utc};

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    /// The last seven days up to Tuesday 29 September 2026, 21:30 in Berlin.
    fn recent(db: &Database) -> RecentPlay {
        let now = utc("2026-09-29T19:30:00Z").timestamp_millis();
        recent_play(
            db,
            &LiveSessions::default(),
            &Berlin,
            7,
            date("2026-09-29"),
            now,
        )
        .unwrap()
    }

    #[test]
    fn recent_play_counts_time_on_the_day_it_happened() {
        let db = database();
        let (a, b, c) = (game(&db, "A"), game(&db, "B"), game(&db, "C"));
        played(&db, &a, "2026-09-29T18:00:00Z", 60);
        let long = played(&db, &b, "2026-09-26T16:00:00Z", 240);
        played(&db, &a, "2026-09-22T22:30:00Z", 30);
        // 23:30 on the day before the seven, half of it past midnight.
        played(&db, &c, "2026-09-22T21:30:00Z", 60);
        let recent = recent(&db);
        assert_eq!(recent.sessions_count, 3);
        assert_eq!(recent.days_count, 3);
        assert_eq!(recent.runtime_ms, 360 * MINUTE);
        let by_game: Vec<(&str, i64)> = recent
            .games
            .iter()
            .map(|time| (time.game_id.as_str(), time.runtime_ms / MINUTE))
            .collect();
        assert_eq!(
            by_game,
            [(b.as_str(), 240), (a.as_str(), 90), (c.as_str(), 30)]
        );
        assert_eq!(recent.longest_session_id, Some(long));
    }

    #[test]
    fn recent_play_counts_games_side_by_side_once() {
        let db = database();
        let (a, b) = (game(&db, "A"), game(&db, "B"));
        played(&db, &a, "2026-09-29T12:00:00Z", 250);
        played(&db, &b, "2026-09-29T14:30:00Z", 190);
        let recent = recent(&db);
        assert_eq!(recent.runtime_ms, 440 * MINUTE);
        assert_eq!(recent.played_ms, 340 * MINUTE);
    }

    #[test]
    fn the_longest_recent_session_leaves_time_set_aside_out() {
        let db = database();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        games::set_steps_aside(&db, &client, true).unwrap();
        played(&db, &client, "2026-09-28T16:00:00Z", 180);
        let inside = played(&db, &match_game, "2026-09-28T16:30:00Z", 150);
        assert_eq!(recent(&db).longest_session_id, Some(inside));
    }

    #[test]
    fn finds_the_longest_streak_and_the_current_one() {
        let db = database();
        let one = game(&db, "One");
        for day in ["03-01", "03-02", "03-03", "03-05", "09-28", "09-29"] {
            played(&db, &one, &format!("2026-{day}T18:00:00Z"), 30);
        }
        let now = utc("2026-09-30T19:00:00Z").timestamp_millis();
        let year = year_play(
            &db,
            &LiveSessions::default(),
            &Berlin,
            2026,
            date("2026-09-30"),
            now,
        )
        .unwrap();
        assert_eq!(
            year.longest_streak,
            Some(Streak {
                days: 3,
                start: "2026-03-01".into(),
                end: "2026-03-03".into()
            })
        );
        // Nothing played today yet, so the streak up to yesterday counts.
        assert_eq!(year.current_streak, 2);
        assert_eq!(year.days_played, 6);
        assert_eq!(year.played_ms, 180 * MINUTE);
    }

    #[test]
    fn counts_sessions_in_the_year_they_started_by_game_and_length() {
        let db = database();
        let (one, two) = (game(&db, "One"), game(&db, "Two"));
        // 23:30 on New Year's Eve in Berlin: it counts in the year it started.
        played(&db, &one, "2025-12-31T22:30:00Z", 90);
        played(&db, &one, "2026-03-02T18:00:00Z", 10);
        let long = played(&db, &two, "2026-03-03T18:00:00Z", 300);
        played(&db, &two, "2026-03-04T18:00:00Z", 90);
        let year = |year| {
            year_play(
                &db,
                &LiveSessions::default(),
                &Berlin,
                year,
                date("2026-10-01"),
                utc("2026-10-01T12:00:00Z").timestamp_millis(),
            )
            .unwrap()
        };
        let this = year(2026);
        assert_eq!(this.sessions_count, 3);
        assert_eq!(
            this.sessions_by_game,
            BTreeMap::from([(one.clone(), 1), (two, 2)])
        );
        assert_eq!(this.session_lengths, [1, 0, 1, 0, 1]);
        assert_eq!(this.longest_session_id, Some(long));
        let before = year(2025);
        assert_eq!(before.sessions_count, 1);
        assert_eq!(before.sessions_by_game, BTreeMap::from([(one, 1)]));
    }

    #[test]
    fn keeps_a_streak_across_the_night_the_clocks_change() {
        let db = database();
        let one = game(&db, "One");
        for day in [28, 29, 30] {
            played(&db, &one, &format!("2026-03-{day}T18:00:00Z"), 30);
        }
        let now = utc("2026-04-02T10:00:00Z").timestamp_millis();
        let year = year_play(
            &db,
            &LiveSessions::default(),
            &Berlin,
            2026,
            date("2026-04-02"),
            now,
        )
        .unwrap();
        assert_eq!(year.longest_streak.map(|streak| streak.days), Some(3));
    }

    #[test]
    fn a_current_streak_reaches_back_past_the_first_look() {
        let db = database();
        let one = game(&db, "One");
        let today = date("2026-10-10");
        for back in 0..5 {
            let day = today - TimeDelta::days(back);
            played(&db, &one, &format!("{day}T18:00:00Z"), 30);
        }
        let now = utc("2026-10-10T19:00:00Z").timestamp_millis();
        let live = LiveSessions::default();
        assert_eq!(
            current_streak(&db, &live, &Berlin, (today, now), 2).unwrap(),
            5
        );
        assert_eq!(
            current_streak(&db, &live, &Berlin, (today, now), 400).unwrap(),
            5
        );
    }

    /// A session of `game_id` the laptop left open after `minutes` of play
    /// from `start`, as a restored backup brings it.
    fn left_open_on_laptop(db: &Database, game_id: &str, start: &str, minutes: i64) {
        let session =
            crate::db::repo::sessions::create_session_at(db, game_id, "laptop", utc(start))
                .unwrap();
        let runtime = minutes * MINUTE;
        db.with_conn(|conn| {
            integrity::append_session_event(
                conn,
                &session.id,
                "heartbeat",
                &integrity::format_timestamp(utc(start) + TimeDelta::milliseconds(runtime)),
                Some(runtime),
                &serde_json::json!({
                    "runtime_ms": runtime, "active_ms": runtime, "idle_ms": 0,
                    "integrity_status": "local",
                })
                .to_string(),
            )?;
            conn.execute(
                "UPDATE sessions SET runtime_ms = ?1, elapsed_monotonic_ms = ?1, active_ms = ?1
                 WHERE id = ?2",
                params![runtime, session.id],
            )
            .map_err(map_db)?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_session_left_open_elsewhere_counts_up_to_its_last_event() {
        let db = database();
        let one = game(&db, "One");
        left_open_on_laptop(&db, &one, "2026-03-01T18:00:00Z", 45);
        for day in ["03-02", "03-03", "09-29"] {
            played(&db, &one, &format!("2026-{day}T18:00:00Z"), 30);
        }
        let live = LiveSessions::default();
        let totals = |from: &str, to: &str, now: &str| {
            play_totals(
                &db,
                &live,
                &Berlin,
                date(from),
                date(to),
                Bucket::Day,
                None,
                utc(now).timestamp_millis(),
            )
            .unwrap()
            .iter()
            .map(|total| (total.bucket.clone(), total.runtime_ms / MINUTE))
            .collect::<Vec<_>>()
        };
        assert_eq!(
            totals("2026-03-01", "2026-03-02", "2026-09-30T10:00:00Z"),
            vec![("2026-03-01".into(), 45)]
        );
        assert_eq!(
            totals("2026-03-02", "2026-10-01", "2026-09-30T10:00:00Z"),
            vec![
                ("2026-03-02".into(), 30),
                ("2026-03-03".into(), 30),
                ("2026-09-29".into(), 30)
            ]
        );
        // Its day still carries a streak that reaches it.
        let now = utc("2026-03-03T19:00:00Z").timestamp_millis();
        for first_look in [1, 400] {
            assert_eq!(
                current_streak(&db, &live, &Berlin, (date("2026-03-03"), now), first_look).unwrap(),
                3
            );
        }
        let now = utc("2026-09-30T10:00:00Z").timestamp_millis();
        assert_eq!(
            current_streak(&db, &live, &Berlin, (date("2026-09-30"), now), 400).unwrap(),
            1
        );
        assert_eq!(
            played_on(&db, &live, &Berlin, date("2026-09-30"), now).unwrap(),
            0
        );
    }

    #[test]
    fn today_counts_only_the_part_after_midnight() {
        let db = database();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        games::set_steps_aside(&db, &client, true).unwrap();
        // 22:00 to 02:00 in Berlin, a match beside the client until 23:00.
        played(&db, &client, "2026-10-01T20:00:00Z", 240);
        played(&db, &match_game, "2026-10-01T20:00:00Z", 60);
        played(&db, &match_game, "2026-09-01T18:00:00Z", 60);
        let now = utc("2026-10-02T10:00:00Z").timestamp_millis();
        let today = today_of(&Berlin, now);
        assert_eq!(today, date("2026-10-02"));
        assert_eq!(
            played_on(&db, &LiveSessions::default(), &Berlin, today, now).unwrap(),
            120 * MINUTE
        );
    }
}
