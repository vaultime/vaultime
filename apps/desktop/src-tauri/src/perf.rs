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

/// Ten years, two sessions of 90 minutes a day over 20 games. The oldest
/// two years have a heartbeat every 5 s, the rest a checkpoint every 30 s.
/// Returns the game ids.
fn build_history(db: &Database) -> Vec<String> {
    devices::ensure_device(db, "device", "test", "0.0.0").unwrap();
    let game_ids: Vec<String> = (0..20)
        .map(|index| {
            games::create_game(
                db,
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
    let years = 10;
    let first = Utc::now() - TimeDelta::days(365 * years);
    for day in 0..365 * years {
        for play in 0..2 {
            let start = first + TimeDelta::days(day) + TimeDelta::hours(18 + 2 * play);
            let step = if day < 365 * 2 { 5 } else { 30 };
            let game = &game_ids[usize::try_from((day * 7 + play * 3) % 20).unwrap()];
            write_session(db, game, start, 90, step);
        }
    }
    game_ids
}

/// The session row and events of `minutes` of play from `start` with a
/// checkpoint every `step_secs`, without a database.
fn session_in_memory(
    start: DateTime<Utc>,
    minutes: i64,
    step_secs: i64,
) -> (
    crate::db::models::Session,
    Vec<crate::db::models::SessionEvent>,
) {
    let total_ms = minutes * 60_000;
    let event = |sequence: i64, kind: &str, at_ms: i64, payload: serde_json::Value| {
        crate::db::models::SessionEvent {
            id: format!("event-{sequence}"),
            session_id: "session".into(),
            sequence,
            event_type: kind.into(),
            event_time_wall: integrity::format_timestamp(start + TimeDelta::milliseconds(at_ms)),
            event_time_monotonic: Some(at_ms),
            payload_json: payload.to_string(),
            hash_prev: None,
            hash_self: None,
            signature: None,
        }
    };
    let counters =
        |at: i64| json!({ "runtime_ms": at, "active_ms": at * 4 / 5, "idle_ms": at - at * 4 / 5 });
    let mut events = vec![event(0, "started", 0, json!({}))];
    let mut at = step_secs * 1_000;
    while at < total_ms {
        events.push(event(
            i64::try_from(events.len()).unwrap(),
            "heartbeat",
            at,
            counters(at),
        ));
        at += step_secs * 1_000;
    }
    events.push(event(
        i64::try_from(events.len()).unwrap(),
        "ended",
        total_ms,
        counters(total_ms),
    ));
    let session = crate::db::models::Session {
        id: "session".into(),
        game_id: "game".into(),
        device_id: "device".into(),
        started_at_wall: integrity::format_timestamp(start),
        ended_at_wall: Some(integrity::format_timestamp(
            start + TimeDelta::milliseconds(total_ms),
        )),
        elapsed_monotonic_ms: total_ms,
        active_ms: total_ms * 4 / 5,
        idle_ms: total_ms - total_ms * 4 / 5,
        runtime_ms: total_ms,
        integrity_status: "local".into(),
        closed_cleanly: true,
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
        counted_until: None,
    };
    (session, events)
}

/// Times slice building alone, without the database:
/// `cargo test --release -- --ignored --nocapture slice_building`.
#[test]
#[ignore = "prints timings"]
fn slice_building() {
    const ROUNDS: u32 = 200;
    let start = Utc::now() - TimeDelta::days(1);
    for (label, step_secs, aside) in [
        ("5 s heartbeats", 5, false),
        ("30 s checkpoints", 30, false),
        ("30 s checkpoints, set aside", 30, true),
    ] {
        let (session, events) = session_in_memory(start, 90, step_secs);
        let begin = start.timestamp_millis();
        let stretches: Vec<(i64, i64)> = if aside {
            (0..3)
                .map(|index| {
                    let from = begin + index * 30 * 60_000 + 7 * 60_000;
                    (from, from + 11 * 60_000)
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut placed = 0;
        let started = Instant::now();
        for _ in 0..ROUNDS {
            placed += slices::slices_of(&session, &events, None, &stretches).len();
        }
        println!(
            "{label}: {:?} per session of {} events, {} slices",
            started.elapsed() / ROUNDS,
            events.len(),
            placed / ROUNDS as usize
        );
    }
}

/// A session of `game_id` running for the last three hours, with a
/// checkpoint every 30 s, at the latest tick in `live`.
fn start_running(db: &Database, game_id: &str, live: &LiveSessions) {
    const MINUTES: i64 = 180;
    const STEP_MS: i64 = 30_000;
    let start = Utc::now() - TimeDelta::minutes(MINUTES);
    let session = sessions::create_session_at(db, game_id, "device", start).unwrap();
    db.with_transaction(|conn| {
        let mut at = STEP_MS;
        while at < MINUTES * 60_000 {
            integrity::append_session_event(
                conn,
                &session.id,
                "heartbeat",
                &integrity::format_timestamp(start + TimeDelta::milliseconds(at)),
                Some(at),
                &json!({
                    "runtime_ms": at, "active_ms": at, "idle_ms": 0,
                    "wall_elapsed_ms": at, "drift_ms": 0, "integrity_status": "local",
                })
                .to_string(),
            )?;
            at += STEP_MS;
        }
        Ok(())
    })
    .unwrap();
    live.update(
        &session.id,
        crate::tracking::live::LiveCounters {
            runtime_ms: MINUTES * 60_000,
            active_ms: MINUTES * 60_000,
            idle_ms: 0,
        },
    );
}

/// What each page asks the core for, timed on the large history, first
/// with nothing running and then with a game running.
fn page_timings(
    db: &Database,
    game_ids: &[String],
    timed: &dyn Fn(&str, &mut dyn FnMut() -> String),
) -> LiveSessions {
    use crate::playtime::{journal, summaries};
    let tz = chrono::Local;
    let now_ms = Utc::now().timestamp_millis();
    let today = summaries::today_of(&tz, now_ms);
    let monday = today
        - TimeDelta::days(i64::from(
            chrono::Datelike::weekday(&today).num_days_from_monday(),
        ));
    let spans: i64 = db
        .with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM play_spans", [], |row| row.get(0))
                .map_err(map_db)
        })
        .unwrap();
    println!("{spans} stretches kept");
    timed("notable events of a game", &mut || {
        let events =
            crate::db::repo::session_events::list_notable_events_for_game(db, &game_ids[1])
                .unwrap();
        format!(
            "{} events, {} bytes",
            events.len(),
            serde_json::to_string(&events).unwrap().len()
        )
    });
    timed("every event of a game", &mut || {
        let events =
            crate::db::repo::session_events::list_events_for_game(db, &game_ids[1]).unwrap();
        format!(
            "{} events, {} bytes",
            events.len(),
            serde_json::to_string(&events).unwrap().len()
        )
    });
    let live = LiveSessions::default();
    for running in [false, true] {
        if running {
            start_running(db, &game_ids[2], &live);
        }
        let label = |what: &str| format!("{what}{}", if running { ", a game running" } else { "" });
        let now_ms = Utc::now().timestamp_millis();
        timed(&label("journal week"), &mut || {
            let week = journal::journal_week(db, &live, &tz, monday, "device", now_ms).unwrap();
            format!(
                "{} days, {} bytes",
                week.days.len(),
                serde_json::to_string(&week).unwrap().len()
            )
        });
        timed(&label("journal week a year back"), &mut || {
            let week = journal::journal_week(
                db,
                &live,
                &tz,
                monday - TimeDelta::days(364),
                "device",
                now_ms,
            )
            .unwrap();
            format!("{} days", week.days.len())
        });
        timed(&label("year play"), &mut || {
            let year = summaries::year_play(
                db,
                &live,
                &tz,
                chrono::Datelike::year(&today),
                today,
                now_ms,
            )
            .unwrap();
            format!(
                "{} day rows, streak {}",
                year.days.len(),
                year.current_streak
            )
        });
        timed(&label("recent play"), &mut || {
            let recent = summaries::recent_play(db, &live, &tz, 7, today, now_ms).unwrap();
            format!("{} sessions", recent.sessions_count)
        });
        timed(&label("played today"), &mut || {
            format!(
                "{} ms",
                summaries::played_on(db, &live, &tz, today, now_ms).unwrap()
            )
        });
    }
    timed("steps aside hint", &mut || {
        format!(
            "{:?}",
            crate::playtime::beside::steps_aside_hint(db, &game_ids[3], "device", now_ms).unwrap()
        )
    });
    live
}

/// A session of `game_id` that another PC left open `years` back after an
/// hour of play, as a restored backup brings it.
fn leave_open_elsewhere(db: &Database, game_id: &str, years: i64) {
    const MINUTES: i64 = 60;
    const STEP_MS: i64 = 30_000;
    devices::ensure_device(db, "other", "test", "0.0.0").unwrap();
    let start = Utc::now() - TimeDelta::days(365 * years) + TimeDelta::hours(3);
    db.with_transaction(|conn| {
        let id = uuid::Uuid::new_v4().to_string();
        let started = integrity::format_timestamp(start);
        let total_ms = MINUTES * 60_000;
        conn.execute(
            "INSERT INTO sessions (id, game_id, device_id, started_at_wall, elapsed_monotonic_ms,
                 active_ms, idle_ms, runtime_ms, integrity_status, closed_cleanly)
             VALUES (?1, ?2, 'other', ?3, ?4, ?4, 0, ?4, 'local', 0)",
            params![id, game_id, started, total_ms],
        )
        .map_err(map_db)?;
        integrity::append_session_event(
            conn,
            &id,
            "started",
            &started,
            Some(0),
            &json!({ "game_id": game_id, "device_id": "other", "integrity_status": "local" })
                .to_string(),
        )?;
        let mut at = STEP_MS;
        while at <= total_ms {
            integrity::append_session_event(
                conn,
                &id,
                "heartbeat",
                &integrity::format_timestamp(start + TimeDelta::milliseconds(at)),
                Some(at),
                &json!({
                    "runtime_ms": at, "active_ms": at, "idle_ms": 0,
                    "wall_elapsed_ms": at, "drift_ms": 0, "integrity_status": "local",
                })
                .to_string(),
            )?;
            at += STEP_MS;
        }
        Ok(())
    })
    .unwrap();
}

/// A launcher that steps aside, open around both evenings of every day of
/// the history. Returns its id.
fn add_launcher(db: &Database, years: i64) -> String {
    const MINUTES: i64 = 270;
    const STEP_SECS: i64 = 300;
    let launcher = games::create_game(
        db,
        &CreateGame {
            title: "Launcher".into(),
            executable_path: Some("launcher.exe".into()),
            install_folder: None,
            launcher_source: None,
        },
    )
    .unwrap()
    .id;
    games::set_steps_aside(db, &launcher, true).unwrap();
    let first = Utc::now() - TimeDelta::days(365 * years);
    for day in 0..365 * years {
        let start = first + TimeDelta::days(day) + TimeDelta::minutes(17 * 60 + 30);
        write_session(db, &launcher, start, MINUTES, STEP_SECS);
    }
    launcher
}

/// Timings with a launcher that steps aside over the whole history and a
/// session another PC left open years back, which stays open by design.
fn launcher_and_open_elsewhere_timings(
    db: &Database,
    game_ids: &[String],
    live: &LiveSessions,
    timed: &dyn Fn(&str, &mut dyn FnMut() -> String),
) {
    use crate::playtime::summaries;
    let years = 10;
    let launcher = add_launcher(db, years);
    leave_open_elsewhere(db, &game_ids[2], years - 1);
    let tz = chrono::Local;
    timed("rebuild all slices, a launcher steps aside", &mut || {
        format!("{} sessions", slices::rebuild_all(db).unwrap())
    });
    timed("rebuild for a game inside the launcher", &mut || {
        slices::rebuild_for_game(db, &game_ids[5]).unwrap();
        String::new()
    });
    timed("rebuild for the launcher", &mut || {
        slices::rebuild_for_game(db, &launcher).unwrap();
        String::new()
    });
    let now_ms = Utc::now().timestamp_millis();
    let today = summaries::today_of(&tz, now_ms);
    timed("live bar, open elsewhere", &mut || {
        let mut active = sessions::get_active_sessions(db, "device").unwrap();
        for session in &mut active {
            live.apply(session);
        }
        totals::show_counted(db, live, &mut active, now_ms).unwrap();
        let played = summaries::played_on(db, live, &tz, today, now_ms).unwrap();
        format!("{} running, {played} ms today", active.len())
    });
    timed("live bar, nothing running, open elsewhere", &mut || {
        let idle = LiveSessions::default();
        let mut active = Vec::new();
        totals::show_counted(db, &idle, &mut active, now_ms).unwrap();
        let played = summaries::played_on(db, &idle, &tz, today, now_ms).unwrap();
        format!("{played} ms today")
    });
    timed("year play, open elsewhere", &mut || {
        let year =
            summaries::year_play(db, live, &tz, chrono::Datelike::year(&today), today, now_ms)
                .unwrap();
        format!("streak {}", year.current_streak)
    });
    timed("totals for a week, open elsewhere", &mut || {
        let rows = totals::play_totals(
            db,
            live,
            &tz,
            today - TimeDelta::days(6),
            today + TimeDelta::days(1),
            totals::Bucket::Day,
            None,
            now_ms,
        )
        .unwrap();
        format!("{} rows", rows.len())
    });
}

#[test]
#[ignore = "builds a large history and prints timings"]
fn large_history() {
    let path = std::env::temp_dir().join(format!("vaultime-perf-{}.db", uuid::Uuid::new_v4()));
    let db = Database::open(&path).unwrap();
    let started = Instant::now();
    let game_ids = build_history(&db);
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
    timed("begin the ledger over the whole history", &mut || {
        db.with_conn(|conn| {
            conn.execute("DELETE FROM ledger_entries", [])
                .map_err(map_db)
        })
        .unwrap();
        let carried = db
            .with_transaction(|conn| integrity::ledger::begin_if_empty(conn, "first_start"))
            .unwrap();
        format!("{carried:?} sessions")
    });
    timed("ledger report", &mut || {
        let report = db.with_conn(integrity::ledger::report).unwrap();
        format!("{} sessions covered", report.covered_sessions)
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
    timed("fill copy overlap", &mut || {
        let mut copy = listed.clone();
        totals::fill_copy_overlap(
            &db,
            &LiveSessions::default(),
            &mut copy,
            Utc::now().timestamp_millis(),
        )
        .unwrap();
        String::new()
    });
    let live = page_timings(&db, &game_ids, &timed);
    launcher_and_open_elsewhere_timings(&db, &game_ids, &live, &timed);
    let bytes = std::fs::metadata(&path).map_or(0, |meta| meta.len());
    println!("database file: {} MB", bytes / 1_000_000);
    drop(db);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
