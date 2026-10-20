// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Merging the backups other PCs keep in the cloud. The first merge of a PC
//! goes through the player, with the game choices and the key check. After
//! that the newest backup of a PC merged before comes in by itself once a
//! day. One that brings keys the player has to vouch for, or sessions that
//! fail their check, waits for the player instead. A new game then links
//! only to a game of the same launcher with the same launcher id.
//!
//! The keys a first merge takes at their word are the ones the backup's PC
//! registered with the server, see `merge::preview_from_cloud`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use log::{info, warn};
use serde::Serialize;

use super::merge::{self, GameChoice, MergePreview, MergeSummary};
use super::remote::{self, CLOUD_STAGING_DIR, PC_NAME_FIELD, RemoteBackupRecord, SOURCE_PC_FIELD};
use super::{cleanup_staging_dir, crypto, load_and_validate_manifest};
use crate::AppContext;
use crate::assets::AssetManager;
use crate::constants::BACKUP_KEY_BYTES;
use crate::db::connection::Database;
use crate::db::models::Device;
use crate::db::repo::devices;
use crate::error::{Result, VaultimeError};
use crate::integrity::ledger;
use crate::secure_storage;

/// The launcher match that links a new game of an automatic merge.
const SAME_LAUNCHER: &str = "launcher";

/// The newest cloud backup of another PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CloudPc {
    pub device_id: String,
    /// Its name from the backup, sealed with the backup key, or as this PC
    /// knows it.
    pub name: Option<String>,
    pub backup_id: String,
    pub backup_created_at: String,
    pub sessions_count: i64,
    /// Whether this PC merged it before, so its newer backups come in by
    /// themselves.
    pub merged_before: bool,
    /// Whether the backup is newer than the last one merged here.
    pub newer: bool,
}

/// What an automatic merge did with a PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutoMerge {
    pub device_id: String,
    pub name: Option<String>,
    pub backup_id: String,
    /// `merged`, or why it waits for the player: `new_key` or `failing`.
    pub outcome: &'static str,
    pub sessions_added: usize,
    pub sessions_grown: usize,
}

/// The newest cloud backup of every other PC of the account.
pub fn list_cloud_pcs(
    db: &Database,
    api_base_url: &str,
    access_token: &str,
    account_id: &str,
) -> Result<Vec<CloudPc>> {
    let key = secure_storage::load_cloud_backup_key(account_id)?;
    let backups = remote::list_backups(api_base_url, access_token)?;
    cloud_pcs(db, &backups, &key)
}

/// `list_cloud_pcs` from the backup records.
fn cloud_pcs(
    db: &Database,
    backups: &[RemoteBackupRecord],
    key: &[u8; BACKUP_KEY_BYTES],
) -> Result<Vec<CloudPc>> {
    let this = ledger::this_device()?;
    let merged = merge::last_merged(db);
    let known: HashMap<String, Device> = devices::list_devices(db)?
        .into_iter()
        .map(|device| (device.id.clone(), device))
        .collect();
    let mut newest: HashMap<&str, &RemoteBackupRecord> = HashMap::new();
    for backup in backups.iter().filter(|backup| backup.status == "complete") {
        let Some(device) = metadata_text(backup, SOURCE_PC_FIELD) else {
            continue;
        };
        if device == this {
            continue;
        }
        let entry = newest.entry(device).or_insert(backup);
        if instant(&backup.uploaded_at) > instant(&entry.uploaded_at) {
            *entry = backup;
        }
    }
    let mut pcs: Vec<CloudPc> = newest
        .into_iter()
        .map(|(device, backup)| {
            let here = known.get(device);
            let last = merged.get(device).and_then(|created| instant(created));
            let merged_at = here
                .and_then(|device| device.merged_at.as_deref())
                .and_then(instant);
            CloudPc {
                device_id: device.to_owned(),
                name: metadata_text(backup, PC_NAME_FIELD)
                    .and_then(|sealed| crypto::open_text(sealed, key))
                    .or_else(|| here.and_then(|device| device.name.clone())),
                backup_id: backup.id.clone(),
                backup_created_at: backup.backup_created_at.clone(),
                sessions_count: backup
                    .metadata_json
                    .as_ref()
                    .and_then(|metadata| metadata.get("sessions_count")?.as_i64())
                    .unwrap_or_default(),
                merged_before: last.is_some() || merged_at.is_some(),
                newer: match (last, merged_at) {
                    (Some(last), _) => instant(&backup.backup_created_at) > Some(last),
                    (None, Some(merged_at)) => instant(&backup.uploaded_at) > Some(merged_at),
                    (None, None) => true,
                },
            }
        })
        .collect();
    pcs.sort_by(|a, b| {
        (a.name.as_deref().unwrap_or(&a.device_id)).cmp(b.name.as_deref().unwrap_or(&b.device_id))
    });
    Ok(pcs)
}

/// Where the account's backups are and the key that opens them.
pub(crate) struct Cloud<'a> {
    pub(crate) api_base_url: &'a str,
    pub(crate) access_token: &'a str,
    pub(crate) key: &'a [u8; BACKUP_KEY_BYTES],
}

/// What merging the cloud backup `backup_id` would bring in. The backup
/// stays downloaded until it is merged or discarded.
pub fn preview_cloud_merge(
    db: &Database,
    app_context: &AppContext,
    api_base_url: &str,
    access_token: &str,
    account_id: &str,
    backup_id: &str,
    trust_new_keys: bool,
) -> Result<MergePreview> {
    let key = secure_storage::load_cloud_backup_key(account_id)?;
    let cloud = Cloud {
        api_base_url,
        access_token,
        key: &key,
    };
    preview_with(db, app_context, &cloud, backup_id, trust_new_keys)
}

fn preview_with(
    db: &Database,
    app_context: &AppContext,
    cloud: &Cloud,
    backup_id: &str,
    trust_new_keys: bool,
) -> Result<MergePreview> {
    let (folder, source) = staged(app_context, cloud, backup_id)?;
    let registered = registered_keys_of(cloud, &source)?;
    merge::preview_from_cloud(db, app_context, &folder, trust_new_keys, &registered)
}

/// Merges the cloud backup `backup_id` with the player's choices, as
/// `merge::apply` does, and lets the downloaded copy go.
#[expect(clippy::too_many_arguments)]
pub fn merge_cloud_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    api_base_url: &str,
    access_token: &str,
    account_id: &str,
    backup_id: &str,
    choices: &[GameChoice],
    trust_new_keys: bool,
) -> Result<MergeSummary> {
    let key = secure_storage::load_cloud_backup_key(account_id)?;
    let cloud = Cloud {
        api_base_url,
        access_token,
        key: &key,
    };
    merge_with(
        db,
        asset_manager,
        app_context,
        &cloud,
        backup_id,
        choices,
        trust_new_keys,
    )
}

fn merge_with(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    cloud: &Cloud,
    backup_id: &str,
    choices: &[GameChoice],
    trust_new_keys: bool,
) -> Result<MergeSummary> {
    let (folder, source) = staged(app_context, cloud, backup_id)?;
    let registered = registered_keys_of(cloud, &source)?;
    let summary = merge::apply_from_cloud(
        db,
        asset_manager,
        app_context,
        &folder,
        choices,
        trust_new_keys,
        &registered,
    );
    discard_cloud_merge(app_context, backup_id);
    summary
}

/// Lets the downloaded copy of the cloud backup `backup_id` go.
pub fn discard_cloud_merge(app_context: &AppContext, backup_id: &str) {
    if let Ok(folder) = staging_folder(app_context, backup_id)
        && folder.exists()
    {
        cleanup_staging_dir(&folder);
    }
}

/// Merges the newest backup of every PC merged here before that has a newer
/// one, as far as it needs no choice of the player.
pub fn merge_due_cloud_backups(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    api_base_url: &str,
    access_token: &str,
    account_id: &str,
) -> Result<Vec<AutoMerge>> {
    let key = secure_storage::load_cloud_backup_key(account_id)?;
    let cloud = Cloud {
        api_base_url,
        access_token,
        key: &key,
    };
    merge_due_with(db, asset_manager, app_context, &cloud)
}

fn merge_due_with(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    cloud: &Cloud,
) -> Result<Vec<AutoMerge>> {
    let backups = remote::list_backups(cloud.api_base_url, cloud.access_token)?;
    let due: Vec<CloudPc> = cloud_pcs(db, &backups, cloud.key)?
        .into_iter()
        .filter(|pc| pc.merged_before && pc.newer)
        .collect();
    if due.is_empty() {
        return Ok(Vec::new());
    }
    let registered = remote::registered_keys(cloud.api_base_url, cloud.access_token)?;
    let mut done = Vec::new();
    for pc in due {
        let keys = registered.get(&pc.device_id).cloned().unwrap_or_default();
        let merged = staged(app_context, cloud, &pc.backup_id).and_then(|(folder, _)| {
            merge_by_itself(db, asset_manager, app_context, &folder, &keys)
        });
        discard_cloud_merge(app_context, &pc.backup_id);
        let mut outcome = AutoMerge {
            device_id: pc.device_id.clone(),
            name: pc.name.clone(),
            backup_id: pc.backup_id.clone(),
            outcome: "merged",
            sessions_added: 0,
            sessions_grown: 0,
        };
        match merged {
            Ok(Ok(summary)) => {
                info!(
                    "merged the cloud backup of {} by itself: {} new, {} grown",
                    pc.device_id, summary.sessions_added, summary.sessions_grown
                );
                outcome.sessions_added = summary.sessions_added;
                outcome.sessions_grown = summary.sessions_grown;
            }
            Ok(Err(reason)) => {
                info!(
                    "the cloud backup of {} waits for the player: {reason}",
                    pc.device_id
                );
                outcome.outcome = reason;
            }
            Err(error) => {
                warn!(
                    "the cloud backup of {} did not merge: {error}",
                    pc.device_id
                );
                continue;
            }
        }
        done.push(outcome);
    }
    Ok(done)
}

/// Merges the staged backup in `folder` without the player, or says why it
/// needs them: `new_key` or `failing`.
fn merge_by_itself(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    folder: &Path,
    registered: &HashSet<String>,
) -> Result<std::result::Result<MergeSummary, &'static str>> {
    let preview = merge::preview_from_cloud(db, app_context, folder, false, registered)?;
    if preview.unknown_keys {
        return Ok(Err("new_key"));
    }
    if preview.failing > 0 || preview.own_failing > 0 {
        return Ok(Err("failing"));
    }
    let choices = automatic_choices(&preview);
    merge::apply_from_cloud(
        db,
        asset_manager,
        app_context,
        folder,
        &choices,
        false,
        registered,
    )
    .map(Ok)
}

/// A new game links to the game the preview suggests only on the same
/// launcher id, and stays on its own otherwise.
fn automatic_choices(preview: &MergePreview) -> Vec<GameChoice> {
    preview
        .games
        .iter()
        .filter(|game| game.counts_as.is_none())
        .map(|game| GameChoice {
            game_id: game.game_id.clone(),
            linked_game_id: game
                .suggested_game_id
                .clone()
                .filter(|_| game.suggested_because.as_deref() == Some(SAME_LAUNCHER)),
        })
        .collect()
}

/// The folder of the downloaded cloud backup `backup_id` and the PC it is
/// of, downloaded first when it is not there yet.
fn staged(app_context: &AppContext, cloud: &Cloud, backup_id: &str) -> Result<(PathBuf, String)> {
    let folder = staging_folder(app_context, backup_id)?;
    let backup = folder.join("backup");
    if !backup.is_dir() {
        let download = folder.join("download");
        cleanup_staging_dir(&folder);
        fs::create_dir_all(&download).map_err(|error| {
            VaultimeError::Backup(format!("failed to stage the backup: {error}"))
        })?;
        let fetched = remote::fetch_backup(
            cloud.api_base_url,
            cloud.access_token,
            cloud.key,
            backup_id,
            &download,
        )
        .and_then(|(_, extracted)| {
            fs::rename(&extracted, &backup).map_err(|error| {
                VaultimeError::Backup(format!("failed to stage the backup: {error}"))
            })
        });
        if let Err(error) = fetched {
            cleanup_staging_dir(&folder);
            return Err(error);
        }
        cleanup_staging_dir(&download);
    }
    let (manifest, _) = load_and_validate_manifest(&backup)?;
    Ok((backup, manifest.source_device_id))
}

fn staging_folder(app_context: &AppContext, backup_id: &str) -> Result<PathBuf> {
    // The id names a folder, so it has to be a plain uuid.
    let id = uuid::Uuid::parse_str(backup_id)
        .map_err(|_| VaultimeError::Invalid("not a cloud backup id".into()))?;
    Ok(app_context
        .app_dir
        .join(CLOUD_STAGING_DIR)
        .join(format!("merge-{id}")))
}

/// The keys `device_id` registered with the server.
fn registered_keys_of(cloud: &Cloud, device_id: &str) -> Result<HashSet<String>> {
    Ok(
        remote::registered_keys(cloud.api_base_url, cloud.access_token)?
            .remove(device_id)
            .unwrap_or_default(),
    )
}

/// Removes what cloud transfers left behind in a run that ended early. Runs
/// at start, before any transfer.
pub fn sweep(app_dir: &Path) {
    let folder = app_dir.join(CLOUD_STAGING_DIR);
    if folder.exists()
        && let Err(error) = fs::remove_dir_all(&folder)
    {
        warn!("could not clear {}: {error}", folder.display());
    }
}

fn metadata_text<'a>(backup: &'a RemoteBackupRecord, field: &str) -> Option<&'a str> {
    backup.metadata_json.as_ref()?.get(field)?.as_str()
}

/// A time the server or a manifest gave, comparable whatever its digits.
/// Anything unreadable comes first.
fn instant(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::constants::MERGED_BACKUPS_SETTING;
    use crate::db::repo::settings;
    use crate::playtime::testing::database;

    const KEY: [u8; BACKUP_KEY_BYTES] = [5; BACKUP_KEY_BYTES];

    fn record(
        id: &str,
        pc: Option<&str>,
        created: &str,
        uploaded: &str,
        name: Option<&str>,
    ) -> RemoteBackupRecord {
        let mut metadata = json!({ "sessions_count": 3 });
        if let Some(pc) = pc {
            metadata[SOURCE_PC_FIELD] = json!(pc);
        }
        if let Some(name) = name {
            metadata[PC_NAME_FIELD] = json!(crypto::seal_text(name, &KEY).unwrap());
        }
        RemoteBackupRecord {
            id: id.into(),
            label: None,
            storage_key: String::new(),
            checksum: String::new(),
            size_bytes: 0,
            backup_created_at: created.into(),
            uploaded_at: uploaded.into(),
            status: "complete".into(),
            client_device_id: None,
            metadata_json: Some(metadata),
        }
    }

    fn backups() -> Vec<RemoteBackupRecord> {
        vec![
            record(
                "older",
                Some("laptop"),
                "2026-10-18T10:00:00Z",
                "2026-10-18T10:01:00.5Z",
                Some("Laptop"),
            ),
            record(
                "newest",
                Some("laptop"),
                "2026-10-19T10:00:00Z",
                "2026-10-19T10:01:00Z",
                Some("Laptop"),
            ),
            record(
                "ours",
                Some("device"),
                "2026-10-19T11:00:00Z",
                "2026-10-19T11:01:00Z",
                Some("Desktop"),
            ),
            record(
                "unnamed",
                None,
                "2026-10-19T12:00:00Z",
                "2026-10-19T12:01:00Z",
                None,
            ),
            record(
                "deck",
                Some("deck"),
                "2026-10-19T08:00:00Z",
                "2026-10-19T08:00:01Z",
                None,
            ),
        ]
    }

    #[test]
    fn lists_the_newest_backup_of_every_other_pc() {
        let db = database();
        ledger::act_as(Some(&ledger::test_pc("device")));
        let pcs = cloud_pcs(&db, &backups(), &KEY).unwrap();
        let listed: Vec<(&str, Option<&str>, &str, bool, bool)> = pcs
            .iter()
            .map(|pc| {
                (
                    pc.device_id.as_str(),
                    pc.name.as_deref(),
                    pc.backup_id.as_str(),
                    pc.merged_before,
                    pc.newer,
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                ("laptop", Some("Laptop"), "newest", false, true),
                ("deck", None, "deck", false, true)
            ]
        );
        assert_eq!(pcs[0].sessions_count, 3);

        // Merged before: newer only past the backup merged last, whatever its digits.
        settings::set_setting(
            &db,
            MERGED_BACKUPS_SETTING,
            r#"{"laptop":"2026-10-19T10:00:00.000Z"}"#,
        )
        .unwrap();
        let laptop = cloud_pcs(&db, &backups(), &KEY).unwrap().remove(0);
        assert!(laptop.merged_before && !laptop.newer);
        settings::set_setting(
            &db,
            MERGED_BACKUPS_SETTING,
            r#"{"laptop":"2026-10-18T10:00:00.000Z"}"#,
        )
        .unwrap();
        let laptop = cloud_pcs(&db, &backups(), &KEY).unwrap().remove(0);
        assert!(laptop.merged_before && laptop.newer);
        ledger::act_as(None);
    }

    #[test]
    fn a_name_sealed_with_another_key_falls_back_to_the_one_known_here() {
        let db = database();
        ledger::act_as(Some(&ledger::test_pc("device")));
        devices::rename_device(&db, "laptop", "Old laptop").unwrap();
        let pcs = cloud_pcs(&db, &backups(), &[6; BACKUP_KEY_BYTES]).unwrap();
        let laptop = pcs.iter().find(|pc| pc.device_id == "laptop").unwrap();
        assert_eq!(laptop.name.as_deref(), Some("Old laptop"));
        ledger::act_as(None);
    }

    /// A PC with its own data folder, database, artwork and ledger.
    struct Pc {
        context: AppContext,
        db: Database,
        assets: AssetManager,
        me: (ed25519_dalek::SigningKey, String),
    }

    impl Pc {
        fn new(device: &str, name: &str) -> Self {
            let app_dir = std::env::temp_dir()
                .join(format!("vaultime-e2e-{device}-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(app_dir.join(crate::constants::ASSET_CACHE_DIR)).unwrap();
            let pc = Self {
                db: Database::open(&app_dir.join(crate::constants::DATABASE_FILE)).unwrap(),
                assets: AssetManager::new(app_dir.join(crate::constants::ASSET_CACHE_DIR)),
                context: AppContext {
                    app_dir,
                    device_id: device.into(),
                    app_version: "0.6.0".into(),
                },
                me: ledger::test_pc(device),
            };
            pc.act();
            devices::ensure_device(&pc.db, device, "linux", "0.6.0").unwrap();
            devices::describe_this_device(&pc.db, device, &ledger::key_id().unwrap(), name)
                .unwrap();
            pc.db
                .with_transaction(|conn| ledger::begin_if_empty(conn, "first_start"))
                .unwrap();
            pc
        }

        fn act(&self) {
            ledger::act_as(Some(&self.me));
        }

        fn play(&self, game: &str) -> String {
            self.act();
            let session =
                crate::db::repo::sessions::create_session(&self.db, game, &self.context.device_id)
                    .unwrap();
            crate::db::repo::sessions::end_session(
                &self.db,
                &session.id,
                600_000,
                600_000,
                0,
                crate::integrity::STATUS_LOCAL,
            )
            .unwrap();
            session.id
        }

        fn upload(&self, cloud: &Cloud) -> remote::RemoteBackupUploadResult {
            self.act();
            remote::upload_with_key(
                &self.db,
                &self.assets,
                &self.context,
                cloud.api_base_url,
                cloud.access_token,
                cloud.key,
                None,
                None,
            )
            .unwrap()
        }
    }

    impl Drop for Pc {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.context.app_dir);
        }
    }

    /// Runs against a real API that allows back to back backups, after
    /// `artwork_is_uploaded_once_and_restores` on the same account, through
    /// `scripts/cloud-e2e.sh`.
    #[test]
    #[ignore = "needs a running API, set VAULTIME_E2E_API and VAULTIME_E2E_TOKEN"]
    fn two_pcs_merge_through_the_cloud() {
        let api = std::env::var("VAULTIME_E2E_API").unwrap();
        let token = std::env::var("VAULTIME_E2E_TOKEN").unwrap();
        let cloud = Cloud {
            api_base_url: &api,
            access_token: &token,
            key: &[3; BACKUP_KEY_BYTES],
        };
        let laptop = Pc::new("laptop-e2e", "Laptop");
        let desktop = Pc::new("desktop-e2e", "Desktop");
        laptop.act();
        let hades = crate::db::repo::games::create_game(
            &laptop.db,
            &crate::db::models::CreateGame {
                title: "Hades".into(),
                executable_path: Some("/games/Hades.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap()
        .id;
        let first = laptop.play(&hades);
        let uploaded = laptop.upload(&cloud);

        // The server keeps the laptop's keys, and its name is sealed.
        laptop.act();
        let registered = remote::registered_keys(&api, &token).unwrap();
        assert_eq!(registered["laptop-e2e"], ledger::own_keys().unwrap());
        let listed = remote::list_backups(&api, &token).unwrap();
        let record = listed
            .iter()
            .find(|backup| backup.id == uploaded.backup.id)
            .unwrap();
        let sealed = record.metadata_json.as_ref().unwrap()[PC_NAME_FIELD]
            .as_str()
            .unwrap();
        assert!(!sealed.contains("Laptop"));

        // The desktop merges the laptop by hand the first time.
        desktop.act();
        let pcs = cloud_pcs(&desktop.db, &listed, cloud.key).unwrap();
        let pc = pcs.iter().find(|pc| pc.device_id == "laptop-e2e").unwrap();
        assert_eq!(pc.name.as_deref(), Some("Laptop"));
        assert!(!pc.merged_before && pc.newer);
        let preview =
            preview_with(&desktop.db, &desktop.context, &cloud, &pc.backup_id, false).unwrap();
        assert!(preview.first_merge && !preview.unknown_keys);
        assert_eq!((preview.new_sessions, preview.failing), (1, 0));
        let merged = merge_with(
            &desktop.db,
            &desktop.assets,
            &desktop.context,
            &cloud,
            &pc.backup_id,
            &[],
            false,
        )
        .unwrap();
        assert_eq!(merged.sessions_added, 1);
        assert!(
            !staging_folder(&desktop.context, &pc.backup_id)
                .unwrap()
                .exists()
        );
        assert!(
            merge_due_with(&desktop.db, &desktop.assets, &desktop.context, &cloud)
                .unwrap()
                .is_empty()
        );

        // A newer backup of the laptop comes in by itself.
        let second = laptop.play(&hades);
        let newest = laptop.upload(&cloud);
        desktop.act();
        let done = merge_due_with(&desktop.db, &desktop.assets, &desktop.context, &cloud).unwrap();
        assert_eq!(done.len(), 1, "{done:?}");
        assert_eq!((done[0].outcome, done[0].sessions_added), ("merged", 1));
        let here: Vec<String> = crate::db::repo::sessions::list_all_sessions(&desktop.db)
            .unwrap()
            .into_iter()
            .map(|session| session.id)
            .collect();
        assert!(here.contains(&first) && here.contains(&second));

        // The desktop's own backups never push out the laptop's newest.
        desktop.upload(&cloud);
        desktop.upload(&cloud);
        let kept = remote::list_backups(&api, &token).unwrap();
        assert!(kept.iter().any(|backup| backup.id == newest.backup.id));
        ledger::act_as(None);
    }

    #[test]
    fn an_automatic_merge_links_new_games_on_the_same_launcher_id_only() {
        let game = |id: &str, because: Option<&str>, counts_as: Option<&str>| merge::MergeGame {
            game_id: id.into(),
            suggested_game_id: because.map(|_| format!("ours-{id}")),
            suggested_because: because.map(str::to_owned),
            counts_as: counts_as.map(str::to_owned),
            ..merge::MergeGame::default()
        };
        let preview = MergePreview {
            games: vec![
                game("hades", Some("launcher"), None),
                game("celeste", Some("title"), None),
                game("doom", None, None),
                game("hades-epic", None, Some("hades")),
            ],
            ..MergePreview::default()
        };
        let choices: Vec<(String, Option<String>)> = automatic_choices(&preview)
            .into_iter()
            .map(|choice| (choice.game_id, choice.linked_game_id))
            .collect();
        assert_eq!(
            choices,
            [
                ("hades".to_owned(), Some("ours-hades".to_owned())),
                ("celeste".to_owned(), None),
                ("doom".to_owned(), None),
            ]
        );
    }
}
