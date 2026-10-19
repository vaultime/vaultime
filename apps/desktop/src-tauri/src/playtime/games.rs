// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the library and a game's page show for each game: its time, its
//! sessions and their trust labels, every install that counts as it included.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::db::models::Session;
use crate::integrity::{STATUS_RECOVERED, STATUS_SUSPICIOUS};

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
/// `recent_since` count as recent too.
pub fn game_totals(sessions: &[Session], recent_since: DateTime<Utc>) -> Vec<GameTotals> {
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
        if totals
            .last_played_at
            .as_ref()
            .is_none_or(|last| start > last)
        {
            totals.last_played_at = Some(start.clone());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn session(game: &str, start: &str, runtime_min: i64, status: &str) -> Session {
        let runtime_ms = runtime_min * 60_000;
        Session {
            id: format!("{game}-{start}"),
            game_id: game.into(),
            started_at_wall: start.into(),
            ended_at_wall: Some(start.into()),
            runtime_ms,
            counted_runtime_ms: runtime_ms,
            counted_active_ms: runtime_ms,
            integrity_status: status.into(),
            ..Session::default()
        }
    }

    fn cutoff() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-01T00:00:00.000Z")
            .unwrap()
            .with_timezone(&Utc)
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
            Some("2026-10-05T19:00:00.000Z")
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
        let totals = game_totals(&[steam, gog, discarded], cutoff());
        assert_eq!(totals[0].runtime_ms, 90 * 60_000);
        assert_eq!(totals[0].sessions_count, 2);
        assert_eq!(
            totals[0].last_played_at.as_deref(),
            Some("2026-10-02T18:30:00.000Z")
        );
    }
}
