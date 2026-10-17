// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Session event hashing, chain validation and trust scoring.

pub mod ledger;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::constants::SESSION_TIME_MAX_MS;
use crate::db::models::{Session, SessionEvent};
use crate::db::repo::session_events::{SESSION_EVENT_COLUMNS, row_to_session_event};
use crate::error::{Result, VaultimeError};

pub const STATUS_LOCAL: &str = "local";
pub const STATUS_SUSPICIOUS: &str = "suspicious";
pub const STATUS_RECOVERED: &str = "recovered";
/// Tracked, then corrected by the player with a reason.
pub const STATUS_EDITED: &str = "edited";
/// Added by the player, never tracked.
pub const STATUS_MANUAL: &str = "manual";

/// Events that close a session. A correction or a session added by hand
/// carries its end in the payload, the others at their own time.
const TERMINAL_EVENTS: [&str; 4] = ["ended", "recovered", "corrected", "added_manually"];
const TIMING_EVENTS: [&str; 6] = [
    "started",
    "heartbeat",
    "ended",
    "recovered",
    "corrected",
    "added_manually",
];
const STATUS_EVENTS: [&str; 7] = [
    "started",
    "integrity_flagged",
    "heartbeat",
    "ended",
    "recovered",
    "corrected",
    "added_manually",
];

/// A wall time in the format of session and event rows.
pub fn format_timestamp(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// The current wall time in the format of session and event rows.
pub fn now_timestamp() -> String {
    format_timestamp(Utc::now())
}

/// Appends a session event while maintaining the per-session hash chain,
/// and pins it in this PC's ledger: every event but checkpoints, and a
/// checkpoint every `CHECKPOINT_PIN_INTERVAL_SECS` of runtime.
pub fn append_session_event(
    conn: &Connection,
    session_id: &str,
    event_type: &str,
    event_time_wall: &str,
    event_time_monotonic: Option<i64>,
    payload_json: &str,
) -> Result<()> {
    // Decided before the checkpoint is in, from the one before it.
    let pinned = event_type != "heartbeat"
        || ledger::pins_checkpoint(conn, session_id, event_time_monotonic)?;
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
    .map_err(|error| {
        VaultimeError::Integrity(format!("failed to append session event: {error}"))
    })?;

    if !pinned {
        return Ok(());
    }
    ledger::pin(conn, session_id, next_sequence, &hash_self, event_type)
}

/// Validates the stored event chain and terminal payloads for a session.
///
/// Returns `Ok(None)` when the local audit trail looks internally consistent,
/// or `Ok(Some(reason))` when the session should be treated as suspicious.
pub fn validate_session_history(conn: &Connection, session: &Session) -> Result<Option<String>> {
    validate_session_history_from(conn, session, ledger::Viewpoint::ThisPc)
}

/// Takes in the sessions of a restored backup that this PC's own keys do
/// not cover yet: those that pass with the keys `trust` counts for the PC
/// that recorded them, and those this PC vouched for before, at a state its
/// pins in `vouched_before` confirm. A session that fails stays as it is
/// and shows Suspicious. Returns how many it took in.
pub(crate) fn adopt_sound_sessions(
    conn: &Connection,
    trust: &ledger::Trust,
    vouched_before: &HashMap<String, Vec<ledger::StoredPin>>,
    via: &str,
    backup_id: &str,
) -> Result<usize> {
    let sessions: Vec<Session> = {
        let failed = |error| VaultimeError::Integrity(format!("failed to read sessions: {error}"));
        let mut stmt = conn
            .prepare("SELECT * FROM sessions ORDER BY started_at_wall")
            .map_err(failed)?;
        let rows = stmt
            .query_map([], crate::db::repo::sessions::row_to_session)
            .map_err(failed)?;
        rows.collect::<rusqlite::Result<_>>().map_err(failed)?
    };
    // Checked first and taken in after, so the ledger check is done once.
    let mut sound = Vec::new();
    for session in &sessions {
        let events = load_session_events(conn, &session.id)?;
        let chain_sound = chain_problem(session, &events).is_none();
        let ledger_problem =
            |viewpoint| ledger::session_problem(conn, session, &events, &TIMING_EVENTS, viewpoint);
        if chain_sound && ledger_problem(ledger::Viewpoint::ThisPc)?.is_none() {
            continue;
        }
        let keys = trust.keys_for(&session.device_id);
        let passes = chain_sound && ledger_problem(ledger::Viewpoint::Keys(&keys))?.is_none();
        // Sound apart from the ledger, and at a state this PC pinned before.
        let known_here = !passes
            && chain_sound
            && vouched_before
                .get(&session.id)
                .is_some_and(|pins| pins_confirm(pins, session, &events));
        if (passes || known_here)
            && let Some(newest) = events.last()
            && let Some(hash) = newest.hash_self.clone()
        {
            sound.push((
                session.id.clone(),
                session.device_id.clone(),
                newest.sequence,
                hash,
            ));
        }
    }
    for (session_id, device, sequence, hash) in &sound {
        ledger::adopt(conn, session_id, *sequence, hash, device, via, backup_id)?;
    }
    Ok(sound.len())
}

/// Whether pins confirm a session: one of them lies within its chain, each
/// that does matches it, a closed session's time lies in a pinned part of
/// it, and its time runs at most one pin interval past its newest pinned
/// checkpoint, as `ledger::session_problem` asks. Pins past its end belong
/// to a later state.
fn pins_confirm(pins: &[ledger::StoredPin], session: &Session, events: &[SessionEvent]) -> bool {
    let mut within = Vec::new();
    let mut points = Vec::new();
    for pin in pins {
        let payload = parse_payload(&pin.2).unwrap_or(Value::Null);
        let (Some(sequence), Some(hash)) = (
            payload_i64(&payload, "sequence"),
            payload_str(&payload, "hash"),
        ) else {
            continue;
        };
        if usize::try_from(sequence).is_ok_and(|n| n >= 1 && n <= events.len()) {
            within.push(pin.clone());
            points.push((sequence, hash.to_owned()));
        }
    }
    let Some(newest_pinned) = points.iter().map(|(sequence, _)| *sequence).max() else {
        return false;
    };
    let matches = points.iter().all(|(sequence, hash)| {
        sequence
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| events.get(index))
            .is_some_and(|event| event.hash_self.as_deref() == Some(hash.as_str()))
    });
    let newest_timing = events
        .iter()
        .rev()
        .find(|event| TIMING_EVENTS.contains(&event.event_type.as_str()));
    let closed = session.ended_at_wall.is_some();
    let counters = [session.runtime_ms, session.active_ms, session.idle_ms];
    matches
        && !(closed && newest_timing.is_some_and(|event| event.sequence > newest_pinned))
        && ledger::pins_vouch_for_time(&within, events, counters)
}

/// `validate_session_history` with the pins of `viewpoint`'s keys counting,
/// as another PC would check the session. Used on a backup of that PC.
pub fn validate_session_history_from(
    conn: &Connection,
    session: &Session,
    viewpoint: ledger::Viewpoint,
) -> Result<Option<String>> {
    let events = load_session_events(conn, &session.id)?;
    if let Some(reason) = chain_problem(session, &events) {
        return Ok(Some(reason.into()));
    }
    Ok(
        ledger::session_problem(conn, session, &events, &TIMING_EVENTS, viewpoint)?
            .map(str::to_owned),
    )
}

/// Why a session disagrees with its own events, `events` in order, leaving
/// the ledgers aside.
fn chain_problem(session: &Session, events: &[SessionEvent]) -> Option<&'static str> {
    let (Some(first_event), Some(last_event)) = (events.first(), events.last()) else {
        return Some("missing_event_chain");
    };
    if let Some(problem) = validate_event_chain(events) {
        return Some(problem);
    }
    let fields: Vec<EventFields> = events
        .iter()
        .map(|event| EventFields::read(&event.payload_json))
        .collect();
    let added_by_hand = first_event.event_type == "added_manually";
    let problem = check_time_range(session, events, &fields)
        .or_else(|| check_after_close(events))
        .or_else(|| check_status_history(events, &fields, added_by_hand))
        .or_else(|| check_corrections(events));
    if problem.is_some() {
        return problem;
    }
    let start = match first_event.event_type.as_str() {
        "started" => Some(first_event.event_time_wall.clone()),
        "added_manually" => wall_in_payload(first_event, "started_at_wall"),
        _ => return Some("event_chain_missing_start"),
    };
    if start.as_deref() != Some(session.started_at_wall.as_str()) {
        return Some("session_start_mismatch");
    }
    // The first event names the game and the PC the time belongs to.
    for (key, value) in [
        ("game_id", &session.game_id),
        ("device_id", &session.device_id),
    ] {
        if wall_in_payload(first_event, key).is_some_and(|recorded| &recorded != value) {
            return Some("session_owner_mismatch");
        }
    }

    let last_is_terminal = TERMINAL_EVENTS.contains(&last_event.event_type.as_str());
    if let Some(ended_at_wall) = session.ended_at_wall.as_deref() {
        if !last_is_terminal {
            return Some("closed_session_missing_terminal_event");
        }

        let end = match last_event.event_type.as_str() {
            "corrected" | "added_manually" => wall_in_payload(last_event, "ended_at_wall"),
            // Recoveries from before the end moved into the payload ended at their own time.
            "recovered" => wall_in_payload(last_event, "ended_at_wall")
                .or_else(|| Some(last_event.event_time_wall.clone())),
            _ => Some(last_event.event_time_wall.clone()),
        };
        if end.as_deref() != Some(ended_at_wall) {
            return Some("session_end_mismatch");
        }
    } else if last_is_terminal {
        return Some("open_session_terminal_event_invalid");
    }

    // A chain that begins with a start holds a timing and a status event.
    let newest_of = |kinds: &[&str]| {
        events
            .iter()
            .rev()
            .find(|event| kinds.contains(&event.event_type.as_str()))
    };
    if !newest_of(&TIMING_EVENTS).is_some_and(|event| timing_event_matches_session(event, session))
    {
        return Some("session_timing_mismatch");
    }
    if !newest_of(&STATUS_EVENTS).is_some_and(|event| status_event_matches_session(event, session))
    {
        return Some("session_status_mismatch");
    }
    None
}

/// What the checks read from the payload of every event.
struct EventFields<'a> {
    /// Runtime, active and idle time, or `None` when they cannot be read.
    amounts: Option<[Option<i64>; 3]>,
    integrity_status: Option<Cow<'a, str>>,
}

impl<'a> EventFields<'a> {
    /// Reads both in one pass, since a chain check reads every payload. One
    /// that pass fails on is read again for each part alone, as a part may
    /// still be readable.
    fn read(payload_json: &'a str) -> Self {
        #[derive(serde::Deserialize)]
        #[expect(clippy::struct_field_names)]
        struct Amounts {
            runtime_ms: Option<i64>,
            active_ms: Option<i64>,
            idle_ms: Option<i64>,
        }
        #[derive(serde::Deserialize)]
        struct Both<'a> {
            runtime_ms: Option<i64>,
            active_ms: Option<i64>,
            idle_ms: Option<i64>,
            #[serde(borrow)]
            integrity_status: Option<Cow<'a, str>>,
        }
        match serde_json::from_str::<Both>(payload_json) {
            Ok(both) => Self {
                amounts: Some([both.runtime_ms, both.active_ms, both.idle_ms]),
                integrity_status: both.integrity_status,
            },
            Err(_) => Self {
                amounts: serde_json::from_str::<Amounts>(payload_json)
                    .ok()
                    .map(|amounts| [amounts.runtime_ms, amounts.active_ms, amounts.idle_ms]),
                integrity_status: status_in_payload(payload_json),
            },
        }
    }
}

/// Every amount of time a session and its events name lies between zero and
/// `SESSION_TIME_MAX_MS`. Others only come from a damaged or made up record.
fn check_time_range(
    session: &Session,
    events: &[SessionEvent],
    fields: &[EventFields],
) -> Option<&'static str> {
    let in_range = |value: i64| (0..=SESSION_TIME_MAX_MS).contains(&value);
    let row = [
        session.runtime_ms,
        session.active_ms,
        session.idle_ms,
        session.elapsed_monotonic_ms,
    ];
    let events_in_range = events.iter().zip(fields).all(|(event, fields)| {
        event.event_time_monotonic.is_none_or(in_range)
            && fields
                .amounts
                .is_none_or(|amounts| amounts.into_iter().flatten().all(in_range))
    });
    (!row.into_iter().all(in_range) || !events_in_range).then_some("time_out_of_range")
}

/// Once a session closed, only corrections follow, so nothing can add time
/// to it after its end.
fn check_after_close(events: &[SessionEvent]) -> Option<&'static str> {
    let closed = events
        .iter()
        .position(|event| TERMINAL_EVENTS.contains(&event.event_type.as_str()))?;
    events[closed + 1..]
        .iter()
        .any(|event| event.event_type != "corrected")
        .then_some("event_after_close")
}

/// A correction only ever takes time out. It starts from the counters the
/// session had just before it, lowers none of them below zero, raises none
/// of them and never moves the end later.
fn check_corrections(events: &[SessionEvent]) -> Option<&'static str> {
    if !events.iter().any(|event| event.event_type == "corrected") {
        return None;
    }
    let counters = |payload: &Value| -> Option<[i64; 3]> {
        Some([
            payload_i64(payload, "runtime_ms")?,
            payload_i64(payload, "active_ms")?,
            payload_i64(payload, "idle_ms")?,
        ])
    };
    let mut before: Option<[i64; 3]> = None;
    for event in events {
        let payload = parse_payload(&event.payload_json);
        match event.event_type.as_str() {
            "started" => before = Some([0; 3]),
            "heartbeat" | "ended" | "recovered" | "added_manually" => {
                before = payload.as_ref().and_then(counters);
            }
            "corrected" => {
                let Some(payload) = payload else {
                    return Some("correction_unreadable");
                };
                let after = counters(&payload);
                let previous = payload.get("previous");
                let from = previous.and_then(counters);
                let (Some(after), Some(from)) = (after, from) else {
                    return Some("correction_unreadable");
                };
                if before.is_some_and(|before| before != from) {
                    return Some("correction_previous_mismatch");
                }
                let new_end = payload_str(&payload, "ended_at_wall").and_then(parse_wall);
                let old_end = previous
                    .and_then(|previous| payload_str(previous, "ended_at_wall"))
                    .and_then(parse_wall);
                let ends_later = matches!((new_end, old_end), (Some(new), Some(old)) if new > old);
                if ends_later
                    || after
                        .iter()
                        .zip(from)
                        .any(|(after, from)| *after > from || *after < 0)
                {
                    return Some("correction_added_time");
                }
                before = Some(after);
            }
            _ => {}
        }
    }
    None
}

fn parse_wall(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

/// A Suspicious flag stays once it is set, and only a session added by hand
/// may carry the Manual label, always unless it was flagged.
fn check_status_history(
    events: &[SessionEvent],
    fields: &[EventFields],
    added_by_hand: bool,
) -> Option<&'static str> {
    let mut flagged = false;
    for (event, fields) in events
        .iter()
        .zip(fields)
        .filter(|(event, _)| STATUS_EVENTS.contains(&event.event_type.as_str()))
    {
        let status = match event.event_type.as_str() {
            "started" => STATUS_LOCAL,
            "integrity_flagged" => STATUS_SUSPICIOUS,
            _ => match fields.integrity_status.as_deref() {
                Some(status) => status,
                None => return Some("session_status_missing"),
            },
        };
        if flagged && status != STATUS_SUSPICIOUS {
            return Some("suspicious_flag_dropped");
        }
        if status == STATUS_MANUAL && !added_by_hand {
            return Some("manual_label_on_tracked_session");
        }
        if added_by_hand && status != STATUS_MANUAL && status != STATUS_SUSPICIOUS {
            return Some("tracked_label_on_manual_session");
        }
        flagged |= status == STATUS_SUSPICIOUS;
    }
    None
}

/// What a closed session was last checked against: the tail of its chain and
/// the row values the check compares.
#[derive(PartialEq)]
struct CheckedState {
    events: i64,
    last_hash: Option<String>,
    row: String,
}

type CheckCache = Mutex<HashMap<String, (CheckedState, Option<String>)>>;

fn check_cache() -> &'static CheckCache {
    static CACHE: OnceLock<CheckCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Validates a session like `validate_session_history`, but reuses the result
/// for a closed session whose chain tail and row did not change since it was
/// last checked. Listing every session then stays fast with long histories.
/// The cache lives in memory, so every start of the app checks each chain in
/// full again. Returns the reason and whether it was checked just now.
pub fn validate_session_history_cached(
    conn: &Connection,
    session: &Session,
) -> Result<(Option<String>, bool)> {
    if session.ended_at_wall.is_none() {
        return Ok((validate_session_history(conn, session)?, true));
    }
    forget_checks_after_outside_writes(conn)?;
    // The newest event, found through the index. A check passes only when
    // the sequence runs from 1 without gaps, so its number is the count.
    let (events, last_hash) = conn
        .prepare_cached(
            "SELECT sequence, hash_self FROM session_events
             WHERE session_id = ?1 ORDER BY sequence DESC LIMIT 1",
        )
        .and_then(|mut stmt| {
            stmt.query_row([&session.id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .optional()
        })
        .map_err(|error| {
            VaultimeError::Integrity(format!("failed to read the event chain: {error}"))
        })?
        .unwrap_or((0, None));
    let state = CheckedState {
        events,
        last_hash,
        row: format!(
            "{}|{:?}|{}|{}|{}|{}|{}",
            session.started_at_wall,
            session.ended_at_wall,
            session.runtime_ms,
            session.active_ms,
            session.idle_ms,
            session.integrity_status,
            session.closed_cleanly
        ),
    };
    let cache = check_cache();
    if let Some((checked, reason)) = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&session.id)
        && *checked == state
    {
        return Ok((reason.clone(), false));
    }
    let reason = validate_session_history(conn, session)?;
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(session.id.clone(), (state, reason.clone()));
    Ok((reason, true))
}

/// Forgets every remembered check, for when the app itself replaced the
/// history, as a restore does, so each chain is checked again.
pub fn forget_checks() {
    check_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    ledger::forget();
}

/// Clears the cache when another program wrote to the database, such as a
/// database tool, so its changes are checked again right away.
fn forget_checks_after_outside_writes(conn: &Connection) -> Result<()> {
    static SEEN_VERSION: Mutex<Option<i64>> = Mutex::new(None);
    let version: i64 = conn
        .prepare_cached("PRAGMA data_version")
        .and_then(|mut stmt| stmt.query_row([], |row| row.get(0)))
        .map_err(|error| {
            VaultimeError::Integrity(format!("failed to read the data version: {error}"))
        })?;
    let mut seen = SEEN_VERSION.lock().unwrap_or_else(PoisonError::into_inner);
    if seen.is_some_and(|seen| seen != version) {
        forget_checks();
    }
    *seen = Some(version);
    Ok(())
}

fn next_sequence_and_previous_hash(
    conn: &Connection,
    session_id: &str,
) -> Result<(i64, Option<String>)> {
    let row = conn
        .prepare_cached(
            "SELECT sequence, hash_self
             FROM session_events
             WHERE session_id = ?1
             ORDER BY sequence DESC
             LIMIT 1",
        )
        .and_then(|mut stmt| {
            stmt.query_row([session_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .optional()
        })
        .map_err(|error| {
            VaultimeError::Integrity(format!("failed to query event chain: {error}"))
        })?;

    Ok(match row {
        Some((last_sequence, last_hash)) => (last_sequence + 1, last_hash),
        None => (1, None),
    })
}

pub(crate) fn compute_event_hash(
    session_id: &str,
    sequence: i64,
    event_type: &str,
    event_time_wall: &str,
    event_time_monotonic: Option<i64>,
    payload_json: &str,
    previous_hash: Option<&str>,
) -> String {
    crate::hex::encode(&event_digest(
        session_id,
        sequence,
        event_type,
        event_time_wall,
        event_time_monotonic,
        payload_json,
        previous_hash,
    ))
}

/// `compute_event_hash` before it is written as hex.
fn event_digest(
    session_id: &str,
    sequence: i64,
    event_type: &str,
    event_time_wall: &str,
    event_time_monotonic: Option<i64>,
    payload_json: &str,
    previous_hash: Option<&str>,
) -> [u8; 32] {
    // Numbers are written in decimal without a buffer on the heap, as a
    // chain check hashes every event.
    fn update_number(hasher: &mut Sha256, value: i64) {
        // `i64::MIN` takes 20 characters.
        let mut buffer = [0_u8; 20];
        let mut cursor = std::io::Cursor::new(&mut buffer[..]);
        match std::io::Write::write_fmt(&mut cursor, format_args!("{value}")) {
            Ok(()) => {
                let written = usize::try_from(cursor.position()).unwrap_or_default();
                hasher.update(&buffer[..written]);
            }
            Err(_) => hasher.update(value.to_string().as_bytes()),
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(b"\n");
    update_number(&mut hasher, sequence);
    hasher.update(b"\n");
    hasher.update(event_type.as_bytes());
    hasher.update(b"\n");
    hasher.update(event_time_wall.as_bytes());
    hasher.update(b"\n");
    if let Some(monotonic) = event_time_monotonic {
        update_number(&mut hasher, monotonic);
    }
    hasher.update(b"\n");
    hasher.update(payload_json.as_bytes());
    hasher.update(b"\n");
    hasher.update(previous_hash.unwrap_or_default().as_bytes());
    hasher.finalize().into()
}

/// Whether `text` is `digest` in lowercase hex, as `crate::hex` writes it.
fn is_hex_of(text: &str, digest: &[u8]) -> bool {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    text.len() == digest.len() * 2
        && text
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .zip(digest)
            .all(|([high, low], byte)| {
                *high == DIGITS[usize::from(byte >> 4)] && *low == DIGITS[usize::from(byte & 0x0f)]
            })
}

fn validate_event_chain(events: &[SessionEvent]) -> Option<&'static str> {
    let mut previous_hash: Option<&str> = None;

    for (index, event) in events.iter().enumerate() {
        let expected_sequence = (index as i64) + 1;
        if event.sequence != expected_sequence {
            return Some("event_sequence_gap");
        }

        if event.hash_prev.as_deref() != previous_hash {
            return Some("event_previous_hash_mismatch");
        }

        let expected = event_digest(
            &event.session_id,
            event.sequence,
            &event.event_type,
            &event.event_time_wall,
            event.event_time_monotonic,
            &event.payload_json,
            previous_hash,
        );
        let Some(hash) = event
            .hash_self
            .as_deref()
            .filter(|hash| is_hex_of(hash, &expected))
        else {
            return Some("event_hash_invalid");
        };

        previous_hash = Some(hash);
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
        "heartbeat" | "ended" | "recovered" | "corrected" | "added_manually" => {
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
        "heartbeat" | "ended" | "recovered" | "corrected" | "added_manually" => {
            let Some(payload) = parse_payload(&event.payload_json) else {
                return false;
            };

            let status_matches = payload_str(&payload, "integrity_status")
                == Some(session.integrity_status.as_str());

            if TERMINAL_EVENTS.contains(&event.event_type.as_str()) {
                return status_matches
                    && payload_bool(&payload, "closed_cleanly") == Some(session.closed_cleanly);
            }

            status_matches
        }
        _ => true,
    }
}

fn wall_in_payload(event: &SessionEvent, key: &str) -> Option<String> {
    parse_payload(&event.payload_json)
        .and_then(|payload| payload_str(&payload, key).map(str::to_owned))
}

/// The events of a session, oldest first.
pub(crate) fn load_session_events(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<SessionEvent>> {
    static QUERY: LazyLock<String> = LazyLock::new(|| {
        format!(
            "SELECT {SESSION_EVENT_COLUMNS} FROM session_events se
             WHERE se.session_id = ?1
             ORDER BY se.sequence ASC"
        )
    });
    let mut stmt = conn.prepare_cached(&QUERY).map_err(|error| {
        VaultimeError::Integrity(format!("failed to prepare session event query: {error}"))
    })?;

    let rows = stmt
        .query_map([session_id], row_to_session_event)
        .map_err(|error| {
            VaultimeError::Integrity(format!("failed to query session events: {error}"))
        })?;

    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|error| {
        VaultimeError::Integrity(format!("failed to collect session events: {error}"))
    })
}

/// The status a payload names, read without building the whole payload.
fn status_in_payload(payload_json: &str) -> Option<Cow<'_, str>> {
    #[derive(serde::Deserialize)]
    struct Status<'a> {
        #[serde(borrow)]
        integrity_status: Option<Cow<'a, str>>,
    }
    serde_json::from_str::<Status>(payload_json)
        .ok()?
        .integrity_status
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
    fn event_hash_writes_numbers_in_decimal() {
        for (sequence, monotonic) in [(1, None), (42, Some(0)), (i64::MAX, Some(i64::MIN))] {
            let previous = (sequence > 1).then_some("previous");
            let mut hasher = Sha256::new();
            let monotonic_text = monotonic.map(|value: i64| value.to_string());
            for part in [
                "session",
                &sequence.to_string(),
                "heartbeat",
                "2026-01-01T00:00:00.000Z",
                monotonic_text.as_deref().unwrap_or_default(),
                "{}",
            ] {
                hasher.update(part.as_bytes());
                hasher.update(b"\n");
            }
            hasher.update(previous.unwrap_or_default().as_bytes());
            let expected = crate::hex::encode(&hasher.finalize());
            let hash = compute_event_hash(
                "session",
                sequence,
                "heartbeat",
                "2026-01-01T00:00:00.000Z",
                monotonic,
                "{}",
                previous,
            );
            assert_eq!(hash, expected);
            assert!(is_hex_of(&hash, &crate::hex::decode(&expected).unwrap()));
            assert!(!is_hex_of(
                &hash.to_uppercase(),
                &crate::hex::decode(&expected).unwrap()
            ));
        }
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
            set_aside_ms: 0,
            set_aside_active_ms: 0,
            set_aside_idle_ms: 0,
            copy_overlap_ms: 0,
            copy_overlap_active_ms: 0,
            copy_overlap_idle_ms: 0,
            recorded_game_id: None,
            still_running_on: None,
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

        let end_payload = r#"{"runtime_ms":300000,"active_ms":240000,"idle_ms":60000,"integrity_status":"local","closed_cleanly":true}"#.to_string();
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

        assert_eq!(validate_event_chain(&events), Some("event_hash_invalid"));
    }
}
