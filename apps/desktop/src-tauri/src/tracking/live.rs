// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Counters of the running sessions as of the latest tick.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

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
    counters: Mutex<HashMap<String, LiveCounters>>,
}

impl LiveSessions {
    pub fn update(&self, session_id: &str, counters: LiveCounters) {
        self.lock().insert(session_id.to_owned(), counters);
    }

    pub fn remove(&self, session_id: &str) {
        self.lock().remove(session_id);
    }

    pub fn get(&self, session_id: &str) -> Option<LiveCounters> {
        self.lock().get(session_id).copied()
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

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, LiveCounters>> {
        self.counters.lock().unwrap_or_else(PoisonError::into_inner)
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
        live.update("running", counters);

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
}
