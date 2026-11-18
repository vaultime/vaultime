// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Polls running processes and turns them into game sessions.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::Serialize;

use crate::constants::{
    BACKGROUND_ACTIVE_SETTING, CHECKPOINT_INTERVAL, CLOCK_BACKWARDS_TOLERANCE_MS,
    CLOCK_STEP_TOLERANCE_MS, CLOCK_TOTAL_DRIFT_TOLERANCE_MS, DEFAULT_IDLE_THRESHOLD_SECS,
    FOLDER_MATCH_LAUNCHERS, FOREGROUND_GRACE, IDLE_THRESHOLD_SETTING, INSTALL_FOLDER_REFRESH,
    MAX_TICK_GAP_MS, MIN_IDLE_THRESHOLD_SECS, MISSED_TICKS_BEFORE_END, POLL_INTERVAL,
    PROCESS_ACTIVITY_CPU_PERCENT, SUSPEND_DETECT_MS,
};
use crate::db::connection::Database;
use crate::db::models::Game;
use crate::db::repo::sessions::Checkpoint;
use crate::db::repo::{games, ignored, sessions, settings};
use crate::discovery::metadata::is_likely_game_executable;
use crate::error::VaultimeError;
use crate::integrity;
use crate::platform;
use crate::platform::activity::{ActivitySnapshot, capture_activity_snapshot};
use crate::platform::process::{
    ExecutableIndex, FolderIndex, InstallFolder, ProcessList, RunningProcess,
};
use crate::tracking::live::{LiveCounters, LiveSessions};
use crate::tracking::seen::SeenPrograms;

/// Tracks a currently running game session.
struct ActiveSession {
    session_id: String,
    game_id: String,
    started_at_wall: DateTime<Utc>,
    last_wall_at: DateTime<Utc>,
    last_tick_at: Instant,
    runtime_ms: i64,
    active_ms: i64,
    idle_ms: i64,
    last_signal_at: Instant,
    last_foreground_at: Option<Instant>,
    /// Wall time skipped by tracking gaps, left out of the drift check.
    skipped_wall_ms: i64,
    /// How long the PC had slept since boot, at the last tick.
    last_asleep_ms: Option<i64>,
    integrity_status: String,
    /// Whether the last counted tick was active. None before the first one.
    last_tick_active: Option<bool>,
    /// The checkpoint fields as of the last counted tick.
    last_checkpoint_fields: Checkpoint,
    /// What the database holds, as of the last checkpoint.
    stored: Checkpoint,
    stored_at: Instant,
    /// Ticks in a row the game was missing from the process list.
    missed_ticks: u32,
    /// Where the session ends when the game stays gone, worked out at the
    /// first of them. A later tick may come only after the PC slept.
    missing_since: Option<DateTime<Utc>>,
}

impl ActiveSession {
    fn new(
        session_id: String,
        game_id: String,
        now: Instant,
        started_at_wall: DateTime<Utc>,
        integrity_status: String,
    ) -> Self {
        Self {
            session_id,
            game_id,
            started_at_wall,
            last_wall_at: started_at_wall,
            last_tick_at: now,
            runtime_ms: 0,
            active_ms: 0,
            idle_ms: 0,
            last_signal_at: now,
            last_foreground_at: None,
            skipped_wall_ms: 0,
            last_asleep_ms: platform::clock::asleep_ms(),
            integrity_status,
            last_tick_active: None,
            last_checkpoint_fields: Checkpoint::default(),
            stored: Checkpoint::default(),
            stored_at: now,
            missed_ticks: 0,
            missing_since: None,
        }
    }

    fn counters(&self) -> LiveCounters {
        LiveCounters {
            runtime_ms: self.runtime_ms,
            active_ms: self.active_ms,
            idle_ms: self.idle_ms,
        }
    }

    /// True when ticks were counted since the last checkpoint.
    fn has_unsaved_time(&self) -> bool {
        self.stored.runtime_ms != self.runtime_ms
    }

    /// The time from the last counted tick to the tick of `clocks`.
    fn elapsed(&self, clocks: &Clocks) -> ElapsedMs {
        ElapsedMs {
            mono: clocks
                .now
                .saturating_duration_since(self.last_tick_at)
                .as_millis() as i64,
            wall: clocks
                .wall
                .signed_duration_since(self.last_wall_at)
                .num_milliseconds(),
            // Only growth counts. A reading that moved back, as WSL2 can
            // have it, is no sleep.
            slept: clocks
                .asleep_ms
                .zip(self.last_asleep_ms)
                .map_or(0, |(now_asleep, before_asleep)| {
                    (now_asleep - before_asleep).max(0)
                }),
        }
    }

    /// Where the session ends when it ends at the tick of `clocks`: there,
    /// or at its last counted tick when a sleep or a stall lies between
    /// them, as the game may have gone any time before it. Never before the
    /// last counted tick.
    fn end_at(&self, clocks: &Clocks) -> DateTime<Utc> {
        if self.elapsed(clocks).is_gap() {
            self.last_wall_at
        } else {
            clocks.wall.max(self.last_wall_at)
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TrackingSettings {
    idle_threshold: Duration,
    treat_background_as_active: bool,
}

impl TrackingSettings {
    fn load(db: &Database) -> Self {
        let idle_threshold_seconds = settings::get_setting(db, IDLE_THRESHOLD_SETTING)
            .ok()
            .flatten()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_IDLE_THRESHOLD_SECS)
            .max(MIN_IDLE_THRESHOLD_SECS);

        let treat_background_as_active = settings::get_setting(db, BACKGROUND_ACTIVE_SETTING)
            .ok()
            .flatten()
            .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"));

        Self {
            idle_threshold: Duration::from_secs(idle_threshold_seconds),
            treat_background_as_active,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct GameObservation {
    is_running: bool,
    has_foreground_window: bool,
    has_process_activity: bool,
}

/// Handle to the background tracker thread.
pub struct TrackingEngine {
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    /// Held for the duration of every tick, so `pause` can wait for one to finish.
    tick_lock: Arc<Mutex<()>>,
    /// Held through a restore, so a second one waits and the first cannot
    /// let tracking go on while the second still swaps the history.
    restoring: Mutex<()>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl TrackingEngine {
    /// Spawns the tracker on a background thread. It keeps `live` up to date
    /// with the counters of the running sessions.
    pub fn start(db: Arc<Database>, device_id: String, live: Arc<LiveSessions>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let paused = Arc::new(AtomicBool::new(false));
        let tick_lock = Arc::new(Mutex::new(()));

        let thread = {
            let running = Arc::clone(&running);
            let paused = Arc::clone(&paused);
            let tick_lock = Arc::clone(&tick_lock);
            std::thread::Builder::new()
                .name("vaultime-tracker".into())
                .spawn(move || poll_loop(&db, &device_id, &live, &running, &paused, &tick_lock))
                .expect("failed to spawn tracking thread")
        };

        info!("tracking engine started");
        Self {
            running,
            paused,
            tick_lock,
            restoring: Mutex::new(()),
            thread: Mutex::new(Some(thread)),
        }
    }

    /// Stops the tracker and waits until it has closed the running sessions,
    /// so a quit does not leave them to be recovered on the next start.
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            thread.thread().unpark();
            if thread.join().is_err() {
                error!("tracking thread panicked during shutdown");
            }
        }
    }

    /// Stops tracking and returns once no tick is in progress.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
        drop(
            self.tick_lock
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        info!("tracking paused");
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        info!("tracking resumed");
    }

    /// Runs a restore with tracking paused. Refuses while a game of
    /// `device_id` is being tracked, because the restore replaces the session
    /// tables. After a restore that made this PC continue as another one,
    /// tracking waits for the restart, as the tracker still records with the
    /// id it started with.
    pub fn while_paused<T>(
        &self,
        db: &Database,
        device_id: &str,
        restore: impl FnOnce() -> crate::error::Result<T>,
    ) -> crate::error::Result<T> {
        let _restoring = self
            .restoring
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.pause();
        let result = match sessions::get_active_sessions(db, device_id) {
            Ok(active) if !active.is_empty() => Err(VaultimeError::Backup(
                "close all running games before restoring a backup".into(),
            )),
            Ok(_) => restore(),
            Err(error) => Err(error),
        };
        if !integrity::ledger::this_device().is_ok_and(|now| now != device_id) {
            self.resume();
        }
        result
    }

    /// True while the tracker thread runs and is not paused.
    pub fn is_running(&self) -> bool {
        let alive = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|thread| !thread.is_finished());
        alive && self.running.load(Ordering::SeqCst) && !self.paused.load(Ordering::SeqCst)
    }
}

impl Drop for TrackingEngine {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Sleeps until the next tick, or returns early when `shutdown` wakes the thread.
fn wait_for_next_tick(running: &AtomicBool) {
    let next_tick = Instant::now() + POLL_INTERVAL;
    while running.load(Ordering::SeqCst) {
        let now = Instant::now();
        if now >= next_tick {
            break;
        }
        std::thread::park_timeout(next_tick - now);
    }
}

fn poll_loop(
    db: &Database,
    device_id: &str,
    live: &LiveSessions,
    running: &AtomicBool,
    paused: &AtomicBool,
    tick_lock: &Mutex<()>,
) {
    let mut active: HashMap<String, ActiveSession> = HashMap::new();
    let mut processes = ProcessList::default();
    let mut folders = FolderCache::default();
    let mut seen = SeenPrograms::default();

    run_guarded("closing the sessions left open", || {
        close_orphaned_sessions(db, device_id);
        Ok(())
    });

    while running.load(Ordering::SeqCst) {
        if !paused.load(Ordering::SeqCst) {
            let _tick = tick_lock.lock().unwrap_or_else(PoisonError::into_inner);
            // Checked again because `pause` may have won the race for the lock.
            if !paused.load(Ordering::SeqCst)
                && run_guarded("tracking poll", || {
                    poll_tick(
                        db,
                        device_id,
                        live,
                        &mut processes,
                        &mut folders,
                        &mut seen,
                        &mut active,
                    )
                }) == Guarded::Panicked
            {
                // The process list may be half refreshed, so it starts over.
                processes = ProcessList::default();
                folders = FolderCache::default();
            }
        }

        wait_for_next_tick(running);
    }

    end_all_sessions(db, live, active);
    info!("tracking engine stopped");
}

/// Ends every tracked session as tracking stops, one whose game went missing
/// where it did.
fn end_all_sessions(db: &Database, live: &LiveSessions, active: HashMap<String, ActiveSession>) {
    let clocks = Clocks::read();
    for session in active.into_values() {
        let ended_at = session
            .missing_since
            .unwrap_or_else(|| session.end_at(&clocks));
        end_active_session(db, live, &session, ended_at);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Guarded {
    Done,
    Failed,
    Panicked,
}

/// Runs one step of the tracker and logs how it failed. A panic from a
/// platform API or a bug must not end tracking for the rest of the run, so it
/// is caught here. The panic hook has logged where it happened.
fn run_guarded(what: &str, step: impl FnOnce() -> crate::error::Result<()>) -> Guarded {
    match catch_unwind(AssertUnwindSafe(step)) {
        Ok(Ok(())) => Guarded::Done,
        Ok(Err(error)) => {
            error!("{what} failed: {error}");
            Guarded::Failed
        }
        Err(_) => {
            error!("{what} panicked, tracking carries on");
            Guarded::Panicked
        }
    }
}

fn poll_tick(
    db: &Database,
    device_id: &str,
    live: &LiveSessions,
    list: &mut ProcessList,
    folders: &mut FolderCache,
    seen: &mut SeenPrograms,
    active: &mut HashMap<String, ActiveSession>,
) -> crate::error::Result<()> {
    let tracking_settings = TrackingSettings::load(db);
    let tracked_games = games::list_local_games(db)?;
    // Programs the player said are no game never count, whatever matches
    // them. They are recognized the way games are, through links and Wine.
    let ignored = ignored::paths(db).unwrap_or_default();
    let processes = list.refresh();
    // Vaultime itself always runs, so an empty list failed to load. Ending
    // every session on it would split them all.
    if processes.is_empty() {
        return Err(VaultimeError::Tracking(
            "the list of running processes came back empty".into(),
        ));
    }
    let ignored = ExecutableIndex::new(ignored.iter().map(String::as_str));
    let processes: Vec<RunningProcess> = processes
        .into_iter()
        .filter(|process| !ignored.any(process))
        .collect();
    let activity_snapshot = capture_activity_snapshot();

    folders.expire();
    let observed_games = observe_games(
        &tracked_games,
        &processes,
        &activity_snapshot,
        list,
        folders,
    );

    let game_in_front = observed_games
        .values()
        .any(|observation| observation.has_foreground_window);
    seen.note(
        db,
        &processes,
        &activity_snapshot,
        game_in_front,
        &tracked_games,
    );

    start_sessions(db, device_id, live, active, &observed_games);

    // All clocks are read together after the work above, which can wait on
    // the database while a backup runs. Reading them apart would look like a
    // clock change.
    let clocks = Clocks::read();
    count_running_sessions(
        db,
        live,
        active,
        &observed_games,
        &tracking_settings,
        &activity_snapshot,
        &clocks,
    );
    end_finished_sessions(db, live, active, &observed_games, &clocks);

    debug!(
        "poll tick: {} tracked, {} running, {} active sessions",
        tracked_games.len(),
        observed_games
            .values()
            .filter(|state| state.is_running)
            .count(),
        active.len(),
    );

    Ok(())
}

/// Opens a session for each game that began running. Its live clock starts
/// at zero as of its start.
fn start_sessions(
    db: &Database,
    device_id: &str,
    live: &LiveSessions,
    active: &mut HashMap<String, ActiveSession>,
    observed_games: &HashMap<String, GameObservation>,
) {
    for (game_id, observation) in observed_games {
        if observation.is_running && !active.contains_key(game_id) {
            let (started, started_wall) = (Instant::now(), Utc::now());
            match sessions::create_session_at(db, game_id, device_id, started_wall) {
                Ok(session) => {
                    info!("session started for game {game_id}: {}", session.id);
                    let mut active_session = ActiveSession::new(
                        session.id,
                        game_id.clone(),
                        started,
                        parse_wall_timestamp(&session.started_at_wall),
                        session.integrity_status.clone(),
                    );
                    if observation.has_foreground_window {
                        active_session.last_foreground_at = Some(started);
                    }
                    live.update(
                        &active_session.session_id,
                        active_session.counters(),
                        active_session.last_wall_at,
                    );
                    active.insert(game_id.clone(), active_session);
                }
                Err(error) => {
                    error!("failed to start session for game {game_id}: {error}");
                }
            }
        }
    }
}

/// Counts the tick of `clocks` for each session whose game runs. A session
/// whose game is missing keeps its counters and when they were counted.
fn count_running_sessions(
    db: &Database,
    live: &LiveSessions,
    active: &mut HashMap<String, ActiveSession>,
    observed_games: &HashMap<String, GameObservation>,
    tracking_settings: &TrackingSettings,
    activity_snapshot: &ActivitySnapshot,
    clocks: &Clocks,
) {
    // The counters show in the live views, so the library stays loaded.
    Database::live_writes(|| {
        count_each(
            db,
            live,
            active,
            observed_games,
            tracking_settings,
            activity_snapshot,
            clocks,
        );
    });
}

fn count_each(
    db: &Database,
    live: &LiveSessions,
    active: &mut HashMap<String, ActiveSession>,
    observed_games: &HashMap<String, GameObservation>,
    tracking_settings: &TrackingSettings,
    activity_snapshot: &ActivitySnapshot,
    clocks: &Clocks,
) {
    for session in active.values_mut() {
        if let Some(observation) = observed_games.get(&session.game_id) {
            if !observation.is_running {
                continue;
            }

            let applied = apply_observation(
                db,
                session,
                *observation,
                tracking_settings,
                activity_snapshot,
                clocks,
            );
            live.update(
                &session.session_id,
                session.counters(),
                session.last_wall_at,
            );
            // One session that fails to store its tick holds up no other.
            if let Err(error) = applied {
                error!(
                    "failed to count a tick of session {}: {error}",
                    session.session_id
                );
            }
        }
    }
}

/// Ends the sessions of games that were missing for
/// `MISSED_TICKS_BEFORE_END` ticks in a row, as of the first of them. A
/// session the database fails to close stays to be ended on the next tick,
/// as long as its row is still open.
fn end_finished_sessions(
    db: &Database,
    live: &LiveSessions,
    active: &mut HashMap<String, ActiveSession>,
    observed_games: &HashMap<String, GameObservation>,
    clocks: &Clocks,
) {
    let mut finished = Vec::new();
    for (game_id, session) in active.iter_mut() {
        let running = observed_games
            .get(game_id)
            .is_some_and(|observation| observation.is_running);
        if running {
            session.missed_ticks = 0;
            session.missing_since = None;
            continue;
        }
        session.missed_ticks += 1;
        if session.missing_since.is_none() {
            session.missing_since = Some(session.end_at(clocks));
        }
        if session.missed_ticks >= MISSED_TICKS_BEFORE_END {
            finished.push(game_id.clone());
        }
    }

    for game_id in finished {
        let Some(session) = active.get(&game_id) else {
            continue;
        };
        let ended_at = session
            .missing_since
            .unwrap_or_else(|| session.end_at(clocks));
        if end_active_session(db, live, session, ended_at) {
            active.remove(&game_id);
        } else if !still_open(db, session) {
            live.remove(&session.session_id);
            active.remove(&game_id);
        }
    }
}

/// Closes a session with the counters of its last counted tick, as of
/// `ended_at`. Returns whether it closed.
fn end_active_session(
    db: &Database,
    live: &LiveSessions,
    session: &ActiveSession,
    ended_at: DateTime<Utc>,
) -> bool {
    let ended = sessions::end_session_at(
        db,
        &session.session_id,
        (session.runtime_ms, session.active_ms, session.idle_ms),
        &session.integrity_status,
        &integrity::format_timestamp(ended_at),
    );
    match ended {
        Ok(_) => {
            live.remove(&session.session_id);
            info!(
                "session ended for game {}: {} (runtime={}ms active={}ms idle={}ms)",
                session.game_id,
                session.session_id,
                session.runtime_ms,
                session.active_ms,
                session.idle_ms
            );
            true
        }
        Err(error) => {
            error!(
                "failed to end session {} for game {}: {error}",
                session.session_id, session.game_id
            );
            false
        }
    }
}

/// Whether the database may still hold the session open, so ending it is
/// worth another try. A session removed or closed meanwhile is not. One the
/// database cannot even read is tried again, as letting it go would leave
/// its row open until the next start.
fn still_open(db: &Database, session: &ActiveSession) -> bool {
    match db.with_conn(|conn| {
        sessions::stored_session(conn, &session.session_id).map_err(crate::db::repo::map_db)
    }) {
        Ok(stored) => stored.ended_at_wall.is_none(),
        Err(VaultimeError::NotFound(_)) => false,
        Err(_) => true,
    }
}

/// What each tracked game with an executable is doing right now.
fn observe_games(
    games: &[Game],
    processes: &[RunningProcess],
    activity_snapshot: &ActivitySnapshot,
    list: &mut ProcessList,
    folders: &mut FolderCache,
) -> HashMap<String, GameObservation> {
    let matched: Vec<(&str, Vec<&RunningProcess>)> = games
        .iter()
        .zip(game_processes(games, processes, folders))
        .filter(|(game, _)| game.executable_path.is_some())
        .map(|(game, found)| (game.id.as_str(), found))
        .collect();

    // CPU usage only stands in where the foreground window or input cannot be
    // read, so it is measured there only and for the game processes alone.
    let cpu = if activity_snapshot.foreground_supported && activity_snapshot.idle_supported {
        HashMap::new()
    } else {
        let pids: Vec<u32> = matched
            .iter()
            .flat_map(|(_, found)| found.iter().map(|process| process.pid))
            .collect();
        list.cpu_usage(&pids)
    };

    matched
        .into_iter()
        .map(|(game_id, found)| {
            (
                game_id.to_owned(),
                observe_game_processes(&found, activity_snapshot, &cpu),
            )
        })
        .collect()
}

/// The running processes of each game, in the order of `games`: those that
/// run its executable and, where its launcher gives it a folder of its own,
/// the game programs in that folder. A game without an executable has none.
fn game_processes<'a>(
    games: &[Game],
    processes: &'a [RunningProcess],
    folders: &mut FolderCache,
) -> Vec<Vec<&'a RunningProcess>> {
    let executables = ExecutableIndex::new(
        games
            .iter()
            .map(|game| game.executable_path.as_deref().unwrap_or_default()),
    );
    let mut own_folders = FolderIndex::default();
    for (position, game) in games.iter().enumerate() {
        if game.executable_path.is_some()
            && let Some(folder) = own_folder(game)
        {
            own_folders.add(position, folders.get(folder));
        }
    }
    let mut found = vec![Vec::new(); games.len()];
    for process in processes {
        let mut positions = executables.matches(process);
        let in_own_folders = own_folders.containing(process);
        if !in_own_folders.is_empty() && is_likely_game_executable(&process.name) {
            positions.extend(in_own_folders);
            positions.sort_unstable();
            positions.dedup();
        }
        for position in positions {
            found[position].push(process);
        }
    }
    found
}

/// The install folder whose game programs count for the game too, as its
/// launcher gives every game a folder of its own.
fn own_folder(game: &Game) -> Option<&str> {
    let source = game.launcher_source.as_deref()?;
    if FOLDER_MATCH_LAUNCHERS.contains(&source) {
        game.install_folder.as_deref()
    } else {
        None
    }
}

/// What the tracker makes of a game's install folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderMatch {
    /// Only the game's program counts, as its launcher gives games no folder
    /// of their own, or the game names no folder or program, or is not
    /// tracked here.
    Unused,
    /// Any game program in the folder counts too.
    Counts,
    /// Only the program counts, as the folder is a whole drive or share.
    Refused,
}

/// Whether the game programs in a game's install folder count for it. This
/// resolves the folder's links and mounts.
pub fn folder_match(game: &Game) -> FolderMatch {
    // The tracker watches the games `list_local_games` lists.
    let this = crate::integrity::ledger::this_device().ok();
    let local = game.origin_device_id.is_none() || game.origin_device_id == this;
    let tracked = game.executable_path.is_some() && local;
    match own_folder(game).filter(|_| tracked) {
        None => FolderMatch::Unused,
        Some(folder) if InstallFolder::new(folder).is_refused() => FolderMatch::Refused,
        Some(_) => FolderMatch::Counts,
    }
}

/// Install folders with their links resolved, so a tick needs no file
/// system calls. Links are resolved again after `INSTALL_FOLDER_REFRESH`.
struct FolderCache {
    folders: HashMap<String, InstallFolder>,
    resolved_at: Instant,
}

impl Default for FolderCache {
    fn default() -> Self {
        Self {
            folders: HashMap::new(),
            resolved_at: Instant::now(),
        }
    }
}

impl FolderCache {
    fn expire(&mut self) {
        if self.resolved_at.elapsed() >= INSTALL_FOLDER_REFRESH {
            self.folders.clear();
            self.resolved_at = Instant::now();
        }
    }

    fn get(&mut self, folder: &str) -> &InstallFolder {
        if !self.folders.contains_key(folder) {
            self.folders
                .insert(folder.to_owned(), InstallFolder::new(folder));
        }
        &self.folders[folder]
    }
}

fn observe_game_processes(
    matched_processes: &[&RunningProcess],
    activity_snapshot: &ActivitySnapshot,
    cpu: &HashMap<u32, f32>,
) -> GameObservation {
    if matched_processes.is_empty() {
        return GameObservation::default();
    }

    let has_foreground_window = activity_snapshot
        .foreground_pid
        .is_some_and(|foreground_pid| {
            matched_processes
                .iter()
                .any(|process| process.pid == foreground_pid)
        });

    let has_process_activity = matched_processes.iter().any(|process| {
        cpu.get(&process.pid)
            .is_some_and(|usage| *usage >= PROCESS_ACTIVITY_CPU_PERCENT)
    });

    GameObservation {
        is_running: true,
        has_foreground_window,
        has_process_activity,
    }
}

/// The three clocks of one tick, read at the same moment.
struct Clocks {
    now: Instant,
    wall: DateTime<Utc>,
    /// How long the PC had slept since boot.
    asleep_ms: Option<i64>,
}

impl Clocks {
    fn read() -> Self {
        Self {
            now: Instant::now(),
            wall: Utc::now(),
            asleep_ms: platform::clock::asleep_ms(),
        }
    }
}

/// The time between two ticks on each clock, in milliseconds.
struct ElapsedMs {
    mono: i64,
    wall: i64,
    slept: i64,
}

impl ElapsedMs {
    /// Whether the PC slept or tracking stalled in between, so the time is
    /// skipped, not counted.
    fn is_gap(&self) -> bool {
        self.mono.max(self.wall) > MAX_TICK_GAP_MS || self.slept > SUSPEND_DETECT_MS
    }
}

fn apply_observation(
    db: &Database,
    session: &mut ActiveSession,
    observation: GameObservation,
    tracking_settings: &TrackingSettings,
    activity_snapshot: &ActivitySnapshot,
    clocks: &Clocks,
) -> crate::error::Result<()> {
    let (now, current_wall) = (clocks.now, clocks.wall);
    if observation.has_foreground_window {
        session.last_foreground_at = Some(now);
    }

    if observation.has_process_activity {
        session.last_signal_at = now;
    }

    let elapsed = session.elapsed(clocks);
    let (delta_ms, wall_delta_ms) = (elapsed.mono, elapsed.wall);
    if delta_ms <= 0 {
        return Ok(());
    }

    // The previous tick, where a checkpoint that closes a stretch belongs.
    let previous_wall = session.last_wall_at;
    // The clocks move on only once the writes before them went through, so
    // a tick whose write fails is taken again in full by the next one.
    let move_clocks = |session: &mut ActiveSession| {
        session.last_tick_at = now;
        session.last_wall_at = current_wall;
        session.last_asleep_ms = clocks.asleep_ms;
    };

    if elapsed.is_gap() {
        save_checkpoint(db, session, previous_wall, now)?;
        sessions::record_tracking_gap(
            db,
            &session.session_id,
            session.runtime_ms,
            wall_delta_ms,
            delta_ms,
        )?;
        move_clocks(session);
        session.skipped_wall_ms += wall_delta_ms.max(0);
        return Ok(());
    }

    let active = should_count_as_active(
        session,
        observation,
        tracking_settings,
        activity_snapshot,
        now,
    );
    // The time between two checkpoints stays all active or all idle.
    if session
        .last_tick_active
        .is_some_and(|was_active| was_active != active)
    {
        save_checkpoint(db, session, previous_wall, now)?;
    }
    move_clocks(session);
    session.last_tick_active = Some(active);

    session.runtime_ms += delta_ms;
    if active {
        session.active_ms += delta_ms;
    } else {
        session.idle_ms += delta_ms;
    }

    let wall_elapsed_ms = current_wall
        .signed_duration_since(session.started_at_wall)
        .num_milliseconds()
        .max(0);
    let drift_ms = wall_elapsed_ms - session.skipped_wall_ms - session.runtime_ms;
    session.last_checkpoint_fields = Checkpoint {
        runtime_ms: session.runtime_ms,
        active_ms: session.active_ms,
        idle_ms: session.idle_ms,
        wall_elapsed_ms,
        drift_ms,
    };

    let mut flagged = false;
    if let Some(reason) = detect_integrity_reason(wall_delta_ms, delta_ms, drift_ms)
        && session.integrity_status != integrity::STATUS_SUSPICIOUS
    {
        sessions::flag_session_suspicious(
            db,
            &session.session_id,
            session.runtime_ms,
            wall_elapsed_ms,
            drift_ms,
            reason,
        )?;
        session.integrity_status = integrity::STATUS_SUSPICIOUS.into();
        // Its trust label shows in the library too.
        db.bump_revision();
        flagged = true;
    }

    if flagged || now.saturating_duration_since(session.stored_at) >= CHECKPOINT_INTERVAL {
        save_checkpoint(db, session, current_wall, now)?;
    }
    Ok(())
}

/// Stores the counters as of the last counted tick, which happened at
/// `wall`, unless the database has them already.
fn save_checkpoint(
    db: &Database,
    session: &mut ActiveSession,
    wall: DateTime<Utc>,
    now: Instant,
) -> crate::error::Result<()> {
    if !session.has_unsaved_time() {
        return Ok(());
    }
    let checkpoint = session.last_checkpoint_fields;
    sessions::record_checkpoint(
        db,
        &session.session_id,
        &checkpoint,
        &session.integrity_status,
        wall,
    )?;
    session.stored = checkpoint;
    session.stored_at = now;
    Ok(())
}

fn should_count_as_active(
    session: &ActiveSession,
    observation: GameObservation,
    tracking_settings: &TrackingSettings,
    activity_snapshot: &ActivitySnapshot,
    now: Instant,
) -> bool {
    if activity_snapshot.idle_supported {
        if activity_snapshot
            .idle_for
            .is_some_and(|idle_for| idle_for >= tracking_settings.idle_threshold)
        {
            return false;
        }
    } else if !observation.has_process_activity
        && now.saturating_duration_since(session.last_signal_at) >= tracking_settings.idle_threshold
    {
        return false;
    }

    if tracking_settings.treat_background_as_active {
        return true;
    }

    if activity_snapshot.foreground_supported {
        if observation.has_foreground_window {
            return true;
        }

        // On Wayland a game that never showed up as the X11 window in front
        // runs as a Wayland window, and whether it has the focus cannot be
        // told. While no X11 window has it, input alone decides for it. A game
        // that had the X11 focus before has lost it to a Wayland app.
        if !activity_snapshot.foreground_known && session.last_foreground_at.is_none() {
            return true;
        }

        if let Some(last_foreground_at) = session.last_foreground_at {
            return now.saturating_duration_since(last_foreground_at) <= FOREGROUND_GRACE;
        }

        return false;
    }

    observation.has_process_activity
        || now.saturating_duration_since(session.last_signal_at) < tracking_settings.idle_threshold
}

fn detect_integrity_reason(
    wall_delta_ms: i64,
    monotonic_delta_ms: i64,
    drift_ms: i64,
) -> Option<&'static str> {
    if wall_delta_ms < -CLOCK_BACKWARDS_TOLERANCE_MS {
        return Some("wall_clock_moved_backwards");
    }

    if (wall_delta_ms - monotonic_delta_ms).abs() > CLOCK_STEP_TOLERANCE_MS {
        return Some("wall_clock_step_mismatch");
    }

    if drift_ms.abs() > CLOCK_TOTAL_DRIFT_TOLERANCE_MS {
        return Some("wall_clock_drift_exceeded");
    }

    None
}

fn parse_wall_timestamp(value: &str) -> DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map_or_else(|_| Utc::now(), |timestamp| timestamp.with_timezone(&Utc))
}

/// Closes any sessions left open from a previous run, for example after a crash.
fn close_orphaned_sessions(db: &Database, device_id: &str) {
    match sessions::get_active_sessions(db, device_id) {
        Ok(orphans) if !orphans.is_empty() => {
            warn!(
                "found {} orphaned session(s) from previous run, closing",
                orphans.len()
            );
            for session in orphans {
                if let Err(error) =
                    sessions::recover_session(db, &session.id, "startup_orphan_cleanup")
                {
                    error!("failed to close orphaned session {}: {error}", session.id);
                }
            }
        }
        Ok(_) => {}
        Err(error) => {
            error!("failed to check for orphaned sessions: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::{devices, session_events};

    /// A game in front that uses the CPU.
    const PLAYING: GameObservation = GameObservation {
        is_running: true,
        has_foreground_window: true,
        has_process_activity: true,
    };

    fn settings() -> TrackingSettings {
        TrackingSettings {
            idle_threshold: Duration::from_secs(300),
            treat_background_as_active: false,
        }
    }

    /// A desktop that reports input and the window in front, with the last
    /// input `idle_for` ago.
    fn desktop(idle_for: Duration) -> ActivitySnapshot {
        ActivitySnapshot {
            foreground_pid: None,
            foreground_supported: true,
            foreground_known: true,
            idle_for: Some(idle_for),
            idle_supported: true,
        }
    }

    /// A database of this PC with one game. Returns the game's id.
    fn library_with(title: &str) -> (Database, String) {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: title.into(),
                executable_path: Some("game.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        (db, game.id)
    }

    /// The processes of a game the way a tick matched them before the
    /// indexes: every process checked against the game.
    fn processes_one_by_one<'a>(
        game: &Game,
        processes: &'a [RunningProcess],
        folders: &mut FolderCache,
    ) -> Vec<&'a RunningProcess> {
        let Some(exe_path) = game.executable_path.as_deref() else {
            return Vec::new();
        };
        let own_folder = own_folder(game).map(|folder| folders.get(folder));
        processes
            .iter()
            .filter(|process| {
                crate::platform::process::matches_executable(process, exe_path)
                    || own_folder.as_ref().is_some_and(|folder| {
                        folder.contains(process) && is_likely_game_executable(&process.name)
                    })
            })
            .collect()
    }

    #[test]
    fn games_are_matched_as_one_by_one() {
        let db = Database::open_in_memory().unwrap();
        let root =
            std::env::temp_dir().join(format!("vaultime-match-test-{}", uuid::Uuid::new_v4()));
        // Files that exist, so paths that differ tell games of the same name apart.
        let file = |rest: &str| {
            let path = root.join(rest);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
            path.to_string_lossy().into_owned()
        };
        let folder = |rest: &str| root.join(rest).to_string_lossy().into_owned();
        for (title, exe, install, launcher) in [
            (
                "Steam game",
                Some("Steam/Ember/bin/ember.exe"),
                Some("Steam/Ember"),
                Some("steam"),
            ),
            (
                "Steam neighbor",
                Some("Steam/EmberTwo/ember.exe"),
                Some("Steam/EmberTwo"),
                Some("steam"),
            ),
            ("Loose game", Some("Loose/run.exe"), Some("Loose"), None),
            ("Same name", Some("Elsewhere/run.exe"), None, None),
            ("No program", None, Some("Steam/Nothing"), Some("steam")),
        ] {
            games::create_game(
                &db,
                &CreateGame {
                    title: title.into(),
                    executable_path: exe.map(file),
                    install_folder: install.map(folder),
                    launcher_source: launcher.map(Into::into),
                },
            )
            .unwrap();
        }
        let library = games::list_local_games(&db).unwrap();
        let running = |pid: u32, name: &str, exe: Option<String>| {
            RunningProcess::new(pid, name.into(), exe, None)
        };
        let processes = vec![
            running(1, "ember.exe", Some(file("Steam/Ember/bin/ember.exe"))),
            running(
                2,
                "ember-dx12.exe",
                Some(file("Steam/Ember/bin/ember-dx12.exe")),
            ),
            running(
                3,
                "UnityCrashHandler64.exe",
                Some(file("Steam/Ember/UnityCrashHandler64.exe")),
            ),
            running(4, "ember.exe", Some(file("Steam/EmberTwo/ember.exe"))),
            running(5, "run.exe", None),
            running(6, "run.exe", Some(file("Loose/run.exe"))),
            running(7, "other.exe", Some(file("Loose/other.exe"))),
            running(8, "game.exe", Some(file("Steam/Nothing/game.exe"))),
            running(9, "explorer.exe", Some(file("Windows/explorer.exe"))),
        ];
        let mut folders = FolderCache::default();
        let found = game_processes(&library, &processes, &mut folders);
        let pids =
            |list: &[&RunningProcess]| list.iter().map(|process| process.pid).collect::<Vec<_>>();
        for (game, found) in library.iter().zip(&found) {
            let one_by_one = processes_one_by_one(game, &processes, &mut folders);
            assert_eq!(pids(found), pids(&one_by_one), "{}", game.title);
        }
        let pids_of = |title: &str| {
            pids(&found[library.iter().position(|game| game.title == title).unwrap()])
        };
        // The folder counts for Steam games, a crash handler never does.
        assert_eq!(pids_of("Steam game"), [1, 2]);
        assert_eq!(pids_of("Steam neighbor"), [4]);
        // A hidden path matches by name alone.
        assert_eq!(pids_of("Loose game"), [5, 6]);
        assert_eq!(pids_of("Same name"), [5]);
        assert!(pids_of("No program").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Starts a real installed game and checks that a session opens and closes.
    /// Set `VAULTIME_GAME_EXE` and `VAULTIME_GAME_FOLDER` (the Steam install
    /// folder) and run `cargo test -- --ignored --nocapture real_game`.
    #[test]
    #[ignore = "starts a real game"]
    fn tracks_a_real_game() {
        let exe = std::env::var("VAULTIME_GAME_EXE").unwrap();
        let folder = std::env::var("VAULTIME_GAME_FOLDER").unwrap();
        let db = Arc::new(Database::open_in_memory().unwrap());
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Real Game".into(),
                executable_path: Some(exe.clone()),
                install_folder: Some(folder.clone()),
                launcher_source: Some("steam".into()),
            },
        )
        .unwrap();
        let engine = TrackingEngine::start(Arc::clone(&db), "device".into(), Arc::default());

        let mut child = std::process::Command::new(&exe)
            .current_dir(std::path::Path::new(&exe).parent().unwrap())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        while sessions::get_active_sessions(&db, "device")
            .unwrap()
            .is_empty()
        {
            assert!(Instant::now() < deadline, "the game was not seen");
            std::thread::sleep(Duration::from_millis(250));
        }
        println!("session opened");
        std::thread::sleep(Duration::from_secs(15));

        // End every process of the game, it may have restarted itself through Steam.
        let _ = child.kill();
        let _ = child.wait();
        let mut system = sysinfo::System::new().unwrap();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        for process in ProcessList::default().refresh() {
            if crate::platform::process::runs_from_folder(&process, &folder)
                && let Some(running) = system.process(sysinfo::Pid::from_u32(process.pid))
            {
                running.kill();
            }
        }
        let deadline = Instant::now() + POLL_INTERVAL * 3;
        while !sessions::get_active_sessions(&db, "device")
            .unwrap()
            .is_empty()
        {
            assert!(Instant::now() < deadline, "the session did not close");
            std::thread::sleep(Duration::from_millis(250));
        }
        engine.shutdown();

        let session = &sessions::list_sessions_for_game(&db, &game.id).unwrap()[0];
        println!(
            "session closed: runtime {} ms, active {} ms, idle {} ms, {}",
            session.runtime_ms, session.active_ms, session.idle_ms, session.integrity_status
        );
        assert!(session.closed_cleanly);
        // Runtime grows once per tick, so a tick is all the test can count on.
        assert!(u128::from(session.runtime_ms.unsigned_abs()) >= POLL_INTERVAL.as_millis());
    }

    /// Times the parts of a tick on a library of 300 games, half of them
    /// matched by their install folder, against the processes running here:
    /// `cargo test --release -- --ignored --nocapture tick_timings`.
    #[test]
    #[ignore = "prints timings"]
    fn tick_timings() {
        const GAMES: usize = 300;
        const ROUNDS: u32 = 50;
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        for index in 0..GAMES {
            let steam = index % 2 == 0;
            games::create_game(
                &db,
                &CreateGame {
                    title: format!("Game {index}"),
                    executable_path: Some(format!(r"C:\Games\Game {index}\Bin\game{index}.exe")),
                    install_folder: Some(format!(r"C:\Games\Game {index}")),
                    launcher_source: steam.then(|| "steam".into()),
                },
            )
            .unwrap();
        }
        let mut list = ProcessList::default();
        let mut folders = FolderCache::default();
        let time = |label: &str, run: &mut dyn FnMut()| {
            run();
            let started = Instant::now();
            for _ in 0..ROUNDS {
                run();
            }
            println!("{label}: {:?}", started.elapsed() / ROUNDS);
        };
        time("settings", &mut || {
            TrackingSettings::load(&db);
        });
        time("local games", &mut || {
            games::list_local_games(&db).unwrap();
        });
        time("ignored", &mut || {
            ignored::paths(&db).unwrap();
        });
        time("processes", &mut || {
            list.refresh();
        });
        time("activity", &mut || {
            capture_activity_snapshot();
        });
        let tracked = games::list_local_games(&db).unwrap();
        let processes = list.refresh();
        let snapshot = capture_activity_snapshot();
        println!("{} processes", processes.len());
        time("observe", &mut || {
            observe_games(&tracked, &processes, &snapshot, &mut list, &mut folders);
        });
        let live = LiveSessions::default();
        let mut active = HashMap::new();
        let mut seen = SeenPrograms::default();
        time("whole tick", &mut || {
            poll_tick(
                &db,
                "device",
                &live,
                &mut list,
                &mut folders,
                &mut seen,
                &mut active,
            )
            .unwrap();
        });
    }

    #[test]
    fn shutdown_closes_running_sessions_without_waiting_for_a_tick() {
        // The test binary itself is the running "game".
        let db = Arc::new(Database::open_in_memory().unwrap());
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let exe = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Test Binary".into(),
                executable_path: Some(exe),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();

        let engine = TrackingEngine::start(Arc::clone(&db), "device".into(), Arc::default());
        let deadline = Instant::now() + POLL_INTERVAL;
        while sessions::get_active_sessions(&db, "device")
            .unwrap()
            .is_empty()
        {
            assert!(Instant::now() < deadline, "no session started");
            std::thread::sleep(Duration::from_millis(20));
        }

        let started = Instant::now();
        engine.shutdown();
        assert!(started.elapsed() < POLL_INTERVAL);
        assert!(
            sessions::get_active_sessions(&db, "device")
                .unwrap()
                .is_empty()
        );
        let closed = &sessions::list_sessions_for_game(&db, &game.id).unwrap()[0];
        assert!(closed.closed_cleanly);
        assert_eq!(closed.integrity_status, "local");
    }

    struct Fixture {
        db: Database,
        game_id: String,
        session_id: String,
    }

    fn fixture() -> Fixture {
        fixture_from(Utc::now())
    }

    /// A game with a session that started at `started` and still runs.
    fn fixture_from(started: DateTime<Utc>) -> Fixture {
        let (db, game_id) = library_with("Gap Game");
        let session = sessions::create_session_at(&db, &game_id, "device", started).unwrap();
        Fixture {
            db,
            game_id,
            session_id: session.id,
        }
    }

    /// An open session whose previous tick was `mono` ago on the monotonic
    /// clock and `wall` ago on the wall clock.
    fn session_after(f: &Fixture, mono: Duration, wall: chrono::Duration) -> ActiveSession {
        ActiveSession::new(
            f.session_id.clone(),
            f.game_id.clone(),
            Instant::now().checked_sub(mono).unwrap(),
            Utc::now() - wall,
            integrity::STATUS_LOCAL.into(),
        )
    }

    fn tick(f: &Fixture, session: &mut ActiveSession) {
        tick_at(f, session, None);
    }

    fn tick_at(f: &Fixture, session: &mut ActiveSession, asleep_ms: Option<i64>) {
        apply_observation(
            &f.db,
            session,
            PLAYING,
            &settings(),
            &desktop(Duration::ZERO),
            &Clocks {
                now: Instant::now(),
                wall: Utc::now(),
                asleep_ms,
            },
        )
        .unwrap();
    }

    #[test]
    fn a_restore_waits_for_running_games_and_tracking_goes_on_after() {
        integrity::ledger::act_as(Some(&integrity::ledger::test_pc("device")));
        let (db, game_id) = library_with("Restored Game");
        let db = Arc::new(db);
        // The engine tracks a PC of its own, so it leaves the session alone.
        let engine = TrackingEngine::start(Arc::clone(&db), "engine-pc".into(), Arc::default());
        let open = sessions::create_session(&db, &game_id, "device").unwrap();

        let refused = engine.while_paused(&db, "device", || Ok(()));
        assert!(refused.is_err());
        assert!(engine.is_running());

        sessions::end_session(&db, &open.id, 0, 0, 0, integrity::STATUS_LOCAL).unwrap();
        assert_eq!(engine.while_paused(&db, "device", || Ok(7)).unwrap(), 7);
        assert!(engine.is_running());
        // A restore that made this PC another one leaves tracking paused.
        engine.while_paused(&db, "another-pc", || Ok(())).unwrap();
        assert!(!engine.is_running());
        engine.shutdown();
    }

    #[test]
    fn a_step_that_fails_or_panics_is_caught() {
        assert_eq!(run_guarded("a step", || Ok(())), Guarded::Done);
        assert_eq!(
            run_guarded("a step", || Err(VaultimeError::Tracking("refused".into()))),
            Guarded::Failed
        );
        assert_eq!(
            run_guarded("a step", || panic!("a platform API went wrong")),
            Guarded::Panicked
        );
    }

    /// One running session of the fixture's game, as the tracker holds it.
    fn tracked(f: &Fixture) -> HashMap<String, ActiveSession> {
        HashMap::from([(
            f.game_id.clone(),
            session_after(f, Duration::from_secs(5), chrono::Duration::seconds(5)),
        )])
    }

    fn is_open(f: &Fixture) -> bool {
        !sessions::get_active_sessions(&f.db, "device")
            .unwrap()
            .is_empty()
    }

    fn ended_at(f: &Fixture) -> Option<String> {
        f.db.with_conn(|conn| {
            sessions::stored_session(conn, &f.session_id).map_err(crate::db::repo::map_db)
        })
        .unwrap()
        .ended_at_wall
    }

    /// The clocks of a tick at `wall` that follows the last one right away.
    fn clocks_at(wall: DateTime<Utc>) -> Clocks {
        Clocks {
            now: Instant::now(),
            wall,
            asleep_ms: None,
        }
    }

    /// The tracker counted a tick of the running game at `wall`.
    fn counted_at(active: &mut HashMap<String, ActiveSession>, f: &Fixture, wall: DateTime<Utc>) {
        let session = active.get_mut(&f.game_id).unwrap();
        session.last_tick_at = Instant::now();
        session.last_wall_at = wall;
    }

    #[test]
    fn a_session_ends_only_once_its_game_was_missing_for_several_ticks() {
        let f = fixture();
        let live = LiveSessions::default();
        let mut active = tracked(&f);
        let missing = HashMap::new();
        let running = HashMap::from([(f.game_id.clone(), PLAYING)]);

        for _ in 1..MISSED_TICKS_BEFORE_END {
            end_finished_sessions(&f.db, &live, &mut active, &missing, &clocks_at(Utc::now()));
        }
        assert!(active.contains_key(&f.game_id));
        // Back in the list, so missing again starts the count over.
        end_finished_sessions(&f.db, &live, &mut active, &running, &clocks_at(Utc::now()));
        for _ in 1..MISSED_TICKS_BEFORE_END {
            end_finished_sessions(&f.db, &live, &mut active, &missing, &clocks_at(Utc::now()));
        }
        assert!(active.contains_key(&f.game_id));
        assert!(is_open(&f));

        end_finished_sessions(&f.db, &live, &mut active, &missing, &clocks_at(Utc::now()));
        assert!(active.is_empty());
        assert!(!is_open(&f));
    }

    #[test]
    fn a_session_ends_where_its_game_went_missing_also_across_a_sleep() {
        let f = fixture();
        let live = LiveSessions::default();
        let mut active = tracked(&f);
        let running = HashMap::from([(f.game_id.clone(), PLAYING)]);
        let missing = HashMap::new();
        let step = chrono::Duration::from_std(POLL_INTERVAL).unwrap();
        let first = Utc::now();
        let back = first + step;
        let gone = back + step;

        // Missing once, back again, then gone for good.
        end_finished_sessions(&f.db, &live, &mut active, &missing, &clocks_at(first));
        counted_at(&mut active, &f, back);
        end_finished_sessions(&f.db, &live, &mut active, &running, &clocks_at(back));
        end_finished_sessions(&f.db, &live, &mut active, &missing, &clocks_at(gone));
        // The next tick came only after the PC slept for hours.
        let woke = gone + chrono::Duration::hours(5);
        for _ in 1..MISSED_TICKS_BEFORE_END {
            end_finished_sessions(&f.db, &live, &mut active, &missing, &clocks_at(woke));
        }
        assert!(active.is_empty());
        assert_eq!(ended_at(&f), Some(integrity::format_timestamp(gone)));
    }

    #[test]
    fn a_game_gone_at_the_first_tick_after_a_sleep_ends_at_its_last_counted_tick() {
        // Hours on the wall clock alone as Linux sleeps, and half a minute
        // only the time asleep tells, too short for the gap limit.
        for (asleep, slept_ms) in [
            (chrono::Duration::hours(8), 0),
            (chrono::Duration::seconds(30), 30_000),
        ] {
            let f = fixture_from(Utc::now() - chrono::Duration::minutes(1));
            let live = LiveSessions::default();
            let mut active = tracked(&f);
            let session = active.get_mut(&f.game_id).unwrap();
            session.last_asleep_ms = Some(1_000);
            let last = session.last_wall_at;
            let woke = Clocks {
                now: Instant::now(),
                wall: last + asleep + chrono::Duration::from_std(POLL_INTERVAL).unwrap(),
                asleep_ms: Some(1_000 + slept_ms),
            };

            for _ in 0..MISSED_TICKS_BEFORE_END {
                end_finished_sessions(&f.db, &live, &mut active, &HashMap::new(), &woke);
            }
            assert!(active.is_empty());
            assert_eq!(ended_at(&f), Some(integrity::format_timestamp(last)));
        }
    }

    #[test]
    fn a_running_game_ends_before_a_sleep_when_tracking_stops_right_after_it() {
        let f = fixture_from(Utc::now() - chrono::Duration::hours(6));
        let session = session_after(&f, Duration::from_secs(5), chrono::Duration::hours(5));
        let last = session.last_wall_at;

        end_all_sessions(
            &f.db,
            &LiveSessions::default(),
            HashMap::from([(f.game_id.clone(), session)]),
        );
        assert_eq!(ended_at(&f), Some(integrity::format_timestamp(last)));
    }

    #[test]
    fn a_session_whose_game_went_missing_ends_there_when_tracking_stops() {
        let f = fixture();
        let live = LiveSessions::default();
        let mut active = tracked(&f);
        let gone = Utc::now();
        end_finished_sessions(&f.db, &live, &mut active, &HashMap::new(), &clocks_at(gone));

        end_all_sessions(&f.db, &live, active);
        assert_eq!(ended_at(&f), Some(integrity::format_timestamp(gone)));
    }

    #[test]
    fn a_session_the_database_failed_to_close_is_ended_on_a_later_tick() {
        let f = fixture();
        let live = LiveSessions::default();
        let mut active = tracked(&f);
        let missing = HashMap::new();
        let refuse = |sql: &str| {
            f.db.with_conn(|conn| conn.execute_batch(sql).map_err(crate::db::repo::map_db))
                .unwrap();
        };
        refuse(
            "CREATE TEMP TRIGGER refuse_end BEFORE UPDATE OF ended_at_wall ON sessions
             BEGIN SELECT RAISE(ABORT, 'disk full'); END;",
        );

        for _ in 0..MISSED_TICKS_BEFORE_END {
            end_finished_sessions(&f.db, &live, &mut active, &missing, &clocks_at(Utc::now()));
        }
        assert!(active.contains_key(&f.game_id));
        assert!(is_open(&f));

        refuse("DROP TRIGGER refuse_end;");
        end_finished_sessions(&f.db, &live, &mut active, &missing, &clocks_at(Utc::now()));
        assert!(active.is_empty());
        assert!(!is_open(&f));
    }

    #[test]
    fn a_session_closed_meanwhile_is_let_go() {
        let f = fixture();
        let live = LiveSessions::default();
        let mut active = tracked(&f);
        sessions::end_session(&f.db, &f.session_id, 0, 0, 0, integrity::STATUS_LOCAL).unwrap();

        for _ in 0..MISSED_TICKS_BEFORE_END {
            end_finished_sessions(
                &f.db,
                &live,
                &mut active,
                &HashMap::new(),
                &clocks_at(Utc::now()),
            );
        }
        assert!(active.is_empty());
    }

    #[test]
    fn a_start_closes_only_the_sessions_this_pc_left_open() {
        let f = fixture();
        devices::ensure_device(&f.db, "laptop", "test", "0.1.0").unwrap();
        let theirs = sessions::create_session(&f.db, &f.game_id, "laptop").unwrap();
        close_orphaned_sessions(&f.db, "device");
        assert!(
            sessions::get_active_sessions(&f.db, "device")
                .unwrap()
                .is_empty()
        );
        let open = sessions::get_active_sessions(&f.db, "laptop").unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, theirs.id);
    }

    #[test]
    fn a_new_session_counts_from_its_start() {
        let (db, game_id) = library_with("New Game");
        let live = LiveSessions::default();
        let mut active = HashMap::new();
        let observed = HashMap::from([
            (game_id.clone(), PLAYING),
            ("not-running".to_owned(), GameObservation::default()),
        ]);
        start_sessions(&db, "device", &live, &mut active, &observed);
        // A game already tracked keeps its session.
        start_sessions(&db, "device", &live, &mut active, &observed);

        let open = sessions::get_active_sessions(&db, "device").unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(active.len(), 1);
        let session = &active[&game_id];
        assert_eq!(session.session_id, open[0].id);
        assert!(session.last_foreground_at.is_some());
        // The page's clock runs on from the start until the first tick counts.
        assert_eq!(live.get(&open[0].id), Some(LiveCounters::default()));
        assert_eq!(
            live.counted_at_ms(&open[0].id),
            Some(parse_wall_timestamp(&open[0].started_at_wall).timestamp_millis())
        );
    }

    #[test]
    fn checkpoints_leave_the_library_revision_and_a_start_and_an_end_move_it() {
        let (db, game_id) = library_with("Long Game");
        let live = LiveSessions::default();
        let mut active = HashMap::new();
        let running = HashMap::from([(game_id.clone(), PLAYING)]);
        let before = db.revision();
        start_sessions(&db, "device", &live, &mut active, &running);
        let started = db.revision();
        assert!(started > before);

        let session = &active[&game_id];
        let session_id = session.session_id.clone();
        let (now, wall) = (session.last_tick_at, session.last_wall_at);
        let step = chrono::Duration::from_std(POLL_INTERVAL).unwrap();
        let tick = |count: u32| Clocks {
            now: now + POLL_INTERVAL * count,
            wall: wall + step * i32::try_from(count).unwrap(),
            asleep_ms: None,
        };
        let events = || {
            db.with_conn(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM session_events WHERE session_id = ?1",
                    [&session_id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(crate::db::repo::map_db)
            })
            .unwrap()
        };
        let events_at_start = events();
        let past_a_checkpoint =
            u32::try_from(CHECKPOINT_INTERVAL.as_secs() / POLL_INTERVAL.as_secs()).unwrap() + 2;
        for count in 1..=past_a_checkpoint {
            count_running_sessions(
                &db,
                &live,
                &mut active,
                &running,
                &settings(),
                &desktop(Duration::ZERO),
                &tick(count),
            );
        }
        assert!(events() > events_at_start, "a checkpoint was written");
        assert_eq!(db.revision(), started);

        let missing = HashMap::from([(game_id.clone(), GameObservation::default())]);
        for count in 1..=MISSED_TICKS_BEFORE_END {
            end_finished_sessions(
                &db,
                &live,
                &mut active,
                &missing,
                &tick(past_a_checkpoint + count),
            );
        }
        assert!(active.is_empty());
        assert!(db.revision() > started);
    }

    #[test]
    fn a_missed_tick_keeps_what_the_tick_before_counted() {
        let f = fixture();
        let live = LiveSessions::default();
        let start = clocks_at(Utc::now());
        let session = ActiveSession::new(
            f.session_id.clone(),
            f.game_id.clone(),
            start.now,
            start.wall,
            integrity::STATUS_LOCAL.into(),
        );
        let mut active = HashMap::from([(f.game_id.clone(), session)]);
        let step = chrono::Duration::from_std(POLL_INTERVAL).unwrap();
        let tick = |count: u32| Clocks {
            now: start.now + POLL_INTERVAL * count,
            wall: start.wall + step * i32::try_from(count).unwrap(),
            asleep_ms: None,
        };
        let running = HashMap::from([(f.game_id.clone(), PLAYING)]);
        let missing = HashMap::from([(f.game_id.clone(), GameObservation::default())]);
        let count = |active: &mut HashMap<String, ActiveSession>,
                     observed: &HashMap<String, GameObservation>,
                     clocks: &Clocks| {
            count_running_sessions(
                &f.db,
                &live,
                active,
                observed,
                &settings(),
                &desktop(Duration::ZERO),
                clocks,
            );
            end_finished_sessions(&f.db, &live, active, observed, clocks);
        };
        let counted = |runtime_ms| LiveCounters {
            runtime_ms,
            active_ms: runtime_ms,
            idle_ms: 0,
        };

        count(&mut active, &running, &tick(1));
        count(&mut active, &missing, &tick(2));
        assert!(active.contains_key(&f.game_id));
        assert_eq!(live.get(&f.session_id), Some(counted(5_000)));
        assert_eq!(
            live.counted_at_ms(&f.session_id),
            Some(tick(1).wall.timestamp_millis())
        );

        // Back at the next tick, it counts the time since the last one.
        count(&mut active, &running, &tick(3));
        assert_eq!(live.get(&f.session_id), Some(counted(15_000)));
        assert_eq!(
            live.counted_at_ms(&f.session_id),
            Some(tick(3).wall.timestamp_millis())
        );
    }

    #[test]
    fn regular_tick_counts_active_time() {
        let f = fixture();
        let mut session = session_after(&f, Duration::from_secs(5), chrono::Duration::seconds(5));
        tick(&f, &mut session);

        assert!(session.runtime_ms >= 5_000);
        assert_eq!(session.active_ms, session.runtime_ms);
        assert_eq!(session.integrity_status, integrity::STATUS_LOCAL);
    }

    #[test]
    fn sleep_gap_is_skipped_without_flagging() {
        let f = fixture_from(Utc::now() - chrono::Duration::hours(3));
        // Linux suspend: the wall clock moved two hours, the monotonic clock did not.
        let mut session = session_after(&f, Duration::from_secs(5), chrono::Duration::hours(2));
        tick(&f, &mut session);

        assert_eq!(session.runtime_ms, 0);
        assert_eq!(session.integrity_status, integrity::STATUS_LOCAL);
        let events = session_events::list_events_for_game(&f.db, &f.game_id).unwrap();
        assert!(events.iter().any(|e| e.event_type == "tracking_gap"));

        // The skipped time must not trip the drift check on the next tick.
        session.last_tick_at = Instant::now().checked_sub(Duration::from_secs(5)).unwrap();
        session.last_wall_at = Utc::now() - chrono::Duration::seconds(5);
        tick(&f, &mut session);
        assert_eq!(session.integrity_status, integrity::STATUS_LOCAL);
    }

    #[test]
    fn windows_style_sleep_gap_is_skipped() {
        let f = fixture_from(Utc::now() - chrono::Duration::hours(2));
        // Both clocks kept running through sleep.
        let mut session = session_after(&f, Duration::from_secs(3_600), chrono::Duration::hours(1));
        tick(&f, &mut session);

        assert_eq!(session.runtime_ms, 0);
        assert_eq!(session.integrity_status, integrity::STATUS_LOCAL);
    }

    #[test]
    fn wayland_focus_elsewhere_counts_only_for_games_that_never_had_it() {
        let f = fixture();
        let session = session_after(&f, Duration::from_secs(5), chrono::Duration::seconds(5));
        let behind = GameObservation {
            has_foreground_window: false,
            ..PLAYING
        };
        let wayland_window_in_front = ActivitySnapshot {
            foreground_known: false,
            ..desktop(Duration::ZERO)
        };
        let settings = settings();
        let now = Instant::now();
        // A native Wayland game never shows up in front, so input decides.
        assert!(should_count_as_active(
            &session,
            behind,
            &settings,
            &wayland_window_in_front,
            now
        ));
        // An XWayland game that had the focus lost it to a Wayland app.
        let mut switched_away = session;
        switched_away.last_foreground_at = now.checked_sub(Duration::from_secs(60));
        assert!(!should_count_as_active(
            &switched_away,
            behind,
            &settings,
            &wayland_window_in_front,
            now
        ));
    }

    #[test]
    fn every_signal_decides_activity_as_documented() {
        let f = fixture();
        let mut session = session_after(&f, Duration::from_secs(5), chrono::Duration::seconds(5));
        let settings = settings();
        let now = Instant::now();
        let quiet = settings.idle_threshold + Duration::from_secs(1);
        let in_front = PLAYING;
        let behind = GameObservation {
            has_foreground_window: false,
            ..in_front
        };
        let still = GameObservation {
            has_process_activity: false,
            ..behind
        };
        let full = desktop(Duration::ZERO);
        let away = desktop(settings.idle_threshold);
        let heuristic = ActivitySnapshot {
            foreground_supported: false,
            idle_for: None,
            idle_supported: false,
            ..full
        };
        let decide = |session: &ActiveSession, observation, settings, snapshot| {
            should_count_as_active(session, observation, settings, snapshot, now)
        };

        // Input and the window in front.
        assert!(decide(&session, in_front, &settings, &full));
        assert!(!decide(&session, in_front, &settings, &away));
        assert!(!decide(&session, behind, &settings, &full));
        let background = TrackingSettings {
            treat_background_as_active: true,
            ..settings
        };
        assert!(decide(&session, behind, &background, &full));
        assert!(!decide(&session, behind, &background, &away));

        // A quick switch away keeps counting for the grace period only.
        session.last_foreground_at = now.checked_sub(FOREGROUND_GRACE);
        assert!(decide(&session, behind, &settings, &full));
        session.last_foreground_at = now.checked_sub(FOREGROUND_GRACE + Duration::from_secs(1));
        assert!(!decide(&session, behind, &settings, &full));

        // Without input or window APIs the game's CPU use stands in, for as
        // long as the idle threshold after its last sign of life.
        assert!(decide(&session, behind, &settings, &heuristic));
        session.last_signal_at = now.checked_sub(Duration::from_secs(60)).unwrap();
        assert!(decide(&session, still, &settings, &heuristic));
        session.last_signal_at = now.checked_sub(quiet).unwrap();
        assert!(!decide(&session, still, &settings, &heuristic));
        assert!(!decide(&session, still, &background, &heuristic));
    }

    #[test]
    fn each_clock_check_names_its_reason() {
        let tick = POLL_INTERVAL.as_millis() as i64;
        assert_eq!(detect_integrity_reason(tick, tick, 0), None);
        assert_eq!(
            detect_integrity_reason(-CLOCK_BACKWARDS_TOLERANCE_MS - 1, tick, 0),
            Some("wall_clock_moved_backwards")
        );
        assert_eq!(
            detect_integrity_reason(tick + CLOCK_STEP_TOLERANCE_MS + 1, tick, 0),
            Some("wall_clock_step_mismatch")
        );
        assert_eq!(
            detect_integrity_reason(tick + CLOCK_STEP_TOLERANCE_MS, tick, 0),
            None
        );
        assert_eq!(
            detect_integrity_reason(tick, tick, CLOCK_TOTAL_DRIFT_TOLERANCE_MS + 1),
            Some("wall_clock_drift_exceeded")
        );
        assert_eq!(
            detect_integrity_reason(tick, tick, -CLOCK_TOTAL_DRIFT_TOLERANCE_MS - 1),
            Some("wall_clock_drift_exceeded")
        );
    }

    #[test]
    fn a_short_suspend_is_skipped_not_flagged() {
        let f = fixture_from(Utc::now() - chrono::Duration::minutes(1));
        // Half a minute asleep: too short for the gap limit, but the time
        // the PC slept grew by it.
        let mut session = session_after(&f, Duration::from_secs(5), chrono::Duration::seconds(35));
        session.last_asleep_ms = Some(1_000);
        tick_at(&f, &mut session, Some(1_000 + 30_000));

        assert_eq!(session.runtime_ms, 0);
        assert_eq!(session.integrity_status, integrity::STATUS_LOCAL);
    }

    #[test]
    fn a_negative_or_falling_sleep_reading_changes_nothing() {
        let f = fixture();
        let mut session = session_after(&f, Duration::from_secs(5), chrono::Duration::seconds(5));
        session.last_asleep_ms = Some(-500);
        tick_at(&f, &mut session, Some(-40_000));

        assert!(session.runtime_ms >= 5_000);
        assert_eq!(session.active_ms, session.runtime_ms);
        assert_eq!(session.integrity_status, integrity::STATUS_LOCAL);
        let events = session_events::list_events_for_game(&f.db, &f.game_id).unwrap();
        assert!(!events.iter().any(|e| e.event_type == "tracking_gap"));
    }

    #[test]
    fn clock_moved_backwards_is_flagged() {
        let f = fixture();
        let mut session = session_after(&f, Duration::from_secs(5), chrono::Duration::minutes(-10));
        tick(&f, &mut session);

        assert_eq!(session.integrity_status, integrity::STATUS_SUSPICIOUS);
    }

    /// A session started at fixed clocks, ticked on clocks that move exactly
    /// one `POLL_INTERVAL` at a time.
    struct Driven {
        f: Fixture,
        session: ActiveSession,
        now: Instant,
        wall: DateTime<Utc>,
    }

    fn driven() -> Driven {
        let (db, game_id) = library_with("Checkpoint Game");
        let wall = parse_wall_timestamp("2026-10-02T18:00:00.000Z");
        let row = sessions::create_session_at(&db, &game_id, "device", wall).unwrap();
        let now = Instant::now();
        let session = ActiveSession::new(
            row.id.clone(),
            game_id.clone(),
            now,
            wall,
            integrity::STATUS_LOCAL.into(),
        );
        Driven {
            f: Fixture {
                db,
                game_id,
                session_id: row.id,
            },
            session,
            now,
            wall,
        }
    }

    impl Driven {
        /// One tick, `active` or idle, after the clocks moved by `wall_step`.
        fn tick(&mut self, active: bool, wall_step: chrono::Duration) {
            self.try_tick(active, wall_step).unwrap();
        }

        fn try_tick(
            &mut self,
            active: bool,
            wall_step: chrono::Duration,
        ) -> crate::error::Result<()> {
            self.now += POLL_INTERVAL;
            self.wall += wall_step;
            let idle_for = if active {
                Duration::ZERO
            } else {
                Duration::from_secs(600)
            };
            apply_observation(
                &self.f.db,
                &mut self.session,
                PLAYING,
                &settings(),
                &desktop(idle_for),
                &Clocks {
                    now: self.now,
                    wall: self.wall,
                    asleep_ms: None,
                },
            )
        }

        fn ticks(&mut self, count: usize, active: bool) {
            let step = chrono::Duration::from_std(POLL_INTERVAL).unwrap();
            for _ in 0..count {
                self.tick(active, step);
            }
        }

        /// The events of one type, oldest first.
        fn events(&self, event_type: &str) -> Vec<crate::db::models::SessionEvent> {
            let mut events: Vec<_> =
                session_events::list_events_for_game(&self.f.db, &self.f.game_id)
                    .unwrap()
                    .into_iter()
                    .filter(|event| event.event_type == event_type)
                    .collect();
            events.sort_by_key(|event| event.sequence);
            events
        }

        fn stored(&self) -> crate::db::models::Session {
            self.f
                .db
                .with_conn(|conn| {
                    sessions::stored_session(conn, &self.f.session_id)
                        .map_err(crate::db::repo::map_db)
                })
                .unwrap()
        }
    }

    fn counters(event: &crate::db::models::SessionEvent) -> (i64, i64, i64) {
        let payload: serde_json::Value = serde_json::from_str(&event.payload_json).unwrap();
        let field = |key: &str| payload[key].as_i64().unwrap();
        (field("runtime_ms"), field("active_ms"), field("idle_ms"))
    }

    #[test]
    fn steady_play_writes_a_checkpoint_every_interval() {
        let mut d = driven();
        let interval_ms = i64::try_from(CHECKPOINT_INTERVAL.as_millis()).unwrap();
        for _ in 0..60 {
            d.ticks(1, true);
            // The database never falls further behind than one interval.
            assert!(d.session.runtime_ms - d.stored().runtime_ms < interval_ms);
        }

        let checkpoints = d.events("heartbeat");
        assert_eq!(checkpoints.len(), 10);
        assert_eq!(counters(checkpoints.last().unwrap()), (300_000, 300_000, 0));
        // The open session still passes every integrity check.
        let open = sessions::get_active_sessions(&d.f.db, "device").unwrap();
        assert_eq!(open[0].integrity_status, integrity::STATUS_LOCAL);
    }

    #[test]
    fn a_switch_to_idle_closes_the_stretch_at_the_tick_before() {
        let mut d = driven();
        d.ticks(4, true);
        d.ticks(7, false);

        let checkpoints = d.events("heartbeat");
        assert_eq!(checkpoints.len(), 2);
        // Written when the first idle tick came, as of the last active one.
        assert_eq!(counters(&checkpoints[0]), (20_000, 20_000, 0));
        assert_eq!(checkpoints[0].event_time_wall, "2026-10-02T18:00:20.000Z");
        // The next stretch is idle only.
        assert_eq!(counters(&checkpoints[1]), (55_000, 20_000, 35_000));
    }

    #[test]
    fn a_failed_write_leaves_the_tick_for_the_next_one() {
        let mut d = driven();
        d.ticks(2, true);
        let before = (
            d.session.last_wall_at,
            d.session.last_tick_at,
            d.session.runtime_ms,
        );
        // A closed row takes no checkpoint, so storing the active stretch fails.
        d.f.db
            .with_conn(|conn| {
                conn.execute(
                    "UPDATE sessions SET ended_at_wall = '2026-10-02T19:00:00.000Z' WHERE id = ?1",
                    [&d.f.session_id],
                )
                .map_err(crate::db::repo::map_db)
            })
            .unwrap();

        let step = chrono::Duration::from_std(POLL_INTERVAL).unwrap();
        assert!(d.try_tick(false, step).is_err());
        assert_eq!(
            (
                d.session.last_wall_at,
                d.session.last_tick_at,
                d.session.runtime_ms
            ),
            before
        );
        // The same holds for a sleep, which must not lose its gap.
        assert!(d.try_tick(true, chrono::Duration::hours(2)).is_err());
        assert_eq!(d.session.skipped_wall_ms, 0);
        assert_eq!(d.session.last_wall_at, before.0);
    }

    #[test]
    fn a_gap_stores_the_time_before_it_first() {
        let mut d = driven();
        d.ticks(3, true);
        assert!(d.events("heartbeat").is_empty());
        d.tick(true, chrono::Duration::hours(2));

        let checkpoints = d.events("heartbeat");
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(counters(&checkpoints[0]), (15_000, 15_000, 0));
        let gap = &d.events("tracking_gap")[0];
        assert!(gap.sequence > checkpoints[0].sequence);
        assert_eq!(d.stored().runtime_ms, 15_000);
    }

    #[test]
    fn a_folder_counts_only_for_launchers_with_folders_of_their_own_and_below_a_drive() {
        let db = Database::open_in_memory().unwrap();
        let folder = std::env::temp_dir().to_string_lossy().into_owned();
        let drive = if cfg!(windows) { r"C:\" } else { "/" };
        let game = |launcher: Option<&str>, program: Option<&str>, folder: &str| {
            games::create_game(
                &db,
                &CreateGame {
                    title: "Folder Game".into(),
                    executable_path: program.map(Into::into),
                    install_folder: Some(folder.into()),
                    launcher_source: launcher.map(Into::into),
                },
            )
            .unwrap()
        };

        let steam = game(Some("steam"), Some("game.exe"), &folder);
        assert_eq!(folder_match(&steam), FolderMatch::Counts);
        let whole_drive = game(Some("steam"), Some("game.exe"), drive);
        assert_eq!(folder_match(&whole_drive), FolderMatch::Refused);
        let by_hand = game(None, Some("game.exe"), &folder);
        assert_eq!(folder_match(&by_hand), FolderMatch::Unused);
        let no_program = game(Some("steam"), None, &folder);
        assert_eq!(folder_match(&no_program), FolderMatch::Unused);
        let theirs = Game {
            origin_device_id: Some("laptop".into()),
            ..steam
        };
        assert_eq!(folder_match(&theirs), FolderMatch::Unused);
        // A game of this PC that came back with a merge is tracked too.
        let ours = Game {
            origin_device_id: Some(crate::integrity::ledger::this_device().unwrap()),
            ..theirs
        };
        assert_eq!(folder_match(&ours), FolderMatch::Counts);
    }
}
