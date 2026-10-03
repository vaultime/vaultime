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

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock, PoisonError};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use log::warn;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::constants::DEVICE_KEY_FILE;
use crate::db::models::{Session, SessionEvent};
use crate::error::{Result, VaultimeError};
use crate::hex;
use crate::integrity::now_timestamp;

/// The ledger began on this PC, or began again after a restore of a backup
/// from before ledgers.
const ENTRY_BEGAN: &str = "began";
/// A session's chain up to one event, as this PC wrote it.
const ENTRY_PINNED: &str = "pinned";
/// A session's chain as it was when the ledger began, so older sessions are
/// covered too. It vouches for nothing before that moment.
const ENTRY_CARRIED: &str = "carried";
/// A session the player removed, with the reason.
const ENTRY_REMOVED: &str = "removed";
/// The history was replaced by a backup. Names where this PC's ledger stood.
const ENTRY_RESTORED: &str = "restored";

/// Session events the ledger pins. Checkpoints are left out, as there is one
/// every half minute and dropping the newest ones only takes time away.
fn pins(event_type: &str) -> bool {
    event_type != "heartbeat"
}

static KEY: OnceLock<SigningKey> = OnceLock::new();

/// Loads this PC's signing key from `app_dir`, or makes one on the first
/// start. Backups leave the file out, so the key stays on this PC. A damaged
/// file is set aside for a new key, and the ledger goes on under that one.
/// Its older entries stay checkable, as each names the key it was signed
/// with.
pub fn load_key(app_dir: &Path) -> Result<()> {
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
    let _ = KEY.set(SigningKey::from_bytes(&seed));
    Ok(())
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

#[cfg(not(test))]
fn key() -> Result<&'static SigningKey> {
    KEY.get()
        .ok_or_else(|| VaultimeError::Integrity("this PC's ledger key is not loaded".into()))
}

/// Tests share one key, made up on first use.
#[cfg(test)]
fn key() -> Result<&'static SigningKey> {
    Ok(KEY.get_or_init(|| SigningKey::from_bytes(&random_seed().unwrap())))
}

/// This PC's public key in hex, which names its ledger.
pub fn key_id() -> Result<String> {
    Ok(hex::encode(key()?.verifying_key().as_bytes()))
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

/// Appends an entry to this PC's ledger and signs it.
fn append(
    conn: &Connection,
    entry_type: &str,
    session_id: Option<&str>,
    payload: &Value,
) -> Result<()> {
    let key = key()?;
    let key_id = hex::encode(key.verifying_key().as_bytes());
    let (sequence, previous) = match tail(conn, &key_id)? {
        Some((sequence, hash)) => (sequence + 1, Some(hash)),
        None => (1, None),
    };
    let recorded_at = now_timestamp();
    let payload_json = payload.to_string();
    let hash = entry_hash(
        &key_id,
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

/// Notes that the player removed a session, so its absence is explained.
pub(crate) fn record_removed(conn: &Connection, session_id: &str, reason: &str) -> Result<()> {
    append(
        conn,
        ENTRY_REMOVED,
        Some(session_id),
        &json!({ "reason": reason }),
    )
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
        &json!({ "reason": reason, "sessions": tails.len() }),
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

/// Ledgers whose chain or newest signature does not hold, by key. A sound
/// chain whose newest entry carries a good signature is sound as a whole,
/// since each hash covers the one before it.
fn broken_keys(conn: &Connection) -> Result<HashSet<String>> {
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
    let mut broken = HashSet::new();
    let mut current: Option<(String, i64, String, String)> = None;
    let finish = |key_id: &str, hash: &str, signature: &str, broken: &mut HashSet<String>| {
        if !signed_by(key_id, hash, signature) {
            broken.insert(key_id.to_owned());
        }
    };
    for row in rows {
        let entry = row.map_err(ledger_error)?;
        let (expected_sequence, previous) = match &current {
            Some((key_id, sequence, hash, _)) if *key_id == entry.key_id => {
                (sequence + 1, Some(hash.clone()))
            }
            Some((key_id, _, hash, signature)) => {
                finish(key_id, hash, signature, &mut broken);
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
            broken.insert(entry.key_id.clone());
        }
        current = Some((
            entry.key_id,
            entry.sequence,
            entry.hash_self,
            entry.signature,
        ));
    }
    if let Some((key_id, _, hash, signature)) = &current {
        finish(key_id, hash, signature, &mut broken);
    }
    Ok(broken)
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

/// The broken ledgers as last checked, with the newest row then.
type BrokenCache = Mutex<Option<(Option<(i64, String)>, HashSet<String>)>>;

fn broken_cache() -> &'static BrokenCache {
    static CACHE: OnceLock<BrokenCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// `broken_keys`, reused while no ledger gained an entry. The app only ever
/// appends, so the newest row tells whether anything changed, in one step
/// down the index. Changes made by other programs, and a restore, clear it
/// through `forget`.
fn broken_keys_cached(conn: &Connection) -> Result<HashSet<String>> {
    let newest: Option<(i64, String)> = conn
        .prepare_cached("SELECT rowid, hash_self FROM ledger_entries ORDER BY rowid DESC LIMIT 1")
        .and_then(|mut stmt| {
            stmt.query_row([], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()
        })
        .map_err(ledger_error)?;
    let mut cache = broken_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some((checked, broken)) = cache.as_ref()
        && *checked == newest
    {
        return Ok(broken.clone());
    }
    let broken = broken_keys(conn)?;
    *cache = Some((newest, broken.clone()));
    Ok(broken)
}

/// Forgets the remembered ledger check.
pub(crate) fn forget() {
    *broken_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
}

/// Why a session's chain disagrees with the ledgers, if it does. Every pin
/// must match the chain, a ledger must cover the session from its start or
/// from when the ledger began, and a closed session's time must lie in a
/// pinned part of its chain. `events` are in order and already checked.
pub(crate) fn session_problem(
    conn: &Connection,
    session: &Session,
    events: &[SessionEvent],
    timing_events: &[&str],
) -> Result<Option<&'static str>> {
    let pins: Vec<(String, String, String)> = {
        let mut stmt = conn
            .prepare_cached(
                "SELECT key_id, entry_type, payload_json FROM ledger_entries
                 WHERE session_id = ?1 AND entry_type IN ('pinned', 'carried')",
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
    let broken = if pins.is_empty() {
        HashSet::new()
    } else {
        broken_keys_cached(conn)?
    };
    let mut covered_from_start = false;
    let mut newest_pinned = 0;
    for (key_id, entry_type, payload_json) in &pins {
        if broken.contains(key_id) {
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
        covered_from_start |= sequence == 1 || entry_type == ENTRY_CARRIED;
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
    /// Sessions that a ledger covers.
    pub covered_sessions: i64,
    /// Sessions a ledger names that are gone without a note that the player
    /// removed them.
    pub missing_sessions: i64,
    /// Whether any ledger fails its check.
    pub broken: bool,
}

pub fn report(conn: &Connection) -> Result<LedgerReport> {
    let key_id = key_id()?;
    let count = |sql: &str, key: &[&dyn rusqlite::ToSql]| -> Result<i64> {
        conn.query_row(sql, key, |row| row.get(0))
            .map_err(ledger_error)
    };
    let began_at = conn
        .query_row(
            "SELECT recorded_at FROM ledger_entries WHERE key_id = ?1 AND sequence = 1",
            [&key_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(ledger_error)?;
    Ok(LedgerReport {
        began_at,
        entries: count(
            "SELECT COUNT(*) FROM ledger_entries WHERE key_id = ?1",
            &[&key_id],
        )?,
        covered_sessions: count(
            "SELECT COUNT(*) FROM sessions s WHERE EXISTS
                 (SELECT 1 FROM ledger_entries l WHERE l.session_id = s.id)",
            &[],
        )?,
        missing_sessions: count(
            "SELECT COUNT(DISTINCT l.session_id) FROM ledger_entries l
             WHERE l.entry_type IN ('pinned', 'carried')
               AND NOT EXISTS (SELECT 1 FROM sessions s WHERE s.id = l.session_id)
               AND NOT EXISTS (SELECT 1 FROM ledger_entries r
                               WHERE r.entry_type = 'removed' AND r.session_id = l.session_id)",
            &[],
        )?,
        broken: !broken_keys_cached(conn)?.is_empty(),
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
        load_key(&dir).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join(DEVICE_KEY_FILE).with_extension("damaged")).unwrap(),
            "not a key"
        );
        let fresh = fs::read_to_string(dir.join(DEVICE_KEY_FILE)).unwrap();
        assert_eq!(hex::decode(&fresh).map(|bytes| bytes.len()), Some(32));
        // Kept from then on.
        load_key(&dir).unwrap();
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
