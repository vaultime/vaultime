// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Honest changes to the play history: correcting the time of a tracked
//! session and adding one by hand. Every change is an event in the session's
//! hash chain with the old values and the reason, and the session carries a
//! label that says it was changed. A suspicious session stays suspicious.

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::params;
use serde_json::json;

use crate::constants::{MANUAL_SESSION_MAX, SESSION_NOTE_MAX_CHARS};
use crate::db::connection::Database;
use crate::db::models::Session;
use crate::db::repo::map_db;
use crate::db::repo::sessions::{attach_validated_status, row_to_session};
use crate::error::{Result, VaultimeError};
use crate::integrity::{self, STATUS_EDITED, STATUS_MANUAL, STATUS_SUSPICIOUS};

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

fn format_time(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Millis, true)
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

fn load(conn: &rusqlite::Connection, session_id: &str) -> Result<Session> {
    conn.query_row(
        "SELECT * FROM sessions WHERE id = ?1",
        [session_id],
        row_to_session,
    )
    .map_err(map_db)
}

/// Counts a session only up to `ended_at`, for a game left running after
/// play. The cut comes out of idle time first, then out of active time.
pub fn trim_session(
    db: &Database,
    session_id: &str,
    ended_at: &str,
    reason: &str,
) -> Result<Session> {
    let reason = check_text(reason, true)?;
    let new_end = parse_time(ended_at)?;
    db.with_conn(|conn| {
        let session = load(conn, session_id)?;
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
        let removed = (old_end - new_end)
            .num_milliseconds()
            .min(session.runtime_ms);
        let idle_cut = removed.min(session.idle_ms);
        let active_cut = (removed - idle_cut).min(session.active_ms);
        let timing = Timing {
            ended_at_wall: format_time(new_end),
            runtime_ms: session.runtime_ms - removed,
            active_ms: session.active_ms - active_cut,
            idle_ms: session.idle_ms - idle_cut,
        };
        apply_correction(conn, &session, &timing, reason)
    })
}

/// Takes all time out of a session that was no play at all. The session
/// stays in the history with its reason.
pub fn discard_session(db: &Database, session_id: &str, reason: &str) -> Result<Session> {
    let reason = check_text(reason, true)?;
    db.with_conn(|conn| {
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
    let status = if session.integrity_status == STATUS_SUSPICIOUS {
        STATUS_SUSPICIOUS
    } else {
        STATUS_EDITED
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
    attach_validated_status(conn, load(conn, &session.id)?)
}

/// Adds play Vaultime did not see, like a session on another PC. It counts
/// as active time and carries the Manual label.
pub fn add_manual_session(
    db: &Database,
    game_id: &str,
    device_id: &str,
    started_at: &str,
    runtime_ms: i64,
    reason: &str,
) -> Result<Session> {
    let reason = check_text(reason, false)?;
    let start = parse_time(started_at)?;
    let max_ms = i64::try_from(MANUAL_SESSION_MAX.as_millis()).unwrap_or(i64::MAX);
    if runtime_ms <= 0 || runtime_ms > max_ms {
        return Err(VaultimeError::Invalid(format!(
            "a session added by hand lasts up to {} hours",
            MANUAL_SESSION_MAX.as_secs() / 3600
        )));
    }
    let end = start + chrono::Duration::milliseconds(runtime_ms);
    if end > Utc::now() {
        return Err(VaultimeError::Invalid(
            "a session cannot end in the future".into(),
        ));
    }
    let (started_at_wall, ended_at_wall) = (format_time(start), format_time(end));
    let id = uuid::Uuid::new_v4().to_string();

    db.with_conn(|conn| {
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
            })
            .to_string(),
        )?;
        attach_validated_status(conn, load(conn, &id)?)
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
        let start = format_time(Utc::now() - chrono::Duration::minutes(runtime));
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
        db.with_conn(|conn| attach_validated_status(conn, load(conn, id)?))
            .unwrap()
    }

    #[test]
    fn trimming_takes_idle_time_first_and_keeps_the_chain_valid() {
        let (db, game) = setup();
        let session = played(&db, &game, 300, 120);
        let end = parse_time(session.ended_at_wall.as_deref().unwrap()).unwrap();
        let new_end = format_time(end - chrono::Duration::minutes(200));

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
        let after_end = format_time(
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
        let start = format_time(Utc::now() - chrono::Duration::hours(5));
        let session =
            add_manual_session(&db, &game, DEVICE, &start, 90 * MINUTE, "On the Steam Deck")
                .unwrap();
        assert_eq!(session.integrity_status, STATUS_MANUAL);
        assert_eq!(
            (session.runtime_ms, session.active_ms, session.idle_ms),
            (90 * MINUTE, 90 * MINUTE, 0)
        );
        assert!(session.closed_cleanly);
        assert_eq!(validated(&db, &session.id).integrity_status, STATUS_MANUAL);

        let future = format_time(Utc::now() - chrono::Duration::minutes(10));
        assert!(
            add_manual_session(&db, &game, DEVICE, &future, 60 * MINUTE, "").is_err(),
            "ends in the future"
        );
        assert!(
            add_manual_session(&db, &game, DEVICE, &start, 0, "").is_err(),
            "no time"
        );
        assert!(
            add_manual_session(&db, &game, DEVICE, "yesterday", 60, "").is_err(),
            "no time format"
        );
    }

    #[test]
    fn a_corrected_manual_session_becomes_edited() {
        let (db, game) = setup();
        let start = format_time(Utc::now() - chrono::Duration::hours(5));
        let session = add_manual_session(&db, &game, DEVICE, &start, 90 * MINUTE, "").unwrap();
        let fixed = discard_session(&db, &session.id, "Wrong game").unwrap();
        assert_eq!(fixed.integrity_status, STATUS_EDITED);
        assert_eq!(validated(&db, &session.id).integrity_status, STATUS_EDITED);
    }
}
