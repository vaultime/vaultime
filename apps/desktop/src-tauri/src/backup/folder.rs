// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Backups in a sync folder, one that a sync service or a NAS keeps the same
//! on every PC. Each PC copies its
//! automatic backups into a folder of its own there, named by its id: the
//! database and the manifest as they are, and the artwork once by checksum,
//! so a sync service uploads an image only once however many backups show
//! it. A PC writes only its own folder.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use log::{info, warn};
use serde::{Deserialize, Serialize};

use super::{
    BACKUP_MANIFEST_FILE, LocalBackupManifest, create_dir, is_artwork_path, read_manifest,
};
use crate::assets::is_plain_name;
use crate::constants::{
    AUTO_BACKUP_PREFIX, DATABASE_FILE, SYNC_ARTWORK_DIR, SYNC_BACKUP_KEEP, SYNC_DIR,
    SYNC_FOLDER_SETTING, SYNC_PC_FILE,
};
use crate::db::connection::Database;
use crate::db::repo::{devices, settings};
use crate::error::{Result, VaultimeError};

/// Name ending of a backup or an image while it is copied in.
const PARTIAL: &str = ".partial";

/// What `pc.json` tells the other PCs about the PC whose folder it is in.
#[derive(Debug, Default, Serialize, Deserialize)]
struct PcFile {
    name: Option<String>,
    app_version: String,
}

/// The folder the player picked to keep backups in sync, if any.
fn picked(db: &Database) -> Option<PathBuf> {
    let picked = settings::get_setting(db, SYNC_FOLDER_SETTING)
        .ok()
        .flatten()?;
    let picked = picked.trim();
    (!picked.is_empty()).then(|| PathBuf::from(picked))
}

/// The folder backups sync in: `Vaultime` in the folder the player picked,
/// or the picked folder itself when it has that name, as when a second PC
/// picks the folder the first one made. None without a sync folder.
pub fn root(db: &Database) -> Option<PathBuf> {
    picked(db).map(|picked| root_in(&picked))
}

fn root_in(picked: &Path) -> PathBuf {
    let named = picked
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(SYNC_DIR));
    if named {
        return picked.to_path_buf();
    }
    // One another PC made with other capitals, as Linux tells them apart.
    fs::read_dir(picked)
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .find(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_dir())
                && entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(SYNC_DIR)
        })
        .map_or_else(|| picked.join(SYNC_DIR), |entry| entry.path())
}

/// The folder of `device_id` in the sync folder at `root`.
fn pc_folder(root: &Path, device_id: &str) -> Result<PathBuf> {
    if !is_plain_name(device_id) {
        return Err(VaultimeError::Invalid(
            "Not a PC of the sync folder.".into(),
        ));
    }
    Ok(root.join(device_id))
}

/// A SHA-256 in lowercase hex, the only name an image in the sync folder has.
fn is_checksum(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Copies the finished automatic backup in `backup` into this PC's folder in
/// the sync folder, then deletes all but the newest `SYNC_BACKUP_KEEP` there
/// and the images none of them shows. Does nothing without a sync folder. It
/// never makes the picked folder, so a drive that is away does not fill up
/// here instead.
pub fn publish(db: &Database, backup: &Path) -> Result<()> {
    let Some(picked) = picked(db) else {
        return Ok(());
    };
    if !picked.is_dir() {
        return Err(VaultimeError::Backup(format!(
            "the sync folder {} is not there",
            picked.display()
        )));
    }
    let manifest = read_manifest(backup)?;
    let own = pc_folder(&root_in(&picked), &manifest.source_device_id)?;
    let name = backup
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.starts_with(&format!("{AUTO_BACKUP_PREFIX}-")))
        .ok_or_else(|| {
            VaultimeError::Backup("only automatic backups go to the sync folder".into())
        })?;

    let artwork = own.join(SYNC_ARTWORK_DIR);
    create_dir(&artwork)?;
    for file in manifest
        .files
        .iter()
        .filter(|file| is_artwork_path(&file.path) && is_checksum(&file.sha256))
    {
        let stored = artwork.join(&file.sha256);
        if !stored.is_file() {
            copy_in(&backup.join(&file.path), &stored)?;
        }
    }

    let target = own.join(name);
    if !target.join(BACKUP_MANIFEST_FILE).is_file() {
        let partial = own.join(format!(".{name}{PARTIAL}"));
        remove(&partial);
        create_dir(&partial)?;
        copy(&backup.join(DATABASE_FILE), &partial.join(DATABASE_FILE))?;
        // The manifest goes last, so a backup that has one is whole.
        copy(
            &backup.join(BACKUP_MANIFEST_FILE),
            &partial.join(BACKUP_MANIFEST_FILE),
        )?;
        remove(&target);
        fs::rename(&partial, &target).map_err(|error| {
            VaultimeError::Backup(format!("could not finish the sync folder backup: {error}"))
        })?;
    }
    write_pc_file(db, &own, &manifest);
    prune(&own, SYNC_BACKUP_KEEP);
    info!("backup copied into the sync folder at {}", target.display());
    Ok(())
}

fn copy(source: &Path, target: &Path) -> Result<()> {
    fs::copy(source, target).map(|_| ()).map_err(|error| {
        VaultimeError::Backup(format!(
            "could not copy {} into the sync folder: {error}",
            source.display()
        ))
    })
}

/// Copies through a partial file, so an image cut short never takes the name
/// of a whole one.
fn copy_in(source: &Path, target: &Path) -> Result<()> {
    let mut partial = target.as_os_str().to_owned();
    partial.push(PARTIAL);
    let partial = PathBuf::from(partial);
    copy(source, &partial)?;
    fs::rename(&partial, target).map_err(|error| {
        remove(&partial);
        VaultimeError::Backup(format!(
            "could not store an image in the sync folder: {error}"
        ))
    })
}

/// Deletes a file or folder that may not be there.
fn remove(path: &Path) {
    let removed = if path.is_dir() {
        fs::remove_dir_all(path)
    } else if path.exists() {
        fs::remove_file(path)
    } else {
        Ok(())
    };
    if let Err(error) = removed {
        warn!(
            "could not delete {} from the sync folder: {error}",
            path.display()
        );
    }
}

/// Names this PC for the others. A name that does not get there leaves its id.
fn write_pc_file(db: &Database, own: &Path, manifest: &LocalBackupManifest) {
    let file = PcFile {
        name: devices::get_device(db, &manifest.source_device_id)
            .ok()
            .and_then(|device| device.name),
        app_version: manifest.app_version.clone(),
    };
    let written = serde_json::to_string_pretty(&file)
        .map_err(|error| error.to_string())
        .and_then(|json| {
            fs::write(own.join(SYNC_PC_FILE), json).map_err(|error| error.to_string())
        });
    if let Err(error) = written {
        warn!("could not name this PC in the sync folder: {error}");
    }
}

/// The whole backups in a PC's folder, oldest first. Their names end in a
/// timestamp that sorts by time, and the manifest is copied in last.
fn backups_in(folder: &Path) -> Vec<PathBuf> {
    let prefix = format!("{AUTO_BACKUP_PREFIX}-");
    let mut backups: Vec<PathBuf> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .map(|entry| entry.path())
        .filter(|path| path.join(BACKUP_MANIFEST_FILE).is_file())
        .collect();
    backups.sort();
    backups
}

/// Deletes all but the newest `keep` backups in this PC's folder `own`, the
/// copies cut short, and the images no kept backup shows. Images go only
/// when every kept manifest reads, so none a kept backup shows goes.
fn prune(own: &Path, keep: usize) {
    let backups = backups_in(own);
    let surplus = backups.len().saturating_sub(keep);
    for backup in &backups[..surplus] {
        remove(backup);
    }
    for entry in fs::read_dir(own)
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && name.ends_with(PARTIAL) {
            remove(&entry.path());
        }
    }
    let mut shown = HashSet::new();
    for backup in &backups[surplus..] {
        let Ok(manifest) = read_manifest(backup) else {
            return;
        };
        shown.extend(
            manifest
                .files
                .into_iter()
                .filter(|file| is_artwork_path(&file.path))
                .map(|file| file.sha256),
        );
    }
    for entry in fs::read_dir(own.join(SYNC_ARTWORK_DIR))
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
    {
        if !shown.contains(entry.file_name().to_string_lossy().as_ref()) {
            remove(&entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::backup::auto;
    use crate::backup::testing::Pc;
    use crate::constants::ASSET_CACHE_DIR;

    /// Points `pc` at a sync folder and returns the folder backups go in.
    fn sync_folder(pc: &Pc, picked: &Path) -> PathBuf {
        fs::create_dir_all(picked).unwrap();
        settings::set_setting(&pc.db, SYNC_FOLDER_SETTING, &picked.to_string_lossy()).unwrap();
        root_in(picked)
    }

    fn art(pc: &Pc, game: &str, name: &str, content: &[u8]) {
        let folder = pc.assets.cache_dir().join(game);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(name), content).unwrap();
    }

    fn back_up(pc: &Pc) -> PathBuf {
        pc.act();
        PathBuf::from(
            auto::back_up_now(&pc.db, &pc.assets, &pc.context)
                .unwrap()
                .backup_path,
        )
    }

    fn images(own: &Path) -> usize {
        fs::read_dir(own.join(SYNC_ARTWORK_DIR)).unwrap().count()
    }

    #[test]
    fn the_sync_folder_is_one_named_vaultime() {
        assert_eq!(
            root_in(Path::new("/d/Dropbox")),
            Path::new("/d/Dropbox").join(SYNC_DIR)
        );
        assert_eq!(
            root_in(Path::new("/d/Dropbox/vaultime")),
            Path::new("/d/Dropbox/vaultime")
        );
        // Another PC picked the folder inside, named in small letters.
        let pc = Pc::new("laptop");
        let picked = pc.context.app_dir.join("Dropbox");
        fs::create_dir_all(picked.join("vaultime")).unwrap();
        assert_eq!(root_in(&picked), picked.join("vaultime"));
    }

    #[test]
    fn stores_each_image_once_and_keeps_the_newest_backups() {
        let laptop = Pc::new("laptop");
        let root = sync_folder(&laptop, &laptop.context.app_dir.join("Dropbox"));
        let game = laptop.game("Hades", None);
        art(&laptop, &game, "cover.jpg", b"cover");
        art(&laptop, &game, "banner.jpg", b"banner");
        let first = back_up(&laptop);
        publish(&laptop.db, &first).unwrap();

        let own = root.join("laptop");
        let copied = own.join(first.file_name().unwrap());
        assert!(copied.join(DATABASE_FILE).is_file());
        assert!(copied.join(BACKUP_MANIFEST_FILE).is_file());
        assert!(
            !copied.join(ASSET_CACHE_DIR).exists(),
            "artwork is kept apart"
        );
        assert_eq!(images(&own), 2);
        let named: PcFile =
            serde_json::from_str(&fs::read_to_string(own.join(SYNC_PC_FILE)).unwrap()).unwrap();
        assert_eq!(named.name.as_deref(), Some("laptop"));

        // A changed image comes in once, and the old one goes with the last
        // backup that shows it.
        art(&laptop, &game, "banner.jpg", b"new banner");
        for _ in 0..SYNC_BACKUP_KEEP {
            let backup = back_up(&laptop);
            publish(&laptop.db, &backup).unwrap();
        }
        assert_eq!(backups_in(&own).len(), SYNC_BACKUP_KEEP);
        assert!(!copied.exists(), "the oldest backup goes");
        assert_eq!(images(&own), 2);
    }

    #[test]
    fn the_daily_backup_goes_to_the_sync_folder_too() {
        let laptop = Pc::new("laptop");
        let root = sync_folder(&laptop, &laptop.context.app_dir.join("Vaultime"));
        laptop.act();
        auto::back_up_if_due(&laptop.db, &laptop.assets, &laptop.context, Duration::ZERO)
            .unwrap()
            .unwrap();
        assert_eq!(backups_in(&root.join("laptop")).len(), 1);
        assert!(
            !root.join(SYNC_DIR).exists(),
            "a picked Vaultime folder is used as it is"
        );
    }

    #[test]
    fn a_sync_folder_that_is_away_stays_away() {
        let laptop = Pc::new("laptop");
        let away = laptop.context.app_dir.join("nas");
        settings::set_setting(&laptop.db, SYNC_FOLDER_SETTING, &away.to_string_lossy()).unwrap();
        let backup = back_up(&laptop);
        assert!(publish(&laptop.db, &backup).is_err());
        assert!(!away.exists());
    }
}
