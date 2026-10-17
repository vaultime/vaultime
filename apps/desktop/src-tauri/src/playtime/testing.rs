// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sessions recorded the way the tracker records them, for the tests of
//! what the pages show.

use chrono::{DateTime, TimeDelta, Utc};
use rusqlite::params;

use crate::db::connection::Database;
use crate::db::models::CreateGame;
use crate::db::repo::{devices, games, map_db, sessions};
use crate::integrity;
use crate::playtime::slices::rebuild_session_and_around;

pub const MINUTE: i64 = 60_000;

/// What a session recorded at a moment, in minutes from its start.
#[derive(Debug, Clone, Copy)]
pub enum Mark {
    /// A checkpoint with the runtime and active minutes so far, the rest
    /// idle.
    Check(i64, i64, i64),
    /// The PC slept or the tracker stalled until here.
    Gap(i64),
}

pub fn utc(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .unwrap()
        .with_timezone(&Utc)
}

/// An empty library on the PC "device", with "laptop" known too.
pub fn database() -> Database {
    let db = Database::open_in_memory().unwrap();
    devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
    devices::ensure_device(&db, "laptop", "test", "0.1.0").unwrap();
    db
}

pub fn game(db: &Database, title: &str) -> String {
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

/// A closed session of `game_id` on `device` from `start`, with `marks`,
/// the last check its end. Sliced along with the sessions that step aside
/// around it. Returns its id.
pub fn recorded(db: &Database, game_id: &str, device: &str, start: &str, marks: &[Mark]) -> String {
    let start = utc(start);
    let session = sessions::create_session_at(db, game_id, device, start).unwrap();
    let at = |minute: i64| integrity::format_timestamp(start + TimeDelta::minutes(minute));
    db.with_conn(|conn| {
        let (mut last, mut runtime, mut active) = (0, 0, 0);
        for (index, mark) in marks.iter().enumerate() {
            match *mark {
                Mark::Gap(minute) => integrity::append_session_event(
                    conn,
                    &session.id,
                    "tracking_gap",
                    &at(minute),
                    Some(runtime * MINUTE),
                    &serde_json::json!({ "wall_gap_ms": (minute - last) * MINUTE }).to_string(),
                )?,
                Mark::Check(minute, run, act) => {
                    (last, runtime, active) = (minute, run, act);
                    let closing = index + 1 == marks.len();
                    let mut payload = serde_json::json!({
                        "runtime_ms": run * MINUTE,
                        "active_ms": act * MINUTE,
                        "idle_ms": (run - act) * MINUTE,
                        "integrity_status": "local",
                    });
                    if closing {
                        payload["closed_cleanly"] = true.into();
                    }
                    integrity::append_session_event(
                        conn,
                        &session.id,
                        if closing { "ended" } else { "heartbeat" },
                        &at(minute),
                        Some(run * MINUTE),
                        &payload.to_string(),
                    )?;
                }
            }
        }
        conn.execute(
            "UPDATE sessions SET ended_at_wall = ?1, runtime_ms = ?2, elapsed_monotonic_ms = ?2,
                 active_ms = ?3, idle_ms = ?4, closed_cleanly = 1
             WHERE id = ?5",
            params![
                at(last),
                runtime * MINUTE,
                active * MINUTE,
                (runtime - active) * MINUTE,
                session.id
            ],
        )
        .map_err(map_db)?;
        rebuild_session_and_around(conn, &session.id)
    })
    .unwrap();
    session.id
}

/// A session of `game_id` on this PC from `start`, all active for `minutes`.
pub fn played(db: &Database, game_id: &str, start: &str, minutes: i64) -> String {
    recorded(
        db,
        game_id,
        "device",
        start,
        &[Mark::Check(minutes, minutes, minutes)],
    )
}
