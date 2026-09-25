// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tracking engine — polls running processes and manages game sessions.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use sysinfo::System;

use crate::db::connection::Database;
use crate::db::repo::{games, sessions, settings};
use crate::integrity;
use crate::platform::activity::{ActivitySnapshot, capture_activity_snapshot};
use crate::platform::process::{RunningProcess, matches_executable, refresh_running_processes};

/// Default polling interval for process detection.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Short grace period for brief alt-tab transitions.
const FOREGROUND_GRACE: Duration = Duration::from_secs(15);
/// Minimum CPU usage considered meaningful activity for a process.
const PROCESS_ACTIVITY_CPU_THRESHOLD: f32 = 0.5;
/// Wall-clock step difference tolerated between ticks before a session is suspicious.
const CLOCK_STEP_TOLERANCE_MS: i64 = 20_000;
/// Total wall-clock vs monotonic drift tolerated across the whole session.
const CLOCK_TOTAL_DRIFT_TOLERANCE_MS: i64 = 45_000;

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
            .unwrap_or(300)
            .max(5);

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

/// The tracking engine state, shared between the polling thread and commands.
pub struct TrackingEngine {
    running: Arc<AtomicBool>,
}

impl TrackingEngine {
    /// Spawns the tracking engine on a background thread.
    pub fn start(db: Arc<Database>, device_id: String) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_flag = running.clone();

        std::thread::Builder::new()
            .name("vaultime-tracker".into())
            .spawn(move || {
                poll_loop(&db, &device_id, &running_flag);
            })
            .expect("failed to spawn tracking thread");

        info!("tracking engine started");

        Self { running }
    }

    /// Signals the engine to stop at its next poll cycle.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        info!("tracking engine stop requested");
    }

    /// Returns whether the engine is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Drop for TrackingEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Main polling loop that runs on the background thread.
fn poll_loop(db: &Database, device_id: &str, running: &AtomicBool) {
    let mut active: HashMap<String, ActiveSession> = HashMap::new();
    let mut system = System::new_all();

    close_orphaned_sessions(db);

    while running.load(Ordering::Relaxed) {
        if let Err(e) = poll_tick(db, device_id, &mut system, &mut active) {
            error!("tracking poll error: {e}");
        }

        std::thread::sleep(DEFAULT_POLL_INTERVAL);
    }

    if let Err(e) = poll_tick(db, device_id, &mut system, &mut active) {
        error!("tracking final poll error: {e}");
    }

    for (_game_id, session) in active.drain() {
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
    let drift_ms = wall_elapsed_ms - session.runtime_ms;

    if let Some(reason) = detect_integrity_reason(wall_delta_ms, delta_ms, drift_ms) {
        if session.integrity_status != integrity::STATUS_SUSPICIOUS {
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
        if let Some(idle_for) = activity_snapshot.idle_for {
            if idle_for >= tracking_settings.idle_threshold {
                return false;
            }
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
    if wall_delta_ms < -1_000 {
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
