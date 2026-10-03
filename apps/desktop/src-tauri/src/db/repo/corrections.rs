// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Honest changes to the play history: correcting the time of a tracked
//! session and adding one by hand. Every change is an event in the session's
//! hash chain with the old values and the reason, and the session carries a
//! label that says it was changed. A suspicious session stays suspicious,
//! and a session added by hand stays Manual, since it was never tracked.

use std::time::Duration;

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde_json::{Value, json};

use crate::constants::{MANUAL_SESSION_MAX, SESSION_NOTE_MAX_CHARS, STEAM_SOURCE};
use crate::db::connection::Database;
use crate::db::models::Session;
use crate::db::repo::map_db;
use crate::db::repo::sessions::{attach_validated_status, row_to_session};
use crate::error::{Result, VaultimeError};
use crate::integrity::{self, STATUS_EDITED, STATUS_MANUAL, STATUS_SUSPICIOUS};
use crate::playtime::slices;

struct Timing {
    ended_at_wall: String,
    runtime_ms: i64,
    active_ms: i64,
    idle_ms: i64,
}

fn parse_time(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|_| VaultimeError::Invalid(format!("{value} is not a valid time")))
}

fn check_text(text: &str, required: bool) -> Result<&str> {
    let text = text.trim();
    if required && text.is_empty() {
        return Err(VaultimeError::Invalid("say why the time changes".into()));
    }
    if text.chars().count() > SESSION_NOTE_MAX_CHARS {
        return Err(VaultimeError::Invalid(format!(
            "a reason has at most {SESSION_NOTE_MAX_CHARS} characters"
        )));
    }
    Ok(text)
}

/// The session with the label its history supports, so a correction never
/// starts from a status someone changed by hand in the database.
fn load(conn: &rusqlite::Connection, session_id: &str) -> Result<Session> {
    let session = conn
        .query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [session_id],
            row_to_session,
        )
        .map_err(map_db)?;
    attach_validated_status(conn, session)
}

/// Wall time between `from` and `to` that tracking gaps, such as sleep, left
/// out of the runtime. A gap event is written when tracking resumes and
/// covers the time before it.
fn gap_ms_between(
    conn: &rusqlite::Connection,
    session_id: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<i64> {
    let mut stmt = conn
        .prepare(
            "SELECT event_time_wall, payload_json FROM session_events
             WHERE session_id = ?1 AND event_type = 'tracking_gap'",
        )
        .map_err(map_db)?;
    let gaps = stmt
        .query_map([session_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
        .map_err(map_db)?;
    let mut total = 0;
    for (resumed_at, payload) in gaps {
        let (Ok(resumed_at), Some(gap_ms)) = (
            parse_time(&resumed_at),
            serde_json::from_str::<Value>(&payload)
                .ok()
                .and_then(|payload| payload["wall_gap_ms"].as_i64()),
        ) else {
            continue;
        };
        let gap_start = resumed_at - chrono::Duration::milliseconds(gap_ms);
        let overlap = resumed_at.min(to) - gap_start.max(from);
        total += overlap.num_milliseconds().max(0);
    }
    Ok(total)
}

/// What a session keeps when it counts only up to `new_end`. Its counters at
/// that moment come from its checkpoints, so exactly the active and idle time
/// after it comes out, and sleep, which never counted, stays out. A session
/// without checkpoints inside, like one added by hand, loses idle time
/// first, then active time, and sleep in the cut part is not taken out again.
fn trim_timing(
    conn: &rusqlite::Connection,
    session: &Session,
    new_end: DateTime<Utc>,
) -> Result<Timing> {
    let Some(old_end) = session.ended_at_wall.as_deref() else {
        return Err(VaultimeError::Invalid(
            "a running session cannot be corrected".into(),
        ));
    };
    let (start, old_end) = (parse_time(&session.started_at_wall)?, parse_time(old_end)?);
    if new_end < start || new_end > old_end {
        return Err(VaultimeError::Invalid(
            "the new end has to lie within the session".into(),
        ));
    }
    let ended_at_wall = integrity::format_timestamp(new_end);
    let events = integrity::load_session_events(conn, &session.id)?;
    if let Some([_, active, idle]) = slices::counters_at(&events, new_end.timestamp_millis()) {
        // Never more than the session holds now, which an earlier cut may have lowered.
        let active_ms = active.clamp(0, session.active_ms);
        let idle_ms = idle.clamp(0, session.idle_ms);
        return Ok(Timing {
            ended_at_wall,
            runtime_ms: active_ms + idle_ms,
            active_ms,
            idle_ms,
        });
    }
    let removed = ((old_end - new_end).num_milliseconds()
        - gap_ms_between(conn, &session.id, new_end, old_end)?)
    .clamp(0, session.runtime_ms);
    let idle_cut = removed.min(session.idle_ms);
    let active_cut = (removed - idle_cut).min(session.active_ms);
    Ok(Timing {
        ended_at_wall,
        runtime_ms: session.runtime_ms - removed,
        active_ms: session.active_ms - active_cut,
        idle_ms: session.idle_ms - idle_cut,
    })
}

/// Counts a session only up to `ended_at`, for a game left running after play.
pub fn trim_session(
    db: &Database,
    session_id: &str,
    ended_at: &str,
    reason: &str,
) -> Result<Session> {
    let reason = check_text(reason, true)?;
    let new_end = parse_time(ended_at)?;
    db.with_transaction(|conn| {
        let session = load(conn, session_id)?;
        let timing = trim_timing(conn, &session, new_end)?;
        apply_correction(conn, &session, &timing, reason)
    })
}

/// The runtime, active and idle time a session would keep when it counted
/// only up to `ended_at`, without changing anything.
pub fn preview_trim(db: &Database, session_id: &str, ended_at: &str) -> Result<TrimPreview> {
    let new_end = parse_time(ended_at)?;
    db.with_conn(|conn| {
        let session = load(conn, session_id)?;
        let timing = trim_timing(conn, &session, new_end)?;
        Ok(TrimPreview {
            runtime_ms: timing.runtime_ms,
            active_ms: timing.active_ms,
            idle_ms: timing.idle_ms,
        })
    })
}

/// What a trim would leave of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct TrimPreview {
    pub runtime_ms: i64,
    pub active_ms: i64,
    pub idle_ms: i64,
}

/// Takes all time out of a session that was no play at all. The session
/// stays in the history with its reason.
pub fn discard_session(db: &Database, session_id: &str, reason: &str) -> Result<Session> {
    let reason = check_text(reason, true)?;
    db.with_transaction(|conn| {
        let session = load(conn, session_id)?;
        let Some(ended_at_wall) = session.ended_at_wall.clone() else {
            return Err(VaultimeError::Invalid(
                "a running session cannot be corrected".into(),
            ));
        };
        let timing = Timing {
            ended_at_wall,
            runtime_ms: 0,
            active_ms: 0,
            idle_ms: 0,
        };
        apply_correction(conn, &session, &timing, reason)
    })
}

fn apply_correction(
    conn: &rusqlite::Connection,
    session: &Session,
    timing: &Timing,
    reason: &str,
) -> Result<Session> {
    // A correction only ever takes time out, whatever asked for it.
    let ends_later = match session.ended_at_wall.as_deref() {
        Some(old_end) => parse_time(&timing.ended_at_wall)? > parse_time(old_end)?,
        None => true,
    };
    if ends_later
        || timing.runtime_ms > session.runtime_ms
        || timing.active_ms > session.active_ms
        || timing.idle_ms > session.idle_ms
    {
        return Err(VaultimeError::Invalid(
            "a correction can only take time out".into(),
        ));
    }
    if timing.runtime_ms == session.runtime_ms {
        return Err(VaultimeError::Invalid(
            "this correction takes no time out".into(),
        ));
    }
    let status = match session.integrity_status.as_str() {
        STATUS_SUSPICIOUS => STATUS_SUSPICIOUS,
        STATUS_MANUAL => STATUS_MANUAL,
        _ => STATUS_EDITED,
    };
    conn.execute(
        "UPDATE sessions
         SET ended_at_wall = ?1, elapsed_monotonic_ms = ?2, runtime_ms = ?2,
             active_ms = ?3, idle_ms = ?4, integrity_status = ?5
         WHERE id = ?6",
        params![
            timing.ended_at_wall,
            timing.runtime_ms,
            timing.active_ms,
            timing.idle_ms,
            status,
            session.id
        ],
    )
    .map_err(map_db)?;
    integrity::append_session_event(
        conn,
        &session.id,
        "corrected",
        &integrity::now_timestamp(),
        Some(timing.runtime_ms),
        &json!({
            "reason": reason,
            "ended_at_wall": timing.ended_at_wall,
            "runtime_ms": timing.runtime_ms,
            "active_ms": timing.active_ms,
            "idle_ms": timing.idle_ms,
            "integrity_status": status,
            "closed_cleanly": session.closed_cleanly,
            "previous": {
                "ended_at_wall": session.ended_at_wall,
                "runtime_ms": session.runtime_ms,
                "active_ms": session.active_ms,
                "idle_ms": session.idle_ms,
                "integrity_status": session.integrity_status,
            },
        })
        .to_string(),
    )?;
    slices::rebuild_session(conn, &session.id)?;
    load(conn, &session.id)
}

/// Adds play Vaultime did not see, like a session on another PC. It counts
/// as active time and carries the Manual label. `launcher` names the
/// launcher that counted the play too, so an import of that launcher's
/// playtime leaves it out.
pub fn add_manual_session(
    db: &Database,
    game_id: &str,
    device_id: &str,
    started_at: &str,
    runtime_ms: i64,
    reason: &str,
    launcher: Option<&str>,
) -> Result<Session> {
    let reason = check_text(reason, false)?;
    if launcher.is_some_and(|name| name != STEAM_SOURCE) {
        return Err(VaultimeError::Invalid(
            "only Steam can count a session added by hand".into(),
        ));
    }
    let start = parse_time(started_at)?;
    let max_ms = i64::try_from(MANUAL_SESSION_MAX.as_millis()).unwrap_or(i64::MAX);
    if runtime_ms <= 0 || runtime_ms > max_ms {
        return Err(VaultimeError::Invalid(format!(
            "a session added by hand lasts up to {} hours",
            MANUAL_SESSION_MAX.as_secs() / Duration::from_hours(1).as_secs()
        )));
    }
    let end = start + chrono::Duration::milliseconds(runtime_ms);
    if end > Utc::now() {
        return Err(VaultimeError::Invalid(
            "a session cannot end in the future".into(),
        ));
    }
    let (started_at_wall, ended_at_wall) = (
        integrity::format_timestamp(start),
        integrity::format_timestamp(end),
    );
    let id = uuid::Uuid::new_v4().to_string();

    db.with_transaction(|conn| {
        // Play of one game cannot happen twice at once. Sessions whose time
        // was all taken out do not count, their slot is free again.
        let overlaps: bool = conn
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sessions
                     WHERE game_id = ?1 AND runtime_ms > 0
                       AND started_at_wall < ?3
                       AND COALESCE(ended_at_wall, ?4) > ?2)",
                params![
                    game_id,
                    started_at_wall,
                    ended_at_wall,
                    integrity::now_timestamp()
                ],
                |row| row.get(0),
            )
            .map_err(map_db)?;
        if overlaps {
            return Err(VaultimeError::Invalid(
                "another session of this game already covers part of that time".into(),
            ));
        }

        conn.execute(
            "INSERT INTO sessions
                (id, game_id, device_id, started_at_wall, ended_at_wall,
                 elapsed_monotonic_ms, active_ms, idle_ms, runtime_ms,
                 integrity_status, closed_cleanly)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 0, ?6, ?7, 1)",
            params![
                id,
                game_id,
                device_id,
                started_at_wall,
                ended_at_wall,
                runtime_ms,
                STATUS_MANUAL
            ],
        )
        .map_err(map_db)?;
        integrity::append_session_event(
            conn,
            &id,
            "added_manually",
            &integrity::now_timestamp(),
            Some(runtime_ms),
            &json!({
                "reason": reason,
                "started_at_wall": started_at_wall,
                "ended_at_wall": ended_at_wall,
                "runtime_ms": runtime_ms,
                "active_ms": runtime_ms,
                "idle_ms": 0,
                "integrity_status": STATUS_MANUAL,
                "closed_cleanly": true,
                "launcher": launcher,
                "game_id": game_id,
                "device_id": device_id,
            })
            .to_string(),
        )?;
        slices::rebuild_session(conn, &id)?;
        load(conn, &id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::{devices, games, sessions};

    const DEVICE: &str = "test-device";
    const MINUTE: i64 = 60_000;

    fn setup() -> (Database, String) {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, DEVICE, "windows", "0.1.0").unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Elden Ring".into(),
                executable_path: Some("C:/Games/eldenring.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        (db, game.id)
    }

    /// A closed session that started `runtime` minutes ago and ended now,
    /// with a start event at that time like the tracker writes.
    fn played(db: &Database, game_id: &str, runtime: i64, active: i64) -> Session {
        let id = uuid::Uuid::new_v4().to_string();
        let start = integrity::format_timestamp(Utc::now() - chrono::Duration::minutes(runtime));
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO sessions
                    (id, game_id, device_id, started_at_wall, elapsed_monotonic_ms,
                     active_ms, idle_ms, runtime_ms, integrity_status, closed_cleanly)
                 VALUES (?1, ?2, ?3, ?4, 0, 0, 0, 0, 'local', 0)",
                params![id, game_id, DEVICE, start],
            )
            .map_err(map_db)?;
            integrity::append_session_event(
                conn,
                &id,
                "started",
                &start,
                Some(0),
                &json!({ "game_id": game_id, "device_id": DEVICE, "integrity_status": "local" })
                    .to_string(),
            )
        })
        .unwrap();
        sessions::end_session(
            db,
            &id,
            runtime * MINUTE,
            active * MINUTE,
            (runtime - active) * MINUTE,
            "local",
        )
        .unwrap()
    }

    fn validated(db: &Database, id: &str) -> Session {
        db.with_conn(|conn| load(conn, id)).unwrap()
    }

    #[test]
    fn trimming_takes_idle_time_first_and_keeps_the_chain_valid() {
        let (db, game) = setup();
        let session = played(&db, &game, 300, 120);
        let end = parse_time(session.ended_at_wall.as_deref().unwrap()).unwrap();
        let new_end = integrity::format_timestamp(end - chrono::Duration::minutes(200));

        let trimmed = trim_session(&db, &session.id, &new_end, "Left the game running").unwrap();
        assert_eq!(trimmed.runtime_ms, 100 * MINUTE);
        assert_eq!(trimmed.idle_ms, 0, "the 180 idle minutes go first");
        assert_eq!(trimmed.active_ms, 100 * MINUTE);
        assert_eq!(trimmed.integrity_status, STATUS_EDITED);
        assert_eq!(trimmed.ended_at_wall.as_deref(), Some(new_end.as_str()));
        assert_eq!(validated(&db, &session.id).integrity_status, STATUS_EDITED);
    }

    #[test]
    fn discarding_keeps_the_session_with_no_time() {
        let (db, game) = setup();
        let session = played(&db, &game, 30, 30);
        let discarded = discard_session(&db, &session.id, "Only the launcher ran").unwrap();
        assert_eq!(
            (discarded.runtime_ms, discarded.active_ms, discarded.idle_ms),
            (0, 0, 0)
        );
        assert_eq!(discarded.ended_at_wall, session.ended_at_wall);
        assert_eq!(validated(&db, &session.id).integrity_status, STATUS_EDITED);
    }

    #[test]
    fn trimming_takes_out_exactly_the_time_after_the_new_end() {
        let (db, game) = setup();
        let session = sessions::create_session(&db, &game, DEVICE).unwrap();
        let start = parse_time(&session.started_at_wall).unwrap();
        // Active for half an hour, then idle for an hour.
        let checkpoint = sessions::Checkpoint {
            runtime_ms: 30 * MINUTE,
            active_ms: 30 * MINUTE,
            idle_ms: 0,
            wall_elapsed_ms: 30 * MINUTE,
            drift_ms: 0,
        };
        sessions::record_checkpoint(
            &db,
            &session.id,
            &checkpoint,
            "local",
            start + chrono::Duration::minutes(30),
        )
        .unwrap();
        let ended = db
            .with_transaction(|conn| {
                let end = integrity::format_timestamp(start + chrono::Duration::minutes(90));
                conn.execute(
                    "UPDATE sessions SET ended_at_wall = ?1, runtime_ms = ?2, elapsed_monotonic_ms = ?2,
                         active_ms = ?3, idle_ms = ?4, closed_cleanly = 1
                     WHERE id = ?5",
                    params![end, 90 * MINUTE, 30 * MINUTE, 60 * MINUTE, session.id],
                )
                .map_err(map_db)?;
                integrity::append_session_event(
                    conn,
                    &session.id,
                    "ended",
                    &end,
                    Some(90 * MINUTE),
                    &json!({
                        "runtime_ms": 90 * MINUTE,
                        "active_ms": 30 * MINUTE,
                        "idle_ms": 60 * MINUTE,
                        "integrity_status": "local",
                        "closed_cleanly": true,
                    })
                    .to_string(),
                )?;
                load(conn, &session.id)
            })
            .unwrap();
        assert_eq!(ended.integrity_status, "local");

        // Cut at 20 minutes: ten active minutes go, and the whole idle hour.
        let new_end = integrity::format_timestamp(start + chrono::Duration::minutes(20));
        let preview = preview_trim(&db, &session.id, &new_end).unwrap();
        assert_eq!(
            (preview.runtime_ms, preview.active_ms, preview.idle_ms),
            (20 * MINUTE, 20 * MINUTE, 0)
        );
        let trimmed = trim_session(&db, &session.id, &new_end, "Fell asleep").unwrap();
        assert_eq!(
            (trimmed.runtime_ms, trimmed.active_ms, trimmed.idle_ms),
            (20 * MINUTE, 20 * MINUTE, 0)
        );
        assert_eq!(validated(&db, &session.id).integrity_status, STATUS_EDITED);
    }

    #[test]
    fn corrections_only_take_time_out() {
        let (db, game) = setup();
        let session = played(&db, &game, 60, 40);
        let end = session.ended_at_wall.clone().unwrap();
        assert!(
            trim_session(&db, &session.id, &end, "Same end").is_err(),
            "a trim that takes nothing out"
        );
        discard_session(&db, &session.id, "No play").unwrap();
        assert!(
            discard_session(&db, &session.id, "Again").is_err(),
            "nothing left to take out"
        );

        // Whatever calls it, the shared step refuses to add time or move the end later.
        let fresh = played(&db, &game, 60, 40);
        let fresh_end = parse_time(fresh.ended_at_wall.as_deref().unwrap()).unwrap();
        for timing in [
            Timing {
                ended_at_wall: fresh.ended_at_wall.clone().unwrap(),
                runtime_ms: fresh.runtime_ms + MINUTE,
                active_ms: fresh.active_ms,
                idle_ms: fresh.idle_ms,
            },
            Timing {
                ended_at_wall: integrity::format_timestamp(
                    fresh_end + chrono::Duration::minutes(5),
                ),
                runtime_ms: fresh.runtime_ms - MINUTE,
                active_ms: fresh.active_ms - MINUTE,
                idle_ms: fresh.idle_ms,
            },
        ] {
            let refused =
                db.with_transaction(|conn| apply_correction(conn, &fresh, &timing, "More"));
            assert!(refused.is_err());
        }
        assert_eq!(validated(&db, &fresh.id).runtime_ms, 60 * MINUTE);
    }

    #[test]
    fn a_recorded_correction_that_adds_time_turns_the_session_suspicious() {
        let (db, game) = setup();
        let session = played(&db, &game, 60, 60);
        // Written past the checks, through the chain, as an edited database could.
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE sessions SET runtime_ms = ?1, elapsed_monotonic_ms = ?1, active_ms = ?1,
                     integrity_status = 'edited'
                 WHERE id = ?2",
                params![120 * MINUTE, session.id],
            )
            .map_err(map_db)?;
            integrity::append_session_event(
                conn,
                &session.id,
                "corrected",
                &integrity::now_timestamp(),
                Some(120 * MINUTE),
                &json!({
                    "reason": "More",
                    "ended_at_wall": session.ended_at_wall,
                    "runtime_ms": 120 * MINUTE,
                    "active_ms": 120 * MINUTE,
                    "idle_ms": 0,
                    "integrity_status": "edited",
                    "closed_cleanly": true,
                    "previous": {
                        "ended_at_wall": session.ended_at_wall,
                        "runtime_ms": 60 * MINUTE,
                        "active_ms": 60 * MINUTE,
                        "idle_ms": 0,
                        "integrity_status": "local",
                    },
                })
                .to_string(),
            )
        })
        .unwrap();
        assert_eq!(
            validated(&db, &session.id).integrity_status,
            STATUS_SUSPICIOUS
        );
    }

    #[test]
    fn suspicious_sessions_stay_suspicious() {
        let (db, game) = setup();
        let session = sessions::create_session(&db, &game, DEVICE).unwrap();
        let session = sessions::end_session(
            &db,
            &session.id,
            60 * MINUTE,
            60 * MINUTE,
            0,
            STATUS_SUSPICIOUS,
        )
        .unwrap();
        let corrected = discard_session(&db, &session.id, "Clock was wrong").unwrap();
        assert_eq!(corrected.integrity_status, STATUS_SUSPICIOUS);
    }

    #[test]
    fn refuses_bad_corrections() {
        let (db, game) = setup();
        let running = sessions::create_session(&db, &game, DEVICE).unwrap();
        assert!(
            discard_session(&db, &running.id, "No").is_err(),
            "a running session"
        );
        let session = played(&db, &game, 60, 60);
        assert!(
            discard_session(&db, &session.id, "  ").is_err(),
            "no reason"
        );
        let after_end = integrity::format_timestamp(
            parse_time(session.ended_at_wall.as_deref().unwrap()).unwrap()
                + chrono::Duration::minutes(1),
        );
        assert!(
            trim_session(&db, &session.id, &after_end, "Later").is_err(),
            "past the end"
        );
    }

    #[test]
    fn changed_timing_without_an_event_is_still_caught() {
        let (db, game) = setup();
        let session = played(&db, &game, 60, 60);
        discard_session(&db, &session.id, "Never played").unwrap();
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE sessions SET runtime_ms = 999 WHERE id = ?1",
                [&session.id],
            )
            .map_err(map_db)
        })
        .unwrap();
        assert_eq!(
            validated(&db, &session.id).integrity_status,
            STATUS_SUSPICIOUS
        );
    }

    #[test]
    fn manual_sessions_count_as_active_and_say_so() {
        let (db, game) = setup();
        let start = integrity::format_timestamp(Utc::now() - chrono::Duration::hours(5));
        let session = add_manual_session(
            &db,
            &game,
            DEVICE,
            &start,
            90 * MINUTE,
            "On the Steam Deck",
            None,
        )
        .unwrap();
        assert_eq!(session.integrity_status, STATUS_MANUAL);
        assert_eq!(
            (session.runtime_ms, session.active_ms, session.idle_ms),
            (90 * MINUTE, 90 * MINUTE, 0)
        );
        assert!(session.closed_cleanly);
        assert_eq!(validated(&db, &session.id).integrity_status, STATUS_MANUAL);

        let future = integrity::format_timestamp(Utc::now() - chrono::Duration::minutes(10));
        assert!(
            add_manual_session(&db, &game, DEVICE, &future, 60 * MINUTE, "", None).is_err(),
            "ends in the future"
        );
        assert!(
            add_manual_session(&db, &game, DEVICE, &start, 0, "", None).is_err(),
            "no time"
        );
        assert!(
            add_manual_session(&db, &game, DEVICE, "yesterday", 60, "", None).is_err(),
            "no time format"
        );
    }

    #[test]
    fn a_corrected_manual_session_stays_manual() {
        let (db, game) = setup();
        let start = integrity::format_timestamp(Utc::now() - chrono::Duration::hours(5));
        let session =
            add_manual_session(&db, &game, DEVICE, &start, 90 * MINUTE, "", None).unwrap();
        let fixed = discard_session(&db, &session.id, "Wrong game").unwrap();
        assert_eq!(fixed.integrity_status, STATUS_MANUAL);
        assert_eq!(validated(&db, &session.id).integrity_status, STATUS_MANUAL);
    }

    #[test]
    fn only_steam_can_count_a_manual_session() {
        let (db, game) = setup();
        let start = integrity::format_timestamp(Utc::now() - chrono::Duration::hours(5));
        assert!(add_manual_session(&db, &game, DEVICE, &start, MINUTE, "", Some("epic")).is_err());
        let session =
            add_manual_session(&db, &game, DEVICE, &start, MINUTE, "", Some(STEAM_SOURCE)).unwrap();
        assert_eq!(session.integrity_status, STATUS_MANUAL);
    }

    #[test]
    fn a_trim_leaves_sleep_out_of_what_it_takes() {
        let (db, game) = setup();
        let now = Utc::now();
        let minutes_ago =
            |minutes| integrity::format_timestamp(now - chrono::Duration::minutes(minutes));
        let id = uuid::Uuid::new_v4().to_string();
        // Two hours on the clock, an hour of it asleep until ten minutes ago.
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO sessions
                    (id, game_id, device_id, started_at_wall, elapsed_monotonic_ms,
                     active_ms, idle_ms, runtime_ms, integrity_status, closed_cleanly)
                 VALUES (?1, ?2, ?3, ?4, 0, 0, 0, 0, 'local', 0)",
                params![id, game, DEVICE, minutes_ago(120)],
            )
            .map_err(map_db)?;
            integrity::append_session_event(
                conn,
                &id,
                "started",
                &minutes_ago(120),
                Some(0),
                &json!({ "game_id": game, "device_id": DEVICE, "integrity_status": "local" })
                    .to_string(),
            )?;
            integrity::append_session_event(
                conn,
                &id,
                "tracking_gap",
                &minutes_ago(10),
                Some(50 * MINUTE),
                &json!({ "wall_gap_ms": 60 * MINUTE, "monotonic_gap_ms": 0 }).to_string(),
            )
        })
        .unwrap();
        sessions::end_session(&db, &id, 60 * MINUTE, 60 * MINUTE, 0, "local").unwrap();

        // Half an hour off the end holds only ten minutes of play.
        let trimmed = trim_session(&db, &id, &minutes_ago(30), "Stopped earlier").unwrap();
        // The real end lies a few milliseconds after the times of this test.
        assert!((trimmed.runtime_ms - 50 * MINUTE).abs() < 1_000);
        assert_eq!(validated(&db, &id).integrity_status, STATUS_EDITED);
    }

    #[test]
    fn a_flag_survives_recovery_and_correction() {
        let (db, game) = setup();
        let session = sessions::create_session(&db, &game, DEVICE).unwrap();
        // A minute on record, so the discard below has time to take out.
        let checkpoint = sessions::Checkpoint {
            runtime_ms: MINUTE,
            active_ms: MINUTE,
            idle_ms: 0,
            wall_elapsed_ms: MINUTE,
            drift_ms: 0,
        };
        sessions::record_checkpoint(&db, &session.id, &checkpoint, "local", Utc::now()).unwrap();
        sessions::flag_session_suspicious(
            &db,
            &session.id,
            MINUTE,
            MINUTE,
            30_000,
            "wall_clock_step_mismatch",
        )
        .unwrap();
        let recovered =
            sessions::recover_session(&db, &session.id, "startup_orphan_cleanup").unwrap();
        assert_eq!(recovered.integrity_status, STATUS_SUSPICIOUS);
        let corrected = discard_session(&db, &session.id, "Not play").unwrap();
        assert_eq!(corrected.integrity_status, STATUS_SUSPICIOUS);
        assert_eq!(
            validated(&db, &session.id).integrity_status,
            STATUS_SUSPICIOUS
        );
    }

    #[test]
    fn a_manual_session_cannot_overlap_play_of_the_same_game() {
        let (db, game) = setup();
        let tracked = played(&db, &game, 90, 90);
        let inside = integrity::format_timestamp(Utc::now() - chrono::Duration::minutes(60));
        assert!(add_manual_session(&db, &game, DEVICE, &inside, 10 * MINUTE, "", None).is_err());

        discard_session(&db, &tracked.id, "The launcher only").unwrap();
        assert!(add_manual_session(&db, &game, DEVICE, &inside, 10 * MINUTE, "", None).is_ok());
    }
}
