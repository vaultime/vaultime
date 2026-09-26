// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

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
    CLOCK_BACKWARDS_TOLERANCE_MS, CLOCK_STEP_TOLERANCE_MS, CLOCK_TOTAL_DRIFT_TOLERANCE_MS,
    DEFAULT_IDLE_THRESHOLD_SECS, FOREGROUND_GRACE, MAX_TICK_GAP_MS, MIN_IDLE_THRESHOLD_SECS,
    POLL_INTERVAL, PROCESS_ACTIVITY_CPU_THRESHOLD,
};
use crate::db::connection::Database;
use crate::db::repo::{games, sessions, settings};
use crate::integrity;
use crate::platform::activity::{ActivitySnapshot, capture_activity_snapshot};
use crate::platform::process::{RunningProcess, matches_executable, refresh_running_processes};

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
        let idle_threshold_seconds = settings::get_setting(db, "idle_threshold_seconds")
            .ok()
            .flatten()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_IDLE_THRESHOLD_SECS)
            .max(MIN_IDLE_THRESHOLD_SECS);

        let treat_background_as_active = settings::get_setting(db, "treat_background_as_active")
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
    let mut system = System::new_all();

    close_orphaned_sessions(db);

    while running.load(Ordering::SeqCst) {
        if !paused.load(Ordering::SeqCst) {
            let _tick = tick_lock.lock().unwrap_or_else(PoisonError::into_inner);
            // Checked again because `pause` may have won the race for the lock.
            if !paused.load(Ordering::SeqCst)
                && let Err(e) = poll_tick(db, device_id, &mut system, &mut active)
            {
                error!("tracking poll error: {e}");
            }
        }

        wait_for_next_tick(running);
    }

    for session in active.into_values() {
        if let Err(e) = sessions::end_session(
            db,
            &session.session_id,
            session.runtime_ms,
            session.active_ms,
            session.idle_ms,
            &session.integrity_status,
        ) {
            error!(
                "failed to close session {} on shutdown: {e}",
                session.session_id
            );
        }
    }

    info!("tracking engine stopped");
}

/// A single tick of the poll loop.
fn poll_tick(
    db: &Database,
    device_id: &str,
    system: &mut System,
    active: &mut HashMap<String, ActiveSession>,
) -> crate::error::Result<()> {
    let now = Instant::now();
    let tracking_settings = TrackingSettings::load(db);
    let tracked_games = games::list_all_games(db)?;
    let processes = refresh_running_processes(system);
    let activity_snapshot = capture_activity_snapshot();

    let mut observed_games: HashMap<String, GameObservation> = HashMap::new();
    for game in &tracked_games {
        if let Some(ref exe_path) = game.executable_path {
            observed_games.insert(
                game.id.clone(),
                observe_game_processes(&processes, exe_path, &activity_snapshot),
            );
        }
    }

    for (game_id, observation) in &observed_games {
        if observation.is_running && !active.contains_key(game_id) {
            match sessions::create_session(db, game_id, device_id) {
                Ok(session) => {
                    info!("session started for game {game_id}: {}", session.id);
                    let mut active_session = ActiveSession::new(
                        session.id,
                        game_id.clone(),
                        now,
                        parse_wall_timestamp(&session.started_at_wall),
                        session.integrity_status.clone(),
                    );
                    if observation.has_foreground_window {
                        active_session.last_foreground_at = Some(now);
                    }
                    active.insert(game_id.clone(), active_session);
                }
                Err(e) => {
                    error!("failed to start session for game {game_id}: {e}");
                }
            }
        }
    }

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
                now,
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
                Err(e) => {
                    error!(
                        "failed to end session {} for game {}: {e}",
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

fn observe_game_processes(
    processes: &[RunningProcess],
    executable_path: &str,
    activity_snapshot: &ActivitySnapshot,
) -> GameObservation {
    let matched_processes: Vec<&RunningProcess> = processes
        .iter()
        .filter(|process| matches_executable(process, executable_path))
        .collect();

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

    let has_process_activity = matched_processes
        .iter()
        .any(|process| process.cpu_usage >= PROCESS_ACTIVITY_CPU_THRESHOLD);

    GameObservation {
        is_running: true,
        has_foreground_window,
        has_process_activity,
    }
}

fn apply_observation(
    db: &Database,
    session: &mut ActiveSession,
    observation: GameObservation,
    tracking_settings: &TrackingSettings,
    activity_snapshot: &ActivitySnapshot,
    now: Instant,
) -> crate::error::Result<()> {
    let current_wall = Utc::now();
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

    if delta_ms.max(wall_delta_ms) > MAX_TICK_GAP_MS {
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

/// Closes any sessions left open from a previous run (crash recovery).
fn close_orphaned_sessions(db: &Database) {
    match sessions::get_active_sessions(db) {
        Ok(orphans) if !orphans.is_empty() => {
            warn!(
                "found {} orphaned session(s) from previous run, closing",
                orphans.len()
            );
            for session in orphans {
                if let Err(e) = sessions::recover_session(db, &session.id, "startup_orphan_cleanup")
                {
                    error!("failed to close orphaned session {}: {e}", session.id);
                }
            }
        }
        Ok(_) => {}
        Err(e) => {
            error!("failed to check for orphaned sessions: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            Instant::now(),
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
    fn clock_moved_backwards_is_flagged() {
        let f = fixture();
        let mut session = session_after(&f, Duration::from_secs(5), chrono::Duration::minutes(-10));
        tick(&f, &mut session);

        assert_eq!(session.integrity_status, integrity::STATUS_SUSPICIOUS);
    }
}
