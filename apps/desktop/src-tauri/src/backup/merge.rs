// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Merging the sessions of another PC from one of its backups.
//!
//! Sessions come over byte for byte, rows and events as that PC wrote them.
//! Nothing is hashed, pointed or signed again, since the game and the PC a
//! session belongs to are part of its hashed history. Before this PC takes a
//! session in, it checks it the way the other PC sees it, inside the backup
//! and against that PC's ledger. Only then does this PC's own ledger vouch
//! for it. A session that fails comes in unvouched and shows Suspicious, as
//! it does there, so nothing is laundered. A session still running there
//! waits for the next merge. Sessions of this PC are never taken from a
//! backup, sessions removed here never come back, and a session that grew
//! there replaces the copy here only when that copy is the start of it.
//! Games that come along are never tracked here. Each can be linked to a
//! game of this PC, which it then counts as.
//!
//! The other PC's keys are the ones its ledgers name. This PC trusts them
//! when it has never seen that PC, the way any new PC is taken at its word,
//! and records that in its own ledger. Later, a key that newly claims to be
//! that PC counts only once the player vouches for it, so a backup changed
//! elsewhere cannot bring a ledger of its own to vouch for made up sessions.
//! See `ledger::Trust` for whose keys count for which session. A backup
//! holds only the database parts this version creates, so the copy takes
//! exactly the rows its check saw.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use log::warn;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use super::{
    LocalBackupManifest, auto, check_integrity, check_plain_schema, check_restored_ids,
    check_same_schema, cleanup_staging_dir, create_dir, ensure_schema_supported,
    load_and_validate_manifest, resolve_backup_dir, table_columns,
};
use crate::AppContext;
use crate::assets::{AssetManager, is_plain_name};
use crate::constants::{ASSET_CACHE_DIR, DATABASE_FILE};
use crate::db::connection::Database;
use crate::db::models::Session;
use crate::db::repo::sessions::row_to_session;
use crate::db::repo::{games, map_db};
use crate::error::{Result, VaultimeError};
use crate::integrity::{self, ledger};
use crate::platform::process::file_name;
use crate::playtime::slices;

/// The schema name a backup is attached under while it is copied.
const SOURCE: &str = "merge_source";

/// A game that comes with the merged sessions.
#[derive(Debug, Clone, Serialize)]
pub struct MergeGame {
    pub game_id: String,
    pub title: String,
    pub launcher_source: Option<String>,
    /// Sessions of it that come in.
    pub sessions: usize,
    pub runtime_ms: i64,
    /// The game of this PC it most likely is.
    pub suggested_game_id: Option<String>,
    /// Why: `launcher` for the same launcher and launcher id, `title` for
    /// the same title.
    pub suggested_because: Option<String>,
}

/// A session that stays as it is here.
#[derive(Debug, Clone, Serialize)]
pub struct MergeConflict {
    pub session_id: String,
    pub game_title: String,
    pub started_at_wall: String,
    /// `changed_on_both` when both copies went on differently,
    /// `update_fails_check` when the copy in the backup grew but fails its
    /// check.
    pub reason: String,
}

/// What a merge would bring in, without changing anything.
#[derive(Debug, Clone, Serialize)]
pub struct MergePreview {
    pub backup_path: String,
    pub backup_created_at: String,
    pub device_id: String,
    pub device_name: Option<String>,
    /// Sessions that are not here yet.
    pub new_sessions: usize,
    /// Sessions here that went on there.
    pub grown_sessions: usize,
    pub already_here: usize,
    /// Sessions that went on further here than in the backup.
    pub newer_here: usize,
    pub removed_here: usize,
    pub running_there: usize,
    /// Sessions that come in without this PC vouching for them, as they fail
    /// their check there too. They show Suspicious.
    pub failing: usize,
    /// Whether the backup holds a ledger that claims to be its PC but signs
    /// with a key this PC does not trust for it. Its pins count only when
    /// the player vouches for them.
    pub unknown_keys: bool,
    /// Whether this PC has never seen the backup's PC, so its ledgers are
    /// taken at their word.
    pub first_merge: bool,
    /// Sessions here that came in unvouched before and pass their check
    /// now, so this PC vouches for them.
    pub vouched_now: usize,
    /// Sessions added by hand here that overlap sessions that come in. They
    /// may be the same play.
    pub overlapping_manual: usize,
    pub conflicts: Vec<MergeConflict>,
    pub games: Vec<MergeGame>,
}

/// What the player chose for a game that comes along.
#[derive(Debug, Clone, Deserialize)]
pub struct GameChoice {
    pub game_id: String,
    /// The game of this PC it counts as, or none to keep it on its own.
    pub linked_game_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MergeSummary {
    pub device_id: String,
    pub device_name: Option<String>,
    pub sessions_added: usize,
    pub sessions_grown: usize,
    /// Sessions here this PC vouches for now.
    pub sessions_vouched: usize,
    pub failing: usize,
    pub games_added: usize,
    pub games_linked: usize,
    /// The backup saved just before, which a restore can go back to.
    pub safety_backup_path: Option<String>,
}

enum Take {
    New,
    Grown {
        from_sequence: i64,
    },
    /// Here already as it is there, and vouched for now.
    Vouch,
}

/// The hash of each event of a session, in order.
type Hashes = Vec<Option<String>>;

/// A session in the backup with the hash of each of its events.
type Candidate = (Session, Hashes);

/// The sessions to take in, and those that stay as they are here with why.
type Sorted = (Vec<(Session, Take, Hashes)>, Vec<(Session, &'static str)>);

struct Planned {
    session: Session,
    take: Take,
    vouched: bool,
    /// The events its check saw. What is copied in has to match them.
    hashes: Hashes,
}

struct Plan {
    preview: MergePreview,
    sessions: Vec<Planned>,
    new_games: HashSet<String>,
    trust: ledger::Trust,
}

/// A backup copied into a folder of this app and opened there, migrated to
/// this version's schema. The backup itself stays untouched.
struct Staged {
    dir: PathBuf,
    db: Option<Database>,
    db_path: PathBuf,
    backup_dir: PathBuf,
    manifest: LocalBackupManifest,
    artwork: Vec<String>,
}

impl Staged {
    fn open(app_context: &AppContext, backup_path: &Path) -> Result<Self> {
        let backup_dir = resolve_backup_dir(backup_path)?;
        let (manifest, artwork) = load_and_validate_manifest(&backup_dir)?;
        ensure_schema_supported(&manifest.schema_migrations)?;
        let dir = app_context
            .app_dir
            .join(format!(".merge-{}", uuid::Uuid::new_v4()));
        create_dir(&dir)?;
        let db_path = dir.join(DATABASE_FILE);
        // The folder goes again when this is dropped, also after a failure.
        let mut staged = Self {
            dir,
            db: None,
            db_path,
            backup_dir,
            manifest,
            artwork,
        };
        fs::copy(staged.backup_dir.join(DATABASE_FILE), &staged.db_path).map_err(|error| {
            VaultimeError::Backup(format!("failed to stage the backup database: {error}"))
        })?;
        check_plain_schema(&staged.db_path)?;
        check_integrity(&staged.db_path)?;
        staged.db = Some(Database::open(&staged.db_path)?);
        check_same_schema(staged.db()?)?;
        check_restored_ids(&staged.db_path)?;
        Ok(staged)
    }

    fn db(&self) -> Result<&Database> {
        self.db
            .as_ref()
            .ok_or_else(|| VaultimeError::Backup("the staged backup is closed".into()))
    }

    fn close(&mut self) {
        self.db = None;
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        self.db = None;
        cleanup_staging_dir(&self.dir);
    }
}

/// What merging the backup at `backup_path` would bring in.
pub fn preview(
    db: &Database,
    app_context: &AppContext,
    backup_path: &Path,
    trust_new_keys: bool,
) -> Result<MergePreview> {
    let staged = Staged::open(app_context, backup_path)?;
    Ok(plan(db, &staged, trust_new_keys)?.preview)
}

/// Merges the backup at `backup_path`, with the player's choice for each
/// game that comes along. With `trust_new_keys`, the player vouches that new
/// keys claiming the backup's PC are its own. Saves a backup of this PC
/// first.
pub fn apply(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    backup_path: &Path,
    choices: &[GameChoice],
    trust_new_keys: bool,
) -> Result<MergeSummary> {
    let mut staged = Staged::open(app_context, backup_path)?;
    let plan = plan(db, &staged, trust_new_keys)?;
    let links = chosen_links(db, &plan, choices)?;
    let source = staged.manifest.source_device_id.clone();
    let mut summary = MergeSummary {
        device_id: source.clone(),
        device_name: plan.preview.device_name.clone(),
        sessions_added: 0,
        sessions_grown: 0,
        sessions_vouched: 0,
        failing: plan
            .sessions
            .iter()
            .filter(|planned| !planned.vouched)
            .count(),
        games_added: plan.new_games.len(),
        games_linked: links.len(),
        safety_backup_path: None,
    };
    if plan.sessions.is_empty() {
        return Ok(summary);
    }
    summary.safety_backup_path =
        Some(auto::back_up_now(db, asset_manager, app_context)?.backup_path);

    // The staged copy is attached for copying, so its own connection closes.
    staged.close();
    let copied_assets = db.with_conn(|conn| {
        attach(conn, &staged.db_path)?;
        // Rolled back when dropped unless committed, also when the commit fails.
        let written = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
            .map_err(map_db)
            .and_then(|transaction| {
                let copied = copy_in(
                    &transaction,
                    &plan,
                    &links,
                    &source,
                    &staged.manifest.backup_id,
                    asset_manager,
                )?;
                transaction.commit().map_err(map_db)?;
                Ok(copied)
            });
        let detached = conn
            .execute_batch(&format!("DETACH DATABASE {SOURCE}"))
            .map_err(map_db);
        let copied = written?;
        detached?;
        Ok(copied)
    })?;
    integrity::forget_checks();
    copy_artwork(&staged, asset_manager, &copied_assets);
    // Quarter hours are a cache, worked out in small batches after the copy.
    let merged: Vec<String> = plan
        .sessions
        .iter()
        .filter(|planned| !matches!(planned.take, Take::Vouch))
        .map(|planned| planned.session.id.clone())
        .collect();
    slices::rebuild_merged(db, &merged)?;

    for planned in &plan.sessions {
        match planned.take {
            Take::New => summary.sessions_added += 1,
            Take::Grown { .. } => summary.sessions_grown += 1,
            Take::Vouch => summary.sessions_vouched += 1,
        }
    }
    Ok(summary)
}

/// Refuses backups that cannot be merged, then sorts every session in it.
fn plan(db: &Database, staged: &Staged, trust_new_keys: bool) -> Result<Plan> {
    let source = staged.manifest.source_device_id.clone();
    let staged_db = staged.db()?;
    // Trust recorded in a ledger that fails its check cannot be relied on.
    if db.with_conn(|conn| ledger::any_broken(conn, &ledger::own_keys()?))? {
        return Err(VaultimeError::Invalid(
            "This PC's ledger was changed outside Vaultime, so it cannot vouch for sessions of another PC now.".into(),
        ));
    }
    let (trust, unknown_keys) = staged_db.with_conn(|staged_conn| {
        db.with_conn(|local| ledger::Trust::new(local, staged_conn, &source, trust_new_keys))
    })?;
    let (device_name, candidates) = read_candidates(staged_db, &source, &trust)?;
    let mut preview = MergePreview {
        backup_path: staged.backup_dir.to_string_lossy().into_owned(),
        backup_created_at: staged.manifest.created_at.clone(),
        device_id: source.clone(),
        device_name,
        new_sessions: 0,
        grown_sessions: 0,
        already_here: 0,
        newer_here: 0,
        removed_here: 0,
        running_there: 0,
        failing: 0,
        unknown_keys,
        first_merge: trust.first_merge(),
        vouched_now: 0,
        overlapping_manual: 0,
        conflicts: Vec::new(),
        games: Vec::new(),
    };
    let (wanted, staying) = sort_candidates(db, candidates, &mut preview)?;
    let sessions = check_candidates(staged_db, &trust, wanted, staying, &mut preview)?;
    let (games, new_games) = games_coming(db, staged_db, &sessions)?;
    preview.games = games;
    preview.overlapping_manual = overlapping_manual(db, &sessions)?;
    Ok(Plan {
        preview,
        sessions,
        new_games,
        trust,
    })
}

/// The sessions of other PCs in the backup, each with its event hashes, and
/// the name of the backup's PC. Read on the backup alone, so the tracker is
/// not held up.
fn read_candidates(
    staged: &Database,
    source: &str,
    trust: &ledger::Trust,
) -> Result<(Option<String>, Vec<Candidate>)> {
    let this = ledger::this_device()?;
    staged.with_conn(|conn| {
        refuse_unmergeable(conn, source, &this, trust.source_keys())?;
        let device_name: Option<String> = conn
            .query_row("SELECT name FROM devices WHERE id = ?1", [source], |row| {
                row.get(0)
            })
            .optional()
            .map_err(map_db)?
            .flatten();
        let mut stmt = conn
            .prepare("SELECT * FROM sessions WHERE device_id != ?1 ORDER BY started_at_wall")
            .map_err(map_db)?;
        let sessions = stmt
            .query_map([&this], row_to_session)
            .map_err(map_db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db)?;
        let mut candidates = Vec::with_capacity(sessions.len());
        for session in sessions {
            let hashes = event_hashes(conn, &session.id)?;
            candidates.push((session, hashes));
        }
        Ok((device_name, candidates))
    })
}

/// How each session stands against this PC's copy: the ones to take in or to
/// vouch for now, and the ones that stay as they are here.
fn sort_candidates(
    db: &Database,
    candidates: Vec<Candidate>,
    preview: &mut MergePreview,
) -> Result<Sorted> {
    let mut wanted = Vec::new();
    let mut staying = Vec::new();
    db.with_conn(|conn| {
        for (session, theirs) in candidates {
            if session.ended_at_wall.is_none() {
                preview.running_there += 1;
                continue;
            }
            if ledger::removed_here(conn, &session.id)? {
                preview.removed_here += 1;
                continue;
            }
            let ours = event_hashes(conn, &session.id)?;
            if ours.is_empty() {
                wanted.push((session, Take::New, theirs));
            } else if ours == theirs {
                if unvouched_here(conn, &session.id)? {
                    wanted.push((session, Take::Vouch, theirs));
                } else {
                    preview.already_here += 1;
                }
            } else if theirs.starts_with(&ours) {
                let from_sequence = i64::try_from(ours.len()).unwrap_or(i64::MAX) + 1;
                wanted.push((session, Take::Grown { from_sequence }, theirs));
            } else if ours.starts_with(&theirs) {
                preview.newer_here += 1;
            } else {
                staying.push((session, "changed_on_both"));
            }
        }
        Ok(())
    })?;
    Ok((wanted, staying))
}

/// Whether this PC's copy of a session is sound but no pin of this PC
/// vouches for it, as after a merge whose keys it did not trust yet.
fn unvouched_here(conn: &Connection, session_id: &str) -> Result<bool> {
    let session = conn
        .query_row(
            "SELECT * FROM sessions WHERE id = ?1",
            [session_id],
            row_to_session,
        )
        .optional()
        .map_err(map_db)?;
    Ok(match session {
        Some(session) => {
            integrity::validate_session_history(conn, &session)?.as_deref() == Some("not_in_ledger")
        }
        None => false,
    })
}

/// Sessions added by hand here that overlap a session that comes in.
fn overlapping_manual(db: &Database, sessions: &[Planned]) -> Result<usize> {
    let manual: Vec<(String, String)> = db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT started_at_wall, ended_at_wall FROM sessions
                 WHERE integrity_status = ?1 AND ended_at_wall IS NOT NULL AND runtime_ms > 0",
            )
            .map_err(map_db)?;
        let rows = stmt
            .query_map([integrity::STATUS_MANUAL], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(map_db)?;
        rows.collect::<rusqlite::Result<_>>().map_err(map_db)
    })?;
    Ok(manual
        .iter()
        .filter(|(start, end)| {
            sessions.iter().any(|planned| {
                !matches!(planned.take, Take::Vouch)
                    && planned.session.started_at_wall < *end
                    && planned
                        .session
                        .ended_at_wall
                        .as_ref()
                        .is_some_and(|their_end| their_end > start)
            })
        })
        .count())
}

/// Checks each session that would come in with the keys that count for
/// the PC that recorded it. A new one that fails still comes in, unvouched.
/// A grown one that fails leaves the copy here as it is, and one here
/// already that fails stays unvouched.
fn check_candidates(
    staged: &Database,
    trust: &ledger::Trust,
    wanted: Vec<(Session, Take, Hashes)>,
    staying: Vec<(Session, &'static str)>,
    preview: &mut MergePreview,
) -> Result<Vec<Planned>> {
    let mut sessions = Vec::with_capacity(wanted.len());
    staged.with_conn(|conn| {
        let mut stmt = conn
            .prepare_cached("SELECT title FROM games WHERE id = ?1")
            .map_err(map_db)?;
        let mut conflict = |session: Session, reason: &str| -> Result<MergeConflict> {
            Ok(MergeConflict {
                game_title: stmt
                    .query_row([&session.game_id], |row| row.get(0))
                    .optional()
                    .map_err(map_db)?
                    .unwrap_or_default(),
                session_id: session.id,
                started_at_wall: session.started_at_wall,
                reason: reason.into(),
            })
        };
        for (session, reason) in staying {
            preview.conflicts.push(conflict(session, reason)?);
        }
        for (session, take, hashes) in wanted {
            let keys = trust.keys_for(&session.device_id);
            let vouched = integrity::validate_session_history_from(
                conn,
                &session,
                ledger::Viewpoint::Keys(&keys),
            )?
            .is_none();
            match take {
                Take::Grown { .. } if !vouched => {
                    preview
                        .conflicts
                        .push(conflict(session, "update_fails_check")?);
                    continue;
                }
                Take::Grown { .. } => preview.grown_sessions += 1,
                Take::New => {
                    preview.new_sessions += 1;
                    preview.failing += usize::from(!vouched);
                }
                Take::Vouch if !vouched => {
                    preview.already_here += 1;
                    continue;
                }
                Take::Vouch => preview.vouched_now += 1,
            }
            sessions.push(Planned {
                session,
                take,
                vouched,
                hashes,
            });
        }
        Ok(())
    })?;
    Ok(sessions)
}

/// Refuses a backup of this PC, of a PC that keeps no ledger, and one whose
/// trusted ledger fails its check.
fn refuse_unmergeable(
    staged: &Connection,
    source: &str,
    this: &str,
    trusted: &HashSet<String>,
) -> Result<()> {
    let claimed = ledger::claimed_keys(staged, source)?;
    if source == this {
        let ours = ledger::own_keys()?;
        return Err(VaultimeError::Invalid(
            if !claimed.is_empty() && claimed.is_disjoint(&ours) {
                "This backup comes from a PC that uses the same id as this one, so their sessions cannot be told apart."
            } else {
                "This backup comes from this PC. Restore it to go back to it."
            }
            .into(),
        ));
    }
    if claimed.is_empty() {
        return Err(VaultimeError::Invalid(
            "The PC this backup comes from keeps no ledger yet. Update Vaultime there and save a new backup.".into(),
        ));
    }
    if ledger::any_broken(staged, trusted)? {
        return Err(VaultimeError::Invalid(
            "The ledger in this backup was changed outside Vaultime, so nothing can be merged from it.".into(),
        ));
    }
    Ok(())
}

/// The hash of each event of a session, in order.
fn event_hashes(conn: &Connection, session_id: &str) -> Result<Hashes> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT hash_self FROM session_events WHERE session_id = ?1 ORDER BY sequence",
        )
        .map_err(map_db)?;
    let rows = stmt
        .query_map([session_id], |row| row.get(0))
        .map_err(map_db)?;
    rows.collect::<rusqlite::Result<_>>().map_err(map_db)
}

/// The games of the incoming sessions that are not here yet, each with the
/// game of this PC it most likely is.
fn games_coming(
    local: &Database,
    staged: &Database,
    sessions: &[Planned],
) -> Result<(Vec<MergeGame>, HashSet<String>)> {
    let ours = games::list_local_games(local)?;
    let known: HashSet<String> = games::list_all_games(local)?
        .into_iter()
        .map(|game| game.id)
        .collect();
    let mut coming: HashMap<String, (usize, i64)> = HashMap::new();
    for planned in sessions {
        if !known.contains(&planned.session.game_id) {
            let entry = coming.entry(planned.session.game_id.clone()).or_default();
            entry.0 += 1;
            entry.1 += planned.session.runtime_ms;
        }
    }
    let by_launcher: HashMap<(String, String), String> = ours
        .iter()
        .filter_map(|game| {
            Some((
                (game.launcher_source.clone()?, games::launcher_id(game)?),
                game.id.clone(),
            ))
        })
        .collect();
    let by_title: HashMap<String, String> = ours
        .iter()
        .map(|game| (comparable_title(&game.title), game.id.clone()))
        .collect();
    let theirs = games::list_all_games(staged)?;
    let mut games = Vec::new();
    for game in theirs
        .into_iter()
        .filter(|game| coming.contains_key(&game.id))
    {
        let (sessions, runtime_ms) = coming[&game.id];
        let launcher_match = game
            .launcher_source
            .clone()
            .zip(games::launcher_id(&game))
            .and_then(|key| by_launcher.get(&key));
        let (suggested_game_id, suggested_because) = match launcher_match {
            Some(id) => (Some(id.clone()), Some("launcher".to_owned())),
            None => match by_title.get(&comparable_title(&game.title)) {
                Some(id) => (Some(id.clone()), Some("title".to_owned())),
                None => (None, None),
            },
        };
        games.push(MergeGame {
            game_id: game.id,
            title: game.title,
            launcher_source: game.launcher_source,
            sessions,
            runtime_ms,
            suggested_game_id,
            suggested_because,
        });
    }
    games.sort_by_key(|game| std::cmp::Reverse(game.runtime_ms));
    let new_games = games.iter().map(|game| game.game_id.clone()).collect();
    Ok((games, new_games))
}

/// A title as compared: lowercase, letters and digits only.
fn comparable_title(title: &str) -> String {
    title
        .chars()
        .filter(|char| char.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The links the player chose, checked: each from a game that comes in to a
/// game of this PC.
fn chosen_links(
    db: &Database,
    plan: &Plan,
    choices: &[GameChoice],
) -> Result<Vec<(String, String)>> {
    let ours: HashSet<String> = games::list_local_games(db)?
        .into_iter()
        .map(|game| game.id)
        .collect();
    let mut links = Vec::new();
    for choice in choices {
        let Some(linked) = &choice.linked_game_id else {
            continue;
        };
        if !plan.new_games.contains(&choice.game_id) || !ours.contains(linked) {
            return Err(VaultimeError::Invalid(
                "A game can only be linked to a game of this PC.".into(),
            ));
        }
        links.push((choice.game_id.clone(), linked.clone()));
    }
    Ok(links)
}

fn attach(conn: &Connection, path: &Path) -> Result<()> {
    conn.execute(
        &format!("ATTACH DATABASE ?1 AS {SOURCE}"),
        [path.to_string_lossy()],
    )
    .map_err(map_db)?;
    Ok(())
}

/// `INSERT` of the given columns of `table` from the attached backup.
fn copy_statement(conn: &Connection, table: &str, conflict: &str, filter: &str) -> Result<String> {
    let columns = table_columns(conn, "main", table)?.join(", ");
    Ok(format!(
        "INSERT {conflict} INTO main.{table} ({columns})
         SELECT {columns} FROM {SOURCE}.{table} WHERE {filter}"
    ))
}

/// A list of ids as the JSON array the copy statements read with
/// `json_each`.
fn json_list<'a>(ids: impl IntoIterator<Item = &'a str>) -> String {
    serde_json::to_string(&ids.into_iter().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into())
}

/// Matches an id against the JSON list in the first parameter.
const IN_LIST: &str = "IN (SELECT value FROM json_each(?1))";

/// Why a merge stops when the copy does not match what was checked.
const READ_DIFFERENTLY: &str =
    "this backup reads differently from one moment to the next, so nothing was merged";

/// Stops the merge unless a copy wrote exactly the rows the plan expects.
fn expect_rows(written: usize, expected: usize) -> Result<()> {
    if written == expected {
        Ok(())
    } else {
        Err(VaultimeError::Backup(READ_DIFFERENTLY.into()))
    }
}

/// Copies everything the plan takes in, inside the open transaction, and
/// vouches for the sessions that passed. Returns the artwork rows it copied,
/// as game id and file name.
fn copy_in(
    conn: &Connection,
    plan: &Plan,
    links: &[(String, String)],
    source: &str,
    backup_id: &str,
    asset_manager: &AssetManager,
) -> Result<Vec<(String, String)>> {
    // The plan was made before the safety backup, so nothing here may have
    // changed since: no session that comes in was removed meanwhile, and
    // the ones vouched for now are as they were checked.
    for planned in &plan.sessions {
        let changed = match planned.take {
            Take::New => ledger::removed_here(conn, &planned.session.id)?,
            Take::Vouch => event_hashes(conn, &planned.session.id)? != planned.hashes,
            Take::Grown { .. } => false,
        };
        if changed {
            return Err(VaultimeError::Invalid(
                "Sessions here changed while merging, so nothing was merged. Try again.".into(),
            ));
        }
    }
    // Until the merged sessions have their quarter hours, the next start
    // builds every slice again.
    slices::mark_stale(conn)?;
    let devices: HashSet<&str> = plan
        .sessions
        .iter()
        .map(|planned| planned.session.device_id.as_str())
        .collect();
    copy_devices(conn, &json_list(devices), source)?;
    // From now on, only these keys count for that PC.
    plan.trust.record(conn, false)?;
    copy_games(conn, plan, links, source)?;
    copy_sessions(conn, plan)?;
    // What came in has to be what was checked, event by event.
    for planned in &plan.sessions {
        if event_hashes(conn, &planned.session.id)? != planned.hashes {
            return Err(VaultimeError::Backup(READ_DIFFERENTLY.into()));
        }
    }
    let linked: HashSet<&str> = links.iter().map(|(game, _)| game.as_str()).collect();
    let copied = copy_artwork_rows(
        conn,
        &json_list(
            plan.new_games
                .iter()
                .map(String::as_str)
                .filter(|game| !linked.contains(game)),
        ),
        asset_manager,
    )?;
    // This PC vouches only for sessions that passed, at the event checked.
    for planned in plan.sessions.iter().filter(|planned| planned.vouched) {
        let (Some(sequence), Some(Some(hash))) = (
            i64::try_from(planned.hashes.len()).ok(),
            planned.hashes.last(),
        ) else {
            continue;
        };
        ledger::adopt(
            conn,
            &planned.session.id,
            sequence,
            hash,
            source,
            "merge",
            backup_id,
        )?;
    }
    Ok(copied)
}

/// The PCs of the sessions, marked as merged, so only they may change their
/// sessions. The backup's PC keeps the name it gave itself.
fn copy_devices(conn: &Connection, devices: &str, source: &str) -> Result<()> {
    conn.execute(
        &copy_statement(conn, "devices", "OR IGNORE", &format!("id {IN_LIST}"))?,
        [devices],
    )
    .map_err(map_db)?;
    conn.execute(
        &format!("UPDATE main.devices SET merged_at = ?2 WHERE id {IN_LIST}"),
        params![devices, integrity::now_timestamp()],
    )
    .map_err(map_db)?;
    conn.execute(
        &format!(
            "UPDATE main.devices SET name = (SELECT name FROM {SOURCE}.devices WHERE id = ?1)
             WHERE id = ?1 AND (SELECT name FROM {SOURCE}.devices WHERE id = ?1) IS NOT NULL"
        ),
        [source],
    )
    .map_err(map_db)?;
    Ok(())
}

/// Games that come along, never tracked here, their status changes and the
/// links the player chose.
fn copy_games(
    conn: &Connection,
    plan: &Plan,
    links: &[(String, String)],
    source: &str,
) -> Result<()> {
    let games = json_list(plan.new_games.iter().map(String::as_str));
    let written = conn
        .execute(
            &copy_statement(conn, "games", "", &format!("id {IN_LIST}"))?,
            [&games],
        )
        .map_err(map_db)?;
    expect_rows(written, plan.new_games.len())?;
    conn.execute(
        &format!(
            "UPDATE main.games SET origin_device_id = CASE
                 WHEN origin_device_id IS NULL OR origin_device_id = ?3 THEN ?2
                 ELSE origin_device_id END
             WHERE id {IN_LIST}"
        ),
        params![games, source, ledger::this_device()?],
    )
    .map_err(map_db)?;
    for (game, local) in links {
        conn.execute(
            "INSERT INTO main.game_links (game_id, linked_game_id) VALUES (?1, ?2)",
            params![game, local],
        )
        .map_err(map_db)?;
    }
    conn.execute(
        &copy_statement(
            conn,
            "game_status_changes",
            "OR IGNORE",
            &format!("game_id {IN_LIST}"),
        )?,
        [&games],
    )
    .map_err(map_db)?;
    Ok(())
}

/// Sessions and their events as they are, the missing events of sessions
/// that grew, and notes, of which the one written last stays.
fn copy_sessions(conn: &Connection, plan: &Plan) -> Result<()> {
    let new: Vec<&Planned> = plan
        .sessions
        .iter()
        .filter(|planned| matches!(planned.take, Take::New))
        .collect();
    let new_ids = json_list(new.iter().map(|planned| planned.session.id.as_str()));
    let written = conn
        .execute(
            &copy_statement(conn, "sessions", "", &format!("id {IN_LIST}"))?,
            [&new_ids],
        )
        .map_err(map_db)?;
    expect_rows(written, new.len())?;
    let written = conn
        .execute(
            &copy_statement(conn, "session_events", "", &format!("session_id {IN_LIST}"))?,
            [&new_ids],
        )
        .map_err(map_db)?;
    expect_rows(
        written,
        new.iter().map(|planned| planned.hashes.len()).sum(),
    )?;
    let session_columns = table_columns(conn, "main", "sessions")?.join(", ");
    for planned in &plan.sessions {
        let Take::Grown { from_sequence } = planned.take else {
            continue;
        };
        let written = conn
            .execute(
                &copy_statement(
                    conn,
                    "session_events",
                    "",
                    "session_id = ?1 AND sequence >= ?2",
                )?,
                params![planned.session.id, from_sequence],
            )
            .map_err(map_db)?;
        let kept = usize::try_from(from_sequence - 1).unwrap_or(usize::MAX);
        expect_rows(written, planned.hashes.len().saturating_sub(kept))?;
        let written = conn
            .execute(
                &format!(
                    "UPDATE main.sessions SET ({session_columns}) =
                         (SELECT {session_columns} FROM {SOURCE}.sessions WHERE id = ?1)
                     WHERE id = ?1"
                ),
                [&planned.session.id],
            )
            .map_err(map_db)?;
        expect_rows(written, 1)?;
    }
    conn.execute(
        &format!(
            "INSERT INTO main.session_notes (session_id, note, updated_at)
             SELECT session_id, note, updated_at FROM {SOURCE}.session_notes
             WHERE session_id {IN_LIST}
             ON CONFLICT(session_id) DO UPDATE SET note = excluded.note,
                 updated_at = excluded.updated_at
             WHERE excluded.updated_at > main.session_notes.updated_at"
        ),
        [json_list(
            plan.sessions
                .iter()
                .map(|planned| planned.session.id.as_str()),
        )],
    )
    .map_err(map_db)?;
    Ok(())
}

/// Artwork rows of games that stay on their own, pointed at this PC's cache.
/// Returns each row's game id and file name, for the files to follow.
fn copy_artwork_rows(
    conn: &Connection,
    games: &str,
    asset_manager: &AssetManager,
) -> Result<Vec<(String, String)>> {
    conn.execute(
        &copy_statement(
            conn,
            "game_assets",
            "OR IGNORE",
            &format!("game_id {IN_LIST}"),
        )?,
        [games],
    )
    .map_err(map_db)?;
    let rows = {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id, game_id, cache_path FROM main.game_assets
                 WHERE cache_path IS NOT NULL AND game_id {IN_LIST}"
            ))
            .map_err(map_db)?;
        stmt.query_map([games], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(map_db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_db)?
    };
    let mut copied = Vec::new();
    for (asset_id, game_id, old_path) in rows {
        let name = file_name(&old_path)
            .filter(|name| is_plain_name(name) && is_plain_name(&game_id))
            .map(str::to_owned);
        let next_path = name.as_ref().map(|name| {
            asset_manager
                .cache_dir()
                .join(&game_id)
                .join(name)
                .to_string_lossy()
                .into_owned()
        });
        conn.execute(
            "UPDATE main.game_assets SET cache_path = ?1 WHERE id = ?2",
            params![next_path, asset_id],
        )
        .map_err(map_db)?;
        if let Some(name) = name {
            copied.push((game_id, name));
        }
    }
    Ok(copied)
}

/// Copies the artwork files of games that came along. A file that fails to
/// copy only leaves its game without the picture.
fn copy_artwork(staged: &Staged, asset_manager: &AssetManager, copied: &[(String, String)]) {
    let listed: HashSet<&str> = staged.artwork.iter().map(String::as_str).collect();
    for (game_id, name) in copied {
        let relative = format!("{ASSET_CACHE_DIR}/{game_id}/{name}");
        if !listed.contains(relative.as_str()) {
            continue;
        }
        let target_dir = asset_manager.cache_dir().join(game_id);
        let copied = fs::create_dir_all(&target_dir).and_then(|()| {
            fs::copy(staged.backup_dir.join(&relative), target_dir.join(name)).map(|_| ())
        });
        if let Err(error) = copied {
            warn!("artwork {relative} not merged: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta, Utc};
    use ed25519_dalek::SigningKey;
    use serde_json::json;

    use super::*;
    use crate::backup::{
        BACKUP_MANIFEST_FILE, collect_backup_files, compute_overall_checksum, export_local_backup,
        import_local_backup, write_manifest,
    };
    use crate::db::models::CreateGame;
    use crate::db::repo::sessions::Checkpoint;
    use crate::db::repo::{corrections, devices, sessions};
    use crate::integrity::STATUS_LOCAL;
    use crate::playtime::totals;
    use crate::tracking::live::LiveSessions;

    const MINUTE: i64 = 60_000;

    /// One PC with a library, a ledger and a key of its own.
    struct Pc {
        context: AppContext,
        db: Database,
        assets: AssetManager,
        me: (SigningKey, String),
    }

    impl Pc {
        fn new(device: &str) -> Self {
            let app_dir = std::env::temp_dir()
                .join(format!("vaultime-merge-{device}-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&app_dir).unwrap();
            let pc = Self {
                db: Database::open(&app_dir.join(DATABASE_FILE)).unwrap(),
                assets: AssetManager::new(app_dir.join(ASSET_CACHE_DIR)),
                context: AppContext {
                    app_dir,
                    device_id: device.into(),
                    app_version: "0.4.0".into(),
                },
                me: ledger::test_pc(device),
            };
            pc.act();
            devices::ensure_device(&pc.db, device, "linux", "0.4.0").unwrap();
            devices::describe_this_device(&pc.db, device, &ledger::key_id().unwrap(), device)
                .unwrap();
            pc.db
                .with_transaction(|conn| ledger::begin_if_empty(conn, "first_start"))
                .unwrap();
            pc
        }

        fn act(&self) {
            ledger::act_as(Some(&self.me));
        }

        fn game(&self, title: &str, steam: Option<&str>) -> String {
            self.act();
            let game = games::create_game(
                &self.db,
                &CreateGame {
                    title: title.into(),
                    executable_path: Some(format!("/games/{title}.exe")),
                    install_folder: None,
                    launcher_source: steam.map(|_| "steam".to_owned()),
                },
            )
            .unwrap();
            if let Some(app_id) = steam {
                games::set_launcher_id(&self.db, &game.id, app_id).unwrap();
            }
            game.id
        }

        /// A tracked session of `minutes`, all active, closed with a checkpoint
        /// on the way.
        fn play(&self, game: &str, minutes: i64) -> String {
            self.act();
            let session =
                sessions::create_session(&self.db, game, &self.context.device_id).unwrap();
            let half = minutes * MINUTE / 2;
            let checkpoint = Checkpoint {
                runtime_ms: half,
                active_ms: half,
                wall_elapsed_ms: half,
                ..Checkpoint::default()
            };
            sessions::record_checkpoint(
                &self.db,
                &session.id,
                &checkpoint,
                STATUS_LOCAL,
                Utc::now(),
            )
            .unwrap();
            sessions::end_session(
                &self.db,
                &session.id,
                minutes * MINUTE,
                minutes * MINUTE,
                0,
                STATUS_LOCAL,
            )
            .unwrap();
            session.id
        }

        /// A closed session from `start`, as the tracker of that time wrote it.
        fn play_at(&self, game: &str, start: &str, minutes: i64) -> String {
            self.act();
            let started = DateTime::parse_from_rfc3339(start)
                .unwrap()
                .with_timezone(&Utc);
            let session =
                sessions::create_session_at(&self.db, game, &self.context.device_id, started)
                    .unwrap();
            let runtime = minutes * MINUTE;
            let end = integrity::format_timestamp(started + TimeDelta::milliseconds(runtime));
            self.db
                .with_transaction(|conn| {
                    integrity::append_session_event(
                        conn,
                        &session.id,
                        "ended",
                        &end,
                        Some(runtime),
                        &json!({
                            "runtime_ms": runtime, "active_ms": runtime, "idle_ms": 0,
                            "integrity_status": "local", "closed_cleanly": true,
                        })
                        .to_string(),
                    )?;
                    conn.execute(
                        "UPDATE sessions SET ended_at_wall = ?1, runtime_ms = ?2,
                             elapsed_monotonic_ms = ?2, active_ms = ?2, closed_cleanly = 1
                         WHERE id = ?3",
                        params![end, runtime, session.id],
                    )
                    .map_err(map_db)?;
                    slices::rebuild_session_and_around(conn, &session.id)
                })
                .unwrap();
            session.id
        }

        fn back_up(&self) -> PathBuf {
            self.act();
            let exports = self.context.app_dir.join("exports");
            fs::create_dir_all(&exports).unwrap();
            // Backups made within the same second need folders of their own.
            let folder = exports.join(uuid::Uuid::new_v4().to_string());
            fs::create_dir_all(&folder).unwrap();
            PathBuf::from(
                export_local_backup(&self.db, &self.assets, &self.context, &folder)
                    .unwrap()
                    .backup_path,
            )
        }

        fn preview(&self, backup: &Path) -> Result<MergePreview> {
            self.act();
            preview(&self.db, &self.context, backup, false)
        }

        fn merge(&self, backup: &Path, choices: &[GameChoice]) -> MergeSummary {
            self.act();
            apply(
                &self.db,
                &self.assets,
                &self.context,
                backup,
                choices,
                false,
            )
            .unwrap()
        }

        /// The label the session list shows, checked afresh.
        fn status(&self, session_id: &str) -> String {
            self.act();
            integrity::forget_checks();
            sessions::list_all_sessions(&self.db)
                .unwrap()
                .into_iter()
                .find(|session| session.id == session_id)
                .map_or_else(|| "missing".into(), |session| session.integrity_status)
        }

        fn events(&self, session_id: &str) -> Vec<(i64, String, String, Option<String>)> {
            self.db
                .with_conn(|conn| {
                    let mut stmt = conn
                        .prepare(
                            "SELECT sequence, event_type, payload_json, hash_self FROM session_events
                             WHERE session_id = ?1 ORDER BY sequence",
                        )
                        .map_err(map_db)?;
                    let rows = stmt
                        .query_map([session_id], |row| {
                            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                        })
                        .map_err(map_db)?;
                    rows.collect::<rusqlite::Result<_>>().map_err(map_db)
                })
                .unwrap()
        }

        fn adopted(&self, session_id: &str) -> bool {
            self.db
                .with_conn(|conn| {
                    conn.query_row(
                        "SELECT COUNT(*) FROM ledger_entries
                         WHERE session_id = ?1 AND entry_type = 'adopted'",
                        [session_id],
                        |row| row.get::<_, i64>(0),
                    )
                    .map_err(map_db)
                })
                .unwrap()
                > 0
        }
    }

    impl Drop for Pc {
        fn drop(&mut self) {
            ledger::act_as(None);
            fs::remove_dir_all(&self.context.app_dir).ok();
        }
    }

    /// Changes a backup the way someone could, its checksums made to match.
    fn rewrite(backup: &Path, change: impl FnOnce(&Connection)) {
        let conn = Connection::open(backup.join(DATABASE_FILE)).unwrap();
        change(&conn);
        drop(conn);
        let manifest_path = backup.join(BACKUP_MANIFEST_FILE);
        let mut manifest: LocalBackupManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest.files = collect_backup_files(backup)
            .unwrap()
            .into_iter()
            .filter(|file| file.path != BACKUP_MANIFEST_FILE)
            .collect();
        manifest.overall_checksum = compute_overall_checksum(
            &manifest.backup_id,
            &manifest.created_at,
            &manifest.app_version,
            &manifest.source_device_id,
            &manifest.schema_migrations,
            &manifest.files,
        );
        write_manifest(backup, &manifest).unwrap();
    }

    /// Writes a longer ended event over the real one, hashed the way the
    /// tracker hashes, with the row to match: a rewrite that keeps the
    /// session's own chain sound.
    fn lengthen(conn: &Connection, session_id: &str) {
        let (sequence, wall, previous): (i64, String, String) = conn
            .query_row(
                "SELECT sequence, event_time_wall, hash_prev FROM session_events
                 WHERE session_id = ?1 AND event_type = 'ended'",
                [session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let payload = r#"{"runtime_ms":7200000,"active_ms":7200000,"idle_ms":0,"integrity_status":"local","closed_cleanly":true}"#;
        let hash = integrity::compute_event_hash(
            session_id,
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
            params![payload, hash, session_id, sequence],
        )
        .unwrap();
        conn.execute(
            "UPDATE sessions SET runtime_ms = 7200000, elapsed_monotonic_ms = 7200000,
                 active_ms = 7200000 WHERE id = ?1",
            [session_id],
        )
        .unwrap();
    }

    fn link(game: &str, to: Option<&str>) -> GameChoice {
        GameChoice {
            game_id: game.into(),
            linked_game_id: to.map(str::to_owned),
        }
    }

    #[test]
    fn takes_in_another_pcs_sessions_byte_for_byte_and_vouches_for_them() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let ours = desktop.game("Hades", Some("1145360"));
        let theirs = laptop.game("Hades", Some("1145360"));
        let first = laptop.play(&theirs, 30);
        let second = laptop.play(&theirs, 45);
        let backup = laptop.back_up();

        let preview = desktop.preview(&backup).unwrap();
        assert_eq!((preview.new_sessions, preview.failing), (2, 0));
        assert_eq!(preview.device_name.as_deref(), Some("laptop"));
        assert_eq!(preview.games.len(), 1);
        assert_eq!(
            preview.games[0].suggested_game_id.as_deref(),
            Some(ours.as_str())
        );
        assert_eq!(
            preview.games[0].suggested_because.as_deref(),
            Some("launcher")
        );

        let summary = desktop.merge(&backup, &[link(&theirs, Some(&ours))]);
        assert_eq!((summary.sessions_added, summary.games_linked), (2, 1));
        assert!(summary.safety_backup_path.is_some());
        for session in [&first, &second] {
            assert_eq!(desktop.events(session), laptop.events(session));
            assert_eq!(desktop.status(session), STATUS_LOCAL);
            assert!(desktop.adopted(session));
        }

        // The linked game counts as this PC's game, and is never tracked here.
        desktop.act();
        let shown: Vec<String> = games::list_shown_games(&desktop.db)
            .unwrap()
            .into_iter()
            .map(|game| game.id)
            .collect();
        assert_eq!(shown, std::slice::from_ref(&ours));
        assert!(
            games::list_local_games(&desktop.db)
                .unwrap()
                .iter()
                .all(|game| game.id != theirs)
        );
        let today = Utc::now().date_naive();
        let played: i64 = totals::play_totals(
            &desktop.db,
            &LiveSessions::default(),
            &Utc,
            today - TimeDelta::days(1),
            today + TimeDelta::days(1),
            totals::Bucket::Day,
            Some(&ours),
            Utc::now().timestamp_millis(),
        )
        .unwrap()
        .iter()
        .map(|total| total.runtime_ms)
        .sum();
        assert_eq!(played, 75 * MINUTE);

        // Only the laptop may correct what it recorded.
        let end = desktop
            .db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT started_at_wall FROM sessions WHERE id = ?1",
                    [&first],
                    |row| row.get::<_, String>(0),
                )
                .map_err(map_db)
            })
            .unwrap();
        let refused = corrections::trim_session(&desktop.db, &first, &end, "Too long").unwrap_err();
        assert!(refused.to_string().contains("laptop"));

        // Merging the same backup again brings nothing.
        let again = desktop.preview(&backup).unwrap();
        assert_eq!((again.new_sessions, again.already_here), (0, 2));
    }

    #[test]
    fn a_session_rewritten_in_the_backup_comes_in_suspicious() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Celeste", None);
        let honest = laptop.play(&game, 20);
        let rewritten = laptop.play(&game, 20);
        let backup = laptop.back_up();
        rewrite(&backup, |conn| lengthen(conn, &rewritten));

        let preview = desktop.preview(&backup).unwrap();
        assert_eq!((preview.new_sessions, preview.failing), (2, 1));
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.status(&honest), STATUS_LOCAL);
        assert_eq!(desktop.status(&rewritten), integrity::STATUS_SUSPICIOUS);
        assert!(!desktop.adopted(&rewritten));
    }

    #[test]
    fn a_session_the_other_pcs_ledger_does_not_know_is_not_vouched_for() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Balatro", None);
        let honest = laptop.play(&game, 20);
        // Its entries are the newest, so the ledger without them still holds,
        // but it no longer knows the session.
        let slipped = laptop.play(&game, 20);
        laptop
            .db
            .with_conn(|conn| {
                conn.execute(
                    "DELETE FROM ledger_entries WHERE session_id = ?1",
                    [&slipped],
                )
                .map_err(map_db)
            })
            .unwrap();
        assert_eq!(laptop.status(&slipped), integrity::STATUS_SUSPICIOUS);

        let backup = laptop.back_up();
        let preview = desktop.preview(&backup).unwrap();
        assert_eq!((preview.new_sessions, preview.failing), (2, 1));
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.status(&honest), STATUS_LOCAL);
        assert_eq!(desktop.status(&slipped), integrity::STATUS_SUSPICIOUS);
        assert!(!desktop.adopted(&slipped));
    }

    #[test]
    fn refuses_backups_it_cannot_merge() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hollow Knight", None);
        laptop.play(&game, 20);

        let own = desktop.back_up();
        assert!(
            desktop
                .preview(&own)
                .unwrap_err()
                .to_string()
                .contains("this PC")
        );

        let changed = laptop.back_up();
        rewrite(&changed, |conn| {
            conn.execute(
                "UPDATE ledger_entries SET payload_json = '{}' WHERE sequence = 2",
                [],
            )
            .unwrap();
        });
        assert!(
            desktop
                .preview(&changed)
                .unwrap_err()
                .to_string()
                .contains("changed outside")
        );

        let without = laptop.back_up();
        rewrite(&without, |conn| {
            conn.execute("DELETE FROM ledger_entries", []).unwrap();
        });
        assert!(
            desktop
                .preview(&without)
                .unwrap_err()
                .to_string()
                .contains("no ledger")
        );
    }

    #[test]
    fn a_session_that_grew_there_grows_here_too() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Outer Wilds", None);
        let session = laptop.play_at(&game, "2026-10-02T18:00:00Z", 60);
        desktop.merge(&laptop.back_up(), &[]);

        // The laptop cuts it short afterwards.
        laptop.act();
        let started: String = laptop
            .db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT started_at_wall FROM sessions WHERE id = ?1",
                    [&session],
                    |row| row.get(0),
                )
                .map_err(map_db)
            })
            .unwrap();
        let start = DateTime::parse_from_rfc3339(&started).unwrap();
        let cut = integrity::format_timestamp((start + TimeDelta::minutes(40)).with_timezone(&Utc));
        corrections::trim_session(&laptop.db, &session, &cut, "Fell asleep").unwrap();

        let backup = laptop.back_up();
        let preview = desktop.preview(&backup).unwrap();
        assert_eq!((preview.grown_sessions, preview.new_sessions), (1, 0));
        let summary = desktop.merge(&backup, &[]);
        assert_eq!(summary.sessions_grown, 1);
        assert_eq!(desktop.events(&session), laptop.events(&session));
        assert_eq!(desktop.status(&session), integrity::STATUS_EDITED);
    }

    #[test]
    fn a_session_changed_on_both_pcs_stays_as_it_is_here() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades II", None);
        let session = laptop.play_at(&game, "2026-10-02T18:00:00Z", 60);
        // The desktop took the laptop's history in by a restore, before merges
        // existed, and cut the session there.
        let earlier = laptop.back_up();
        desktop.act();
        import_local_backup(&desktop.db, &desktop.assets, &desktop.context, &earlier).unwrap();
        integrity::forget_checks();
        let start = |pc: &Pc| -> DateTime<Utc> {
            let started: String = pc
                .db
                .with_conn(|conn| {
                    conn.query_row(
                        "SELECT started_at_wall FROM sessions WHERE id = ?1",
                        [&session],
                        |row| row.get(0),
                    )
                    .map_err(map_db)
                })
                .unwrap();
            DateTime::parse_from_rfc3339(&started)
                .unwrap()
                .with_timezone(&Utc)
        };
        let here = integrity::format_timestamp(start(&desktop) + TimeDelta::minutes(50));
        desktop.act();
        corrections::trim_session(&desktop.db, &session, &here, "Left it on").unwrap();
        assert_eq!(desktop.status(&session), integrity::STATUS_EDITED);
        laptop.act();
        let there = integrity::format_timestamp(start(&laptop) + TimeDelta::minutes(40));
        corrections::trim_session(&laptop.db, &session, &there, "Fell asleep").unwrap();

        let backup = laptop.back_up();
        let preview = desktop.preview(&backup).unwrap();
        assert_eq!(preview.conflicts.len(), 1);
        assert_eq!(preview.conflicts[0].reason, "changed_on_both");
        let kept = desktop.events(&session);
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.events(&session), kept);
        assert_eq!(desktop.status(&session), integrity::STATUS_EDITED);
    }

    #[test]
    fn sessions_removed_here_do_not_come_back() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Tunic", None);
        laptop.play(&game, 20);
        let backup = laptop.back_up();
        desktop.merge(&backup, &[]);
        desktop.act();
        assert!(games::delete_game(&desktop.db, &game).unwrap());

        let again = desktop.preview(&backup).unwrap();
        assert_eq!((again.new_sessions, again.removed_here), (0, 1));
        assert_eq!(
            desktop
                .db
                .with_conn(ledger::report)
                .unwrap()
                .missing_sessions,
            0
        );
    }

    #[test]
    fn sessions_of_this_pc_are_never_taken_from_a_backup() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = desktop.game("Celeste", None);
        let mine = desktop.play(&game, 30);
        laptop.merge(&desktop.back_up(), &[]);
        let backup = laptop.back_up();
        // Even rewritten there, this PC's own session is left alone.
        rewrite(&backup, |conn| lengthen(conn, &mine));
        let kept = desktop.events(&mine);

        let preview = desktop.preview(&backup).unwrap();
        assert_eq!(preview.new_sessions, 0);
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.events(&mine), kept);
        assert_eq!(desktop.status(&mine), STATUS_LOCAL);
    }

    #[test]
    fn a_merged_session_changed_here_turns_suspicious() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hollow Knight", None);
        let session = laptop.play(&game, 30);
        desktop.merge(&laptop.back_up(), &[]);
        desktop
            .db
            .with_conn(|conn| {
                lengthen(conn, &session);
                Ok(())
            })
            .unwrap();
        assert_eq!(desktop.status(&session), integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn sessions_pass_through_a_pc_in_between() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let handheld = Pc::new("handheld");
        let game = handheld.game("Balatro", None);
        let session = handheld.play(&game, 25);
        laptop.merge(&handheld.back_up(), &[]);
        let backup = laptop.back_up();

        let preview = desktop.preview(&backup).unwrap();
        assert_eq!((preview.new_sessions, preview.failing), (1, 0));
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.events(&session), handheld.events(&session));
        assert_eq!(desktop.status(&session), STATUS_LOCAL);
    }

    #[test]
    fn a_restore_keeps_sessions_merged_there_sound() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let handheld = Pc::new("handheld");
        let game = handheld.game("Balatro", None);
        let session = handheld.play(&game, 25);
        laptop.merge(&handheld.back_up(), &[]);
        let backup = laptop.back_up();

        desktop.act();
        import_local_backup(&desktop.db, &desktop.assets, &desktop.context, &backup).unwrap();
        assert_eq!(desktop.status(&session), STATUS_LOCAL);
    }

    #[test]
    fn a_ledger_that_only_claims_to_be_this_pc_vouches_for_nothing() {
        let desktop = Pc::new("desktop");
        let game = desktop.game("Celeste", None);
        // Someone starts a ledger that names this PC, with a key of their own.
        let impostor = ledger::test_pc("desktop");
        ledger::act_as(Some(&impostor));
        let session = sessions::create_session(&desktop.db, &game, "desktop").unwrap();
        sessions::end_session(
            &desktop.db,
            &session.id,
            3_600_000,
            3_600_000,
            0,
            STATUS_LOCAL,
        )
        .unwrap();
        assert_eq!(desktop.status(&session.id), integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn a_key_that_newly_claims_a_merged_pc_does_not_count() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        let honest = laptop.play(&game, 20);
        desktop.merge(&laptop.back_up(), &[]);

        // A second ledger that claims to be the laptop vouches for a session.
        let impostor = ledger::test_pc("laptop");
        ledger::act_as(Some(&impostor));
        let session = sessions::create_session(&laptop.db, &game, "laptop").unwrap();
        sessions::end_session(
            &laptop.db,
            &session.id,
            7_200_000,
            7_200_000,
            0,
            STATUS_LOCAL,
        )
        .unwrap();
        let later = laptop.play(&game, 10);
        let backup = laptop.back_up();

        let preview = desktop.preview(&backup).unwrap();
        assert!(preview.unknown_keys);
        assert_eq!((preview.new_sessions, preview.failing), (2, 1));
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.status(&session.id), integrity::STATUS_SUSPICIOUS);
        assert!(!desktop.adopted(&session.id));
        assert_eq!(desktop.status(&honest), STATUS_LOCAL);
        assert_eq!(desktop.status(&later), STATUS_LOCAL);
    }

    #[test]
    fn a_restore_counts_only_the_keys_a_merge_trusted() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        let honest = laptop.play(&game, 20);
        desktop.merge(&laptop.back_up(), &[]);

        let impostor = ledger::test_pc("laptop");
        ledger::act_as(Some(&impostor));
        let forged = sessions::create_session(&laptop.db, &game, "laptop").unwrap();
        sessions::end_session(
            &laptop.db,
            &forged.id,
            7_200_000,
            7_200_000,
            0,
            STATUS_LOCAL,
        )
        .unwrap();
        let backup = laptop.back_up();

        desktop.act();
        import_local_backup(&desktop.db, &desktop.assets, &desktop.context, &backup).unwrap();
        assert_eq!(desktop.status(&honest), STATUS_LOCAL);
        assert_eq!(desktop.status(&forged.id), integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn refuses_a_backup_with_views_or_triggers() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        laptop.play(&game, 20);
        let viewed = laptop.back_up();
        rewrite(&viewed, |conn| {
            conn.execute_batch(
                "ALTER TABLE session_events RENAME TO real_events;
                 CREATE VIEW session_events AS SELECT * FROM real_events;",
            )
            .unwrap();
        });
        let refused = desktop.preview(&viewed).unwrap_err();
        assert!(refused.to_string().contains("never writes"));
        desktop.act();
        assert!(
            import_local_backup(&desktop.db, &desktop.assets, &desktop.context, &viewed).is_err()
        );

        let triggered = laptop.back_up();
        rewrite(&triggered, |conn| {
            conn.execute_batch(
                "CREATE TRIGGER sneak AFTER INSERT ON ledger_entries
                 BEGIN UPDATE sessions SET runtime_ms = runtime_ms * 2; END;",
            )
            .unwrap();
        });
        assert!(
            desktop
                .preview(&triggered)
                .unwrap_err()
                .to_string()
                .contains("never writes")
        );
    }

    #[test]
    fn a_new_pc_cannot_vouch_for_sessions_of_pcs_this_one_knows() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        laptop.play(&game, 20);
        desktop.merge(&laptop.back_up(), &[]);

        // A made up PC whose backup holds sessions that claim to be the
        // laptop's and this PC's, vouched for by its own ledger.
        let phantom = Pc::new("phantom");
        let its_game = phantom.game("Hades", None);
        let own = phantom.play(&its_game, 15);
        phantom.act();
        for device in ["laptop", "desktop"] {
            devices::ensure_device(&phantom.db, device, "linux", "0.4.0").unwrap();
        }
        let as_laptop = sessions::create_session(&phantom.db, &its_game, "laptop").unwrap();
        sessions::end_session(
            &phantom.db,
            &as_laptop.id,
            7_200_000,
            7_200_000,
            0,
            STATUS_LOCAL,
        )
        .unwrap();
        let as_desktop = sessions::create_session(&phantom.db, &its_game, "desktop").unwrap();
        sessions::end_session(
            &phantom.db,
            &as_desktop.id,
            7_200_000,
            7_200_000,
            0,
            STATUS_LOCAL,
        )
        .unwrap();
        let backup = phantom.back_up();

        let preview = desktop.preview(&backup).unwrap();
        assert_eq!((preview.new_sessions, preview.failing), (2, 1));
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.status(&own), STATUS_LOCAL);
        assert_eq!(desktop.status(&as_laptop.id), integrity::STATUS_SUSPICIOUS);
        assert_eq!(desktop.status(&as_desktop.id), "missing");

        // A restore of that backup lets neither through either.
        let other = Pc::new("other");
        other.merge(&laptop.back_up(), &[]);
        other.act();
        import_local_backup(&other.db, &other.assets, &other.context, &backup).unwrap();
        assert_eq!(other.status(&own), STATUS_LOCAL);
        assert_eq!(other.status(&as_laptop.id), integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn a_restore_does_not_vouch_for_a_backup_changed_and_stripped_of_its_ledger() {
        let desktop = Pc::new("desktop");
        let game = desktop.game("Celeste", None);
        let honest = desktop.play(&game, 20);
        let changed = desktop.play(&game, 20);
        let backup = desktop.back_up();
        rewrite(&backup, |conn| {
            lengthen(conn, &changed);
            conn.execute("DELETE FROM ledger_entries", []).unwrap();
        });

        desktop.act();
        import_local_backup(&desktop.db, &desktop.assets, &desktop.context, &backup).unwrap();
        assert_eq!(desktop.status(&honest), STATUS_LOCAL);
        assert_eq!(desktop.status(&changed), integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn restoring_a_backup_of_a_pc_that_merged_this_one_keeps_this_pcs_games_its_own() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = desktop.game("Celeste", None);
        let session = desktop.play_at(&game, "2026-10-02T18:00:00Z", 60);
        laptop.merge(&desktop.back_up(), &[]);
        let backup = laptop.back_up();

        desktop.act();
        import_local_backup(&desktop.db, &desktop.assets, &desktop.context, &backup).unwrap();
        assert!(
            games::list_local_games(&desktop.db)
                .unwrap()
                .iter()
                .any(|local| local.id == game)
        );
        assert_eq!(desktop.status(&session), STATUS_LOCAL);
        let cut = "2026-10-02T18:30:00.000Z";
        corrections::trim_session(&desktop.db, &session, cut, "Left it on").unwrap();
    }

    #[test]
    fn trust_from_earlier_merges_survives_a_restore() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        laptop.play(&game, 20);
        let summary = desktop.merge(&laptop.back_up(), &[]);
        // Going back to the backup saved just before the merge.
        desktop.act();
        import_local_backup(
            &desktop.db,
            &desktop.assets,
            &desktop.context,
            Path::new(&summary.safety_backup_path.unwrap()),
        )
        .unwrap();

        let impostor = ledger::test_pc("laptop");
        ledger::act_as(Some(&impostor));
        let forged = sessions::create_session(&laptop.db, &game, "laptop").unwrap();
        sessions::end_session(
            &laptop.db,
            &forged.id,
            7_200_000,
            7_200_000,
            0,
            STATUS_LOCAL,
        )
        .unwrap();
        let backup = laptop.back_up();
        let preview = desktop.preview(&backup).unwrap();
        assert!(preview.unknown_keys);
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.status(&forged.id), integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn the_player_can_vouch_for_a_new_key_of_a_merged_pc() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        laptop.play(&game, 20);
        desktop.merge(&laptop.back_up(), &[]);
        // The laptop's key file was lost and it signs with a new key.
        let renewed = ledger::test_pc("laptop");
        ledger::act_as(Some(&renewed));
        let later = sessions::create_session(&laptop.db, &game, "laptop").unwrap();
        sessions::end_session(&laptop.db, &later.id, 600_000, 600_000, 0, STATUS_LOCAL).unwrap();
        let backup = laptop.back_up();

        desktop.act();
        let preview = preview(&desktop.db, &desktop.context, &backup, true).unwrap();
        assert_eq!(preview.failing, 0);
        apply(
            &desktop.db,
            &desktop.assets,
            &desktop.context,
            &backup,
            &[],
            true,
        )
        .unwrap();
        assert_eq!(desktop.status(&later.id), STATUS_LOCAL);
    }

    #[test]
    fn refuses_to_merge_while_this_pcs_ledger_fails_its_check() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        laptop.play(&game, 20);
        desktop
            .db
            .with_conn(|conn| {
                conn.execute(
                    "UPDATE ledger_entries SET payload_json = '{}' WHERE sequence = 1",
                    [],
                )
                .map_err(map_db)
            })
            .unwrap();
        integrity::forget_checks();
        let refused = desktop.preview(&laptop.back_up()).unwrap_err();
        assert!(refused.to_string().contains("This PC's ledger"));
    }

    #[test]
    fn deleting_a_game_takes_the_linked_games_of_other_pcs_with_it() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let ours = desktop.game("Hades", None);
        let theirs = laptop.game("Hades", None);
        let session = laptop.play(&theirs, 20);
        desktop.merge(&laptop.back_up(), &[link(&theirs, Some(&ours))]);

        desktop.act();
        crate::assets::delete_game(&desktop.db, &desktop.assets, &ours).unwrap();
        assert!(games::list_all_games(&desktop.db).unwrap().is_empty());
        assert_eq!(desktop.status(&session), "missing");
        assert_eq!(
            desktop
                .db
                .with_conn(ledger::report)
                .unwrap()
                .missing_sessions,
            0
        );
    }

    #[test]
    fn a_launcher_steps_aside_only_for_games_on_its_own_pc() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let client = desktop.game("Client", None);
        desktop.act();
        games::set_steps_aside(&desktop.db, &client, true).unwrap();
        desktop.play_at(&client, "2026-10-02T18:00:00Z", 180);
        let other = laptop.game("Match", None);
        laptop.play_at(&other, "2026-10-02T19:00:00Z", 60);
        desktop.merge(&laptop.back_up(), &[]);

        desktop.act();
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let minutes = |game: &str| -> i64 {
            totals::play_totals(
                &desktop.db,
                &LiveSessions::default(),
                &Utc,
                day,
                day + TimeDelta::days(1),
                totals::Bucket::Day,
                Some(game),
                0,
            )
            .unwrap()
            .iter()
            .map(|total| total.runtime_ms)
            .sum::<i64>()
                / MINUTE
        };
        assert_eq!(minutes(&client), 180);
        assert_eq!(minutes(&other), 60);
    }

    /// Writes a two hour end at `sequence` in place of the events from there
    /// on, hashed the way the tracker hashes, with the row to match.
    fn end_early(conn: &Connection, session_id: &str, sequence: i64) {
        let wall: String = conn
            .query_row(
                "SELECT ended_at_wall FROM sessions WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .unwrap();
        let previous: String = conn
            .query_row(
                "SELECT hash_self FROM session_events WHERE session_id = ?1 AND sequence = ?2",
                params![session_id, sequence - 1],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "DELETE FROM session_events WHERE session_id = ?1 AND sequence >= ?2",
            params![session_id, sequence],
        )
        .unwrap();
        let payload = r#"{"runtime_ms":7200000,"active_ms":7200000,"idle_ms":0,"integrity_status":"local","closed_cleanly":true}"#;
        let hash = integrity::compute_event_hash(
            session_id,
            sequence,
            "ended",
            &wall,
            Some(7_200_000),
            payload,
            Some(&previous),
        );
        conn.execute(
            "INSERT INTO session_events (id, session_id, sequence, event_type, event_time_wall,
                 event_time_monotonic, payload_json, hash_prev, hash_self, signature)
             VALUES (?1, ?2, ?3, 'ended', ?4, 7200000, ?5, ?6, ?7, NULL)",
            params![
                uuid::Uuid::new_v4().to_string(),
                session_id,
                sequence,
                wall,
                payload,
                previous,
                hash
            ],
        )
        .unwrap();
        conn.execute(
            "UPDATE sessions SET runtime_ms = 7200000, elapsed_monotonic_ms = 7200000,
                 active_ms = 7200000 WHERE id = ?1",
            [session_id],
        )
        .unwrap();
    }

    #[test]
    fn a_restore_does_not_vouch_for_a_session_cut_back_and_lengthened() {
        let desktop = Pc::new("desktop");
        let game = desktop.game("Celeste", None);
        let honest = desktop.play(&game, 20);
        let changed = desktop.play(&game, 20);
        let backup = desktop.back_up();
        // A made up end in place of the checkpoint before the real end, so
        // the pin of the real end lies past the chain and only the start is
        // pinned within it.
        rewrite(&backup, |conn| {
            end_early(conn, &changed, 2);
            conn.execute("DELETE FROM ledger_entries", []).unwrap();
        });

        desktop.act();
        import_local_backup(&desktop.db, &desktop.assets, &desktop.context, &backup).unwrap();
        assert_eq!(desktop.status(&honest), STATUS_LOCAL);
        assert_eq!(desktop.status(&changed), integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn a_backup_that_claims_to_be_this_pc_vouches_with_this_pcs_keys_alone() {
        let desktop = Pc::new("desktop");
        let game = desktop.game("Celeste", None);
        desktop.play(&game, 20);
        // A ledger of another key that names this PC vouches for a session of
        // a made up PC, in a backup that says it comes from this PC.
        let impostor = Pc::new("desktop");
        let its_game = impostor.game("Hades", None);
        impostor.act();
        devices::ensure_device(&impostor.db, "made-up", "linux", "0.4.0").unwrap();
        let made_up = sessions::create_session(&impostor.db, &its_game, "made-up").unwrap();
        sessions::end_session(
            &impostor.db,
            &made_up.id,
            7_200_000,
            7_200_000,
            0,
            STATUS_LOCAL,
        )
        .unwrap();
        let backup = impostor.back_up();

        desktop.act();
        import_local_backup(&desktop.db, &desktop.assets, &desktop.context, &backup).unwrap();
        assert_eq!(desktop.status(&made_up.id), integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn refuses_a_backup_whose_database_differs_from_the_one_vaultime_creates() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        laptop.play(&game, 20);
        let refused = |backup: &Path| {
            assert!(
                desktop
                    .preview(backup)
                    .unwrap_err()
                    .to_string()
                    .contains("never writes")
            );
            desktop.act();
            assert!(
                import_local_backup(&desktop.db, &desktop.assets, &desktop.context, backup)
                    .is_err()
            );
        };

        // A column that compares without case, so a filter on it takes more
        // rows than its check saw.
        let other_case = laptop.back_up();
        rewrite(&other_case, |conn| {
            conn.execute_batch("PRAGMA writable_schema = ON").unwrap();
            conn.execute(
                "UPDATE sqlite_master SET sql = replace(sql,
                     'integrity_status    TEXT NOT NULL', 'integrity_status    TEXT COLLATE NOCASE NOT NULL')
                 WHERE name = 'sessions'",
                [],
            )
            .unwrap();
        });
        refused(&other_case);

        let extra_index = laptop.back_up();
        rewrite(&extra_index, |conn| {
            conn.execute_batch("CREATE INDEX extra ON sessions (integrity_status)")
                .unwrap();
        });
        refused(&extra_index);

        // A virtual table whose statement no longer starts the usual way.
        let hidden = laptop.back_up();
        rewrite(&hidden, |conn| {
            conn.execute_batch(
                "CREATE VIRTUAL TABLE notes_search USING fts5(note);
                 PRAGMA writable_schema = ON;
                 UPDATE sqlite_master SET sql = replace(sql, 'CREATE VIRTUAL', 'CREATE  VIRTUAL')
                 WHERE name = 'notes_search';",
            )
            .unwrap();
        });
        refused(&hidden);
    }

    #[test]
    fn a_pc_seen_through_another_cannot_be_claimed_by_a_new_key() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let tablet = Pc::new("tablet");
        let game = tablet.game("Hades", None);
        tablet.play(&game, 20);
        laptop.merge(&tablet.back_up(), &[]);
        let through_laptop = laptop.back_up();
        assert!(desktop.preview(&through_laptop).unwrap().first_merge);
        desktop.merge(&through_laptop, &[]);

        // Someone else starts a ledger that names the tablet.
        let impostor = Pc::new("tablet");
        let its_game = impostor.game("Hades", None);
        let made_up = impostor.play(&its_game, 120);
        let backup = impostor.back_up();
        let preview = desktop.preview(&backup).unwrap();
        assert!(preview.unknown_keys);
        assert!(!preview.first_merge);
        assert_eq!(preview.failing, 1);
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.status(&made_up), integrity::STATUS_SUSPICIOUS);
    }

    #[test]
    fn a_session_cannot_go_on_after_its_end() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        let session = laptop.play(&game, 20);
        desktop.merge(&laptop.back_up(), &[]);

        // More time after the end, written with the laptop's own key.
        laptop.act();
        laptop
            .db
            .with_transaction(|conn| {
                let wall = integrity::now_timestamp();
                let payload = json!({
                    "runtime_ms": 7_200_000, "active_ms": 7_200_000, "idle_ms": 0,
                    "integrity_status": "local", "closed_cleanly": true,
                })
                .to_string();
                integrity::append_session_event(
                    conn,
                    &session,
                    "heartbeat",
                    &wall,
                    Some(7_200_000),
                    &payload,
                )?;
                integrity::append_session_event(
                    conn,
                    &session,
                    "ended",
                    &wall,
                    Some(7_200_000),
                    &payload,
                )?;
                conn.execute(
                    "UPDATE sessions SET ended_at_wall = ?1, runtime_ms = 7200000,
                         elapsed_monotonic_ms = 7200000, active_ms = 7200000 WHERE id = ?2",
                    params![wall, session],
                )
                .map_err(map_db)?;
                Ok(())
            })
            .unwrap();
        let reason = laptop
            .db
            .with_conn(|conn| {
                let row = conn
                    .query_row(
                        "SELECT * FROM sessions WHERE id = ?1",
                        [&session],
                        row_to_session,
                    )
                    .map_err(map_db)?;
                integrity::validate_session_history(conn, &row)
            })
            .unwrap();
        assert_eq!(reason.as_deref(), Some("event_after_close"));

        let backup = laptop.back_up();
        let preview = desktop.preview(&backup).unwrap();
        assert_eq!(preview.grown_sessions, 0);
        assert_eq!(preview.conflicts[0].reason, "update_fails_check");
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.status(&session), STATUS_LOCAL);
    }

    #[test]
    fn sessions_that_came_in_unvouched_are_vouched_for_once_the_player_trusts_their_key() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let game = laptop.game("Hades", None);
        laptop.play(&game, 20);
        desktop.merge(&laptop.back_up(), &[]);
        let renewed = ledger::test_pc("laptop");
        ledger::act_as(Some(&renewed));
        let later = sessions::create_session(&laptop.db, &game, "laptop").unwrap();
        sessions::end_session(&laptop.db, &later.id, 600_000, 600_000, 0, STATUS_LOCAL).unwrap();
        let backup = laptop.back_up();
        desktop.merge(&backup, &[]);
        assert_eq!(desktop.status(&later.id), integrity::STATUS_SUSPICIOUS);

        desktop.act();
        let preview = preview(&desktop.db, &desktop.context, &backup, true).unwrap();
        assert_eq!((preview.new_sessions, preview.vouched_now), (0, 1));
        let summary = apply(
            &desktop.db,
            &desktop.assets,
            &desktop.context,
            &backup,
            &[],
            true,
        )
        .unwrap();
        assert_eq!(summary.sessions_vouched, 1);
        assert_eq!(desktop.status(&later.id), STATUS_LOCAL);
    }

    #[test]
    fn play_added_by_hand_here_is_named_when_the_same_play_comes_in() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let ours = desktop.game("Hades", None);
        let theirs = laptop.game("Hades", None);
        laptop.play_at(&theirs, "2026-10-02T18:00:00Z", 60);
        desktop.act();
        corrections::add_manual_session(
            &desktop.db,
            &ours,
            "desktop",
            "2026-10-02T18:10:00.000Z",
            30 * MINUTE,
            "On the laptop",
            None,
        )
        .unwrap();
        let backup = laptop.back_up();
        assert_eq!(desktop.preview(&backup).unwrap().overlapping_manual, 1);
        desktop.merge(&backup, &[link(&theirs, Some(&ours))]);

        // Linked, the two games are one: the laptop's time cannot be added
        // again here, and the laptop's game takes no play added here.
        desktop.act();
        let again = corrections::add_manual_session(
            &desktop.db,
            &ours,
            "desktop",
            "2026-10-02T18:40:00.000Z",
            10 * MINUTE,
            "Again",
            None,
        );
        assert!(again.unwrap_err().to_string().contains("already covers"));
        let on_theirs = corrections::add_manual_session(
            &desktop.db,
            &theirs,
            "desktop",
            "2026-10-01T18:00:00.000Z",
            10 * MINUTE,
            "Elsewhere",
            None,
        );
        assert!(on_theirs.unwrap_err().to_string().contains("another PC"));
    }

    #[test]
    fn a_restore_keeps_the_pcs_this_one_has_seen() {
        let desktop = Pc::new("desktop");
        let laptop = Pc::new("laptop");
        let tablet = Pc::new("tablet");
        let game = tablet.game("Hades", None);
        tablet.play(&game, 20);
        let before = desktop.back_up();
        laptop.merge(&tablet.back_up(), &[]);
        desktop.merge(&laptop.back_up(), &[]);
        // Back to a backup from before the tablet's sessions came through
        // the laptop.
        desktop.act();
        import_local_backup(&desktop.db, &desktop.assets, &desktop.context, &before).unwrap();

        let impostor = Pc::new("tablet");
        let its_game = impostor.game("Hades", None);
        impostor.play(&its_game, 120);
        let preview = desktop.preview(&impostor.back_up()).unwrap();
        assert!(!preview.first_merge);
        assert!(preview.unknown_keys);
        assert_eq!(preview.failing, 1);
    }
}
