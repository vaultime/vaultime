// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Integrity system — event hashing, chain validation, trust scoring.

use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::db::models::{Session, SessionEvent};
use crate::error::{Result, VaultimeError};

pub const STATUS_LOCAL: &str = "local";
pub const STATUS_SUSPICIOUS: &str = "suspicious";
pub const STATUS_RECOVERED: &str = "recovered";

/// Returns the canonical wall timestamp format used for session and event rows.
pub fn now_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Appends a session event while maintaining the per-session hash chain.
pub fn append_session_event(
    conn: &Connection,
    session_id: &str,
    event_type: &str,
    event_time_wall: &str,
    event_time_monotonic: Option<i64>,
    payload_json: &str,
) -> Result<()> {
    let (next_sequence, previous_hash) = next_sequence_and_previous_hash(conn, session_id)?;
    let hash_self = compute_event_hash(
        session_id,
        next_sequence,
        event_type,
        event_time_wall,
        event_time_monotonic,
        payload_json,
        previous_hash.as_deref(),
    );

    conn.execute(
        "INSERT INTO session_events
            (id, session_id, sequence, event_type, event_time_wall,
             event_time_monotonic, payload_json, hash_prev, hash_self, signature)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
        params![
            uuid::Uuid::new_v4().to_string(),
            session_id,
            next_sequence,
            event_type,
            event_time_wall,
            event_time_monotonic,
            payload_json,
            previous_hash,
            hash_self,
        ],
    )
    .map_err(|e| VaultimeError::Integrity(format!("failed to append session event: {e}")))?;

    Ok(())
}

/// Validates the stored event chain and terminal payloads for a session.
///
/// Returns `Ok(None)` when the local audit trail looks internally consistent,
/// or `Ok(Some(reason))` when the session should be treated as suspicious.
pub fn validate_session_history(conn: &Connection, session: &Session) -> Result<Option<String>> {
    let events = load_session_events(conn, &session.id)?;
    if events.is_empty() {
        return Ok(Some("missing_event_chain".into()));
    }

    if let Some(reason) = validate_event_chain(&events) {
        return Ok(Some(reason));
    }

    let first_event = events.first().expect("events checked non-empty");
    if first_event.event_type != "started" {
        return Ok(Some("event_chain_missing_start".into()));
    }

    if first_event.event_time_wall != session.started_at_wall {
        return Ok(Some("session_start_mismatch".into()));
    }

    let last_event = events.last().expect("events checked non-empty");
    if let Some(ended_at_wall) = session.ended_at_wall.as_deref() {
        if !matches!(last_event.event_type.as_str(), "ended" | "recovered") {
            return Ok(Some("closed_session_missing_terminal_event".into()));
        }

        if last_event.event_time_wall != ended_at_wall {
            return Ok(Some("session_end_mismatch".into()));
        }
    } else if matches!(last_event.event_type.as_str(), "ended" | "recovered") {
        return Ok(Some("open_session_terminal_event_invalid".into()));
    }

    let latest_timing_event = events
        .iter()
        .rev()
        .find(|event| {
            matches!(
                event.event_type.as_str(),
                "started" | "heartbeat" | "ended" | "recovered"
            )
        })
        .expect("started event guarantees a timing event");
    if !timing_event_matches_session(latest_timing_event, session) {
        return Ok(Some("session_timing_mismatch".into()));
    }

    let latest_status_event = events
        .iter()
        .rev()
        .find(|event| {
            matches!(
                event.event_type.as_str(),
                "started" | "integrity_flagged" | "heartbeat" | "ended" | "recovered"
            )
        })
        .expect("started event guarantees a status event");
    if !status_event_matches_session(latest_status_event, session) {
        return Ok(Some("session_status_mismatch".into()));
    }

    Ok(None)
}

fn next_sequence_and_previous_hash(
    conn: &Connection,
    session_id: &str,
) -> Result<(i64, Option<String>)> {
    let row = conn
        .query_row(
            "SELECT sequence, hash_self
             FROM session_events
             WHERE session_id = ?1
             ORDER BY sequence DESC
             LIMIT 1",
            [session_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|e| VaultimeError::Integrity(format!("failed to query event chain: {e}")))?;

    Ok(match row {
        Some((last_sequence, last_hash)) => (last_sequence + 1, last_hash),
        None => (1, None),
    })
}

fn compute_event_hash(
    session_id: &str,
    sequence: i64,
    event_type: &str,
    event_time_wall: &str,
    event_time_monotonic: Option<i64>,
    payload_json: &str,
    previous_hash: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(sequence.to_string().as_bytes());
    hasher.update(b"\n");
    hasher.update(event_type.as_bytes());
    hasher.update(b"\n");
    hasher.update(event_time_wall.as_bytes());
    hasher.update(b"\n");
    hasher.update(
        event_time_monotonic
            .map(|value| value.to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(b"\n");
    hasher.update(payload_json.as_bytes());
    hasher.update(b"\n");
    hasher.update(previous_hash.unwrap_or_default().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn validate_event_chain(events: &[SessionEvent]) -> Option<String> {
    let mut previous_hash: Option<String> = None;

    for (index, event) in events.iter().enumerate() {
        let expected_sequence = (index as i64) + 1;
        if event.sequence != expected_sequence {
            return Some("event_sequence_gap".into());
        }

        if event.hash_prev != previous_hash {
            return Some("event_previous_hash_mismatch".into());
        }

        let expected_hash = compute_event_hash(
            &event.session_id,
            event.sequence,
            &event.event_type,
            &event.event_time_wall,
            event.event_time_monotonic,
            &event.payload_json,
            previous_hash.as_deref(),
        );

        if event.hash_self.as_deref() != Some(expected_hash.as_str()) {
            return Some("event_hash_invalid".into());
        }

        previous_hash = Some(expected_hash);
    }

    None
}

fn timing_event_matches_session(event: &SessionEvent, session: &Session) -> bool {
    match event.event_type.as_str() {
        "started" => {
            session.runtime_ms == 0
                && session.active_ms == 0
                && session.idle_ms == 0
                && event.event_time_monotonic == Some(0)
        }
        "heartbeat" | "ended" | "recovered" => {
            let Some(payload) = parse_payload(&event.payload_json) else {
                return false;
            };

            payload_i64(&payload, "runtime_ms") == Some(session.runtime_ms)
                && payload_i64(&payload, "active_ms") == Some(session.active_ms)
                && payload_i64(&payload, "idle_ms") == Some(session.idle_ms)
                && event.event_time_monotonic == Some(session.runtime_ms)
        }
        _ => true,
    }
}

fn status_event_matches_session(event: &SessionEvent, session: &Session) -> bool {
    match event.event_type.as_str() {
        "started" => session.integrity_status == STATUS_LOCAL && !session.closed_cleanly,
        "integrity_flagged" => session.integrity_status == STATUS_SUSPICIOUS,
        "heartbeat" | "ended" | "recovered" => {
            let Some(payload) = parse_payload(&event.payload_json) else {
                return false;
            };

            let status_matches = payload_str(&payload, "integrity_status")
                == Some(session.integrity_status.as_str());

            if matches!(event.event_type.as_str(), "ended" | "recovered") {
                return status_matches
                    && payload_bool(&payload, "closed_cleanly") == Some(session.closed_cleanly);
            }

            status_matches
        }
        _ => true,
    }
}

fn load_session_events(conn: &Connection, session_id: &str) -> Result<Vec<SessionEvent>> {
    let mut stmt = conn
        .prepare(
            "SELECT *
             FROM session_events
             WHERE session_id = ?1
             ORDER BY sequence ASC",
        )
        .map_err(|e| {
            VaultimeError::Integrity(format!("failed to prepare session event query: {e}"))
        })?;

    let rows = stmt
        .query_map([session_id], row_to_session_event)
        .map_err(|e| VaultimeError::Integrity(format!("failed to query session events: {e}")))?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| VaultimeError::Integrity(format!("failed to collect session events: {e}")))
}

fn row_to_session_event(row: &Row) -> rusqlite::Result<SessionEvent> {
    Ok(SessionEvent {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        sequence: row.get("sequence")?,
        event_type: row.get("event_type")?,
        event_time_wall: row.get("event_time_wall")?,
        event_time_monotonic: row.get("event_time_monotonic")?,
        payload_json: row.get("payload_json")?,
        hash_prev: row.get("hash_prev")?,
        hash_self: row.get("hash_self")?,
        signature: row.get("signature")?,
    })
}

fn parse_payload(payload_json: &str) -> Option<Value> {
    serde_json::from_str(payload_json).ok()
}

fn payload_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

fn payload_str<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload.get(key).and_then(Value::as_str)
}

fn payload_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::Session;

    #[test]
    fn timestamp_contains_timezone() {
        assert!(now_timestamp().ends_with('Z'));
    }

    #[test]
    fn event_hash_changes_with_sequence() {
        let first = compute_event_hash(
            "session",
            1,
            "start",
            "2026-01-01T00:00:00Z",
            Some(0),
            "{}",
            None,
        );
        let second = compute_event_hash(
            "session",
            2,
            "start",
            "2026-01-01T00:00:00Z",
            Some(0),
            "{}",
            None,
        );
        assert_ne!(first, second);
    }

    #[test]
    fn validation_detects_hash_mismatch() {
        let session = Session {
            id: "session-1".into(),
            game_id: "game-1".into(),
            device_id: "device-1".into(),
            started_at_wall: "2026-01-01T00:00:00.000Z".into(),
            ended_at_wall: Some("2026-01-01T00:05:00.000Z".into()),
            elapsed_monotonic_ms: 300_000,
            active_ms: 240_000,
            idle_ms: 60_000,
            runtime_ms: 300_000,
            integrity_status: STATUS_LOCAL.into(),
            closed_cleanly: true,
        };
        let mut events = vec![SessionEvent {
            id: "event-1".into(),
            session_id: session.id.clone(),
            sequence: 1,
            event_type: "started".into(),
            event_time_wall: session.started_at_wall.clone(),
            event_time_monotonic: Some(0),
            payload_json: "{\"game_id\":\"game-1\"}".into(),
            hash_prev: None,
            hash_self: Some("bad-hash".into()),
            signature: None,
        }];

        let end_payload = format!(
            "{{\"runtime_ms\":300000,\"active_ms\":240000,\"idle_ms\":60000,\"integrity_status\":\"local\",\"closed_cleanly\":true}}"
        );
        let end_hash = compute_event_hash(
            &session.id,
            2,
            "ended",
            session.ended_at_wall.as_deref().unwrap(),
            Some(300_000),
            &end_payload,
            Some("bad-hash"),
        );
        events.push(SessionEvent {
            id: "event-2".into(),
            session_id: session.id.clone(),
            sequence: 2,
            event_type: "ended".into(),
            event_time_wall: session.ended_at_wall.clone().unwrap(),
            event_time_monotonic: Some(300_000),
            payload_json: end_payload,
            hash_prev: Some("bad-hash".into()),
            hash_self: Some(end_hash),
            signature: None,
        });

        assert_eq!(
            validate_event_chain(&events).as_deref(),
            Some("event_hash_invalid")
        );
    }
}
