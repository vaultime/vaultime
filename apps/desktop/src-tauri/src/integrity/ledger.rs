// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ledger of each PC: a hash chain of the moments that shape its
//! sessions, signed with a key that never leaves the PC. Each entry pins a
//! session's event chain at one event, so a session rewritten or removed
//! outside Vaultime shows even when its own chain was rebuilt to match. A
//! backup carries the ledger but not the key, so a backup changed elsewhere
//! cannot sign again. Like the session chains, it scores integrity and
//! cannot prevent changes on the PC that holds the key. Dropping the newest
//! entries together with their sessions leaves a shorter ledger that still
//! holds, which only a copy kept elsewhere, like a later backup, can show.
//!
//! On this PC only its own keys count, listed in a file next to its key.
//! Every session it holds carries a pin of one of them: written here, carried
//! in when the ledger began, or taken in from a backup of another PC after it
//! passed its check as that PC sees it. That PC's keys are the ones its
//! ledgers name in their first entry, trusted on the first merge and then
//! recorded in this PC's own ledger, so a key that later claims to be that
//! PC does not count.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock, PoisonError};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use log::warn;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::constants::{DEVICE_KEY_FILE, DEVICE_KEYS_FILE, LEDGER_CHECKS_REMEMBERED};
use crate::db::models::{Session, SessionEvent};
use crate::error::{Result, VaultimeError};
use crate::hex;
use crate::integrity::now_timestamp;

/// The first entry of every ledger. It names the PC the key belongs to.
const ENTRY_BEGAN: &str = "began";
/// A session's chain up to one event, as this PC wrote it.
const ENTRY_PINNED: &str = "pinned";
/// A session's chain as it was when the ledger began, so older sessions are
/// covered too. It vouches for nothing before that moment.
const ENTRY_CARRIED: &str = "carried";
/// A session this PC took in from a backup, at the state it had then, after
/// it passed its check as the PC the backup came from sees it.
const ENTRY_ADOPTED: &str = "adopted";
/// A session the player removed, with the reason.
const ENTRY_REMOVED: &str = "removed";
/// The history was replaced by a backup. Names where this PC's ledger stood.
const ENTRY_RESTORED: &str = "restored";
/// A key of another PC that this PC trusts, from its first merge on.
const ENTRY_TRUSTED: &str = "trusted";

/// The field of a session's start that names how often its checkpoints are
/// pinned, in seconds. Sessions from before checkpoint pins lack it.
pub(crate) const CHECKPOINT_PIN_FIELD: &str = "checkpoint_pin_secs";
/// Events whose counters a pin vouches for: the start at zero, checkpoints,
/// the end and a session added by hand.
const COUNTER_EVENTS: [&str; 4] = ["started", "heartbeat", "ended", "added_manually"];
/// Events that state counters an earlier event or the player gave, so their
/// pins vouch for no time: a recovery takes the last checkpoint's counters
/// and a correction the player's.
const RESTATING_EVENTS: [&str; 2] = ["recovered", "corrected"];

/// A fresh install took over the PC whose backup it restored, see
/// `Takeover`. Names that PC, the PC this one was before and the keys it
/// counts as its own from then on.
const ENTRY_TOOK_OVER: &str = "took_over";

static KEY: OnceLock<SigningKey> = OnceLock::new();
/// The device id this PC's ledger names, which a takeover changes.
#[cfg(not(test))]
static DEVICE: Mutex<Option<String>> = Mutex::new(None);
/// Every key this PC counts as its own besides the current one.
#[cfg(not(test))]
static OWN_KEYS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Loads this PC's signing key from `app_dir`, or makes one on the first
/// start, and remembers the device id its ledger names. Backups leave the
/// key file out, so the key stays on this PC. A damaged file is set aside
/// for a new key, and the ledger goes on under that one. Its older entries
/// stay checkable, as each names the key it was signed with.
pub fn load_key(app_dir: &Path, device_id: &str) -> Result<()> {
    let path = app_dir.join(DEVICE_KEY_FILE);
    let saved = match fs::read_to_string(&path) {
        Ok(saved) => Some(saved),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(VaultimeError::Integrity(format!(
                "failed to read {}: {error}",
                path.display()
            )));
        }
    };
    let parsed = saved.as_deref().and_then(|saved| {
        hex::decode(saved.trim()).and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
    });
    let seed = if let Some(seed) = parsed {
        seed
    } else {
        if saved.is_some() {
            let aside = path.with_extension("damaged");
            warn!(
                "{} is damaged, set aside as {}",
                path.display(),
                aside.display()
            );
            fs::rename(&path, &aside).map_err(|error| {
                VaultimeError::Integrity(format!("failed to set {} aside: {error}", path.display()))
            })?;
        }
        let seed = random_seed()?;
        write_private(&path, &hex::encode(&seed))?;
        seed
    };
    let key = SigningKey::from_bytes(&seed);
    let own = remember_own_key(app_dir, &key)?;
    let _ = KEY.set(key);
    #[cfg(not(test))]
    {
        *OWN_KEYS.lock().unwrap_or_else(PoisonError::into_inner) = Some(own);
        *DEVICE.lock().unwrap_or_else(PoisonError::into_inner) = Some(device_id.to_owned());
    }
    #[cfg(test)]
    let _ = (own, device_id);
    Ok(())
}

/// Adds the public half of `key` to the list of keys this PC has signed
/// with, and returns that list.
fn remember_own_key(app_dir: &Path, key: &SigningKey) -> Result<HashSet<String>> {
    let path = app_dir.join(DEVICE_KEYS_FILE);
    let listed = match fs::read_to_string(&path) {
        Ok(listed) => listed,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(VaultimeError::Integrity(format!(
                "failed to read {}: {error}",
                path.display()
            )));
        }
    };
    let mut keys: HashSet<String> = listed
        .lines()
        .map(str::trim)
        .filter(|line| hex::decode(line).is_some_and(|bytes| bytes.len() == 32))
        .map(str::to_owned)
        .collect();
    let current = hex::encode(key.verifying_key().as_bytes());
    if keys.insert(current.clone()) {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| {
                VaultimeError::Integrity(format!("failed to open {}: {error}", path.display()))
            })?;
        std::io::Write::write_all(&mut file, format!("{current}\n").as_bytes()).map_err(
            |error| {
                VaultimeError::Integrity(format!("failed to write {}: {error}", path.display()))
            },
        )?;
    }
    Ok(keys)
}

fn random_seed() -> Result<[u8; 32]> {
    let mut seed = [0; 32];
    getrandom::fill(&mut seed)
        .map_err(|error| VaultimeError::Integrity(format!("no randomness for a key: {error}")))?;
    Ok(seed)
}

fn write_private(path: &Path, contents: &str) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path).map_err(|error| {
        VaultimeError::Integrity(format!("failed to create {}: {error}", path.display()))
    })?;
    std::io::Write::write_all(&mut file, contents.as_bytes()).map_err(|error| {
        VaultimeError::Integrity(format!("failed to write {}: {error}", path.display()))
    })
}

/// This PC's signing key and device id.
#[cfg(not(test))]
fn identity() -> Result<(SigningKey, String)> {
    let key = KEY
        .get()
        .ok_or_else(|| VaultimeError::Integrity("this PC's ledger key is not loaded".into()))?;
    let device = DEVICE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or_else(|| VaultimeError::Integrity("this PC's device id is not known".into()))?;
    Ok((key.clone(), device))
}

#[cfg(test)]
thread_local! {
    /// The PC a test acts as, so one test can play several PCs.
    static TEST_PC: std::cell::RefCell<Option<(SigningKey, String)>> =
        const { std::cell::RefCell::new(None) };
    /// What test PCs took over, by their key: the device id they continue
    /// as and the keys they count as their own.
    static TEST_TAKEN: std::cell::RefCell<HashMap<String, (String, HashSet<String>)>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Tests share one key, made up on first use, unless they act as a PC of
/// their own.
#[cfg(test)]
fn identity() -> Result<(SigningKey, String)> {
    let (key, device) = TEST_PC.with(|pc| pc.borrow().clone()).unwrap_or_else(|| {
        let key = KEY.get_or_init(|| SigningKey::from_bytes(&random_seed().unwrap()));
        (key.clone(), "test-pc".to_owned())
    });
    let taken = TEST_TAKEN.with(|taken| {
        taken
            .borrow()
            .get(&hex::encode(key.verifying_key().as_bytes()))
            .map(|(device, _)| device.clone())
    });
    Ok((key, taken.unwrap_or(device)))
}

/// A PC for tests, with a key of its own.
#[cfg(test)]
pub(crate) fn test_pc(device_id: &str) -> (SigningKey, String) {
    (
        SigningKey::from_bytes(&random_seed().unwrap()),
        device_id.to_owned(),
    )
}

/// Makes this thread act as `pc`, or as the shared test PC again.
#[cfg(test)]
pub(crate) fn act_as(pc: Option<&(SigningKey, String)>) {
    TEST_PC.with(|current| *current.borrow_mut() = pc.cloned());
}

/// This PC's public key in hex, which names its ledger.
pub fn key_id() -> Result<String> {
    Ok(hex::encode(identity()?.0.verifying_key().as_bytes()))
}

/// Every key this PC counts as its own: the ones it has signed with, the
/// current one included, and the ones it took over.
#[cfg(not(test))]
pub(crate) fn own_keys() -> Result<HashSet<String>> {
    let mut keys = OWN_KEYS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_default();
    keys.insert(key_id()?);
    Ok(keys)
}

/// A test PC signs with one key only, and counts the ones it took over.
#[cfg(test)]
pub(crate) fn own_keys() -> Result<HashSet<String>> {
    let key = key_id()?;
    let mut keys = TEST_TAKEN.with(|taken| {
        taken
            .borrow()
            .get(&key)
            .map(|(_, keys)| keys.clone())
            .unwrap_or_default()
    });
    keys.insert(key);
    Ok(keys)
}

/// The device id this PC's ledger names.
pub fn this_device() -> Result<String> {
    Ok(identity()?.1)
}

/// Makes this PC continue as `device_id`, counting `keys` as its own too,
/// for as long as this process runs. Returns what it was before.
#[cfg(not(test))]
fn switch_identity(device_id: &str, keys: &HashSet<String>) -> Result<(String, HashSet<String>)> {
    let before = (this_device()?, own_keys()?);
    let mut own = before.1.clone();
    own.extend(keys.iter().cloned());
    *OWN_KEYS.lock().unwrap_or_else(PoisonError::into_inner) = Some(own);
    *DEVICE.lock().unwrap_or_else(PoisonError::into_inner) = Some(device_id.to_owned());
    Ok(before)
}

#[cfg(not(test))]
fn restore_identity(before: (String, HashSet<String>)) {
    *OWN_KEYS.lock().unwrap_or_else(PoisonError::into_inner) = Some(before.1);
    *DEVICE.lock().unwrap_or_else(PoisonError::into_inner) = Some(before.0);
}

#[cfg(test)]
fn switch_identity(device_id: &str, keys: &HashSet<String>) -> Result<(String, HashSet<String>)> {
    let before = (this_device()?, own_keys()?);
    let mut own = before.1.clone();
    own.extend(keys.iter().cloned());
    let key = key_id()?;
    TEST_TAKEN.with(|taken| taken.borrow_mut().insert(key, (device_id.to_owned(), own)));
    Ok(before)
}

#[cfg(test)]
fn restore_identity(before: (String, HashSet<String>)) {
    if let Ok(key) = key_id() {
        TEST_TAKEN.with(|taken| taken.borrow_mut().insert(key, before));
    }
}

fn entry_hash(
    key_id: &str,
    sequence: i64,
    entry_type: &str,
    session_id: Option<&str>,
    recorded_at: &str,
    payload_json: &str,
    previous_hash: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    for field in [
        key_id,
        &sequence.to_string(),
        entry_type,
        session_id.unwrap_or_default(),
        recorded_at,
        payload_json,
        previous_hash.unwrap_or_default(),
    ] {
        hasher.update(field.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(&hasher.finalize())
}

/// The newest entry of a ledger: its sequence and hash.
fn tail(conn: &Connection, key_id: &str) -> Result<Option<(i64, String)>> {
    conn.prepare_cached(
        "SELECT sequence, hash_self FROM ledger_entries
         WHERE key_id = ?1 ORDER BY sequence DESC LIMIT 1",
    )
    .and_then(|mut stmt| {
        stmt.query_row([key_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()
    })
    .map_err(ledger_error)
}

fn ledger_error(error: rusqlite::Error) -> VaultimeError {
    VaultimeError::Integrity(format!("ledger: {error}"))
}

/// Appends an entry to this PC's ledger and signs it. A ledger that has no
/// entries yet first gets the one that names this PC.
fn append(
    conn: &Connection,
    entry_type: &str,
    session_id: Option<&str>,
    payload: &Value,
) -> Result<()> {
    let (key, device) = identity()?;
    let key_id = hex::encode(key.verifying_key().as_bytes());
    let mut last = tail(conn, &key_id)?;
    if last.is_none() && entry_type != ENTRY_BEGAN {
        write_entry(
            conn,
            &key,
            &key_id,
            None,
            ENTRY_BEGAN,
            None,
            &json!({ "device_id": device, "reason": "first_entry" }),
        )?;
        last = tail(conn, &key_id)?;
    }
    write_entry(conn, &key, &key_id, last, entry_type, session_id, payload)
}

fn write_entry(
    conn: &Connection,
    key: &SigningKey,
    key_id: &str,
    last: Option<(i64, String)>,
    entry_type: &str,
    session_id: Option<&str>,
    payload: &Value,
) -> Result<()> {
    let (sequence, previous) = match last {
        Some((sequence, hash)) => (sequence + 1, Some(hash)),
        None => (1, None),
    };
    let recorded_at = now_timestamp();
    let payload_json = payload.to_string();
    let hash = entry_hash(
        key_id,
        sequence,
        entry_type,
        session_id,
        &recorded_at,
        &payload_json,
        previous.as_deref(),
    );
    let signature = hex::encode(&key.sign(hash.as_bytes()).to_bytes());
    conn.execute(
        "INSERT INTO ledger_entries
            (key_id, sequence, entry_type, session_id, recorded_at, payload_json,
             hash_prev, hash_self, signature)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            key_id,
            sequence,
            entry_type,
            session_id,
            recorded_at,
            payload_json,
            previous,
            hash,
            signature
        ],
    )
    .map_err(ledger_error)?;
    Ok(())
}

/// Pins a session's chain at an event just appended. See
/// `pins_checkpoint` for the checkpoints it pins.
pub(crate) fn pin(
    conn: &Connection,
    session_id: &str,
    sequence: i64,
    hash: &str,
    event_type: &str,
) -> Result<()> {
    append(
        conn,
        ENTRY_PINNED,
        Some(session_id),
        &json!({ "sequence": sequence, "hash": hash, "event_type": event_type }),
    )
}

/// Whether a checkpoint about to be appended with `runtime_ms` gets pinned:
/// when its session's start names a pin interval and the checkpoint's
/// runtime lies in another stretch of it than the start or checkpoint before
/// it. The newest pinned checkpoint then always lies less than one interval
/// behind the newest one.
pub(crate) fn pins_checkpoint(
    conn: &Connection,
    session_id: &str,
    runtime_ms: Option<i64>,
) -> Result<bool> {
    let start: Option<String> = conn
        .prepare_cached(
            "SELECT payload_json FROM session_events
             WHERE session_id = ?1 AND sequence = 1 AND event_type = 'started'",
        )
        .and_then(|mut stmt| stmt.query_row([session_id], |row| row.get(0)).optional())
        .map_err(ledger_error)?;
    let Some(interval_ms) = start.as_deref().and_then(interval_in_start) else {
        return Ok(false);
    };
    let previous: Option<Option<i64>> = conn
        .prepare_cached(
            "SELECT event_time_monotonic FROM session_events
             WHERE session_id = ?1 AND event_type IN ('started', 'heartbeat')
             ORDER BY sequence DESC LIMIT 1",
        )
        .and_then(|mut stmt| stmt.query_row([session_id], |row| row.get(0)).optional())
        .map_err(ledger_error)?;
    let stretch = |ms: i64| ms.div_euclid(interval_ms);
    Ok(match (previous.flatten(), runtime_ms) {
        (Some(previous), Some(runtime)) => stretch(runtime) != stretch(previous),
        _ => true,
    })
}

/// The pin interval a session's start names, in milliseconds.
fn interval_in_start(payload_json: &str) -> Option<i64> {
    let payload: Value = serde_json::from_str(payload_json).ok()?;
    payload
        .get(CHECKPOINT_PIN_FIELD)?
        .as_i64()?
        .checked_mul(1_000)
        .filter(|interval| *interval > 0)
}

/// The counters an event states, the start at zero.
fn counters_of(event: &SessionEvent) -> Option<[i64; 3]> {
    if !COUNTER_EVENTS.contains(&event.event_type.as_str()) {
        return None;
    }
    if event.event_type == "started" {
        return Some([0; 3]);
    }
    let payload: Value = serde_json::from_str(&event.payload_json).ok()?;
    let read = |key: &str| payload.get(key).and_then(Value::as_i64);
    Some([read("runtime_ms")?, read("active_ms")?, read("idle_ms")?])
}

/// How far a session's time may run past what its pins vouch for: the
/// counters of its newest event with counters up to the newest pinned one,
/// when that was, and the interval its start names. None for a session from
/// before checkpoint pins, whose time counts as recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TailLimit {
    pub counters: [i64; 3],
    pub wall: String,
    pub interval_ms: i64,
}

impl TailLimit {
    /// Works out the limit from `events`, in order, of which the pins vouch
    /// for those up to `vouched_through`.
    pub(crate) fn of(events: &[SessionEvent], vouched_through: i64) -> Option<Self> {
        let first = events
            .first()
            .filter(|event| event.event_type == "started")?;
        let interval_ms = interval_in_start(&first.payload_json)?;
        let (wall, counters) = events
            .iter()
            .rev()
            .filter(|event| event.sequence <= vouched_through)
            .find_map(|event| counters_of(event).map(|counters| (&event.event_time_wall, counters)))
            .unwrap_or((&first.event_time_wall, [0; 3]));
        Some(Self {
            counters,
            wall: wall.clone(),
            interval_ms,
        })
    }

    /// Whether any of `counters` runs more than the interval past the
    /// vouched ones.
    pub(crate) fn exceeded_by(&self, counters: [i64; 3]) -> bool {
        counters
            .iter()
            .zip(self.counters)
            .any(|(value, vouched)| *value > vouched.saturating_add(self.interval_ms))
    }

    /// `counters` with the time past the vouched ones shrunk alike until
    /// none runs more than the interval past them.
    pub(crate) fn cap(&self, counters: [i64; 3]) -> [i64; 3] {
        let tail: [i64; 3] = std::array::from_fn(|index| {
            counters[index].saturating_sub(self.counters[index]).max(0)
        });
        let longest = tail.iter().copied().max().unwrap_or(0);
        if longest <= self.interval_ms {
            return counters;
        }
        std::array::from_fn(|index| {
            let shrunk =
                i128::from(tail[index]) * i128::from(self.interval_ms) / i128::from(longest);
            self.counters[index]
                .saturating_add(i64::try_from(shrunk).unwrap_or(self.interval_ms))
                .min(counters[index])
        })
    }
}

/// A pin as stored: the key that signed it, its entry type and payload.
pub(crate) type StoredPin = (String, String, String);

/// The event a pin's payload names, by its sequence and hash.
pub(crate) fn pin_point(payload: &Value) -> Option<(i64, &str)> {
    Some((
        payload.get("sequence")?.as_i64()?,
        payload.get("hash")?.as_str()?,
    ))
}

/// Whether `events`, in order, hold the event at `sequence` with `hash`.
pub(crate) fn pin_matches(events: &[SessionEvent], sequence: i64, hash: &str) -> bool {
    sequence
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| events.get(index))
        .is_some_and(|event| event.hash_self.as_deref() == Some(hash))
}

/// The newest event of `events` that `pins` vouch for, other than by a
/// recovery or a correction. A pin at an event vouches for every event
/// before it, as each event's hash covers the one before.
fn vouched_through<'a>(
    pins: impl IntoIterator<Item = &'a StoredPin>,
    events: &[SessionEvent],
) -> i64 {
    let mut newest = 0;
    for (_, entry_type, payload_json) in pins {
        let payload: Value = serde_json::from_str(payload_json).unwrap_or(Value::Null);
        let Some((sequence, hash)) = pin_point(&payload) else {
            continue;
        };
        let restating = entry_type == ENTRY_PINNED
            && payload
                .get("event_type")
                .and_then(Value::as_str)
                .is_some_and(|event_type| RESTATING_EVENTS.contains(&event_type));
        if pin_matches(events, sequence, hash) && !restating {
            newest = newest.max(sequence);
        }
    }
    newest
}

/// Whether `pins` vouch for a session's time: none of `counters` runs more
/// than its start's pin interval past what they pin, see `TailLimit`.
pub(crate) fn pins_vouch_for_time(
    pins: &[StoredPin],
    events: &[SessionEvent],
    counters: [i64; 3],
) -> bool {
    TailLimit::of(events, vouched_through(pins, events))
        .is_none_or(|limit| !limit.exceeded_by(counters))
}

/// The pins of a session as stored.
fn session_pins(conn: &Connection, session_id: &str) -> Result<Vec<StoredPin>> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT key_id, entry_type, payload_json FROM ledger_entries
             WHERE session_id = ?1 AND entry_type IN ('pinned', 'carried', 'adopted')",
        )
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([session_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(ledger_error)?;
    rows.collect::<rusqlite::Result<_>>().map_err(ledger_error)
}

/// The limit this PC's own pins set for a session of `events`, see
/// `TailLimit`.
pub(crate) fn own_tail_limit(
    conn: &Connection,
    session_id: &str,
    events: &[SessionEvent],
) -> Result<Option<TailLimit>> {
    let mine = own_keys()?;
    let pins = session_pins(conn, session_id)?;
    let through = vouched_through(pins.iter().filter(|(key, _, _)| mine.contains(key)), events);
    Ok(TailLimit::of(events, through))
}

/// Vouches for a session taken in from a backup, at the event `sequence`
/// with `hash`, the newest one its check saw. Call it only for a session
/// that passed that check.
pub(crate) fn adopt(
    conn: &Connection,
    session_id: &str,
    sequence: i64,
    hash: &str,
    from_device: &str,
    via: &str,
    backup_id: &str,
) -> Result<()> {
    append(
        conn,
        ENTRY_ADOPTED,
        Some(session_id),
        &json!({
            "sequence": sequence,
            "hash": hash,
            "from_device": from_device,
            "via": via,
            "backup_id": backup_id,
        }),
    )
}

/// Notes that the player removed a session, so its absence is explained.
pub(crate) fn record_removed(conn: &Connection, session_id: &str, reason: &str) -> Result<()> {
    append(
        conn,
        ENTRY_REMOVED,
        Some(session_id),
        &json!({ "reason": reason }),
    )
}

/// Whether this PC's ledger notes that the player removed the session here.
pub(crate) fn removed_here(conn: &Connection, session_id: &str) -> Result<bool> {
    let mine = own_keys()?;
    let mut stmt = conn
        .prepare_cached(
            "SELECT key_id FROM ledger_entries WHERE session_id = ?1 AND entry_type = 'removed'",
        )
        .map_err(ledger_error)?;
    let keys = stmt
        .query_map([session_id], |row| row.get::<_, String>(0))
        .map_err(ledger_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(ledger_error)?;
    Ok(keys.iter().any(|key| mine.contains(key)))
}

/// Begins the ledger when it holds no entries at all, which is on the first
/// start of a version with ledgers and after restoring a backup from before
/// them. Every session gets a carried entry for its chain as it is now.
/// Returns how many sessions it carried, or `None` when the ledger had begun.
pub fn begin_if_empty(conn: &Connection, reason: &str) -> Result<Option<usize>> {
    let any: Option<i64> = conn
        .query_row("SELECT 1 FROM ledger_entries LIMIT 1", [], |row| row.get(0))
        .optional()
        .map_err(ledger_error)?;
    if any.is_some() {
        return Ok(None);
    }
    let tails: Vec<(String, i64, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT e.session_id, e.sequence, e.hash_self
                 FROM session_events e
                 JOIN (SELECT session_id, MAX(sequence) AS newest
                       FROM session_events GROUP BY session_id) n
                   ON n.session_id = e.session_id AND n.newest = e.sequence
                 WHERE e.hash_self IS NOT NULL
                 ORDER BY e.session_id",
            )
            .map_err(ledger_error)?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(ledger_error)?;
        rows.collect::<rusqlite::Result<_>>()
            .map_err(ledger_error)?
    };
    append(
        conn,
        ENTRY_BEGAN,
        None,
        &json!({ "device_id": this_device()?, "reason": reason, "sessions": tails.len() }),
    )?;
    for (session_id, sequence, hash) in &tails {
        append(
            conn,
            ENTRY_CARRIED,
            Some(session_id),
            &json!({ "sequence": sequence, "hash": hash }),
        )?;
    }
    Ok(Some(tails.len()))
}

/// Notes that a backup replaced the history, with where this PC's ledger
/// stood before, which the restored copy no longer holds.
pub fn record_restored(
    conn: &Connection,
    backup_id: &str,
    previous: Option<(i64, String)>,
) -> Result<()> {
    let previous = previous.map(|(sequence, hash)| json!({ "sequence": sequence, "hash": hash }));
    append(
        conn,
        ENTRY_RESTORED,
        None,
        &json!({ "backup_id": backup_id, "previous": previous }),
    )
}

/// Where this PC's ledger stands, for a restore to note.
pub fn this_tail(conn: &Connection) -> Result<Option<(i64, String)>> {
    tail(conn, &key_id()?)
}

/// Whether this PC is a fresh install with no history of its own: it holds
/// no session at all and has merged no PC, and its ledger is sound and holds
/// nothing but the entries that began it, signed with its own keys. Every
/// restore and merge leaves an entry, so it was never restored or merged.
pub(crate) fn fresh_install(conn: &Connection) -> Result<bool> {
    let count = |sql: &str| -> Result<i64> {
        conn.query_row(sql, [], |row| row.get(0))
            .map_err(ledger_error)
    };
    if count("SELECT COUNT(*) FROM sessions")? > 0
        || count("SELECT COUNT(*) FROM devices WHERE merged_at IS NOT NULL")? > 0
    {
        return Ok(false);
    }
    let mine = own_keys()?;
    let entries: Vec<(String, String)> = {
        let mut stmt = conn
            .prepare("SELECT key_id, entry_type FROM ledger_entries")
            .map_err(ledger_error)?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(ledger_error)?;
        rows.collect::<rusqlite::Result<_>>()
            .map_err(ledger_error)?
    };
    if entries
        .iter()
        .any(|(key, entry_type)| entry_type != ENTRY_BEGAN || !mine.contains(key))
    {
        return Ok(false);
    }
    Ok(!any_broken(conn, &mine)?)
}

/// A fresh install that restores a backup of a PC it is not continues as
/// that PC: the backup's sessions of that PC are its own from then on, and
/// the keys the backup's ledgers name for that PC count as its own beside
/// its new key. This is the trust a fresh install gives any PC it has never
/// seen, so a backup made up for it gains nothing more. A PC with history of
/// its own never takes over, see `fresh_install`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Takeover {
    pub device_id: String,
    pub name: Option<String>,
    #[serde(skip)]
    keys: HashSet<String>,
}

/// The takeover a restore of a backup of `source`, whose database is
/// `staged`, would bring this PC in `local`. None unless this PC is a fresh
/// install, `source` is another PC and the backup holds a sound ledger that
/// names it.
pub(crate) fn plan_takeover(
    local: &Connection,
    staged: &Connection,
    source: &str,
) -> Result<Option<Takeover>> {
    if source == this_device()? || !fresh_install(local)? {
        return Ok(None);
    }
    let keys = claimed_keys(staged, source)?;
    if keys.is_empty() || any_broken(staged, &keys)? {
        return Ok(None);
    }
    Ok(Some(Takeover {
        device_id: source.to_owned(),
        name: crate::db::repo::devices::clean_name_of(staged, source)?,
        keys,
    }))
}

/// A takeover under way. Dropped before `keep`, it puts this PC's identity
/// and the files it wrote back as they were.
pub(crate) struct TakingOver {
    takeover: Takeover,
    before: Option<(String, HashSet<String>)>,
    written: Vec<(std::path::PathBuf, Option<String>)>,
}

impl TakingOver {
    /// Makes this PC continue as the PC `takeover` names from now on.
    pub(crate) fn begin(takeover: &Takeover) -> Result<Self> {
        let before = switch_identity(&takeover.device_id, &takeover.keys)?;
        Ok(Self {
            takeover: takeover.clone(),
            before: Some(before),
            written: Vec::new(),
        })
    }

    /// Notes the takeover in this PC's ledger in `conn`, signed with its new
    /// key, so it shows and can be checked.
    pub(crate) fn record(&self, conn: &Connection, backup_id: &str) -> Result<()> {
        let mut keys: Vec<&String> = self.takeover.keys.iter().collect();
        keys.sort();
        append(
            conn,
            ENTRY_TOOK_OVER,
            None,
            &json!({
                "device_id": self.takeover.device_id,
                "from_device": self.before.as_ref().map(|(device, _)| device),
                "key_ids": keys,
                "backup_id": backup_id,
            }),
        )
    }

    /// Writes the device id and the keys taken over next to the database,
    /// so the next start continues as that PC. The key file stays as it is.
    pub(crate) fn write_files(&mut self, app_dir: &Path) -> Result<()> {
        let keys_path = app_dir.join(DEVICE_KEYS_FILE);
        let listed = read_optional(&keys_path)?;
        let mut keys = listed.clone().unwrap_or_default();
        let mut new: Vec<&String> = self
            .takeover
            .keys
            .iter()
            .filter(|key| !keys.lines().any(|line| line.trim() == key.as_str()))
            .collect();
        new.sort();
        for key in new {
            if !keys.is_empty() && !keys.ends_with('\n') {
                keys.push('\n');
            }
            keys.push_str(key);
            keys.push('\n');
        }
        let id_path = app_dir.join(crate::constants::DEVICE_ID_FILE);
        // The keys first: a start that finds them without the new id only
        // counts more keys as its own.
        for (path, contents) in [
            (keys_path, keys),
            (id_path, self.takeover.device_id.clone()),
        ] {
            let previous = read_optional(&path)?;
            self.written.push((path.clone(), previous));
            replace_file(&path, &contents).map_err(|error| {
                VaultimeError::Integrity(format!("failed to write {}: {error}", path.display()))
            })?;
        }
        Ok(())
    }

    /// Keeps the takeover.
    pub(crate) fn keep(mut self) {
        self.before = None;
        self.written.clear();
    }
}

impl Drop for TakingOver {
    fn drop(&mut self) {
        if let Some(before) = self.before.take() {
            restore_identity(before);
        }
        for (path, previous) in self.written.drain(..).rev() {
            let undone = match previous {
                Some(contents) => replace_file(&path, &contents),
                None => fs::remove_file(&path),
            };
            if let Err(error) = undone {
                warn!("could not put {} back: {error}", path.display());
            }
        }
    }
}

/// Writes a file next to `path` and moves it in place, so a crash leaves
/// either the old contents or the new ones.
fn replace_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut staged = path.as_os_str().to_owned();
    staged.push(".new");
    let staged = std::path::PathBuf::from(staged);
    {
        let mut file = fs::File::create(&staged)?;
        std::io::Write::write_all(&mut file, contents.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&staged, path)
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(VaultimeError::Integrity(format!(
            "failed to read {}: {error}",
            path.display()
        ))),
    }
}

/// The keys this PC took over, as its own ledgers note them.
pub(crate) fn taken_over_keys(conn: &Connection) -> Result<HashSet<String>> {
    let mine = own_keys()?;
    if any_broken(conn, &mine)? {
        return Ok(HashSet::new());
    }
    Ok(signed_payloads(conn, ENTRY_TOOK_OVER, &mine)?
        .iter()
        .filter_map(|payload| payload.get("key_ids")?.as_array())
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect())
}

/// The payloads of the entries of `entry_type` that `signers` signed.
fn signed_payloads(
    conn: &Connection,
    entry_type: &str,
    signers: &HashSet<String>,
) -> Result<Vec<Value>> {
    let mut stmt = conn
        .prepare_cached("SELECT key_id, payload_json FROM ledger_entries WHERE entry_type = ?1")
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([entry_type], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(ledger_error)?;
    let mut payloads = Vec::new();
    for row in rows {
        let (signer, payload) = row.map_err(ledger_error)?;
        if signers.contains(&signer) {
            payloads.push(serde_json::from_str(&payload).unwrap_or(Value::Null));
        }
    }
    Ok(payloads)
}

/// Every key the ledgers of `signers` trust, with the PC they trust it for.
fn trust_entries(conn: &Connection, signers: &HashSet<String>) -> Result<Vec<(String, String)>> {
    Ok(signed_payloads(conn, ENTRY_TRUSTED, signers)?
        .iter()
        .filter_map(|payload| {
            Some((
                payload.get("device_id")?.as_str()?.to_owned(),
                payload.get("key_id")?.as_str()?.to_owned(),
            ))
        })
        .collect())
}

/// The keys the ledgers of `signers` in `conn` trust for `device_id`.
pub(crate) fn trusted_by(
    conn: &Connection,
    signers: &HashSet<String>,
    device_id: &str,
) -> Result<HashSet<String>> {
    Ok(trust_entries(conn, signers)?
        .into_iter()
        .filter(|(device, _)| device == device_id)
        .map(|(_, key)| key)
        .collect())
}

/// One ledger entry as stored.
struct LedgerRow {
    key_id: String,
    sequence: i64,
    entry_type: String,
    session_id: Option<String>,
    recorded_at: String,
    payload_json: String,
    hash_prev: Option<String>,
    hash_self: String,
    signature: String,
}

/// What all ledgers together say: which fail their check, and which PC each
/// key belongs to, as its first entry names it.
#[derive(Debug, Clone, Default)]
struct LedgerState {
    broken: HashSet<String>,
    devices: HashMap<String, String>,
}

/// Checks every ledger. A sound chain whose newest entry carries a good
/// signature is sound as a whole, since each hash covers the one before it.
fn check_ledgers(conn: &Connection) -> Result<LedgerState> {
    let mut stmt = conn
        .prepare(
            "SELECT key_id, sequence, entry_type, session_id, recorded_at, payload_json,
                    hash_prev, hash_self, signature
             FROM ledger_entries ORDER BY key_id, sequence",
        )
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(LedgerRow {
                key_id: row.get(0)?,
                sequence: row.get(1)?,
                entry_type: row.get(2)?,
                session_id: row.get(3)?,
                recorded_at: row.get(4)?,
                payload_json: row.get(5)?,
                hash_prev: row.get(6)?,
                hash_self: row.get(7)?,
                signature: row.get(8)?,
            })
        })
        .map_err(ledger_error)?;
    let mut state = LedgerState::default();
    let mut current: Option<(String, i64, String, String)> = None;
    let finish = |key_id: &str, hash: &str, signature: &str, state: &mut LedgerState| {
        if !signed_by(key_id, hash, signature) {
            state.broken.insert(key_id.to_owned());
        }
    };
    for row in rows {
        let entry = row.map_err(ledger_error)?;
        let (expected_sequence, previous) = match &current {
            Some((key_id, sequence, hash, _)) if *key_id == entry.key_id => {
                (sequence + 1, Some(hash.clone()))
            }
            Some((key_id, _, hash, signature)) => {
                finish(key_id, hash, signature, &mut state);
                (1, None)
            }
            None => (1, None),
        };
        let expected_hash = entry_hash(
            &entry.key_id,
            entry.sequence,
            &entry.entry_type,
            entry.session_id.as_deref(),
            &entry.recorded_at,
            &entry.payload_json,
            previous.as_deref(),
        );
        if entry.sequence != expected_sequence
            || entry.hash_prev != previous
            || entry.hash_self != expected_hash
        {
            state.broken.insert(entry.key_id.clone());
        }
        if entry.sequence == 1 && entry.entry_type == ENTRY_BEGAN {
            let payload: Value = serde_json::from_str(&entry.payload_json).unwrap_or(Value::Null);
            if let Some(device) = payload.get("device_id").and_then(Value::as_str) {
                state
                    .devices
                    .insert(entry.key_id.clone(), device.to_owned());
            }
        }
        current = Some((
            entry.key_id,
            entry.sequence,
            entry.hash_self,
            entry.signature,
        ));
    }
    if let Some((key_id, _, hash, signature)) = &current {
        finish(key_id, hash, signature, &mut state);
    }
    // Ledgers begun before their first entry named the PC are known by the
    // key the PC's row records. Both are claims of the same kind.
    let mut stmt = conn
        .prepare("SELECT id, key_id FROM devices WHERE key_id IS NOT NULL")
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(ledger_error)?;
    let begun: HashSet<String> = {
        let mut stmt = conn
            .prepare("SELECT DISTINCT key_id FROM ledger_entries")
            .map_err(ledger_error)?;
        stmt.query_map([], |row| row.get::<_, String>(0))
            .map_err(ledger_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(ledger_error)?
    };
    for row in rows {
        let (device, key) = row.map_err(ledger_error)?;
        if begun.contains(&key) && !state.devices.contains_key(&key) {
            state.devices.insert(key, device);
        }
    }
    Ok(state)
}

fn signed_by(key_id: &str, hash: &str, signature: &str) -> bool {
    let key = hex::decode(key_id)
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok());
    let signature = hex::decode(signature)
        .and_then(|bytes| <[u8; 64]>::try_from(bytes).ok())
        .map(|bytes| Signature::from_bytes(&bytes));
    matches!((key, signature), (Some(key), Some(signature)) if key.verify(hash.as_bytes(), &signature).is_ok())
}

/// The ledger check as last done for each database, by its path, with the
/// newest row then.
type StateCache = Mutex<HashMap<String, (Option<(i64, String)>, LedgerState)>>;

fn state_cache() -> &'static StateCache {
    static CACHE: OnceLock<StateCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `check_ledgers`, reused while no ledger gained an entry. The app only
/// ever appends, so the newest row tells whether anything changed, in one
/// step down the index. Changes made by other programs, and a restore, clear
/// it through `forget`.
fn ledger_state(conn: &Connection) -> Result<LedgerState> {
    let newest: Option<(i64, String)> = conn
        .prepare_cached("SELECT rowid, hash_self FROM ledger_entries ORDER BY rowid DESC LIMIT 1")
        .and_then(|mut stmt| {
            stmt.query_row([], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()
        })
        .map_err(ledger_error)?;
    let database = conn.path().unwrap_or_default().to_owned();
    if let Some((checked, state)) = state_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&database)
        && *checked == newest
    {
        return Ok(state.clone());
    }
    // Checked without holding the cache, so checks of other databases go on.
    let state = check_ledgers(conn)?;
    let mut cache = state_cache().lock().unwrap_or_else(PoisonError::into_inner);
    // Staged backups come and go, so only a few databases are remembered.
    if cache.len() >= LEDGER_CHECKS_REMEMBERED {
        cache.clear();
    }
    cache.insert(database, (newest, state.clone()));
    Ok(state)
}

/// Forgets the remembered ledger check.
pub(crate) fn forget() {
    state_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

/// Whose keys a session's pins count from: this PC's own, or a given set,
/// like the keys of the PC a backup came from, to see a session the way that
/// PC sees it.
#[derive(Debug, Clone, Copy)]
pub enum Viewpoint<'a> {
    ThisPc,
    Keys(&'a HashSet<String>),
}

/// The keys whose ledgers name `device_id` in their first entry. Anyone can
/// start a ledger that names any PC, so these are claims, not proof.
pub(crate) fn claimed_keys(conn: &Connection, device_id: &str) -> Result<HashSet<String>> {
    Ok(ledger_state(conn)?
        .devices
        .into_iter()
        .filter(|(_, device)| device == device_id)
        .map(|(key, _)| key)
        .collect())
}

/// Whether any of `keys` has a ledger that fails its check.
pub(crate) fn any_broken(conn: &Connection, keys: &HashSet<String>) -> Result<bool> {
    Ok(!ledger_state(conn)?.broken.is_disjoint(keys))
}

/// The keys of another PC that this PC's own ledgers trust.
pub(crate) fn trusted_keys(conn: &Connection, device_id: &str) -> Result<HashSet<String>> {
    let mine = own_keys()?;
    if any_broken(conn, &mine)? {
        return Ok(HashSet::new());
    }
    trusted_by(conn, &mine, device_id)
}

/// Every PC this PC's own ledgers trust keys of, with those keys.
pub(crate) fn all_trusted(conn: &Connection) -> Result<HashMap<String, HashSet<String>>> {
    let mine = own_keys()?;
    if any_broken(conn, &mine)? {
        return Ok(HashMap::new());
    }
    let mut trusted: HashMap<String, HashSet<String>> = HashMap::new();
    for (device, key) in trust_entries(conn, &mine)? {
        trusted.entry(device).or_default().insert(key);
    }
    Ok(trusted)
}

/// The PCs this PC has seen: those with sessions here and those merged.
fn seen_devices(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT device_id FROM sessions
             UNION SELECT id FROM devices WHERE merged_at IS NOT NULL",
        )
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(ledger_error)?;
    rows.collect::<rusqlite::Result<_>>().map_err(ledger_error)
}

/// A PC this PC has seen, as its row: id, platform, app version, name and
/// when it was merged.
pub(crate) type SeenPc = (String, String, String, Option<String>, Option<String>);

/// The PCs other than this one that this PC has seen, see `seen_devices`.
pub(crate) fn seen_pcs(conn: &Connection) -> Result<Vec<SeenPc>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, platform, app_version, name, merged_at FROM devices
             WHERE id != ?1
               AND (merged_at IS NOT NULL OR id IN (SELECT device_id FROM sessions))",
        )
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([this_device()?], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .map_err(ledger_error)?;
    rows.collect::<rusqlite::Result<_>>().map_err(ledger_error)
}

/// Keeps `seen` PCs seen in `conn`, a database that may not know them, as
/// after a restore of an older backup. Rows it has stay as they are, but a
/// PC merged here stays merged, also when the backup is that PC's own, so
/// its sessions are still corrected only there.
pub(crate) fn keep_seen(conn: &Connection, seen: &[SeenPc]) -> Result<()> {
    let now = super::now_timestamp();
    for (id, platform, app_version, name, merged_at) in seen {
        conn.execute(
            "INSERT OR IGNORE INTO devices (id, platform, app_version, name, merged_at)
             VALUES (?1, ?2, ?3, ?4, COALESCE(?5, ?6))",
            rusqlite::params![id, platform, app_version, name, merged_at, now],
        )
        .map_err(ledger_error)?;
        conn.execute(
            "UPDATE devices SET merged_at = ?2
             WHERE id = ?1 AND merged_at IS NULL AND ?2 IS NOT NULL",
            rusqlite::params![id, merged_at],
        )
        .map_err(ledger_error)?;
    }
    Ok(())
}

/// Whose pins count for each session of a backup taken in here. A session
/// recorded on this PC counts only with this PC's own keys. One of a PC this
/// PC trusts keys of counts with those keys alone. One of a PC seen without
/// keys of its own, whose sessions came through another PC, counts with the
/// keys of the backup's PC if this PC trusted that one before, as it took
/// the session in after its own check. One of a PC never seen counts with
/// the keys of the backup's PC. The backup's PC counts with the keys its ledgers
/// name only when this PC has never seen it, the way any new PC is taken at
/// its word, or when the player vouches for them. So a backup cannot bring a
/// ledger of its own to vouch for sessions of PCs this PC knows, and one
/// that claims to be this PC counts with this PC's keys alone.
#[derive(Debug, Clone)]
pub(crate) struct Trust {
    this: String,
    own: HashSet<String>,
    source: String,
    source_keys: HashSet<String>,
    /// Whether this PC trusted keys of the backup's PC before.
    source_trusted: bool,
    first_merge: bool,
    known: HashMap<String, HashSet<String>>,
    seen: HashSet<String>,
}

impl Trust {
    /// The trust for a backup of `source`, from what this PC's ledger in
    /// `local` trusts and the keys the backup's ledgers in `staged` claim.
    /// With `trust_new_keys`, the player vouches that new keys claiming that
    /// PC are its own. Also returns whether the backup holds such new keys.
    pub(crate) fn new(
        local: &Connection,
        staged: &Connection,
        source: &str,
        trust_new_keys: bool,
    ) -> Result<(Self, bool)> {
        let this = this_device()?;
        let own = own_keys()?;
        let known = all_trusted(local)?;
        let seen = seen_devices(local)?;
        let trusted = known.get(source).cloned().unwrap_or_default();
        let first_merge = source != this && trusted.is_empty() && !seen.contains(source);
        let claimed = claimed_keys(staged, source)?;
        let (source_keys, unknown) = if source == this {
            (own.clone(), false)
        } else if first_merge {
            (claimed, false)
        } else {
            // Once the player vouches for them, new keys are not unknown.
            let unknown = !trust_new_keys && !claimed.is_subset(&trusted);
            let mut keys = trusted.clone();
            if trust_new_keys {
                keys.extend(claimed);
            }
            (keys, unknown)
        };
        Ok((
            Self {
                source_trusted: !trusted.is_empty(),
                this,
                own,
                source: source.to_owned(),
                source_keys,
                first_merge,
                known,
                seen,
            },
            unknown,
        ))
    }

    /// The keys whose pins count for a session recorded on `device`.
    pub(crate) fn keys_for(&self, device: &str) -> HashSet<String> {
        if device == self.this {
            return self.own.clone();
        }
        if device == self.source {
            return self.source_keys.clone();
        }
        let trusted = self.known.get(device).cloned().unwrap_or_default();
        if !trusted.is_empty() {
            return trusted;
        }
        if !self.seen.contains(device) || self.source_trusted {
            return self.source_keys.clone();
        }
        HashSet::new()
    }

    /// The keys of the backup's PC whose pins count.
    pub(crate) fn source_keys(&self) -> &HashSet<String> {
        &self.source_keys
    }

    /// Whether this PC has never seen the backup's PC, so its ledgers are
    /// taken at their word.
    pub(crate) fn first_merge(&self) -> bool {
        self.first_merge
    }

    /// Records the trust in `conn`'s ledger: the backup's PC's keys, and on a
    /// restore, everything this PC trusted before, which the restored ledger
    /// may not hold.
    pub(crate) fn record(&self, conn: &Connection, carry_known: bool) -> Result<()> {
        if !self.source_keys.is_empty() && self.source != self.this {
            trust_keys(conn, &self.source, &self.source_keys)?;
        }
        if carry_known {
            let mut devices: Vec<&String> = self.known.keys().collect();
            devices.sort();
            for device in devices {
                trust_keys(conn, device, &self.known[device])?;
            }
        }
        Ok(())
    }
}

/// The pins this PC's own keys hold in `conn`, by session, so a restore can
/// tell which sessions of a backup this PC vouched for before.
pub(crate) fn own_pins(conn: &Connection) -> Result<HashMap<String, Vec<StoredPin>>> {
    let mine = own_keys()?;
    let mut stmt = conn
        .prepare(
            "SELECT key_id, session_id, entry_type, payload_json FROM ledger_entries
             WHERE session_id IS NOT NULL AND entry_type IN ('pinned', 'carried', 'adopted')",
        )
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(ledger_error)?;
    let mut pins: HashMap<String, Vec<StoredPin>> = HashMap::new();
    for row in rows {
        let (key, session, entry_type, payload) = row.map_err(ledger_error)?;
        if mine.contains(&key) {
            pins.entry(session)
                .or_default()
                .push((key, entry_type, payload));
        }
    }
    Ok(pins)
}

/// Records in this PC's ledger that it trusts `keys` as keys of `device_id`,
/// skipping the ones it trusts already.
pub(crate) fn trust_keys(conn: &Connection, device_id: &str, keys: &HashSet<String>) -> Result<()> {
    let known = trusted_keys(conn, device_id)?;
    let mut new: Vec<&String> = keys.difference(&known).collect();
    new.sort();
    for key in new {
        append(
            conn,
            ENTRY_TRUSTED,
            None,
            &json!({ "device_id": device_id, "key_id": key }),
        )?;
    }
    Ok(())
}

/// Why a session's chain disagrees with the ledgers, if it does, seen from
/// `viewpoint`. Only the pins of its keys count: each must match the chain,
/// one must cover the session from its start or from when it was carried or
/// taken in, a closed session's time must lie in a pinned part of its chain,
/// and its time may run at most one pin interval past its newest pinned
/// checkpoint. `events` are in order and checked.
pub(crate) fn session_problem(
    conn: &Connection,
    session: &Session,
    events: &[SessionEvent],
    timing_events: &[&str],
    viewpoint: Viewpoint,
) -> Result<Option<&'static str>> {
    let pins = session_pins(conn, &session.id)?;
    if pins.is_empty() {
        return Ok(Some("not_in_ledger"));
    }
    let state = ledger_state(conn)?;
    let own;
    let counted = match viewpoint {
        Viewpoint::ThisPc => {
            own = own_keys()?;
            &own
        }
        Viewpoint::Keys(keys) => keys,
    };
    let counted_pins: Vec<StoredPin> = pins
        .into_iter()
        .filter(|(key, _, _)| counted.contains(key))
        .collect();
    let mut covered_from_start = false;
    let mut newest_pinned = 0;
    for (key_id, entry_type, payload_json) in &counted_pins {
        if state.broken.contains(key_id) {
            return Ok(Some("ledger_broken"));
        }
        let payload: Value = serde_json::from_str(payload_json).unwrap_or(Value::Null);
        let Some((sequence, hash)) = pin_point(&payload) else {
            return Ok(Some("ledger_mismatch"));
        };
        if !pin_matches(events, sequence, hash) {
            return Ok(Some("ledger_mismatch"));
        }
        covered_from_start |= sequence == 1 || entry_type != ENTRY_PINNED;
        newest_pinned = newest_pinned.max(sequence);
    }
    if !covered_from_start {
        return Ok(Some("not_in_ledger"));
    }
    if session.ended_at_wall.is_some() {
        let newest_timing = events
            .iter()
            .rev()
            .find(|event| timing_events.contains(&event.event_type.as_str()));
        if newest_timing.is_some_and(|event| event.sequence > newest_pinned) {
            return Ok(Some("not_in_ledger"));
        }
    }
    // Time past the newest pinned checkpoint counts up to one interval,
    // also when a recovery or a correction restates it later.
    let counters = [session.runtime_ms, session.active_ms, session.idle_ms];
    if !pins_vouch_for_time(&counted_pins, events, counters) {
        return Ok(Some("unpinned_time_too_long"));
    }
    Ok(None)
}

/// What the ledgers say about the history as a whole.
#[derive(Debug, Clone, Serialize)]
pub struct LedgerReport {
    /// This PC's public key in hex, which names its ledger.
    pub key_id: String,
    /// When this PC's ledger began, if it has.
    pub began_at: Option<String>,
    /// Entries in this PC's ledger.
    pub entries: i64,
    /// Sessions that this PC's ledger covers.
    pub covered_sessions: i64,
    /// Sessions this PC's ledger names that are gone without a note that the
    /// player removed them.
    pub missing_sessions: i64,
    /// Whether this PC's ledger fails its check.
    pub broken: bool,
}

pub fn report(conn: &Connection) -> Result<LedgerReport> {
    let key_id = key_id()?;
    let state = ledger_state(conn)?;
    let mine = own_keys()?;
    let mine_json = serde_json::to_string(&mine)
        .map_err(|error| VaultimeError::Integrity(format!("ledger keys: {error}")))?;
    let began_at = conn
        .query_row(
            "SELECT recorded_at FROM ledger_entries WHERE key_id = ?1 AND sequence = 1",
            [&key_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(ledger_error)?;
    let entries = conn
        .query_row(
            "SELECT COUNT(*) FROM ledger_entries WHERE key_id = ?1",
            [&key_id],
            |row| row.get(0),
        )
        .map_err(ledger_error)?;
    let covered: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sessions s WHERE EXISTS (
                 SELECT 1 FROM ledger_entries l
                 WHERE l.session_id = s.id
                   AND l.entry_type IN ('pinned', 'carried', 'adopted')
                   AND l.key_id IN (SELECT value FROM json_each(?1)))",
            [&mine_json],
            |row| row.get(0),
        )
        .map_err(ledger_error)?;
    let missing_sessions = conn
        .query_row(
            "SELECT COUNT(DISTINCT l.session_id) FROM ledger_entries l
             WHERE l.entry_type IN ('pinned', 'carried', 'adopted')
               AND l.key_id IN (SELECT value FROM json_each(?1))
               AND NOT EXISTS (SELECT 1 FROM sessions s WHERE s.id = l.session_id)
               AND NOT EXISTS (SELECT 1 FROM ledger_entries r
                               WHERE r.entry_type = 'removed' AND r.session_id = l.session_id
                                 AND r.key_id IN (SELECT value FROM json_each(?1)))",
            [&mine_json],
            |row| row.get(0),
        )
        .map_err(ledger_error)?;
    Ok(LedgerReport {
        began_at,
        entries,
        covered_sessions: covered,
        missing_sessions,
        broken: mine.iter().any(|key| state.broken.contains(key)),
        key_id,
    })
}

/// Raises the newest checkpoint of a session to `runtime_ms`, hashed the
/// way the tracker hashes, with the row to match, as a database tool can.
#[cfg(test)]
pub(crate) fn raise_last_checkpoint(conn: &Connection, session: &str, runtime_ms: i64) {
    let (sequence, wall, previous): (i64, String, Option<String>) = conn
        .query_row(
            "SELECT sequence, event_time_wall, hash_prev FROM session_events
             WHERE session_id = ?1 ORDER BY sequence DESC LIMIT 1",
            [session],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let payload = json!({
        "runtime_ms": runtime_ms, "active_ms": runtime_ms, "idle_ms": 0,
        "wall_elapsed_ms": runtime_ms, "drift_ms": 0, "integrity_status": "local",
    })
    .to_string();
    let hash = super::compute_event_hash(
        session,
        sequence,
        "heartbeat",
        &wall,
        Some(runtime_ms),
        &payload,
        previous.as_deref(),
    );
    conn.execute(
        "UPDATE session_events SET payload_json = ?1, event_time_monotonic = ?2, hash_self = ?3
         WHERE session_id = ?4 AND sequence = ?5",
        rusqlite::params![payload, runtime_ms, hash, session, sequence],
    )
    .unwrap();
    conn.execute(
        "UPDATE sessions SET runtime_ms = ?1, elapsed_monotonic_ms = ?1, active_ms = ?1
         WHERE id = ?2",
        rusqlite::params![runtime_ms, session],
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::CHECKPOINT_PIN_INTERVAL_SECS;
    use crate::db::connection::Database;
    use crate::db::models::CreateGame;
    use crate::db::repo::sessions::Checkpoint;
    use crate::db::repo::{devices, games, map_db, sessions};
    use crate::integrity::{self, STATUS_LOCAL, STATUS_SUSPICIOUS};

    const DEVICE: &str = "device";

    fn setup() -> (Database, String) {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, DEVICE, "linux", "0.0.0").unwrap();
        db.with_transaction(|conn| begin_if_empty(conn, "first_start"))
            .unwrap();
        let game = add_game(&db, "Game");
        (db, game)
    }

    fn add_game(db: &Database, title: &str) -> String {
        games::create_game(
            db,
            &CreateGame {
                title: title.into(),
                executable_path: Some(format!("/games/{title}")),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap()
        .id
    }

    /// A session of a minute with one checkpoint on the way.
    fn played(db: &Database, game: &str) -> String {
        let session = sessions::create_session(db, game, DEVICE).unwrap();
        let checkpoint = Checkpoint {
            runtime_ms: 30_000,
            active_ms: 30_000,
            wall_elapsed_ms: 30_000,
            ..Checkpoint::default()
        };
        sessions::record_checkpoint(
            db,
            &session.id,
            &checkpoint,
            STATUS_LOCAL,
            chrono::Utc::now(),
        )
        .unwrap();
        sessions::end_session(db, &session.id, 60_000, 60_000, 0, STATUS_LOCAL).unwrap();
        session.id
    }

    fn listed(db: &Database, id: &str) -> crate::db::models::Session {
        sessions::list_all_sessions(db)
            .unwrap()
            .into_iter()
            .find(|session| session.id == id)
            .unwrap()
    }

    /// Why the stored session fails its check, read from its row as stored,
    /// since the listing relabels a session it finds Suspicious.
    fn problem(db: &Database, id: &str) -> Option<String> {
        db.with_conn(|conn| {
            let session = conn
                .query_row(
                    "SELECT * FROM sessions WHERE id = ?1",
                    [id],
                    sessions::row_to_session,
                )
                .map_err(map_db)?;
            integrity::validate_session_history(conn, &session)
        })
        .unwrap()
    }

    /// Changes the database the way another program could. The app's own
    /// connection does not raise the data version, so the checks are
    /// forgotten by hand.
    fn outside(db: &Database, statement: &str) {
        db.with_conn(|conn| conn.execute_batch(statement).map_err(map_db))
            .unwrap();
        integrity::forget_checks();
    }

    fn ledger_report(db: &Database) -> LedgerReport {
        db.with_conn(report).unwrap()
    }

    /// A checkpoint after `seconds` of active play.
    fn checkpoint_at(db: &Database, session: &str, seconds: i64) {
        let ms = seconds * 1_000;
        let checkpoint = Checkpoint {
            runtime_ms: ms,
            active_ms: ms,
            wall_elapsed_ms: ms,
            ..Checkpoint::default()
        };
        sessions::record_checkpoint(db, session, &checkpoint, STATUS_LOCAL, chrono::Utc::now())
            .unwrap();
    }

    /// A session left open as by a crash, with a checkpoint every half
    /// minute up to `seconds`.
    fn crashed_after(db: &Database, game: &str, seconds: i64) -> String {
        let session = sessions::create_session(db, game, DEVICE).unwrap();
        for at in (30..=seconds).step_by(30) {
            checkpoint_at(db, &session.id, at);
        }
        session.id
    }

    /// The event types the ledger pinned for a session, in order.
    fn pinned_types(db: &Database, session: &str) -> Vec<String> {
        db.with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT json_extract(payload_json, '$.event_type') FROM ledger_entries
                     WHERE session_id = ?1 AND entry_type = 'pinned' ORDER BY sequence",
                )
                .map_err(map_db)?;
            let rows = stmt
                .query_map([session], |row| row.get(0))
                .map_err(map_db)?;
            rows.collect::<rusqlite::Result<_>>().map_err(map_db)
        })
        .unwrap()
    }

    const HOUR_MS: i64 = 3_600_000;

    #[test]
    fn pins_a_checkpoint_each_time_play_enters_a_new_interval() {
        let (db, game) = setup();
        let interval = CHECKPOINT_PIN_INTERVAL_SECS;
        let session = crashed_after(&db, &game, 2 * interval + 60);
        assert_eq!(
            pinned_types(&db, &session),
            ["started", "heartbeat", "heartbeat"]
        );
        assert_eq!(problem(&db, &session), None);
    }

    #[test]
    fn an_honest_crash_keeps_its_time_as_recovered() {
        let (db, game) = setup();
        let seconds = CHECKPOINT_PIN_INTERVAL_SECS + 120;
        let session = crashed_after(&db, &game, seconds);
        let recovered = sessions::recover_session(&db, &session, "startup_orphan_cleanup").unwrap();
        assert_eq!(recovered.runtime_ms, seconds * 1_000);
        assert_eq!(recovered.integrity_status, integrity::STATUS_RECOVERED);
        assert_eq!(problem(&db, &session), None);
    }

    #[test]
    fn a_checkpoint_raised_after_a_crash_counts_one_interval_at_most() {
        let (db, game) = setup();
        let interval_ms = CHECKPOINT_PIN_INTERVAL_SECS * 1_000;
        // Pinned at the first interval, then two more minutes unpinned.
        let session = crashed_after(&db, &game, CHECKPOINT_PIN_INTERVAL_SECS + 120);
        db.with_conn(|conn| {
            raise_last_checkpoint(conn, &session, 100 * HOUR_MS);
            Ok(())
        })
        .unwrap();
        integrity::forget_checks();
        assert_eq!(
            problem(&db, &session).as_deref(),
            Some("unpinned_time_too_long")
        );

        let recovered = sessions::recover_session(&db, &session, "startup_orphan_cleanup").unwrap();
        assert_eq!(recovered.runtime_ms, 2 * interval_ms);
        assert_eq!(recovered.active_ms, 2 * interval_ms);
        assert_eq!(recovered.integrity_status, STATUS_SUSPICIOUS);
        assert_eq!(listed(&db, &session).integrity_status, STATUS_SUSPICIOUS);
        let counted: i64 = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT COALESCE(SUM(runtime_ms), 0) FROM play_slices WHERE session_id = ?1",
                    [&session],
                    |row| row.get(0),
                )
                .map_err(map_db)
            })
            .unwrap();
        assert_eq!(counted, 2 * interval_ms);
    }

    #[test]
    fn sessions_from_before_checkpoint_pins_keep_their_time() {
        let (db, game) = setup();
        // A start as versions before checkpoint pins wrote it.
        let session = uuid::Uuid::new_v4().to_string();
        db.with_transaction(|conn| {
            let wall = integrity::now_timestamp();
            conn.execute(
                "INSERT INTO sessions (id, game_id, device_id, started_at_wall,
                     elapsed_monotonic_ms, active_ms, idle_ms, runtime_ms,
                     integrity_status, closed_cleanly)
                 VALUES (?1, ?2, ?3, ?4, 0, 0, 0, 0, 'local', 0)",
                rusqlite::params![session, game, DEVICE, wall],
            )
            .map_err(map_db)?;
            integrity::append_session_event(
                conn,
                &session,
                "started",
                &wall,
                Some(0),
                &json!({ "game_id": game, "device_id": DEVICE, "integrity_status": "local" })
                    .to_string(),
            )
        })
        .unwrap();
        for at in (30..=CHECKPOINT_PIN_INTERVAL_SECS * 3).step_by(30) {
            checkpoint_at(&db, &session, at);
        }
        assert_eq!(pinned_types(&db, &session), ["started"]);
        let recovered = sessions::recover_session(&db, &session, "startup_orphan_cleanup").unwrap();
        assert_eq!(
            recovered.runtime_ms,
            CHECKPOINT_PIN_INTERVAL_SECS * 3 * 1_000
        );
        assert_eq!(recovered.integrity_status, integrity::STATUS_RECOVERED);
        assert_eq!(problem(&db, &session), None);
    }

    #[test]
    fn pins_every_event_of_a_session_but_its_checkpoints() {
        let (db, game) = setup();
        let session = played(&db, &game);
        let pinned: Vec<String> = db
            .with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT json_extract(payload_json, '$.event_type') FROM ledger_entries
                         WHERE session_id = ?1 ORDER BY sequence",
                    )
                    .map_err(map_db)?;
                let rows = stmt
                    .query_map([&session], |row| row.get(0))
                    .map_err(map_db)?;
                rows.collect::<rusqlite::Result<_>>().map_err(map_db)
            })
            .unwrap();
        assert_eq!(pinned, ["started", "ended"]);
        assert_eq!(listed(&db, &session).integrity_status, STATUS_LOCAL);
        let report = ledger_report(&db);
        assert!(!report.broken);
        assert_eq!(report.covered_sessions, 1);
        assert_eq!(report.missing_sessions, 0);
    }

    #[test]
    fn finds_a_session_rebuilt_with_a_sound_chain() {
        let (db, game) = setup();
        let session = played(&db, &game);
        // A longer session written over the ended event, hashed like the
        // tracker would, with the row to match.
        db.with_conn(|conn| {
            let (sequence, wall, previous): (i64, String, String) = conn
                .query_row(
                    "SELECT sequence, event_time_wall, hash_prev FROM session_events
                     WHERE session_id = ?1 AND event_type = 'ended'",
                    [&session],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(map_db)?;
            let payload = r#"{"runtime_ms":7200000,"active_ms":7200000,"idle_ms":0,"integrity_status":"local","closed_cleanly":true}"#;
            let hash = integrity::compute_event_hash(
                &session,
                sequence,
                "ended",
                &wall,
                Some(7_200_000),
                payload,
                Some(&previous),
            );
            conn.execute(
                "UPDATE session_events SET payload_json = ?1, event_time_monotonic = 7200000,
                     hash_self = ?2 WHERE session_id = ?3 AND sequence = ?4",
                rusqlite::params![payload, hash, session, sequence],
            )
            .map_err(map_db)?;
            conn.execute(
                "UPDATE sessions SET runtime_ms = 7200000, elapsed_monotonic_ms = 7200000,
                     active_ms = 7200000 WHERE id = ?1",
                [&session],
            )
            .map_err(map_db)
        })
        .unwrap();
        integrity::forget_checks();
        assert_eq!(problem(&db, &session).as_deref(), Some("ledger_mismatch"));
        assert_eq!(listed(&db, &session).integrity_status, STATUS_SUSPICIOUS);
    }

    #[test]
    fn a_session_no_ledger_knows_is_suspicious() {
        let (db, game) = setup();
        let slipped_in = played(&db, &game);
        let kept = played(&db, &game);
        outside(
            &db,
            &format!("DELETE FROM ledger_entries WHERE session_id = '{slipped_in}'"),
        );
        assert_eq!(problem(&db, &slipped_in).as_deref(), Some("not_in_ledger"));
        // Taking entries out of the middle broke the ledger the later session
        // relies on.
        assert_eq!(problem(&db, &kept).as_deref(), Some("ledger_broken"));
        assert!(ledger_report(&db).broken);
    }

    #[test]
    fn a_changed_entry_breaks_the_ledger() {
        let (db, game) = setup();
        let session = played(&db, &game);
        outside(
            &db,
            "UPDATE ledger_entries SET payload_json = '{}' WHERE sequence = 1",
        );
        assert_eq!(problem(&db, &session).as_deref(), Some("ledger_broken"));
    }

    #[test]
    fn an_entry_hashed_again_still_needs_the_key() {
        let (db, game) = setup();
        let session = played(&db, &game);
        let key_id = key_id().unwrap();
        db.with_conn(|conn| {
            let (sequence, entry_type, recorded_at, session_id, previous): (
                i64,
                String,
                String,
                Option<String>,
                String,
            ) = conn
                .query_row(
                    "SELECT sequence, entry_type, recorded_at, session_id, hash_prev
                     FROM ledger_entries ORDER BY sequence DESC LIMIT 1",
                    [],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .map_err(map_db)?;
            let payload = r#"{"sequence":1,"hash":"x"}"#;
            let hash = entry_hash(
                &key_id,
                sequence,
                &entry_type,
                session_id.as_deref(),
                &recorded_at,
                payload,
                Some(&previous),
            );
            conn.execute(
                "UPDATE ledger_entries SET payload_json = ?1, hash_self = ?2 WHERE sequence = ?3",
                rusqlite::params![payload, hash, sequence],
            )
            .map_err(map_db)
        })
        .unwrap();
        integrity::forget_checks();
        assert_eq!(problem(&db, &session).as_deref(), Some("ledger_broken"));
    }

    #[test]
    fn tells_removed_sessions_from_missing_ones() {
        let (db, game) = setup();
        let other = add_game(&db, "Other");
        played(&db, &game);
        let vanished = played(&db, &other);
        games::delete_game(&db, &game).unwrap();
        assert_eq!(ledger_report(&db).missing_sessions, 0);

        outside(
            &db,
            &format!("DELETE FROM sessions WHERE id = '{vanished}'"),
        );
        let report = ledger_report(&db);
        assert_eq!(report.missing_sessions, 1);
        assert!(!report.broken);
    }

    #[test]
    fn a_damaged_key_file_is_set_aside_for_a_new_key() {
        let dir = std::env::temp_dir().join(format!("vaultime-key-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(DEVICE_KEY_FILE), "not a key").unwrap();
        load_key(&dir, "key-test").unwrap();
        assert_eq!(
            fs::read_to_string(dir.join(DEVICE_KEY_FILE).with_extension("damaged")).unwrap(),
            "not a key"
        );
        let fresh = fs::read_to_string(dir.join(DEVICE_KEY_FILE)).unwrap();
        assert_eq!(hex::decode(&fresh).map(|bytes| bytes.len()), Some(32));
        let public = hex::encode(
            SigningKey::from_bytes(&<[u8; 32]>::try_from(hex::decode(&fresh).unwrap()).unwrap())
                .verifying_key()
                .as_bytes(),
        );
        let listed = fs::read_to_string(dir.join(DEVICE_KEYS_FILE)).unwrap();
        assert_eq!(listed.lines().collect::<Vec<_>>(), [public.as_str()]);
        // Kept from then on.
        load_key(&dir, "key-test").unwrap();
        assert_eq!(
            fs::read_to_string(dir.join(DEVICE_KEY_FILE)).unwrap(),
            fresh
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn absurd_values_turn_a_session_suspicious_without_a_panic() {
        let (db, game) = setup();
        let session = played(&db, &game);
        // A pin at a place no chain reaches, signed with this PC's key.
        db.with_conn(|conn| {
            append(
                conn,
                ENTRY_PINNED,
                Some(&session),
                &json!({ "sequence": i64::MIN, "hash": "x" }),
            )
        })
        .unwrap();
        integrity::forget_checks();
        assert_eq!(problem(&db, &session).as_deref(), Some("ledger_mismatch"));

        // Time far past any session, written through the chain with the row
        // to match.
        let open = sessions::create_session(&db, &game, DEVICE).unwrap();
        db.with_conn(|conn| {
            integrity::append_session_event(
                conn,
                &open.id,
                "heartbeat",
                &integrity::now_timestamp(),
                Some(i64::MAX),
                &json!({
                    "runtime_ms": i64::MAX, "active_ms": i64::MAX, "idle_ms": 0,
                    "integrity_status": "local",
                })
                .to_string(),
            )?;
            conn.execute(
                "UPDATE sessions SET runtime_ms = ?1, elapsed_monotonic_ms = ?1, active_ms = ?1
                 WHERE id = ?2",
                rusqlite::params![i64::MAX, open.id],
            )
            .map_err(map_db)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(problem(&db, &open.id).as_deref(), Some("time_out_of_range"));
        assert_eq!(listed(&db, &open.id).integrity_status, STATUS_SUSPICIOUS);
    }

    #[test]
    fn a_takeover_given_up_before_it_is_kept_leaves_this_pc_as_it_was() {
        let pc = test_pc("before");
        act_as(Some(&pc));
        let dir = std::env::temp_dir().join(format!("vaultime-takeover-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let id_file = dir.join(crate::constants::DEVICE_ID_FILE);
        fs::write(&id_file, "before").unwrap();
        let old_key = hex::encode(&[7; 32]);
        let takeover = Takeover {
            device_id: "after".into(),
            name: None,
            keys: HashSet::from([old_key.clone()]),
        };
        {
            let mut change = TakingOver::begin(&takeover).unwrap();
            change.write_files(&dir).unwrap();
            assert_eq!(this_device().unwrap(), "after");
            assert!(own_keys().unwrap().contains(&old_key));
            assert_eq!(fs::read_to_string(&id_file).unwrap(), "after");
        }
        assert_eq!(this_device().unwrap(), "before");
        assert!(!own_keys().unwrap().contains(&old_key));
        assert_eq!(fs::read_to_string(&id_file).unwrap(), "before");
        assert!(!dir.join(DEVICE_KEYS_FILE).exists());
        act_as(None);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn begins_with_the_sessions_it_finds() {
        let (db, game) = setup();
        let older = played(&db, &game);
        // As before ledgers existed.
        outside(&db, "DELETE FROM ledger_entries");
        assert_eq!(problem(&db, &older).as_deref(), Some("not_in_ledger"));

        let carried = db
            .with_transaction(|conn| begin_if_empty(conn, "first_start"))
            .unwrap();
        integrity::forget_checks();
        assert_eq!(carried, Some(1));
        assert_eq!(problem(&db, &older), None);
        let later = played(&db, &game);
        assert_eq!(problem(&db, &later), None);
        assert_eq!(
            db.with_transaction(|conn| begin_if_empty(conn, "first_start"))
                .unwrap(),
            None
        );
    }
}
