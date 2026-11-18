// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Automatic local backups, once a day while Vaultime runs and when it quits,
//! and before each merge. Each sort gets its own folder name prefix, so only
//! the newest few of each are kept, merges never push out the daily ones,
//! and backups made by hand are never touched. A PC counts only the backups
//! its manifests name, so several PCs can share a folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use chrono::{NaiveDateTime, Utc};
use log::{info, warn};

use super::{BACKUP_MANIFEST_FILE, DATABASE_FILE, LocalBackupSummary, export_backup_to};
use crate::AppContext;
use crate::assets::AssetManager;
use crate::constants::{
    AUTO_BACKUP_DIR, AUTO_BACKUP_ENABLED_SETTING, AUTO_BACKUP_FOLDER_SETTING, AUTO_BACKUP_KEEP,
    AUTO_BACKUP_ON_QUIT_MIN_AGE, AUTO_BACKUP_PREFIX, AUTO_BACKUP_UNFINISHED_AGE,
    SAFETY_BACKUP_KEEP, SAFETY_BACKUP_PREFIX,
};
use crate::db::connection::Database;
use crate::db::repo::{backup_snapshots, settings};
use crate::error::{Result, VaultimeError};
use crate::integrity;

/// A sort of automatic backup: its folder name prefix, how many are kept and
/// its label in the backup history.
struct Kind {
    prefix: &'static str,
    keep: usize,
    label: &'static str,
}

const DAILY: Kind = Kind {
    prefix: AUTO_BACKUP_PREFIX,
    keep: AUTO_BACKUP_KEEP,
    label: "Automatic",
};

const BEFORE_MERGE: Kind = Kind {
    prefix: SAFETY_BACKUP_PREFIX,
    keep: SAFETY_BACKUP_KEEP,
    label: "Before a merge",
};

/// Where automatic backups go: the folder picked in Settings, otherwise a
/// folder in Vaultime's data folder.
pub fn folder(db: &Database, app_dir: &Path) -> PathBuf {
    settings::get_setting(db, AUTO_BACKUP_FOLDER_SETTING)
        .ok()
        .flatten()
        .filter(|folder| !folder.trim().is_empty())
        .map_or_else(|| app_dir.join(AUTO_BACKUP_DIR), PathBuf::from)
}

/// On unless the setting says `false`.
pub fn enabled(db: &Database) -> bool {
    settings::get_setting(db, AUTO_BACKUP_ENABLED_SETTING)
        .ok()
        .flatten()
        .is_none_or(|value| value != "false")
}

/// Held while an automatic backup is written. The daily check, the backup on
/// quit and the one before a merge may overlap, and each prunes the folder,
/// which would take a backup the others are still writing.
static RUNNING: Mutex<()> = Mutex::new(());

/// Makes an automatic backup when the newest one is at least `min_age` old
/// and holds less than this one would, then deletes all but the newest few,
/// and copies it into the sync folder. Returns the new backup, if any.
pub fn back_up_if_due(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    min_age: Duration,
) -> Result<Option<LocalBackupSummary>> {
    back_up(db, asset_manager, app_context, min_age, true)
}

/// The backup on quit. It leaves the sync folder to the check at the next
/// start, so a folder that is away, like a NAS that is off, cannot hold up
/// quitting.
pub fn back_up_on_quit(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
) -> Result<Option<LocalBackupSummary>> {
    back_up(
        db,
        asset_manager,
        app_context,
        AUTO_BACKUP_ON_QUIT_MIN_AGE,
        false,
    )
}

fn back_up(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    min_age: Duration,
    to_sync_folder: bool,
) -> Result<Option<LocalBackupSummary>> {
    let _running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);

    if !enabled(db) {
        return Ok(None);
    }
    let folder = made_folder(db, app_context)?;
    let device = this_pc(app_context);
    if newest_age(&folder, &device).is_some_and(|age| age < min_age) {
        // A sync folder that was away gets the newest backup once it is back.
        if to_sync_folder && let Some(newest) = newest_of(&folder, &device, &DAILY) {
            copy_to_sync_folder(db, &newest);
        }
        return Ok(None);
    }
    // A copy of what the newest backup holds already would add nothing.
    let fingerprint = super::fingerprint::fingerprint(db, asset_manager.cache_dir())?;
    if let Some(newest) = newest_of(&folder, &device, &DAILY)
        && fingerprint_of(&newest).is_some_and(|noted| noted == fingerprint)
    {
        info!("nothing changed since the backup {}", newest.display());
        if to_sync_folder {
            copy_to_sync_folder(db, &newest);
        }
        return Ok(None);
    }
    let summary = write_backup(db, asset_manager, app_context, &folder, &DAILY)?;
    if to_sync_folder {
        copy_to_sync_folder(db, Path::new(&summary.backup_path));
    }
    Ok(Some(summary))
}

/// Copies an automatic backup into the sync folder, if one is set. One that
/// is away, like a NAS that is off, gets a backup at a later check.
fn copy_to_sync_folder(db: &Database, backup: &Path) {
    if let Err(error) = super::folder::publish(db, backup) {
        warn!("could not copy the backup into the sync folder: {error}");
    }
}

/// Saves a backup before a merge, also when daily backups are off, so a
/// restore can undo the merge. It never goes to the sync folder.
pub fn back_up_before_merge(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
) -> Result<LocalBackupSummary> {
    let _running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    let folder = made_folder(db, app_context)?;
    write_backup(db, asset_manager, app_context, &folder, &BEFORE_MERGE)
}

/// A daily automatic backup now, for tests.
#[cfg(test)]
pub(crate) fn back_up_now(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
) -> Result<LocalBackupSummary> {
    let _running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    let folder = made_folder(db, app_context)?;
    write_backup(db, asset_manager, app_context, &folder, &DAILY)
}

/// Saves an automatic backup now and copies it into the sync folder, as when
/// the player picks one, so the other PCs find this one right away.
pub fn back_up_to_sync_folder(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
) -> Result<LocalBackupSummary> {
    let _running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
    let folder = made_folder(db, app_context)?;
    let summary = write_backup(db, asset_manager, app_context, &folder, &DAILY)?;
    super::folder::publish(db, Path::new(&summary.backup_path))?;
    Ok(summary)
}

/// The folder automatic backups go to, made when it is not there yet.
fn made_folder(db: &Database, app_context: &AppContext) -> Result<PathBuf> {
    let folder = folder(db, &app_context.app_dir);
    fs::create_dir_all(&folder).map_err(|error| {
        VaultimeError::Backup(format!(
            "could not create the backup folder {}: {error}",
            folder.display()
        ))
    })?;
    Ok(folder)
}

fn write_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    folder: &Path,
    kind: &Kind,
) -> Result<LocalBackupSummary> {
    let device = this_pc(app_context);
    let previous = newest_complete(folder, &device);
    let summary = export_backup_to(
        db,
        asset_manager,
        app_context,
        folder,
        kind.prefix,
        previous.as_deref(),
    )?;
    if let Err(error) = backup_snapshots::create_snapshot(
        db,
        Some(&summary.source_device_id),
        &summary.overall_checksum,
        Some(&summary.backup_path),
        Some(kind.label),
    ) {
        warn!("failed to record the automatic backup: {error}");
    }
    prune(folder, kind, &device);
    info!("automatic backup written to {}", summary.backup_path);
    Ok(summary)
}

/// The PC automatic backups made here name, as `export_backup_to` writes it.
fn this_pc(app_context: &AppContext) -> String {
    integrity::ledger::this_device().unwrap_or_else(|_| app_context.device_id.clone())
}

/// Age of the newest complete daily backup of `device`. The manifest is
/// written last, so its time is when the backup finished.
fn newest_age(folder: &Path, device: &str) -> Option<Duration> {
    backups_of(folder, &DAILY)
        .iter()
        .filter(|backup| made_by(backup, device))
        .filter_map(|backup| {
            fs::metadata(backup.join(BACKUP_MANIFEST_FILE))
                .ok()?
                .modified()
                .ok()
        })
        .max()
        .map(|newest| SystemTime::now().duration_since(newest).unwrap_or_default())
}

/// The newest automatic backup of `device` in `folder` that finished, of
/// either sort, to start from when the database does not open.
pub fn newest_complete(folder: &Path, device: &str) -> Option<PathBuf> {
    [&DAILY, &BEFORE_MERGE]
        .into_iter()
        .filter_map(|kind| newest_of(folder, device, kind))
        .max_by_key(|backup| started_at(backup))
}

/// The newest backup of one sort of `device` in `folder` that finished.
fn newest_of(folder: &Path, device: &str, kind: &Kind) -> Option<PathBuf> {
    backups_of(folder, kind)
        .into_iter()
        .rev()
        .find(|backup| made_by(backup, device))
}

/// Whether `backup` finished and its manifest names `device` as its PC.
fn made_by(backup: &Path, device: &str) -> bool {
    #[derive(serde::Deserialize)]
    struct Source {
        source_device_id: String,
    }
    fs::read_to_string(backup.join(BACKUP_MANIFEST_FILE))
        .ok()
        .and_then(|manifest| serde_json::from_str::<Source>(&manifest).ok())
        .is_some_and(|source| source.source_device_id == device)
}

/// What `backup` holds, as its manifest notes it. Older versions noted
/// nothing.
fn fingerprint_of(backup: &Path) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Noted {
        fingerprint: Option<String>,
        files: Vec<NotedFile>,
    }
    #[derive(serde::Deserialize)]
    struct NotedFile {
        path: String,
        bytes: u64,
    }
    let manifest = fs::read_to_string(backup.join(BACKUP_MANIFEST_FILE)).ok()?;
    let noted: Noted = serde_json::from_str(&manifest).ok()?;
    // A backup whose database went missing or changed size since is made again.
    let database = noted.files.iter().find(|file| file.path == DATABASE_FILE)?;
    let size = fs::metadata(backup.join(DATABASE_FILE)).ok()?.len();
    noted.fingerprint.filter(|_| size == database.bytes)
}

/// When an automatic backup started, from the timestamp its name ends in.
fn started_at(backup: &Path) -> Option<NaiveDateTime> {
    let name = backup.file_name()?.to_str()?;
    let stamp = [&DAILY, &BEFORE_MERGE]
        .into_iter()
        .find_map(|kind| name.strip_prefix(&format!("{}-", kind.prefix)))?;
    NaiveDateTime::parse_from_str(stamp.get(..15)?, "%Y%m%dT%H%M%S").ok()
}

/// The backup folders of one sort, oldest first. Their names end in a
/// timestamp that sorts by time.
fn backups_of(folder: &Path, kind: &Kind) -> Vec<PathBuf> {
    let prefix = format!("{}-", kind.prefix);
    let mut backups: Vec<PathBuf> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_dir())
                && entry.file_name().to_string_lossy().starts_with(&prefix)
        })
        .map(|entry| entry.path())
        .collect();
    backups.sort();
    backups
}

/// Deletes all but the newest few backups of one sort of `device`, and
/// unfinished ones that started long enough ago to be left over. Another
/// PC's backups stay, also one it is writing into the same folder now.
fn prune(folder: &Path, kind: &Kind, device: &str) {
    let backups = backups_of(folder, kind);
    let now = Utc::now().naive_utc();
    let unfinished = backups.iter().filter(|backup| {
        !backup.join(BACKUP_MANIFEST_FILE).is_file()
            && started_at(backup).is_some_and(|started| {
                (now - started)
                    .to_std()
                    .is_ok_and(|age| age > AUTO_BACKUP_UNFINISHED_AGE)
            })
    });
    let mine: Vec<&PathBuf> = backups
        .iter()
        .filter(|backup| made_by(backup, device))
        .collect();
    let surplus = mine.len().saturating_sub(kind.keep);
    for backup in unfinished.chain(mine.into_iter().take(surplus)) {
        if let Err(error) = fs::remove_dir_all(backup) {
            warn!(
                "could not delete the old backup {}: {error}",
                backup.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::ASSET_CACHE_DIR;

    fn backup(folder: &Path, name: &str, device: Option<&str>) -> PathBuf {
        let path = folder.join(name);
        fs::create_dir_all(&path).unwrap();
        if let Some(device) = device {
            let manifest = serde_json::json!({ "source_device_id": device }).to_string();
            fs::write(path.join(BACKUP_MANIFEST_FILE), manifest).unwrap();
        }
        path
    }

    #[test]
    fn backs_up_once_until_due_and_follows_the_settings() {
        let root = std::env::temp_dir().join(format!("vaultime-auto-run-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("asset-cache")).unwrap();
        let context = AppContext {
            app_dir: root.clone(),
            device_id: "test-device".into(),
            app_version: "0.1.0".into(),
        };
        let db = Database::open(&root.join("vaultime.db")).unwrap();
        let assets = AssetManager::new(root.join("asset-cache"));
        let hour = Duration::from_hours(1);

        let first = back_up_if_due(&db, &assets, &context, hour).unwrap();
        assert!(first.is_some(), "the first backup is always due");
        assert!(
            back_up_if_due(&db, &assets, &context, hour)
                .unwrap()
                .is_none()
        );
        assert_eq!(backups_of(&root.join(AUTO_BACKUP_DIR), &DAILY).len(), 1);

        let elsewhere = root.join("elsewhere");
        settings::set_setting(
            &db,
            AUTO_BACKUP_FOLDER_SETTING,
            &elsewhere.to_string_lossy(),
        )
        .unwrap();
        assert!(
            back_up_if_due(&db, &assets, &context, hour)
                .unwrap()
                .is_some()
        );
        assert_eq!(backups_of(&elsewhere, &DAILY).len(), 1);

        settings::set_setting(&db, AUTO_BACKUP_ENABLED_SETTING, "false").unwrap();
        assert!(
            back_up_if_due(&db, &assets, &context, Duration::ZERO)
                .unwrap()
                .is_none()
        );
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_day_without_changes_adds_no_backup() {
        let root =
            std::env::temp_dir().join(format!("vaultime-auto-same-{}", uuid::Uuid::new_v4()));
        let cache = root.join("asset-cache");
        fs::create_dir_all(&cache).unwrap();
        let context = AppContext {
            app_dir: root.clone(),
            device_id: "test-device".into(),
            app_version: "0.1.0".into(),
        };
        let db = Database::open(&root.join("vaultime.db")).unwrap();
        let assets = AssetManager::new(cache.clone());
        let daily = || backups_of(&root.join(AUTO_BACKUP_DIR), &DAILY);

        assert!(
            back_up_if_due(&db, &assets, &context, Duration::ZERO)
                .unwrap()
                .is_some()
        );
        assert!(
            back_up_if_due(&db, &assets, &context, Duration::ZERO)
                .unwrap()
                .is_none(),
            "nothing changed"
        );
        assert_eq!(daily().len(), 1);
        let newest = daily().into_iter().next().unwrap();
        let database = fs::read(newest.join(DATABASE_FILE)).unwrap();
        fs::write(newest.join(DATABASE_FILE), &database[..database.len() / 2]).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(
            back_up_if_due(&db, &assets, &context, Duration::ZERO)
                .unwrap()
                .is_some(),
            "the newest backup lost half its database"
        );
        fs::remove_dir_all(&newest).unwrap();
        assert_eq!(daily().len(), 1);

        settings::set_setting(&db, AUTO_BACKUP_ENABLED_SETTING, "true").unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(
            back_up_if_due(&db, &assets, &context, Duration::ZERO)
                .unwrap()
                .is_some()
        );
        assert_eq!(daily().len(), 2);

        // A newest backup that notes nothing, as older versions wrote them.
        let newest = daily().pop().unwrap();
        let manifest = newest.join(BACKUP_MANIFEST_FILE);
        let mut noted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
        noted.as_object_mut().unwrap().remove("fingerprint");
        fs::write(&manifest, noted.to_string()).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(
            back_up_if_due(&db, &assets, &context, Duration::ZERO)
                .unwrap()
                .is_some()
        );
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unchanged_artwork_is_stored_once() {
        let root =
            std::env::temp_dir().join(format!("vaultime-auto-link-{}", uuid::Uuid::new_v4()));
        let cache = root.join("asset-cache");
        fs::create_dir_all(cache.join("game")).unwrap();
        fs::write(cache.join("game/cover.jpg"), b"cover").unwrap();
        fs::write(cache.join("game/banner.jpg"), b"banner").unwrap();
        let context = AppContext {
            app_dir: root.clone(),
            device_id: "test-device".into(),
            app_version: "0.1.0".into(),
        };
        let db = Database::open(&root.join("vaultime.db")).unwrap();
        let assets = AssetManager::new(cache.clone());
        let artwork = |backup: &LocalBackupSummary, name: &str| {
            PathBuf::from(&backup.backup_path)
                .join(ASSET_CACHE_DIR)
                .join("game")
                .join(name)
        };

        let first = back_up_if_due(&db, &assets, &context, Duration::ZERO)
            .unwrap()
            .unwrap();
        fs::write(cache.join("game/banner.jpg"), b"new banner").unwrap();
        std::thread::sleep(Duration::from_millis(5));
        let second = back_up_if_due(&db, &assets, &context, Duration::ZERO)
            .unwrap()
            .unwrap();

        // A write through the first backup shows in the second when both are one file.
        fs::write(artwork(&first, "cover.jpg"), b"one file").unwrap();
        assert_eq!(
            fs::read(artwork(&second, "cover.jpg")).unwrap(),
            b"one file"
        );
        assert_eq!(fs::read(artwork(&first, "banner.jpg")).unwrap(), b"banner");
        assert_eq!(
            fs::read(artwork(&second, "banner.jpg")).unwrap(),
            b"new banner"
        );
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn keeps_the_newest_and_never_touches_other_backups() {
        let folder =
            std::env::temp_dir().join(format!("vaultime-auto-test-{}", uuid::Uuid::new_v4()));
        let me = Some("desktop");
        let oldest = backup(&folder, "vaultime-auto-20260901T010000Z", me);
        let middle = backup(&folder, "vaultime-auto-20260902T010000Z", me);
        let newest = backup(&folder, "vaultime-auto-20260903T010000Z", me);
        let unfinished = backup(&folder, "vaultime-auto-20260904T010000Z", None);
        let by_hand = backup(&folder, "vaultime-backup-20250101T010000Z", me);

        prune(&folder, &Kind { keep: 2, ..DAILY }, "desktop");
        assert!(!oldest.exists());
        assert!(middle.exists() && newest.exists());
        assert!(!unfinished.exists(), "an unfinished backup is removed");
        assert!(by_hand.exists(), "backups made by hand stay");
        assert!(newest_age(&folder, "desktop").is_some_and(|age| age < Duration::from_secs(60)));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn backups_before_a_merge_leave_the_daily_ones_alone() {
        let folder =
            std::env::temp_dir().join(format!("vaultime-auto-merges-{}", uuid::Uuid::new_v4()));
        let me = Some("desktop");
        let daily = backup(&folder, "vaultime-auto-20260901T010000Z", me);
        for day in 2..=6 {
            backup(
                &folder,
                &format!("vaultime-safety-202609{day:02}T010000Z"),
                me,
            );
        }

        prune(&folder, &BEFORE_MERGE, "desktop");
        assert!(daily.exists());
        assert_eq!(backups_of(&folder, &BEFORE_MERGE).len(), SAFETY_BACKUP_KEEP);
        assert_eq!(newest_of(&folder, "desktop", &DAILY), Some(daily));
        assert_eq!(
            newest_complete(&folder, "desktop"),
            Some(folder.join("vaultime-safety-20260906T010000Z")),
            "a recovery starts from the newest of either sort"
        );
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn pcs_sharing_a_folder_keep_their_own_backups() {
        let folder =
            std::env::temp_dir().join(format!("vaultime-auto-shared-{}", uuid::Uuid::new_v4()));
        let mine = backup(&folder, "vaultime-auto-20260901T010000Z", Some("desktop"));
        let theirs = backup(&folder, "vaultime-auto-20260902T010000Z", Some("laptop"));
        let stamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
        let writing = backup(&folder, &format!("vaultime-auto-{stamp}"), None);

        prune(&folder, &Kind { keep: 1, ..DAILY }, "desktop");
        assert!(mine.exists() && theirs.exists());
        assert!(writing.exists(), "a backup another PC is writing stays");
        assert_eq!(newest_complete(&folder, "desktop"), Some(mine.clone()));
        assert_eq!(newest_complete(&folder, "laptop"), Some(theirs));
        assert_eq!(newest_complete(&folder, "tablet"), None);

        let newer = backup(&folder, "vaultime-auto-20260903T010000Z", Some("desktop"));
        prune(&folder, &Kind { keep: 1, ..DAILY }, "desktop");
        assert!(!mine.exists() && newer.exists());
        fs::remove_dir_all(folder).unwrap();
    }
}
