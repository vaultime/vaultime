// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the library and a game's page show for each game: its time, its
//! sessions and their trust labels, every install that counts as it included.

use std::collections::{BTreeMap, HashMap};
use std::hash::BuildHasher;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::db::models::Session;
use crate::integrity::{STATUS_RECOVERED, STATUS_SUSPICIOUS, format_timestamp};

/// One game's totals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GameTotals {
    /// The game shown, which installs linked to it count as.
    pub game_id: String,
    /// Time that counts, a stretch two installs shared counted once.
    pub runtime_ms: i64,
    pub active_ms: i64,
    pub idle_ms: i64,
    /// Sessions with play. One whose time was all taken out is none.
    pub sessions_count: i64,
    /// The runtime of the longest session.
    pub longest_session_ms: i64,
    pub first_played_at: Option<String>,
    /// When the newest play ended, now while the game runs here.
    pub last_played_at: Option<String>,
    pub suspicious_count: i64,
    pub recovered_count: i64,
    /// Suspicious sessions that started from the cutoff on.
    pub recent_suspicious_count: i64,
    /// Recovered sessions that started from the cutoff on.
    pub recent_recovered_count: i64,
}

/// Whether a session is play: a running one, or a finished one with time
/// left after corrections.
pub fn counts_as_play(session: &Session) -> bool {
    session.ended_at_wall.is_none() || session.runtime_ms > 0
}

/// Each game's totals from `sessions` as the pages get them, each shown as
/// the game it counts as. Trust labels of sessions that started at or after
/// `recent_since` count as recent too. A session still running here was last
/// played `now`, and one whose clock went back ended no earlier than it
/// started.
pub fn game_totals(
    sessions: &[Session],
    recent_since: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Vec<GameTotals> {
    let now = format_timestamp(now);
    let mut by_game: BTreeMap<&str, GameTotals> = BTreeMap::new();
    for session in sessions.iter().filter(|session| counts_as_play(session)) {
        let totals = by_game
            .entry(session.game_id.as_str())
            .or_insert_with(|| GameTotals {
                game_id: session.game_id.clone(),
                ..GameTotals::default()
            });
        totals.runtime_ms += session.counted_runtime_ms;
        totals.active_ms += session.counted_active_ms;
        totals.idle_ms += session.counted_idle_ms;
        totals.sessions_count += 1;
        totals.longest_session_ms = totals.longest_session_ms.max(session.runtime_ms);
        let start = &session.started_at_wall;
        if totals
            .first_played_at
            .as_ref()
            .is_none_or(|first| start < first)
        {
            totals.first_played_at = Some(start.clone());
        }
        let end = session.ended_at_wall.as_ref().unwrap_or(&now).max(start);
        if totals.last_played_at.as_ref().is_none_or(|last| end > last) {
            totals.last_played_at = Some(end.clone());
        }
        let recent = DateTime::parse_from_rfc3339(start)
            .is_ok_and(|started| started.with_timezone(&Utc) >= recent_since);
        match session.integrity_status.as_str() {
            STATUS_SUSPICIOUS => {
                totals.suspicious_count += 1;
                totals.recent_suspicious_count += i64::from(recent);
            }
            STATUS_RECOVERED => {
                totals.recovered_count += 1;
                totals.recent_recovered_count += i64::from(recent);
            }
            _ => {}
        }
    }
    by_game.into_values().collect()
}

/// The time one launcher's copies of a game recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LauncherTime {
    /// The game shown, which installs linked to it count as.
    pub game_id: String,
    /// Where the copies came from as their game names it, like `steam` or
    /// `folder_scan`, none for a game added by hand.
    pub launcher: Option<String>,
    /// Time that counts, a stretch two copies shared counted once.
    pub runtime_ms: i64,
}

/// The time each launcher's copies of each game recorded, so a launcher's
/// page shows what was played through it. A session counts for the launcher
/// of the install that recorded it, which may be another install or a game
/// of another PC linked to the game shown. `launcher_of` maps every game to
/// its launcher.
pub fn launcher_times<S: BuildHasher>(
    sessions: &[Session],
    launcher_of: &HashMap<String, Option<String>, S>,
) -> Vec<LauncherTime> {
    let mut times: BTreeMap<(&str, Option<&str>), i64> = BTreeMap::new();
    for session in sessions.iter().filter(|session| counts_as_play(session)) {
        let recorded = session
            .recorded_game_id
            .as_ref()
            .unwrap_or(&session.game_id);
        let launcher = launcher_of.get(recorded).and_then(Option::as_deref);
        *times
            .entry((session.game_id.as_str(), launcher))
            .or_default() += session.counted_runtime_ms;
    }
    times
        .into_iter()
        .map(|((game_id, launcher), runtime_ms)| LauncherTime {
            game_id: game_id.into(),
            launcher: launcher.map(Into::into),
            runtime_ms,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(time: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(time)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn session(game: &str, start: &str, runtime_min: i64, status: &str) -> Session {
        let runtime_ms = runtime_min * 60_000;
        let end = at(start) + chrono::TimeDelta::milliseconds(runtime_ms);
        Session {
            id: format!("{game}-{start}"),
            game_id: game.into(),
            started_at_wall: start.into(),
            ended_at_wall: Some(format_timestamp(end)),
            runtime_ms,
            counted_runtime_ms: runtime_ms,
            counted_active_ms: runtime_ms,
            integrity_status: status.into(),
            ..Session::default()
        }
    }

    fn cutoff() -> DateTime<Utc> {
        at("2026-10-01T00:00:00.000Z")
    }

    fn now() -> DateTime<Utc> {
        at("2026-10-07T12:00:00.000Z")
    }

    #[test]
    fn a_game_sums_its_sessions_and_counts_their_labels() {
        let totals = game_totals(
            &[
                session("hades", "2026-09-20T19:00:00.000Z", 120, "local"),
                session("hades", "2026-10-05T19:00:00.000Z", 30, "suspicious"),
                session("hades", "2026-09-01T19:00:00.000Z", 45, "recovered"),
                session("celeste", "2026-10-06T08:00:00.000Z", 60, "local"),
            ],
            cutoff(),
            now(),
        );
        assert_eq!(totals.len(), 2);
        let hades = &totals[1];
        assert_eq!(hades.game_id, "hades");
        assert_eq!(hades.runtime_ms, 195 * 60_000);
        assert_eq!(hades.sessions_count, 3);
        assert_eq!(hades.longest_session_ms, 120 * 60_000);
        assert_eq!(
            hades.first_played_at.as_deref(),
            Some("2026-09-01T19:00:00.000Z")
        );
        assert_eq!(
            hades.last_played_at.as_deref(),
            Some("2026-10-05T19:30:00.000Z")
        );
        assert_eq!(
            (hades.suspicious_count, hades.recent_suspicious_count),
            (1, 1)
        );
        assert_eq!(
            (hades.recovered_count, hades.recent_recovered_count),
            (1, 0)
        );
    }

    #[test]
    fn copies_count_once_and_a_session_without_time_is_no_play() {
        let mut steam = session("cyberpunk", "2026-10-02T18:00:00.000Z", 60, "local");
        let mut gog = session("cyberpunk", "2026-10-02T18:30:00.000Z", 60, "local");
        // The half hour both ran counts once.
        steam.counted_runtime_ms = 30 * 60_000;
        gog.counted_runtime_ms = 60 * 60_000;
        let mut discarded = session("cyberpunk", "2026-10-03T18:00:00.000Z", 0, "edited");
        discarded.runtime_ms = 0;
        let totals = game_totals(&[steam, gog, discarded], cutoff(), now());
        assert_eq!(totals[0].runtime_ms, 90 * 60_000);
        assert_eq!(totals[0].sessions_count, 2);
        assert_eq!(
            totals[0].last_played_at.as_deref(),
            Some("2026-10-02T19:30:00.000Z")
        );
    }

    #[test]
    fn a_game_was_last_played_when_its_play_ended() {
        let evening = session("hades", "2026-10-02T18:00:00.000Z", 180, "local");
        let break_in_it = session("hades", "2026-10-02T19:00:00.000Z", 30, "local");
        let totals = game_totals(&[evening.clone(), break_in_it], cutoff(), now());
        assert_eq!(
            totals[0].last_played_at.as_deref(),
            Some("2026-10-02T21:00:00.000Z")
        );

        let mut running = session("hades", "2026-10-07T11:00:00.000Z", 0, "local");
        running.ended_at_wall = None;
        let totals = game_totals(&[evening, running], cutoff(), now());
        assert_eq!(
            totals[0].last_played_at.as_deref(),
            Some("2026-10-07T12:00:00.000Z")
        );
    }

    #[test]
    fn each_launcher_gets_the_time_its_copies_recorded() {
        let launcher_of = HashMap::from([
            ("cyberpunk".to_string(), Some("steam".to_string())),
            ("cyberpunk-gog".to_string(), Some("gog".to_string())),
            ("celeste".to_string(), Some("folder_scan".to_string())),
            ("tetris".to_string(), None),
        ]);
        let steam = session("cyberpunk", "2026-10-02T18:00:00.000Z", 60, "local");
        let mut gog = session("cyberpunk", "2026-10-03T18:00:00.000Z", 45, "local");
        gog.recorded_game_id = Some("cyberpunk-gog".into());
        let mut shared = session("cyberpunk", "2026-10-04T18:00:00.000Z", 30, "local");
        shared.recorded_game_id = Some("cyberpunk-gog".into());
        // Its half hour ran beside the Steam copy and counts there.
        shared.counted_runtime_ms = 0;
        let mut discarded = session("cyberpunk", "2026-10-05T18:00:00.000Z", 0, "edited");
        discarded.runtime_ms = 0;
        let found = session("celeste", "2026-10-06T08:00:00.000Z", 20, "local");
        let by_hand = session("tetris", "2026-10-06T09:00:00.000Z", 10, "local");

        let times = launcher_times(
            &[steam, gog, shared, discarded, found, by_hand],
            &launcher_of,
        );
        let minutes = |game: &str, launcher: Option<&str>| {
            times
                .iter()
                .find(|time| time.game_id == game && time.launcher.as_deref() == launcher)
                .map(|time| time.runtime_ms / 60_000)
        };
        assert_eq!(minutes("cyberpunk", Some("steam")), Some(60));
        assert_eq!(minutes("cyberpunk", Some("gog")), Some(45));
        assert_eq!(minutes("celeste", Some("folder_scan")), Some(20));
        assert_eq!(minutes("tetris", None), Some(10));
        assert_eq!(times.len(), 4);
    }

    #[test]
    fn a_session_of_a_game_no_longer_known_counts_without_launcher() {
        let gone = session("removed", "2026-10-02T18:00:00.000Z", 30, "local");
        let times = launcher_times(&[gone], &HashMap::new());
        assert_eq!(times[0].launcher, None);
        assert_eq!(times[0].runtime_ms, 30 * 60_000);
    }

    #[test]
    fn last_played_never_comes_before_the_start() {
        let mut clock_back = session("hades", "2026-10-03T18:00:00.000Z", 60, "local");
        clock_back.ended_at_wall = Some("2026-10-03T17:00:00.000Z".into());
        let totals = game_totals(&[clock_back], cutoff(), now());
        assert_eq!(
            totals[0].last_played_at.as_deref(),
            Some("2026-10-03T18:00:00.000Z")
        );
    }
}
