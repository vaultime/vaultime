// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tracking engine — polls running processes and manages game sessions.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};

use crate::db::connection::Database;
use crate::db::repo::{games, sessions};
use crate::platform::process::{list_running_processes, matches_executable};

/// Default polling interval for process detection.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Tracks a currently running game session.
struct ActiveSession {
    session_id: String,
    #[allow(dead_code)]
    game_id: String,
    started_at: Instant,
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
    // Maps game_id → active session info.
    let mut active: HashMap<String, ActiveSession> = HashMap::new();

    // Recover any sessions left open from a previous crash.
    close_orphaned_sessions(db);

    while running.load(Ordering::Relaxed) {
        if let Err(e) = poll_tick(db, device_id, &mut active) {
            error!("tracking poll error: {e}");
        }

        std::thread::sleep(DEFAULT_POLL_INTERVAL);
    }

    // Clean shutdown — close all active sessions.
    for (_game_id, session) in active.drain() {
        let runtime = session.started_at.elapsed().as_millis() as i64;
        if let Err(e) = sessions::end_session(db, &session.session_id, runtime) {
            error!("failed to close session {} on shutdown: {e}", session.session_id);
        }
    }

    info!("tracking engine stopped");
}

/// A single tick of the poll loop.
fn poll_tick(
    db: &Database,
    device_id: &str,
    active: &mut HashMap<String, ActiveSession>,
) -> crate::error::Result<()> {
    let tracked_games = games::list_all_games(db)?;
    let processes = list_running_processes();

    // Determine which tracked games have a matching running process.
    let mut running_game_ids: HashMap<String, bool> = HashMap::new();
    for game in &tracked_games {
        if let Some(ref exe_path) = game.executable_path {
            let is_running = processes
                .iter()
                .any(|p| matches_executable(p, exe_path));
            running_game_ids.insert(game.id.clone(), is_running);
        }
    }

    // Start sessions for newly detected games.
    for (game_id, &is_running) in &running_game_ids {
        if is_running && !active.contains_key(game_id) {
            match sessions::create_session(db, game_id, device_id) {
                Ok(session) => {
                    info!("session started for game {game_id}: {}", session.id);
                    active.insert(
                        game_id.clone(),
                        ActiveSession {
                            session_id: session.id,
                            game_id: game_id.clone(),
                            started_at: Instant::now(),
                        },
                    );
                }
                Err(e) => {
                    error!("failed to start session for game {game_id}: {e}");
                }
            }
        }
    }

    // End sessions for games that are no longer running.
    let finished: Vec<String> = active
        .keys()
        .filter(|gid| !running_game_ids.get(*gid).copied().unwrap_or(false))
        .cloned()
        .collect();

    for game_id in finished {
        if let Some(session) = active.remove(&game_id) {
            let runtime = session.started_at.elapsed().as_millis() as i64;
            match sessions::end_session(db, &session.session_id, runtime) {
                Ok(ended) => {
                    info!(
                        "session ended for game {}: {} ({}ms)",
                        game_id, ended.id, runtime
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
        running_game_ids.values().filter(|&&v| v).count(),
        active.len(),
    );

    Ok(())
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
                if let Err(e) = db.with_conn(|conn| {
                    conn.execute(
                        "UPDATE sessions
                         SET ended_at_wall = datetime('now'),
                             integrity_status = 'recovered',
                             closed_cleanly = 0
                         WHERE id = ?1",
                        [&session.id],
                    )
                    .map_err(|e| crate::error::VaultimeError::Database(format!("{e}")))?;
                    Ok(())
                }) {
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
