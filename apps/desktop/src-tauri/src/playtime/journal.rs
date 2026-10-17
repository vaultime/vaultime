// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The journal's week: each day's strip and time played, the part of every
//! session on each day it ran, and the numbers of the week's sentence.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, NaiveDate, TimeDelta, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::constants::DAYS_PER_WEEK;
use crate::db::connection::Database;
use crate::db::models::Session;
use crate::db::repo::sessions::row_to_session;
use crate::db::repo::{games, map_db};
use crate::error::Result;
use crate::integrity;
use crate::playtime::slices::{STEPS_ASIDE, Span, apportion, aside_stretches, wall_ms};
use crate::playtime::strip::{PlayRun, SideBySideGroup, StripPart, play_runs, side_by_side};
use crate::playtime::totals::{DayTotals, day_start_ms, day_totals, open_session_spans};
use crate::tracking::live::LiveSessions;

/// The part of a session that ran on one local day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DayShare {
    /// "2026-10-05".
    pub day: String,
    /// Where the part starts and ends on the clock, inside the day, in
    /// milliseconds since 1970 in UTC.
    pub start_ms: i64,
    pub end_ms: i64,
    /// The session's time that counts on this day, from where it happened.
    pub runtime_ms: i64,
    /// The session counted time on an earlier day.
    pub from_before: bool,
    /// The session counts time on a later day.
    pub into_next: bool,
}

/// A game's time over some days.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameTime {
    pub game_id: String,
    pub runtime_ms: i64,
    pub active_ms: i64,
    pub idle_ms: i64,
}

/// One day of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JournalDay {
    /// "2026-10-05".
    pub day: String,
    /// Clock time with a game running, games side by side counted once.
    pub played_ms: i64,
    /// Sessions that started on this day, in the order they started.
    pub session_ids: Vec<String>,
    /// Every session's parts on the strip, in the order they began.
    pub parts: Vec<StripPart>,
    pub runs: Vec<PlayRun>,
    pub side_by_side: Vec<SideBySideGroup>,
}

/// A week of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JournalWeek {
    /// Days with play or with sessions that started on them, newest first.
    pub days: Vec<JournalDay>,
    /// Each session with time in the week, cut at local midnights, the day it
    /// started first and later days only where it ran.
    pub shares: BTreeMap<String, Vec<DayShare>>,
    /// Games that played in the week, in the order they first showed up.
    pub games: Vec<String>,
    /// Clock time with a game running, games side by side counted once.
    pub played_ms: i64,
    /// Each game's time in the week, the most first.
    pub game_times: Vec<GameTime>,
    /// Sessions that started in the week.
    pub sessions_count: usize,
    pub days_played: usize,
    /// The day with the most time played, the latest on a tie.
    pub longest_day: Option<String>,
    /// Sessions that began before the week and ran on into it.
    pub carried_over: usize,
}

/// A session as the journal places it.
struct Placed {
    session: Session,
    /// The game it counts as.
    game_id: String,
    start_ms: i64,
    /// Up to now for a session running here, up to its last event for one
    /// another PC left running.
    end_ms: i64,
    running_here: bool,
    counts: bool,
    /// Where the strip draws it: its span on the clock, without the stretches
    /// a game that steps aside set aside.
    parts: Vec<(i64, i64)>,
    /// Where its counted time fell.
    spans: Vec<Span>,
}

/// The local day `ms` falls in.
pub(crate) fn local_day<Tz: TimeZone>(tz: &Tz, ms: i64) -> Option<NaiveDate> {
    Some(
        DateTime::<Utc>::from_timestamp_millis(ms)?
            .with_timezone(tz)
            .date_naive(),
    )
}

/// Whether a span on the clock falls between `from` and `to`. A span without
/// length counts where it starts.
fn touches(start: i64, end: i64, from: i64, to: i64) -> bool {
    if end <= start {
        return start >= from && start < to;
    }
    start.max(from) < end.min(to)
}

/// `span` without the stretches of `aside`, sorted and apart.
fn without(span: (i64, i64), aside: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let (start, end) = span;
    if end <= start || aside.is_empty() {
        return vec![span];
    }
    let mut parts = Vec::new();
    let mut cursor = start;
    for &(from, to) in aside {
        if to <= cursor || from >= end {
            continue;
        }
        if from > cursor {
            parts.push((cursor, from));
        }
        cursor = cursor.max(to);
    }
    if cursor < end {
        parts.push((cursor, end));
    }
    parts
}

fn format_ms(ms: i64) -> String {
    DateTime::from_timestamp_millis(ms).map_or_else(String::new, integrity::format_timestamp)
}

/// Sessions with time on the clock between `from_ms` and `to_ms`, placed up
/// to `now_ms`, with their parts and stretches.
fn placed_between(
    conn: &Connection,
    live: &LiveSessions,
    links: &HashMap<String, String>,
    this_device: &str,
    (from_ms, to_ms): (i64, i64),
    now_ms: i64,
) -> Result<Vec<Placed>> {
    // Read with a day to spare, as stored times may differ in form, then
    // checked on the clock.
    let margin = TimeDelta::days(1).num_milliseconds();
    let mut stmt = conn
        .prepare_cached(
            "SELECT * FROM sessions
             WHERE started_at_wall < ?2 AND (ended_at_wall IS NULL OR ended_at_wall >= ?1)
             ORDER BY started_at_wall, id",
        )
        .map_err(map_db)?;
    let stored = stmt
        .query_map(
            params![format_ms(from_ms - margin), format_ms(to_ms + margin)],
            row_to_session,
        )
        .map_err(map_db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_db)?;
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
    let mut placed = Vec::new();
    for mut session in stored {
        live.apply(&mut session);
        let Some(start_ms) = wall_ms(&session.started_at_wall) else {
            continue;
        };
        let running_here = session.ended_at_wall.is_none() && session.device_id == this_device;
        let end_ms = match session.ended_at_wall.as_deref() {
            Some(end) => wall_ms(end).unwrap_or(start_ms),
            None if running_here => now_ms,
            None => last_event
                .query_row([&session.id], |row| row.get::<_, String>(0))
                .optional()
                .map_err(map_db)?
                .as_deref()
                .and_then(wall_ms)
                .unwrap_or(start_ms),
        };
        if !touches(start_ms, end_ms, from_ms, to_ms) {
            continue;
        }
        let counts = session.ended_at_wall.is_none() || session.runtime_ms > 0;
        let (parts, spans) = if counts {
            let aside = if stepping.contains(&session.game_id) {
                aside_stretches(conn, &session, now_ms)?
            } else {
                Vec::new()
            };
            let spans = if session.ended_at_wall.is_none() {
                open_session_spans(conn, live, &session, now_ms)?
            } else {
                Vec::new()
            };
            (without((start_ms, end_ms), &aside), spans)
        } else {
            (Vec::new(), Vec::new())
        };
        placed.push(Placed {
            game_id: links
                .get(&session.game_id)
                .cloned()
                .unwrap_or_else(|| session.game_id.clone()),
            session,
            start_ms,
            end_ms,
            running_here,
            counts,
            parts,
            spans,
        });
    }
    // Closed sessions read the stretches they keep.
    let closed: Vec<&str> = placed
        .iter()
        .filter(|placed| placed.counts && placed.session.ended_at_wall.is_some())
        .map(|placed| placed.session.id.as_str())
        .collect();
    let mut stored = stored_spans(conn, &closed)?;
    for each in &mut placed {
        if let Some(spans) = stored.remove(&each.session.id) {
            each.spans = spans;
        }
    }
    Ok(placed)
}

/// The stretches kept for the closed sessions `ids`, by session.
fn stored_spans(conn: &Connection, ids: &[&str]) -> Result<HashMap<String, Vec<Span>>> {
    let mut stored: HashMap<String, Vec<Span>> = HashMap::new();
    if ids.is_empty() {
        return Ok(stored);
    }
    let mut stmt = conn
        .prepare_cached(
            "SELECT session_id, span_start, span_end, runtime_ms, active_ms, idle_ms
             FROM play_spans WHERE session_id IN (SELECT value FROM json_each(?1))
             ORDER BY session_id, span_start",
        )
        .map_err(map_db)?;
    let rows = stmt
        .query_map([serde_json::to_string(ids).unwrap_or_default()], |row| {
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
        let (id, span) = row.map_err(map_db)?;
        stored.entry(id).or_default().push(span);
    }
    Ok(stored)
}

/// A session's counted time per local day, from where its stretches fell.
fn runtime_by_day<Tz: TimeZone>(tz: &Tz, spans: &[Span]) -> BTreeMap<NaiveDate, i64> {
    let mut by_day: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    for span in spans.iter().filter(|span| span.runtime_ms > 0) {
        let Some(first) = local_day(tz, span.start_ms) else {
            continue;
        };
        if span.length_ms() == 0 {
            *by_day.entry(first).or_default() += span.runtime_ms;
            continue;
        }
        let mut days = Vec::new();
        let mut overlaps = Vec::new();
        let mut day = first;
        loop {
            let (from, to) = (
                day_start_ms(tz, day),
                day_start_ms(tz, day + TimeDelta::days(1)),
            );
            let overlap = span.end_ms.min(to) - span.start_ms.max(from);
            if overlap > 0 {
                days.push(day);
                overlaps.push(overlap);
            }
            if to >= span.end_ms {
                break;
            }
            day += TimeDelta::days(1);
        }
        for (day, part) in days.into_iter().zip(apportion(&overlaps, span.runtime_ms)) {
            *by_day.entry(day).or_default() += part;
        }
    }
    by_day
}

/// A session cut at local midnights, first day first. The day it started is
/// always there, a later day only where the strip draws it or where it
/// counted time.
fn shares_of<Tz: TimeZone>(tz: &Tz, placed: &Placed) -> Vec<DayShare> {
    let Some(mut day) = local_day(tz, placed.start_ms) else {
        return Vec::new();
    };
    let by_day = runtime_by_day(tz, &placed.spans);
    let mut shares: Vec<DayShare> = Vec::new();
    loop {
        let (from, to) = (
            day_start_ms(tz, day),
            day_start_ms(tz, day + TimeDelta::days(1)),
        );
        let runtime_ms = by_day.get(&day).copied().unwrap_or(0);
        let drawn = placed
            .parts
            .iter()
            .any(|&(start, end)| touches(start, end, from, to));
        if drawn || runtime_ms > 0 || shares.is_empty() {
            let start_ms = placed.start_ms.max(from);
            shares.push(DayShare {
                day: day.format("%Y-%m-%d").to_string(),
                start_ms,
                end_ms: start_ms.max(placed.end_ms.min(to)),
                runtime_ms,
                from_before: !shares.is_empty(),
                into_next: false,
            });
        }
        if to >= placed.end_ms {
            break;
        }
        day += TimeDelta::days(1);
    }
    let last = shares.len().saturating_sub(1);
    for share in &mut shares[..last] {
        share.into_next = true;
    }
    shares
}

/// The parts of every session that counts on the day from `from` to `to`,
/// in the order they began.
fn parts_on_day(placed: &[Placed], from: i64, to: i64) -> Vec<StripPart> {
    let mut parts: Vec<(i64, StripPart)> = Vec::new();
    for each in placed.iter().filter(|each| each.counts) {
        for &(start, end) in &each.parts {
            if !touches(start, end, from, to) {
                continue;
            }
            let (start_ms, end_ms) = if end <= start {
                (start, end)
            } else {
                (start.max(from), end.min(to))
            };
            parts.push((
                each.start_ms,
                StripPart {
                    session_id: each.session.id.clone(),
                    game_id: each.game_id.clone(),
                    start_ms,
                    end_ms,
                    open_start: start_ms == from && each.start_ms < from,
                    open_end: end_ms == to && (each.running_here || each.end_ms > to),
                },
            ));
        }
    }
    // Parts cut at midnight keep the order their sessions began in, so games
    // side by side past midnight keep their order and stripes the next day.
    parts.sort_by(|(began, a), (other_began, b)| {
        (a.start_ms, began, &a.session_id).cmp(&(b.start_ms, other_began, &b.session_id))
    });
    parts.into_iter().map(|(_, part)| part).collect()
}

/// Each game's time over `totals`, the most first.
pub(crate) fn game_times(totals: &DayTotals) -> Vec<GameTime> {
    let mut by_game: BTreeMap<&str, [i64; 3]> = BTreeMap::new();
    for total in &totals.games {
        let sum = by_game.entry(&total.game_id).or_default();
        sum[0] += total.runtime_ms;
        sum[1] += total.active_ms;
        sum[2] += total.idle_ms;
    }
    let mut times: Vec<GameTime> = by_game
        .into_iter()
        .filter(|(_, [runtime_ms, _, _])| *runtime_ms > 0)
        .map(|(game_id, [runtime_ms, active_ms, idle_ms])| GameTime {
            game_id: game_id.to_owned(),
            runtime_ms,
            active_ms,
            idle_ms,
        })
        .collect();
    times.sort_by_key(|time| std::cmp::Reverse(time.runtime_ms));
    times
}

/// The week of the journal from the local day `from`, a Monday. Running
/// sessions count up to the tracker's latest tick, at `now_ms`.
pub fn journal_week<Tz: TimeZone>(
    db: &Database,
    live: &LiveSessions,
    tz: &Tz,
    from: NaiveDate,
    this_device: &str,
    now_ms: i64,
) -> Result<JournalWeek> {
    let dates: Vec<NaiveDate> = (0..=DAYS_PER_WEEK)
        .map(|offset| from + TimeDelta::days(offset))
        .collect();
    let bounds: Vec<i64> = dates.iter().map(|date| day_start_ms(tz, *date)).collect();
    let (week_start, week_end) = (bounds[0], bounds[bounds.len() - 1]);
    let links = games::links(db)?;
    let placed = db.with_transaction(|conn| {
        placed_between(
            conn,
            live,
            &links,
            this_device,
            (week_start, week_end),
            now_ms,
        )
    })?;
    let totals = day_totals(db, live, tz, dates[0], dates[dates.len() - 1], now_ms)?;
    let days_played = totals.days_played();

    let mut days = Vec::new();
    for (index, date) in dates[..dates.len() - 1].iter().enumerate() {
        let (from_ms, to_ms) = (bounds[index], bounds[index + 1]);
        let key = date.format("%Y-%m-%d").to_string();
        let session_ids: Vec<String> = placed
            .iter()
            .filter(|each| each.start_ms >= from_ms && each.start_ms < to_ms)
            .map(|each| each.session.id.clone())
            .collect();
        let parts = parts_on_day(&placed, from_ms, to_ms);
        if parts.is_empty() && session_ids.is_empty() {
            continue;
        }
        let runs = play_runs(&parts);
        days.push(JournalDay {
            played_ms: totals.played.get(&key).copied().unwrap_or(0),
            day: key,
            session_ids,
            side_by_side: side_by_side(&runs),
            parts,
            runs,
        });
    }

    let mut games: Vec<String> = Vec::new();
    for part in days.iter().flat_map(|day| &day.parts) {
        if !games.contains(&part.game_id) {
            games.push(part.game_id.clone());
        }
    }
    let starts_in_week = |each: &&Placed| each.start_ms >= week_start && each.start_ms < week_end;
    let sessions_count = placed
        .iter()
        .filter(starts_in_week)
        .filter(|each| each.counts)
        .count();
    let carried_over = placed
        .iter()
        .filter(|each| each.counts && each.start_ms < week_start)
        .count();
    days.reverse();
    let mut longest_day: Option<&JournalDay> = None;
    for day in &days {
        if day.played_ms > longest_day.map_or(0, |longest| longest.played_ms) {
            longest_day = Some(day);
        }
    }
    let shares = placed
        .iter()
        .filter(|each| each.counts)
        .map(|each| (each.session.id.clone(), shares_of(tz, each)))
        .collect();
    Ok(JournalWeek {
        longest_day: longest_day.map(|day| day.day.clone()),
        days_played: days_played.len(),
        played_ms: totals.played_ms(),
        game_times: game_times(&totals),
        days,
        shares,
        games,
        sessions_count,
        carried_over,
    })
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;
    use chrono_tz::{Asia::Kathmandu, Europe::Berlin};

    use super::*;
    use crate::db::repo::sessions;
    use crate::playtime::summaries::year_play;
    use crate::playtime::testing::{MINUTE, Mark, database, game, played, recorded, utc};
    use crate::playtime::totals::{Bucket, play_totals};
    use crate::tracking::live::LiveCounters;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn week_of<Tz: TimeZone>(db: &Database, tz: &Tz, monday: &str, now: &str) -> JournalWeek {
        journal_week(
            db,
            &LiveSessions::default(),
            tz,
            date(monday),
            "device",
            utc(now).timestamp_millis(),
        )
        .unwrap()
    }

    fn minutes(shares: &[DayShare]) -> Vec<i64> {
        shares
            .iter()
            .map(|share| share.runtime_ms / MINUTE)
            .collect()
    }

    #[test]
    fn counts_play_past_midnight_on_the_day_it_happened() {
        let db = database();
        let one = game(&db, "One");
        // Monday 23:00 to Tuesday 01:00 in Berlin.
        let late = played(&db, &one, "2026-10-05T21:00:00Z", 120);
        let week = week_of(&db, &Berlin, "2026-10-05", "2026-10-07T10:00:00Z");

        let days: Vec<(&str, i64)> = week
            .days
            .iter()
            .map(|day| (day.day.as_str(), day.played_ms / MINUTE))
            .collect();
        assert_eq!(days, [("2026-10-06", 60), ("2026-10-05", 60)]);
        assert_eq!(week.days[1].session_ids, std::slice::from_ref(&late));
        assert!(week.days[0].session_ids.is_empty());
        assert_eq!(minutes(&week.shares[&late]), [60, 60]);
        assert!(week.days[1].parts[0].open_end);
        assert!(week.days[0].parts[0].open_start);
        assert_eq!(week.sessions_count, 1);
        assert_eq!(week.days_played, 2);
    }

    #[test]
    fn a_session_from_the_week_before_is_carried_over() {
        let db = database();
        let one = game(&db, "One");
        let sunday = played(&db, &one, "2026-10-04T21:00:00Z", 120);
        let week = week_of(&db, &Berlin, "2026-10-05", "2026-10-07T10:00:00Z");
        assert_eq!(week.carried_over, 1);
        assert_eq!(week.sessions_count, 0);
        assert_eq!(week.days.len(), 1);
        assert!(week.days[0].session_ids.is_empty());
        assert_eq!(week.days[0].parts[0].session_id, sunday);
        // The week before shows it where it began.
        let before = week_of(&db, &Berlin, "2026-09-28", "2026-10-07T10:00:00Z");
        assert_eq!(before.days[0].session_ids, [sunday]);
    }

    #[test]
    fn names_the_games_in_the_order_they_first_played() {
        let db = database();
        let (a, b) = (game(&db, "A"), game(&db, "B"));
        played(&db, &b, "2026-10-06T18:00:00Z", 60);
        played(&db, &a, "2026-10-05T18:00:00Z", 60);
        played(&db, &a, "2026-10-06T20:00:00Z", 60);
        let week = week_of(
            &db,
            &FixedOffset::east_opt(0).unwrap(),
            "2026-10-05",
            "2026-10-07T10:00:00Z",
        );
        assert_eq!(week.games, [a.clone(), b]);
        assert_eq!(week.game_times[0].game_id, a);
        assert_eq!(week.longest_day.as_deref(), Some("2026-10-06"));
    }

    #[test]
    fn a_split_at_midnight_follows_where_the_time_happened() {
        let db = database();
        let one = game(&db, "One");
        // Monday 22:00 in Berlin, asleep from 22:30 to 00:30, played on to 01:00.
        // Spread evenly, two hours of the hour would land before midnight.
        let night = recorded(
            &db,
            &one,
            "device",
            "2026-10-05T20:00:00Z",
            &[
                Mark::Check(30, 30, 30),
                Mark::Gap(150),
                Mark::Check(180, 60, 60),
            ],
        );
        let week = week_of(&db, &Berlin, "2026-10-05", "2026-10-07T10:00:00Z");
        assert_eq!(minutes(&week.shares[&night]), [30, 30]);
        let played: Vec<i64> = week.days.iter().map(|day| day.played_ms / MINUTE).collect();
        assert_eq!(played, [30, 30]);
    }

    #[test]
    fn a_launcher_is_drawn_only_where_it_ran_alone() {
        let db = database();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        games::set_steps_aside(&db, &client, true).unwrap();
        let launcher = played(&db, &client, "2026-10-05T18:00:00Z", 180);
        played(&db, &match_game, "2026-10-05T19:00:00Z", 60);
        let week = week_of(
            &db,
            &FixedOffset::east_opt(0).unwrap(),
            "2026-10-05",
            "2026-10-07T10:00:00Z",
        );

        let hours = |ms: i64| (ms - utc("2026-10-05T00:00:00Z").timestamp_millis()) / (60 * MINUTE);
        let drawn: Vec<(i64, i64)> = week.days[0]
            .parts
            .iter()
            .filter(|part| part.session_id == launcher)
            .map(|part| (hours(part.start_ms), hours(part.end_ms)))
            .collect();
        assert_eq!(drawn, [(18, 19), (20, 21)]);
        assert_eq!(minutes(&week.shares[&launcher]), [120]);
        assert_eq!(week.days[0].played_ms, 180 * MINUTE);
        assert!(week.days[0].side_by_side.is_empty());
    }

    #[test]
    fn a_launcher_steps_aside_only_for_games_on_the_same_pc() {
        let db = database();
        let (client, match_game) = (game(&db, "Client"), game(&db, "Match"));
        games::set_steps_aside(&db, &client, true).unwrap();
        let launcher = played(&db, &client, "2026-10-05T18:00:00Z", 180);
        recorded(
            &db,
            &match_game,
            "laptop",
            "2026-10-05T19:00:00Z",
            &[Mark::Check(60, 60, 60)],
        );
        let week = week_of(
            &db,
            &FixedOffset::east_opt(0).unwrap(),
            "2026-10-05",
            "2026-10-07T10:00:00Z",
        );
        let parts = week.days[0]
            .parts
            .iter()
            .filter(|part| part.session_id == launcher)
            .count();
        assert_eq!(parts, 1);
        assert_eq!(minutes(&week.shares[&launcher]), [180]);
    }

    #[test]
    fn splits_at_local_midnight_on_the_night_the_clocks_go_back() {
        let db = database();
        let one = game(&db, "One");
        // Saturday 24 October 2026, 22:00 in Berlin, for five hours of a 25 hour night.
        let late = played(&db, &one, "2026-10-24T20:00:00Z", 300);
        let week = week_of(&db, &Berlin, "2026-10-19", "2026-10-26T10:00:00Z");
        assert_eq!(minutes(&week.shares[&late]), [120, 180]);
        assert_eq!(week.shares[&late][1].day, "2026-10-25");
    }

    #[test]
    fn a_quarter_hour_offset_still_splits_at_midnight() {
        let db = database();
        let one = game(&db, "One");
        // Kathmandu is UTC+5:45. 23:45 to 00:45 local.
        let late = played(&db, &one, "2026-03-04T18:00:00Z", 60);
        let week = week_of(&db, &Kathmandu, "2026-03-02", "2026-03-06T10:00:00Z");
        assert_eq!(minutes(&week.shares[&late]), [15, 45]);
    }

    #[test]
    fn a_running_session_runs_up_to_now() {
        let db = database();
        let one = game(&db, "One");
        // Saturday 23:00 in Berlin, an hour and a half at the latest tick.
        let start = utc("2026-10-17T21:00:00Z");
        let running = sessions::create_session_at(&db, &one, "device", start).unwrap();
        let live = LiveSessions::default();
        live.update(
            &running.id,
            LiveCounters {
                runtime_ms: 90 * MINUTE,
                active_ms: 90 * MINUTE,
                idle_ms: 0,
            },
        );
        let now = (start + TimeDelta::minutes(90)).timestamp_millis();
        let week = journal_week(&db, &live, &Berlin, date("2026-10-12"), "device", now).unwrap();
        assert_eq!(minutes(&week.shares[&running.id]), [60, 30]);
        assert_eq!(week.days[0].parts[0].end_ms, now);
        assert!(!week.days[0].parts[0].open_end);
    }

    #[test]
    fn the_journal_days_match_the_stats_days() {
        let db = database();
        let (a, b, client, match_game) = (
            game(&db, "A"),
            game(&db, "B"),
            game(&db, "Client"),
            game(&db, "Match"),
        );
        games::set_steps_aside(&db, &client, true).unwrap();
        // Monday in Berlin: A from 21:00 to 23:30, B beside it from 22:00 to
        // 01:00. Tuesday: a client from 18:00 to 21:00 with a match inside.
        // Wednesday: B asleep for an hour in the middle of two.
        played(&db, &a, "2026-10-05T19:00:00Z", 150);
        played(&db, &b, "2026-10-05T20:00:00Z", 180);
        played(&db, &client, "2026-10-06T16:00:00Z", 180);
        played(&db, &match_game, "2026-10-06T17:00:00Z", 60);
        recorded(
            &db,
            &b,
            "device",
            "2026-10-07T16:00:00Z",
            &[
                Mark::Check(30, 30, 20),
                Mark::Gap(90),
                Mark::Check(120, 60, 50),
            ],
        );
        let now = "2026-10-08T10:00:00Z";
        let now_ms = utc(now).timestamp_millis();
        let live = LiveSessions::default();
        let week = week_of(&db, &Berlin, "2026-10-05", now);
        let stats = day_totals(
            &db,
            &live,
            &Berlin,
            date("2026-01-01"),
            date("2027-01-01"),
            now_ms,
        )
        .unwrap();

        for day in &week.days {
            assert_eq!(day.played_ms, stats.played[&day.day], "{}", day.day);
        }
        let played: Vec<i64> = week.days.iter().map(|day| day.played_ms / MINUTE).collect();
        assert_eq!(played, [60, 240, 180]);
        let year = year_play(&db, &live, &Berlin, 2026, date("2026-10-08"), now_ms).unwrap();
        assert_eq!(year.played_ms, stats.played_ms());
        assert_eq!(year.played_ms, week.played_ms);
        // Each game's week is its time in the stats' days.
        let days = play_totals(
            &db,
            &live,
            &Berlin,
            date("2026-10-05"),
            date("2026-10-12"),
            Bucket::Day,
            None,
            now_ms,
        )
        .unwrap();
        for time in &week.game_times {
            let summed: i64 = days
                .iter()
                .filter(|total| total.game_id == time.game_id)
                .map(|total| total.runtime_ms)
                .sum();
            assert_eq!(time.runtime_ms, summed);
        }
    }
}
