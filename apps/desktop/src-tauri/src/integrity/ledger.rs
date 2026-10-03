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
use serde::Serialize;
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

/// Session events the ledger pins. Checkpoints are left out, as there is one
/// every half minute and dropping the newest ones only takes time away.
fn pins(event_type: &str) -> bool {
    event_type != "heartbeat"
}

static KEY: OnceLock<SigningKey> = OnceLock::new();
static DEVICE: OnceLock<String> = OnceLock::new();
static OWN_KEYS: OnceLock<HashSet<String>> = OnceLock::new();

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
    let _ = OWN_KEYS.set(remember_own_key(app_dir, &key)?);
    let _ = KEY.set(key);
    let _ = DEVICE.set(device_id.to_owned());
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
        .get()
        .ok_or_else(|| VaultimeError::Integrity("this PC's device id is not known".into()))?;
    Ok((key.clone(), device.clone()))
}

#[cfg(test)]
thread_local! {
    /// The PC a test acts as, so one test can play several PCs.
    static TEST_PC: std::cell::RefCell<Option<(SigningKey, String)>> =
        const { std::cell::RefCell::new(None) };
}

/// Tests share one key, made up on first use, unless they act as a PC of
/// their own.
#[cfg(test)]
fn identity() -> Result<(SigningKey, String)> {
    if let Some(pc) = TEST_PC.with(|pc| pc.borrow().clone()) {
        return Ok(pc);
    }
    let key = KEY.get_or_init(|| SigningKey::from_bytes(&random_seed().unwrap()));
    let device = DEVICE
        .get()
        .cloned()
        .unwrap_or_else(|| "test-pc".to_owned());
    Ok((key.clone(), device))
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

/// Every key this PC has signed with, the current one included.
#[cfg(not(test))]
pub(crate) fn own_keys() -> Result<HashSet<String>> {
    let mut keys = OWN_KEYS.get().cloned().unwrap_or_default();
    keys.insert(key_id()?);
    Ok(keys)
}

/// A test PC signs with one key only.
#[cfg(test)]
pub(crate) fn own_keys() -> Result<HashSet<String>> {
    Ok(HashSet::from([key_id()?]))
}

/// The device id this PC's ledger names.
pub fn this_device() -> Result<String> {
    Ok(identity()?.1)
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
    conn.query_row(
        "SELECT sequence, hash_self FROM ledger_entries
         WHERE key_id = ?1 ORDER BY sequence DESC LIMIT 1",
        [key_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
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

/// Pins a session's chain at an event just appended, unless the ledger
/// leaves that kind of event out.
pub(crate) fn pin(
    conn: &Connection,
    session_id: &str,
    sequence: i64,
    hash: &str,
    event_type: &str,
) -> Result<()> {
    if !pins(event_type) {
        return Ok(());
    }
    append(
        conn,
        ENTRY_PINNED,
        Some(session_id),
        &json!({ "sequence": sequence, "hash": hash, "event_type": event_type }),
    )
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
    let mut cache = state_cache().lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((checked, state)) = cache.get(&database)
        && *checked == newest
    {
        return Ok(state.clone());
    }
    let state = check_ledgers(conn)?;
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
    let mut stmt = conn
        .prepare_cached(
            "SELECT key_id, payload_json FROM ledger_entries WHERE entry_type = 'trusted'",
        )
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(ledger_error)?;
    let mut trusted = HashSet::new();
    for row in rows {
        let (signer, payload) = row.map_err(ledger_error)?;
        let payload: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
        if mine.contains(&signer)
            && payload.get("device_id").and_then(Value::as_str) == Some(device_id)
            && let Some(key) = payload.get("key_id").and_then(Value::as_str)
        {
            trusted.insert(key.to_owned());
        }
    }
    Ok(trusted)
}

/// Every PC this PC's own ledgers trust keys of, with those keys.
pub(crate) fn all_trusted(conn: &Connection) -> Result<HashMap<String, HashSet<String>>> {
    let mine = own_keys()?;
    if any_broken(conn, &mine)? {
        return Ok(HashMap::new());
    }
    let mut stmt = conn
        .prepare_cached(
            "SELECT key_id, payload_json FROM ledger_entries WHERE entry_type = 'trusted'",
        )
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(ledger_error)?;
    let mut trusted: HashMap<String, HashSet<String>> = HashMap::new();
    for row in rows {
        let (signer, payload) = row.map_err(ledger_error)?;
        let payload: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
        if mine.contains(&signer)
            && let (Some(device), Some(key)) = (
                payload.get("device_id").and_then(Value::as_str),
                payload.get("key_id").and_then(Value::as_str),
            )
        {
            trusted
                .entry(device.to_owned())
                .or_default()
                .insert(key.to_owned());
        }
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
/// after a restore of an older backup. Rows it has stay as they are.
pub(crate) fn keep_seen(conn: &Connection, seen: &[SeenPc]) -> Result<()> {
    let now = super::now_timestamp();
    for (id, platform, app_version, name, merged_at) in seen {
        conn.execute(
            "INSERT OR IGNORE INTO devices (id, platform, app_version, name, merged_at)
             VALUES (?1, ?2, ?3, ?4, COALESCE(?5, ?6))",
            rusqlite::params![id, platform, app_version, name, merged_at, now],
        )
        .map_err(ledger_error)?;
    }
    Ok(())
}

/// Whose pins count for each session of a backup taken in here. A session
/// recorded on this PC counts only with this PC's own keys. One of a PC this
/// PC has seen counts with the keys it trusts for that PC, and with those of
/// the backup's PC if this PC trusted that one before, as it took the
/// session in after its own check. One of a PC never seen counts with the
/// keys of the backup's PC. The backup's PC counts with the keys its ledgers
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
            let unknown = !claimed.is_subset(&trusted);
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
        if trusted.is_empty() && !self.seen.contains(device) {
            return self.source_keys.clone();
        }
        if self.source_trusted {
            trusted.union(&self.source_keys).cloned().collect()
        } else {
            trusted
        }
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

/// The pins this PC's own keys hold in `conn`, by session, as sequence and
/// hash, so a restore can tell which sessions of a backup this PC vouched
/// for before.
pub(crate) fn own_pins(conn: &Connection) -> Result<HashMap<String, Vec<(i64, String)>>> {
    let mine = own_keys()?;
    let mut stmt = conn
        .prepare(
            "SELECT key_id, session_id, payload_json FROM ledger_entries
             WHERE session_id IS NOT NULL AND entry_type IN ('pinned', 'carried', 'adopted')",
        )
        .map_err(ledger_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(ledger_error)?;
    let mut pins: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    for row in rows {
        let (key, session, payload) = row.map_err(ledger_error)?;
        if !mine.contains(&key) {
            continue;
        }
        let payload: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
        if let (Some(sequence), Some(hash)) = (
            payload.get("sequence").and_then(Value::as_i64),
            payload.get("hash").and_then(Value::as_str),
        ) {
            pins.entry(session)
                .or_default()
                .push((sequence, hash.to_owned()));
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
/// taken in, and a closed session's time must lie in a pinned part of its
/// chain. `events` are in order and checked.
pub(crate) fn session_problem(
    conn: &Connection,
    session: &Session,
    events: &[SessionEvent],
    timing_events: &[&str],
    viewpoint: Viewpoint,
) -> Result<Option<&'static str>> {
    let pins: Vec<(String, String, String)> = {
        let mut stmt = conn
            .prepare_cached(
                "SELECT key_id, entry_type, payload_json FROM ledger_entries
                 WHERE session_id = ?1 AND entry_type IN ('pinned', 'carried', 'adopted')",
            )
            .map_err(ledger_error)?;
        let rows = stmt
            .query_map([&session.id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(ledger_error)?;
        rows.collect::<rusqlite::Result<_>>()
            .map_err(ledger_error)?
    };
    if pins.is_empty() {
        return Ok(Some("not_in_ledger"));
    }
    let state = ledger_state(conn)?;
    let counted = match viewpoint {
        Viewpoint::ThisPc => own_keys()?,
        Viewpoint::Keys(keys) => keys.clone(),
    };
    let mut covered_from_start = false;
    let mut newest_pinned = 0;
    for (key_id, entry_type, payload_json) in
        pins.iter().filter(|(key, _, _)| counted.contains(key))
    {
        if state.broken.contains(key_id) {
            return Ok(Some("ledger_broken"));
        }
        let payload: Value = serde_json::from_str(payload_json).unwrap_or(Value::Null);
        let (Some(sequence), Some(hash)) = (
            payload.get("sequence").and_then(Value::as_i64),
            payload.get("hash").and_then(Value::as_str),
        ) else {
            return Ok(Some("ledger_mismatch"));
        };
        let pinned = usize::try_from(sequence - 1)
            .ok()
            .and_then(|index| events.get(index));
        if pinned.is_none_or(|event| event.hash_self.as_deref() != Some(hash)) {
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

#[cfg(test)]
mod tests {
    use super::*;
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
