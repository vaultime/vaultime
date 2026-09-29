// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Polls running processes and turns them into game sessions.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use sysinfo::System;

use crate::constants::{
    BACKGROUND_ACTIVE_SETTING, CLOCK_BACKWARDS_TOLERANCE_MS, CLOCK_STEP_TOLERANCE_MS,
    CLOCK_TOTAL_DRIFT_TOLERANCE_MS, DEFAULT_IDLE_THRESHOLD_SECS, FOREGROUND_GRACE,
    IDLE_THRESHOLD_SETTING, INSTALL_FOLDER_REFRESH, MAX_TICK_GAP_MS, MIN_IDLE_THRESHOLD_SECS,
    POLL_INTERVAL, PROCESS_ACTIVITY_CPU_PERCENT, SUSPEND_DETECT_MS,
};
use crate::db::connection::Database;
use crate::db::models::Game;
use crate::db::repo::{games, sessions, settings};
use crate::discovery::metadata::is_likely_game_executable;
use crate::integrity;
use crate::platform;
use crate::platform::activity::{ActivitySnapshot, capture_activity_snapshot};
use crate::platform::process::{
    InstallFolder, RunningProcess, cpu_usage, matches_executable, refresh_running_processes,
};

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
    /// The clock that counts through sleep, at the last tick.
    last_boot_ms: Option<i64>,
    integrity_status: String,
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
            last_boot_ms: platform::clock::since_boot_ms(),
            integrity_status,
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
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl TrackingEngine {
    /// Spawns the tracker on a background thread.
    pub fn start(db: Arc<Database>, device_id: String) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let paused = Arc::new(AtomicBool::new(false));
        let tick_lock = Arc::new(Mutex::new(()));

        let thread = {
            let running = Arc::clone(&running);
            let paused = Arc::clone(&paused);
            let tick_lock = Arc::clone(&tick_lock);
            std::thread::Builder::new()
                .name("vaultime-tracker".into())
                .spawn(move || poll_loop(&db, &device_id, &running, &paused, &tick_lock))
                .expect("failed to spawn tracking thread")
        };

        info!("tracking engine started");
        Self {
            running,
            paused,
            tick_lock,
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

    /// True while the tracker thread runs and is not paused.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst) && !self.paused.load(Ordering::SeqCst)
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
    running: &AtomicBool,
    paused: &AtomicBool,
    tick_lock: &Mutex<()>,
) {
    let mut active: HashMap<String, ActiveSession> = HashMap::new();
    let mut system = System::new();
    let mut folders = FolderCache::default();

    close_orphaned_sessions(db);

    while running.load(Ordering::SeqCst) {
        if !paused.load(Ordering::SeqCst) {
            let _tick = tick_lock.lock().unwrap_or_else(PoisonError::into_inner);
            // Checked again because `pause` may have won the race for the lock.
            if !paused.load(Ordering::SeqCst)
                && let Err(error) = poll_tick(db, device_id, &mut system, &mut folders, &mut active)
            {
                error!("tracking poll error: {error}");
            }
        }

        wait_for_next_tick(running);
    }

    for session in active.into_values() {
        if let Err(error) = sessions::end_session(
            db,
            &session.session_id,
            session.runtime_ms,
            session.active_ms,
            session.idle_ms,
            &session.integrity_status,
        ) {
            error!(
                "failed to close session {} on shutdown: {error}",
                session.session_id
            );
        }
    }

    info!("tracking engine stopped");
}

fn poll_tick(
    db: &Database,
    device_id: &str,
    system: &mut System,
    folders: &mut FolderCache,
    active: &mut HashMap<String, ActiveSession>,
) -> crate::error::Result<()> {
    let tracking_settings = TrackingSettings::load(db);
    let tracked_games = games::list_all_games(db)?;
    let processes = refresh_running_processes(system);
    let activity_snapshot = capture_activity_snapshot();

    folders.expire();
    let observed_games = observe_games(
        &tracked_games,
        &processes,
        &activity_snapshot,
        system,
        folders,
    );

    for (game_id, observation) in &observed_games {
        if observation.is_running && !active.contains_key(game_id) {
            match sessions::create_session(db, game_id, device_id) {
                Ok(session) => {
                    info!("session started for game {game_id}: {}", session.id);
                    let started = Instant::now();
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
                    active.insert(game_id.clone(), active_session);
                }
                Err(error) => {
                    error!("failed to start session for game {game_id}: {error}");
                }
            }
        }
    }

    // All clocks are read together after the work above, which can wait on
    // the database while a backup runs. Reading them apart would look like a
    // clock change.
    let clocks = Clocks {
        now: Instant::now(),
        wall: Utc::now(),
        boot_ms: platform::clock::since_boot_ms(),
    };
    for session in active.values_mut() {
        if let Some(observation) = observed_games.get(&session.game_id) {
            if !observation.is_running {
                continue;
            }

            apply_observation(
                db,
                session,
                *observation,
                &tracking_settings,
                &activity_snapshot,
                &clocks,
            )?;
        }
    }

    let finished: Vec<String> = active
        .keys()
        .filter(|game_id| {
            !observed_games
                .get(*game_id)
                .is_some_and(|observation| observation.is_running)
        })
        .cloned()
        .collect();

    for game_id in finished {
        if let Some(session) = active.remove(&game_id) {
            match sessions::end_session(
                db,
                &session.session_id,
                session.runtime_ms,
                session.active_ms,
                session.idle_ms,
                &session.integrity_status,
            ) {
                Ok(ended) => {
                    info!(
                        "session ended for game {}: {} (runtime={}ms active={}ms idle={}ms)",
                        game_id, ended.id, session.runtime_ms, session.active_ms, session.idle_ms
                    );
                }
                Err(error) => {
                    error!(
                        "failed to end session {} for game {}: {error}",
                        session.session_id, game_id
                    );
                }
            }
        }
    }

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

/// What each tracked game with an executable is doing right now.
fn observe_games(
    games: &[Game],
    processes: &[RunningProcess],
    activity_snapshot: &ActivitySnapshot,
    system: &mut System,
    folders: &mut FolderCache,
) -> HashMap<String, GameObservation> {
    let matched: Vec<(&str, Vec<&RunningProcess>)> = games
        .iter()
        .filter_map(|game| Some((game.id.as_str(), game_processes(game, processes, folders)?)))
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
        cpu_usage(system, &pids)
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

/// The running processes of a game, or `None` for a game without an
/// executable.
fn game_processes<'a>(
    game: &Game,
    processes: &'a [RunningProcess],
    folders: &mut FolderCache,
) -> Option<Vec<&'a RunningProcess>> {
    let exe_path = game.executable_path.as_deref()?;
    // Launchers give every game a folder of its own, so any game program
    // in it counts, whichever build or launcher step is running.
    let own_folder = if matches!(
        game.launcher_source.as_deref(),
        Some(
            "steam"
                | "epic"
                | "gog"
                | "heroic"
                | "battlenet"
                | "riot"
                | "hoyoplay"
                | "ubisoft"
                | "ea"
                | "rockstar"
                | "xbox"
                | "amazon"
                | "itch"
        )
    ) {
        game.install_folder
            .as_deref()
            .map(|folder| folders.get(folder))
    } else {
        None
    };
    Some(
        processes
            .iter()
            .filter(|process| {
                matches_executable(process, exe_path)
                    || own_folder.as_ref().is_some_and(|folder| {
                        folder.contains(process) && is_likely_game_executable(&process.name)
                    })
            })
            .collect(),
    )
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
        self.folders
            .entry(folder.to_owned())
            .or_insert_with(|| InstallFolder::new(folder))
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
            matched_processes.iter().any(|process| {
                process.pid == foreground_pid
                    || platform::process::inner_pid(process.pid) == Some(foreground_pid)
            })
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
    /// Counts through sleep, unlike `now`.
    boot_ms: Option<i64>,
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

    let delta_ms = now
        .saturating_duration_since(session.last_tick_at)
        .as_millis() as i64;

    if delta_ms <= 0 {
        return Ok(());
    }

    session.last_tick_at = now;
    let wall_delta_ms = current_wall
        .signed_duration_since(session.last_wall_at)
        .num_milliseconds();
    session.last_wall_at = current_wall;
    // The monotonic clock stops while the PC sleeps on some systems. Time the
    // clock that counts through sleep has on top of it was spent asleep.
    let slept_ms = clocks
        .boot_ms
        .zip(session.last_boot_ms)
        .map_or(0, |(boot_now, boot_before)| {
            boot_now - boot_before - delta_ms
        });
    session.last_boot_ms = clocks.boot_ms;

    if delta_ms.max(wall_delta_ms) > MAX_TICK_GAP_MS || slept_ms > SUSPEND_DETECT_MS {
        session.skipped_wall_ms += wall_delta_ms.max(0);
        return sessions::record_tracking_gap(
            db,
            &session.session_id,
            session.runtime_ms,
            wall_delta_ms,
            delta_ms,
        );
    }

    session.runtime_ms += delta_ms;

    if should_count_as_active(
        session,
        observation,
        tracking_settings,
        activity_snapshot,
        now,
    ) {
        session.active_ms += delta_ms;
    } else {
        session.idle_ms += delta_ms;
    }

    let wall_elapsed_ms = current_wall
        .signed_duration_since(session.started_at_wall)
        .num_milliseconds()
        .max(0);
    let drift_ms = wall_elapsed_ms - session.skipped_wall_ms - session.runtime_ms;

    if let Some(reason) = detect_integrity_reason(wall_delta_ms, delta_ms, drift_ms)
        && session.integrity_status != integrity::STATUS_SUSPICIOUS
    {
        sessions::flag_session_suspicious(
            db,
            &session.session_id,
            session.runtime_ms,
            wall_elapsed_ms,
            drift_ms,
            &reason,
        )?;
        session.integrity_status = integrity::STATUS_SUSPICIOUS.into();
    }

    sessions::update_session_timing(
        db,
        &session.session_id,
        session.runtime_ms,
        session.active_ms,
        session.idle_ms,
        wall_elapsed_ms,
        drift_ms,
        &session.integrity_status,
    )
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
) -> Option<String> {
    if wall_delta_ms < -CLOCK_BACKWARDS_TOLERANCE_MS {
        return Some("wall_clock_moved_backwards".into());
    }

    if (wall_delta_ms - monotonic_delta_ms).abs() > CLOCK_STEP_TOLERANCE_MS {
        return Some("wall_clock_step_mismatch".into());
    }

    if drift_ms.abs() > CLOCK_TOTAL_DRIFT_TOLERANCE_MS {
        return Some("wall_clock_drift_exceeded".into());
    }

    None
}

fn parse_wall_timestamp(value: &str) -> DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map_or_else(|_| Utc::now(), |timestamp| timestamp.with_timezone(&Utc))
}

/// Closes any sessions left open from a previous run, for example after a crash.
fn close_orphaned_sessions(db: &Database) {
    match sessions::get_active_sessions(db) {
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
        let engine = TrackingEngine::start(Arc::clone(&db), "device".into());

        let mut child = std::process::Command::new(&exe)
            .current_dir(std::path::Path::new(&exe).parent().unwrap())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        while sessions::get_active_sessions(&db).unwrap().is_empty() {
            assert!(Instant::now() < deadline, "the game was not seen");
            std::thread::sleep(Duration::from_millis(250));
        }
        println!("session opened");
        std::thread::sleep(Duration::from_secs(15));

        // End every process of the game, it may have restarted itself through Steam.
        let _ = child.kill();
        let _ = child.wait();
        let mut system = System::new();
        for process in refresh_running_processes(&mut system) {
            if crate::platform::process::runs_from_folder(&process, &folder)
                && let Some(running) = system.process(sysinfo::Pid::from_u32(process.pid))
            {
                running.kill();
            }
        }
        let deadline = Instant::now() + POLL_INTERVAL * 3;
        while !sessions::get_active_sessions(&db).unwrap().is_empty() {
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

        let engine = TrackingEngine::start(Arc::clone(&db), "device".into());
        let deadline = Instant::now() + POLL_INTERVAL;
        while sessions::get_active_sessions(&db).unwrap().is_empty() {
            assert!(Instant::now() < deadline, "no session started");
            std::thread::sleep(Duration::from_millis(20));
        }

        let started = Instant::now();
        engine.shutdown();
        assert!(started.elapsed() < POLL_INTERVAL);
        assert!(sessions::get_active_sessions(&db).unwrap().is_empty());
        let closed = &sessions::list_sessions_for_game(&db, &game.id).unwrap()[0];
        assert!(closed.closed_cleanly);
        assert_eq!(closed.integrity_status, "local");
    }
    use crate::db::models::CreateGame;
    use crate::db::repo::{devices, session_events};

    struct Fixture {
        db: Database,
        game_id: String,
        session_id: String,
    }

    fn fixture() -> Fixture {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "device", "test", "0.1.0").unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Gap Game".into(),
                executable_path: Some("game.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        let session = sessions::create_session(&db, &game.id, "device").unwrap();
        Fixture {
            db,
            game_id: game.id,
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

    fn tick_at(f: &Fixture, session: &mut ActiveSession, boot_ms: Option<i64>) {
        let observation = GameObservation {
            is_running: true,
            has_foreground_window: true,
            has_process_activity: true,
        };
        let snapshot = ActivitySnapshot {
            foreground_pid: None,
            foreground_supported: true,
            idle_for: Some(Duration::ZERO),
            idle_supported: true,
        };
        let settings = TrackingSettings {
            idle_threshold: Duration::from_secs(300),
            treat_background_as_active: false,
        };
        apply_observation(
            &f.db,
            session,
            observation,
            &settings,
            &snapshot,
            &Clocks {
                now: Instant::now(),
                wall: Utc::now(),
                boot_ms,
            },
        )
        .unwrap();
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
        let f = fixture();
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
        let f = fixture();
        // Both clocks kept running through sleep.
        let mut session = session_after(&f, Duration::from_secs(3_600), chrono::Duration::hours(1));
        tick(&f, &mut session);

        assert_eq!(session.runtime_ms, 0);
        assert_eq!(session.integrity_status, integrity::STATUS_LOCAL);
    }

    #[test]
    fn a_short_suspend_is_skipped_not_flagged() {
        let f = fixture();
        // Half a minute asleep: too short for the gap limit, but the clock
        // that counts through sleep ran ahead of the monotonic one.
        let mut session = session_after(&f, Duration::from_secs(5), chrono::Duration::seconds(35));
        session.last_boot_ms = Some(1_000);
        tick_at(&f, &mut session, Some(1_000 + 35_000));

        assert_eq!(session.runtime_ms, 0);
        assert_eq!(session.integrity_status, integrity::STATUS_LOCAL);
    }

    #[test]
    fn clock_moved_backwards_is_flagged() {
        let f = fixture();
        let mut session = session_after(&f, Duration::from_secs(5), chrono::Duration::minutes(-10));
        tick(&f, &mut session);

        assert_eq!(session.integrity_status, integrity::STATUS_SUSPICIOUS);
    }
}
