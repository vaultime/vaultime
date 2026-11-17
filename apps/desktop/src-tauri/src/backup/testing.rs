// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! A PC for tests of backups that go from one PC to another.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeDelta, Utc};
use ed25519_dalek::SigningKey;
use rusqlite::params;
use serde_json::json;

use super::export_local_backup;
use super::merge::{self, GameChoice, MergePreview, MergeSummary};
use crate::AppContext;
use crate::assets::AssetManager;
use crate::constants::{ASSET_CACHE_DIR, DATABASE_FILE};
use crate::db::connection::Database;
use crate::db::models::CreateGame;
use crate::db::repo::sessions::{self, Checkpoint};
use crate::db::repo::{devices, games, map_db};
use crate::error::Result;
use crate::integrity::{self, STATUS_LOCAL, ledger};
use crate::playtime::slices;

pub(crate) const MINUTE: i64 = 60_000;

/// One PC with a library, a ledger and a key of its own.
pub(crate) struct Pc {
    pub(crate) context: AppContext,
    pub(crate) db: Database,
    pub(crate) assets: AssetManager,
    pub(crate) me: (SigningKey, String),
}

impl Pc {
    pub(crate) fn new(device: &str) -> Self {
        let app_dir =
            std::env::temp_dir().join(format!("vaultime-merge-{device}-{}", uuid::Uuid::new_v4()));
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
        devices::describe_this_device(&pc.db, device, &ledger::key_id().unwrap(), device).unwrap();
        pc.db
            .with_transaction(|conn| ledger::begin_if_empty(conn, "first_start"))
            .unwrap();
        pc
    }

    pub(crate) fn act(&self) {
        ledger::act_as(Some(&self.me));
    }

    pub(crate) fn game(&self, title: &str, steam: Option<&str>) -> String {
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
    pub(crate) fn play(&self, game: &str, minutes: i64) -> String {
        self.act();
        let session = sessions::create_session(&self.db, game, &self.context.device_id).unwrap();
        let half = minutes * MINUTE / 2;
        let checkpoint = Checkpoint {
            runtime_ms: half,
            active_ms: half,
            wall_elapsed_ms: half,
            ..Checkpoint::default()
        };
        sessions::record_checkpoint(&self.db, &session.id, &checkpoint, STATUS_LOCAL, Utc::now())
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
    pub(crate) fn play_at(&self, game: &str, start: &str, minutes: i64) -> String {
        self.act();
        let started = DateTime::parse_from_rfc3339(start)
            .unwrap()
            .with_timezone(&Utc);
        let session =
            sessions::create_session_at(&self.db, game, &self.context.device_id, started).unwrap();
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

    pub(crate) fn back_up(&self) -> PathBuf {
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

    pub(crate) fn preview(&self, backup: &Path) -> Result<MergePreview> {
        self.act();
        merge::preview(&self.db, &self.context, backup, false)
    }

    pub(crate) fn merge(&self, backup: &Path, choices: &[GameChoice]) -> MergeSummary {
        self.act();
        merge::apply(
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
    pub(crate) fn status(&self, session_id: &str) -> String {
        self.act();
        integrity::forget_checks();
        sessions::list_all_sessions(&self.db)
            .unwrap()
            .into_iter()
            .find(|session| session.id == session_id)
            .map_or_else(|| "missing".into(), |session| session.integrity_status)
    }

    pub(crate) fn events(&self, session_id: &str) -> Vec<(i64, String, String, Option<String>)> {
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

    pub(crate) fn adopted(&self, session_id: &str) -> bool {
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
