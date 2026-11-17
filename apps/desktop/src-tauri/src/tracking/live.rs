// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Counters of the running sessions as of the latest tick.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Utc};

use crate::db::models::Session;

/// Runtime, active and idle time of a running session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LiveCounters {
    pub runtime_ms: i64,
    pub active_ms: i64,
    pub idle_ms: i64,
}

/// The database holds a running session as of its last checkpoint, up to
/// `CHECKPOINT_INTERVAL` behind. Pages and the tray read the tracker's own
/// counters from here instead.
#[derive(Debug, Default)]
pub struct LiveSessions {
    counted: Mutex<HashMap<String, Counted>>,
}

/// A session's counters and when its latest tick counted them.
#[derive(Debug, Clone, Copy)]
struct Counted {
    counters: LiveCounters,
    /// Unix milliseconds.
    at_ms: i64,
}

impl LiveSessions {
    /// Takes a session's counters as of `wall`, when its tick counted them.
    /// Each session keeps its own time, as a tick that misses a game leaves
    /// that session's counters as they were.
    pub fn update(&self, session_id: &str, counters: LiveCounters, wall: DateTime<Utc>) {
        let at_ms = wall.timestamp_millis();
        self.lock()
            .insert(session_id.to_owned(), Counted { counters, at_ms });
    }

    /// When the tracker last counted a session, so its clock can run on from
    /// there.
    pub fn counted_at_ms(&self, session_id: &str) -> Option<i64> {
        self.lock().get(session_id).map(|counted| counted.at_ms)
    }

    /// A session's counters with when its latest tick counted them, in Unix
    /// milliseconds, both of the same tick.
    pub fn counted(&self, session_id: &str) -> Option<(i64, LiveCounters)> {
        self.lock()
            .get(session_id)
            .map(|counted| (counted.at_ms, counted.counters))
    }

    /// A copy as of now, for reads that have to agree with each other while
    /// the tracker counts on.
    #[must_use]
    pub fn snapshot(&self) -> Self {
        Self {
            counted: Mutex::new(self.lock().clone()),
        }
    }

    pub fn remove(&self, session_id: &str) {
        self.lock().remove(session_id);
    }

    pub fn get(&self, session_id: &str) -> Option<LiveCounters> {
        self.lock().get(session_id).map(|counted| counted.counters)
    }

    /// Brings an open session up to the latest tick. A closed session, or one
    /// the tracker is not running, stays as stored.
    pub fn apply(&self, session: &mut Session) {
        if session.ended_at_wall.is_some() {
            return;
        }
        if let Some(live) = self.get(&session.id) {
            session.runtime_ms = live.runtime_ms;
            session.elapsed_monotonic_ms = live.runtime_ms;
            session.active_ms = live.active_ms;
            session.idle_ms = live.idle_ms;
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Counted>> {
        self.counted.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_session(id: &str) -> Session {
        Session {
            id: id.into(),
            game_id: "game".into(),
            device_id: "device".into(),
            started_at_wall: "2026-10-02T18:00:00.000Z".into(),
            ended_at_wall: None,
            elapsed_monotonic_ms: 30_000,
            active_ms: 30_000,
            idle_ms: 0,
            runtime_ms: 30_000,
            integrity_status: "local".into(),
            closed_cleanly: false,
            set_aside_ms: 0,
            set_aside_active_ms: 0,
            set_aside_idle_ms: 0,
            copy_overlap_ms: 0,
            copy_overlap_active_ms: 0,
            copy_overlap_idle_ms: 0,
            counted_runtime_ms: 0,
            counted_active_ms: 0,
            counted_idle_ms: 0,
            recorded_game_id: None,
            still_running_on: None,
        }
    }

    #[test]
    fn brings_open_sessions_up_to_the_latest_tick() {
        let live = LiveSessions::default();
        let counters = LiveCounters {
            runtime_ms: 45_000,
            active_ms: 40_000,
            idle_ms: 5_000,
        };
        live.update("running", counters, Utc::now());

        let mut session = open_session("running");
        live.apply(&mut session);
        assert_eq!(session.runtime_ms, 45_000);
        assert_eq!(session.elapsed_monotonic_ms, 45_000);
        assert_eq!((session.active_ms, session.idle_ms), (40_000, 5_000));

        let mut closed = open_session("running");
        closed.ended_at_wall = Some("2026-10-02T18:01:00.000Z".into());
        live.apply(&mut closed);
        assert_eq!(closed.runtime_ms, 30_000);

        live.remove("running");
        let mut stored = open_session("running");
        live.apply(&mut stored);
        assert_eq!(stored.runtime_ms, 30_000);
    }

    #[test]
    fn keeps_when_each_session_was_counted() {
        let live = LiveSessions::default();
        let tick = |ms| DateTime::from_timestamp_millis(ms).unwrap();
        let counters = LiveCounters {
            runtime_ms: 5_000,
            active_ms: 5_000,
            idle_ms: 0,
        };
        live.update("running", counters, tick(1_790_000_005_000));
        // Its game was missing at the latest tick, which left it as it was.
        live.update("missing", counters, tick(1_790_000_000_000));
        assert_eq!(live.counted_at_ms("running"), Some(1_790_000_005_000));
        assert_eq!(live.counted_at_ms("missing"), Some(1_790_000_000_000));
        assert_eq!(live.counted_at_ms("ended"), None);

        let seen = live.snapshot();
        live.remove("running");
        assert_eq!(seen.get("running"), Some(counters));
        assert_eq!(seen.counted_at_ms("running"), Some(1_790_000_005_000));
    }
}
