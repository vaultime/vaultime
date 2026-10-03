// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Timings on a large made-up history, for the performance checks. Run with
//! `cargo test --release -- --ignored --nocapture large_history`.

use std::time::Instant;

use chrono::{DateTime, TimeDelta, Utc};
use rusqlite::params;
use serde_json::json;

use crate::db::connection::Database;
use crate::db::models::CreateGame;
use crate::db::repo::{devices, games, map_db, sessions};
use crate::integrity;
use crate::playtime::{slices, totals};
use crate::tracking::live::LiveSessions;

/// Writes a closed session of `minutes`, with a timing event every
/// `step_secs`, as the tracker of that time would have.
fn write_session(db: &Database, game_id: &str, start: DateTime<Utc>, minutes: i64, step_secs: i64) {
    db.with_transaction(|conn| {
        let id = uuid::Uuid::new_v4().to_string();
        let started = integrity::format_timestamp(start);
        conn.execute(
            "INSERT INTO sessions (id, game_id, device_id, started_at_wall, elapsed_monotonic_ms,
                 active_ms, idle_ms, runtime_ms, integrity_status, closed_cleanly)
             VALUES (?1, ?2, 'device', ?3, 0, 0, 0, 0, 'local', 0)",
            params![id, game_id, started],
        )
        .map_err(map_db)?;
        integrity::append_session_event(
            conn,
            &id,
            "started",
            &started,
            Some(0),
            &json!({ "game_id": game_id, "device_id": "device", "integrity_status": "local" })
                .to_string(),
        )?;
        let total_ms = minutes * 60_000;
        let mut at = 0;
        while at + step_secs * 1_000 < total_ms {
            at += step_secs * 1_000;
            let (active, idle) = (at * 4 / 5, at - at * 4 / 5);
            integrity::append_session_event(
                conn,
                &id,
                "heartbeat",
                &integrity::format_timestamp(start + TimeDelta::milliseconds(at)),
                Some(at),
                &json!({
                    "runtime_ms": at, "active_ms": active, "idle_ms": idle,
                    "wall_elapsed_ms": at, "drift_ms": 0, "integrity_status": "local",
                })
                .to_string(),
            )?;
        }
        let (active, idle) = (total_ms * 4 / 5, total_ms - total_ms * 4 / 5);
        let end = integrity::format_timestamp(start + TimeDelta::milliseconds(total_ms));
        integrity::append_session_event(
            conn,
            &id,
            "ended",
            &end,
            Some(total_ms),
            &json!({
                "runtime_ms": total_ms, "active_ms": active, "idle_ms": idle,
                "integrity_status": "local", "closed_cleanly": true,
            })
            .to_string(),
        )?;
        conn.execute(
            "UPDATE sessions SET ended_at_wall = ?1, runtime_ms = ?2, elapsed_monotonic_ms = ?2,
                 active_ms = ?3, idle_ms = ?4, closed_cleanly = 1 WHERE id = ?5",
            params![end, total_ms, active, idle, id],
        )
        .map_err(map_db)?;
        Ok(())
    })
    .unwrap();
}

#[test]
#[ignore = "builds a large history and prints timings"]
fn large_history() {
    let path = std::env::temp_dir().join(format!("vaultime-perf-{}.db", uuid::Uuid::new_v4()));
    let db = Database::open(&path).unwrap();
    devices::ensure_device(&db, "device", "test", "0.0.0").unwrap();
    let game_ids: Vec<String> = (0..20)
        .map(|index| {
            games::create_game(
                &db,
                &CreateGame {
                    title: format!("Game {index}"),
                    executable_path: Some(format!("game{index}.exe")),
                    install_folder: None,
                    launcher_source: None,
                },
            )
            .unwrap()
            .id
        })
        .collect();

    // Ten years, two sessions of 90 minutes a day. The oldest two years have
    // a heartbeat every 5 s, the rest a checkpoint every 30 s.
    let years = 10;
    let first = Utc::now() - TimeDelta::days(365 * years);
    let started = Instant::now();
    for day in 0..365 * years {
        for play in 0..2 {
            let start = first + TimeDelta::days(day) + TimeDelta::hours(18 + 2 * play);
            let step = if day < 365 * 2 { 5 } else { 30 };
            let game = &game_ids[usize::try_from((day * 7 + play * 3) % 20).unwrap()];
            write_session(&db, game, start, 90, step);
        }
    }
    let events: i64 = db
        .with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM session_events", [], |row| row.get(0))
                .map_err(map_db)
        })
        .unwrap();
    println!("built {events} events in {:?}", started.elapsed());

    let timed = |label: &str, run: &mut dyn FnMut() -> String| {
        let begin = Instant::now();
        let note = run();
        println!("{label}: {:?} {note}", begin.elapsed());
    };
    timed("rebuild all slices", &mut || {
        format!("{} sessions", slices::rebuild_all(&db).unwrap())
    });
    let mut listed = Vec::new();
    timed("list sessions, cold", &mut || {
        listed = sessions::list_all_sessions(&db).unwrap();
        format!("{} sessions", listed.len())
    });
    timed("list sessions, warm", &mut || {
        format!(
            "{} sessions",
            sessions::list_all_sessions(&db).unwrap().len()
        )
    });
    timed("session list as JSON", &mut || {
        format!("{} bytes", serde_json::to_string(&listed).unwrap().len())
    });
    let today = Utc::now().date_naive();
    timed("totals for a year by day", &mut || {
        let rows = totals::play_totals(
            &db,
            &LiveSessions::default(),
            &chrono::Local,
            today - TimeDelta::days(365),
            today,
            totals::Bucket::Day,
            None,
            Utc::now().timestamp_millis(),
        )
        .unwrap();
        format!("{} rows", rows.len())
    });
    timed("fill set aside, one stepping game", &mut || {
        games::set_steps_aside(&db, &game_ids[0], true).unwrap();
        let mut copy = listed.clone();
        totals::fill_set_aside(
            &db,
            &LiveSessions::default(),
            &mut copy,
            Utc::now().timestamp_millis(),
        )
        .unwrap();
        String::new()
    });
    let bytes = std::fs::metadata(&path).map_or(0, |meta| meta.len());
    println!("database file: {} MB", bytes / 1_000_000);
    drop(db);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
