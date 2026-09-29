// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Automatic local backups, once a day while Vaultime runs and when it quits.
//! They get their own folder name prefix, so only the newest few automatic
//! backups are kept and backups made by hand are never touched.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use log::{info, warn};

use super::{BACKUP_MANIFEST_FILE, LocalBackupSummary, export_backup_to};
use crate::AppContext;
use crate::assets::AssetManager;
use crate::constants::{
    AUTO_BACKUP_DIR, AUTO_BACKUP_ENABLED_SETTING, AUTO_BACKUP_FOLDER_SETTING, AUTO_BACKUP_KEEP,
    AUTO_BACKUP_PREFIX,
};
use crate::db::connection::Database;
use crate::db::repo::{backup_snapshots, settings};
use crate::error::{Result, VaultimeError};

/// Snapshot label in the backup history.
const LABEL: &str = "Automatic";

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

/// Makes an automatic backup when the newest one is at least `min_age` old,
/// then deletes all but the newest few. Returns the new backup, if any.
pub fn back_up_if_due(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    min_age: Duration,
) -> Result<Option<LocalBackupSummary>> {
    // The daily check and the backup on quit may overlap.
    static RUNNING: Mutex<()> = Mutex::new(());
    let _running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);

    if !enabled(db) {
        return Ok(None);
    }
    let folder = folder(db, &app_context.app_dir);
    fs::create_dir_all(&folder).map_err(|error| {
        VaultimeError::Backup(format!(
            "could not create the backup folder {}: {error}",
            folder.display()
        ))
    })?;
    if newest_age(&folder).is_some_and(|age| age < min_age) {
        return Ok(None);
    }

    let previous = automatic_backups(&folder)
        .into_iter()
        .rev()
        .find(|backup| backup.join(BACKUP_MANIFEST_FILE).is_file());
    let summary = export_backup_to(
        db,
        asset_manager,
        app_context,
        &folder,
        AUTO_BACKUP_PREFIX,
        previous.as_deref(),
    )?;
    if let Err(error) = backup_snapshots::create_snapshot(
        db,
        Some(&app_context.device_id),
        &summary.overall_checksum,
        Some(&summary.backup_path),
        Some(LABEL),
    ) {
        warn!("failed to record the automatic backup: {error}");
    }
    prune(&folder, AUTO_BACKUP_KEEP);
    info!("automatic backup written to {}", summary.backup_path);
    Ok(Some(summary))
}

/// Age of the newest complete automatic backup. The manifest is written last,
/// so its time is when the backup finished.
fn newest_age(folder: &Path) -> Option<Duration> {
    automatic_backups(folder)
        .iter()
        .filter_map(|backup| {
            fs::metadata(backup.join(BACKUP_MANIFEST_FILE))
                .ok()?
                .modified()
                .ok()
        })
        .max()
        .map(|newest| SystemTime::now().duration_since(newest).unwrap_or_default())
}

/// Automatic backup folders, oldest first. Their names end in a timestamp
/// that sorts by time.
fn automatic_backups(folder: &Path) -> Vec<PathBuf> {
    let prefix = format!("{AUTO_BACKUP_PREFIX}-");
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

/// Deletes unfinished automatic backups and all but the newest `keep` others.
fn prune(folder: &Path, keep: usize) {
    let (complete, unfinished): (Vec<PathBuf>, Vec<PathBuf>) = automatic_backups(folder)
        .into_iter()
        .partition(|backup| backup.join(BACKUP_MANIFEST_FILE).is_file());
    let surplus = complete.len().saturating_sub(keep);
    for backup in unfinished.iter().chain(complete.iter().take(surplus)) {
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
    use super::super::BACKUP_ASSET_DIR;
    use super::*;

    fn backup(folder: &Path, name: &str, complete: bool) -> PathBuf {
        let path = folder.join(name);
        fs::create_dir_all(&path).unwrap();
        if complete {
            fs::write(path.join(BACKUP_MANIFEST_FILE), "{}").unwrap();
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
        assert_eq!(automatic_backups(&root.join(AUTO_BACKUP_DIR)).len(), 1);

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
        assert_eq!(automatic_backups(&elsewhere).len(), 1);

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
                .join(BACKUP_ASSET_DIR)
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
        let oldest = backup(&folder, "vaultime-auto-20260901T010000Z", true);
        let middle = backup(&folder, "vaultime-auto-20260902T010000Z", true);
        let newest = backup(&folder, "vaultime-auto-20260903T010000Z", true);
        let unfinished = backup(&folder, "vaultime-auto-20260904T010000Z", false);
        let by_hand = backup(&folder, "vaultime-backup-20250101T010000Z", true);

        prune(&folder, 2);
        assert!(!oldest.exists());
        assert!(middle.exists() && newest.exists());
        assert!(!unfinished.exists(), "an unfinished backup is removed");
        assert!(by_hand.exists(), "backups made by hand stay");
        assert!(newest_age(&folder).is_some_and(|age| age < Duration::from_secs(60)));
        fs::remove_dir_all(folder).unwrap();
    }
}
