// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Backups in a sync folder, one that a sync service or a NAS keeps the same
//! on every PC. Each PC copies its
//! automatic backups into a folder of its own there, named by its id: the
//! database and the manifest as they are, and the artwork once by checksum,
//! so a sync service uploads an image only once however many backups show
//! it. A PC writes only its own folder and reads the others, whose backups
//! it puts together in staging, images back in place, and merges or
//! restores like any backup. One a sync service is still bringing in fails
//! its check there and waits. As with the cloud, the first backup of a PC
//! merges through the player and its newer ones by themselves, as long as
//! they need no choice of the player.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use log::{info, warn};
use serde::{Deserialize, Serialize};

use super::cloud_merge::{AutoMerge, automatic_choices};
use super::merge::{self, GameChoice, MergePreview, MergeSummary, instant};
use super::{
    BACKUP_MANIFEST_FILE, LocalBackupManifest, LocalBackupSummary, cleanup_staging_dir, create_dir,
    import_local_backup, inspect_local_backup, is_artwork_path, load_and_validate_manifest,
    read_manifest, resolve_manifest_file_path,
};
use crate::AppContext;
use crate::assets::{AssetManager, is_plain_name};
use crate::constants::{
    AUTO_BACKUP_PREFIX, BACKUP_VERSION, DATABASE_FILE, SYNC_ARTWORK_DIR, SYNC_BACKUP_KEEP,
    SYNC_DIR, SYNC_FOLDER_SETTING, SYNC_PC_FILE, SYNC_STAGING_DIR,
};
use crate::db::connection::Database;
use crate::db::models::Device;
use crate::db::repo::{devices, settings};
use crate::error::{Result, VaultimeError};
use crate::integrity::ledger;

/// Name ending of a backup or an image while it is copied in.
const PARTIAL: &str = ".partial";

/// Held while a backup from the sync folder is put together or let go, so no
/// call reads a copy another one is removing.
static STAGING: Mutex<()> = Mutex::new(());

fn staging() -> MutexGuard<'static, ()> {
    STAGING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The backups the last automatic merge left for the player, by PC, so the
/// next check does not put them together again.
static WAITING: LazyLock<Mutex<HashMap<String, AutoMerge>>> = LazyLock::new(Mutex::default);

/// The newest backup of a PC in the sync folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderPc {
    pub device_id: String,
    /// Its name from its folder, or as this PC knows it.
    pub name: Option<String>,
    pub backup_id: String,
    pub backup_created_at: String,
    pub sessions_count: i64,
    /// Whether the folder is this PC's own.
    pub this_pc: bool,
    /// Whether this PC merged a backup of it before, so its newer backups
    /// come in by themselves.
    pub merged_before: bool,
    /// Whether the backup is newer than the last one merged here.
    pub newer: bool,
}

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
        VaultimeError::Backup(format!("could not copy {}: {error}", source.display()))
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

/// The newest backup in a PC's folder whose manifest reads, with it.
fn newest_backup(folder: &Path) -> Option<(PathBuf, LocalBackupManifest)> {
    backups_in(folder).into_iter().rev().find_map(|backup| {
        read_manifest(&backup)
            .ok()
            .map(|manifest| (backup, manifest))
    })
}

/// The newest backup of every PC in the sync folder, this one's too. A
/// folder counts only with the backups of the PC it is named after.
pub fn list_pcs(db: &Database) -> Result<Vec<FolderPc>> {
    let Some(root) = root(db) else {
        return Ok(Vec::new());
    };
    let this = ledger::this_device()?;
    let merged = merge::last_merged(db);
    let known: HashMap<String, Device> = devices::list_devices(db)?
        .into_iter()
        .map(|device| (device.id.clone(), device))
        .collect();
    let mut pcs = Vec::new();
    for entry in fs::read_dir(&root)
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
    {
        let device_id = entry.file_name().to_string_lossy().into_owned();
        if !is_plain_name(&device_id) {
            continue;
        }
        let Some((_, manifest)) = newest_backup(&entry.path()) else {
            continue;
        };
        if manifest.source_device_id != device_id {
            continue;
        }
        let named = fs::read_to_string(entry.path().join(SYNC_PC_FILE))
            .ok()
            .and_then(|json| serde_json::from_str::<PcFile>(&json).ok())
            .and_then(|file| file.name);
        let last = merged.get(&device_id).map(|created| instant(created));
        pcs.push(FolderPc {
            name: named.or_else(|| known.get(&device_id).and_then(|device| device.name.clone())),
            backup_id: manifest.backup_id,
            sessions_count: manifest.sessions_count,
            this_pc: device_id == this,
            merged_before: last.is_some(),
            newer: last.is_none_or(|last| instant(&manifest.created_at) > last),
            backup_created_at: manifest.created_at,
            device_id,
        });
    }
    pcs.sort_by(|a, b| {
        (a.name.as_deref().unwrap_or(&a.device_id)).cmp(b.name.as_deref().unwrap_or(&b.device_id))
    });
    Ok(pcs)
}

/// Where a backup of `device_id` from the sync folder is put together.
fn staging_folder(app_context: &AppContext, device_id: &str) -> Result<PathBuf> {
    if !is_plain_name(device_id) {
        return Err(VaultimeError::Invalid(
            "Not a PC of the sync folder.".into(),
        ));
    }
    Ok(app_context.app_dir.join(SYNC_STAGING_DIR).join(device_id))
}

/// The newest backup of `device_id` in the sync folder, put together in
/// staging with its images back in place, and checked. With `reuse`, a copy
/// put together before stays, so a merge takes the backup its preview showed.
fn staged(
    db: &Database,
    app_context: &AppContext,
    device_id: &str,
    reuse: bool,
) -> Result<PathBuf> {
    let folder = staging_folder(app_context, device_id)?;
    let staged = folder.join("backup");
    if reuse && staged.join(BACKUP_MANIFEST_FILE).is_file() {
        return Ok(staged);
    }
    let root = root(db).ok_or_else(|| VaultimeError::Invalid("No sync folder is set.".into()))?;
    let own = pc_folder(&root, device_id)?;
    let (newest, manifest) = newest_backup(&own).ok_or_else(|| {
        VaultimeError::Invalid("That PC has no backup in the sync folder.".into())
    })?;
    cleanup_staging_dir(&folder);
    let put_together = (|| {
        create_dir(&staged)?;
        copy(&newest.join(DATABASE_FILE), &staged.join(DATABASE_FILE))?;
        copy(
            &newest.join(BACKUP_MANIFEST_FILE),
            &staged.join(BACKUP_MANIFEST_FILE),
        )?;
        for file in manifest
            .files
            .iter()
            .filter(|file| is_artwork_path(&file.path) && is_checksum(&file.sha256))
        {
            // An image the sync service has not brought yet is left out, as a
            // damaged one is.
            let stored = own.join(SYNC_ARTWORK_DIR).join(&file.sha256);
            if stored.is_file() {
                let target = resolve_manifest_file_path(&staged, &file.path)?;
                if let Some(parent) = target.parent() {
                    create_dir(parent)?;
                }
                copy(&stored, &target)?;
            }
        }
        load_and_validate_manifest(&staged).map(|_| ()).map_err(|_| {
            let other_version = read_manifest(&staged)
                .is_ok_and(|manifest| manifest.backup_version != BACKUP_VERSION);
            VaultimeError::Invalid(
                if other_version {
                    "The newest backup of that PC comes from a Vaultime this one cannot read. Update Vaultime on both PCs."
                } else {
                    "The newest backup of that PC is not whole in the sync folder yet. It may still be syncing."
                }
                .into(),
            )
        })
    })();
    if let Err(error) = put_together {
        cleanup_staging_dir(&folder);
        return Err(error);
    }
    Ok(staged)
}

fn discard_staged(app_context: &AppContext, device_id: &str) {
    if let Ok(folder) = staging_folder(app_context, device_id)
        && folder.exists()
    {
        cleanup_staging_dir(&folder);
    }
}

/// What merging the newest sync folder backup of `device_id` would bring in.
/// It stays put together until it is merged or let go.
pub fn preview_merge(
    db: &Database,
    app_context: &AppContext,
    device_id: &str,
    trust_new_keys: bool,
) -> Result<MergePreview> {
    let _staging = staging();
    let staged = staged(db, app_context, device_id, false)?;
    merge::preview(db, app_context, &staged, trust_new_keys)
}

/// Merges the backup the preview put together with the player's choices, as
/// `merge::apply` does, and lets it go.
pub fn merge_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    device_id: &str,
    choices: &[GameChoice],
    trust_new_keys: bool,
) -> Result<MergeSummary> {
    let _staging = staging();
    let staged = staged(db, app_context, device_id, true)?;
    let summary = merge::apply(
        db,
        asset_manager,
        app_context,
        &staged,
        choices,
        trust_new_keys,
    );
    discard_staged(app_context, device_id);
    summary
}

/// Lets the backup of `device_id` a preview put together go.
pub fn discard(app_context: &AppContext, device_id: &str) {
    let _staging = staging();
    discard_staged(app_context, device_id);
}

/// The newest sync folder backup of `device_id` as a restore would see it.
pub fn inspect(
    db: &Database,
    app_context: &AppContext,
    device_id: &str,
) -> Result<LocalBackupSummary> {
    let _staging = staging();
    let staged = staged(db, app_context, device_id, false)?;
    inspect_local_backup(db, &staged)
}

/// Restores the backup `inspect` put together and lets it go. `take_over`
/// works as in `import_local_backup`.
pub fn restore(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    device_id: &str,
    take_over: bool,
) -> Result<LocalBackupSummary> {
    let _staging = staging();
    let staged = staged(db, app_context, device_id, true)?;
    let summary = import_local_backup(db, asset_manager, app_context, &staged, take_over);
    discard_staged(app_context, device_id);
    summary
}

/// Merges the newest sync folder backup of every PC merged here before that
/// has a newer one, as far as it needs no choice of the player. Returns what
/// it did, with the backups that still wait for the player.
pub fn merge_due(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
) -> Result<Vec<AutoMerge>> {
    let _staging = staging();
    let pcs = list_pcs(db)?;
    let mut waiting = WAITING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut done = Vec::new();
    for pc in pcs
        .into_iter()
        .filter(|pc| !pc.this_pc && pc.merged_before && pc.newer)
    {
        if let Some(waited) = waiting
            .get(&pc.device_id)
            .filter(|waited| waited.backup_id == pc.backup_id)
        {
            done.push(waited.clone());
            continue;
        }
        let Some(outcome) = merge_one(db, asset_manager, app_context, pc) else {
            continue;
        };
        if outcome.outcome == "merged" {
            waiting.remove(&outcome.device_id);
        } else {
            waiting.insert(outcome.device_id.clone(), outcome.clone());
        }
        done.push(outcome);
    }
    Ok(done)
}

/// Merges the newest backup of `pc` by itself, or notes why it waits for the
/// player. None when it could not be read, which the next check tries again.
fn merge_one(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    pc: FolderPc,
) -> Option<AutoMerge> {
    // A preview the player has open keeps the copy it shows. The next check
    // merges.
    if staging_folder(app_context, &pc.device_id)
        .is_ok_and(|folder| folder.join("backup").join(BACKUP_MANIFEST_FILE).is_file())
    {
        return None;
    }
    let merged = staged(db, app_context, &pc.device_id, false)
        .and_then(|staged| merge_by_itself(db, asset_manager, app_context, &staged));
    discard_staged(app_context, &pc.device_id);
    let mut outcome = AutoMerge {
        device_id: pc.device_id,
        name: pc.name,
        backup_id: pc.backup_id,
        outcome: "merged",
        sessions_added: 0,
        sessions_grown: 0,
    };
    match merged {
        Ok(Ok(summary)) => {
            info!(
                "merged the sync folder backup of {} by itself: {} new, {} grown",
                outcome.device_id, summary.sessions_added, summary.sessions_grown
            );
            outcome.sessions_added = summary.sessions_added;
            outcome.sessions_grown = summary.sessions_grown;
        }
        Ok(Err(reason)) => {
            info!(
                "the sync folder backup of {} waits for the player: {reason}",
                outcome.device_id
            );
            outcome.outcome = reason;
        }
        Err(error) => {
            warn!(
                "the sync folder backup of {} did not merge: {error}",
                outcome.device_id
            );
            return None;
        }
    }
    Some(outcome)
}

/// Merges the staged backup without the player, or says why it needs them:
/// `new_key` or `failing`.
fn merge_by_itself(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    staged: &Path,
) -> Result<std::result::Result<MergeSummary, &'static str>> {
    let preview = merge::preview(db, app_context, staged, false)?;
    if preview.unknown_keys {
        return Ok(Err("new_key"));
    }
    if preview.failing > 0 || preview.own_failing > 0 {
        return Ok(Err("failing"));
    }
    let choices = automatic_choices(&preview);
    merge::apply(db, asset_manager, app_context, staged, &choices, false).map(Ok)
}

/// Clears what an earlier run left in staging.
pub fn sweep(app_dir: &Path) {
    let folder = app_dir.join(SYNC_STAGING_DIR);
    if folder.exists()
        && let Err(error) = fs::remove_dir_all(&folder)
    {
        warn!("could not clear {}: {error}", folder.display());
    }
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
    use crate::db::repo::sessions;

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

    fn link(game: &str) -> GameChoice {
        GameChoice {
            game_id: game.into(),
            linked_game_id: None,
        }
    }

    /// Two PCs that share one sync folder, with the folder.
    fn two_pcs() -> (Pc, Pc, PathBuf) {
        let laptop = Pc::new("laptop");
        let desktop = Pc::new("desktop");
        let shared = laptop.context.app_dir.join("Dropbox");
        let root = sync_folder(&laptop, &shared);
        sync_folder(&desktop, &shared);
        (laptop, desktop, root)
    }

    #[test]
    fn another_pc_finds_and_merges_a_backup_through_the_folder() {
        let (laptop, desktop, _) = two_pcs();
        let game = laptop.game("Hades", Some("1145360"));
        art(&laptop, &game, "cover.jpg", b"cover");
        let first = laptop.play(&game, 30);
        publish(&laptop.db, &back_up(&laptop)).unwrap();

        desktop.act();
        let pcs = list_pcs(&desktop.db).unwrap();
        assert_eq!(pcs.len(), 1);
        assert_eq!(pcs[0].device_id, "laptop");
        assert_eq!(pcs[0].name.as_deref(), Some("laptop"));
        assert_eq!(pcs[0].sessions_count, 1);
        assert!(!pcs[0].this_pc && !pcs[0].merged_before && pcs[0].newer);

        let preview = preview_merge(&desktop.db, &desktop.context, "laptop", false).unwrap();
        assert_eq!((preview.new_sessions, preview.failing), (1, 0));
        let staged = staging_folder(&desktop.context, "laptop")
            .unwrap()
            .join("backup");
        assert_eq!(
            fs::read(staged.join(ASSET_CACHE_DIR).join(&game).join("cover.jpg")).unwrap(),
            b"cover",
            "the images are back in place"
        );

        let summary = merge_backup(
            &desktop.db,
            &desktop.assets,
            &desktop.context,
            "laptop",
            &[link(&game)],
            false,
        )
        .unwrap();
        assert_eq!(summary.sessions_added, 1);
        assert_eq!(desktop.events(&first), laptop.events(&first));
        assert!(!staged.exists(), "the merged copy goes");

        desktop.act();
        publish(&desktop.db, &back_up(&desktop)).unwrap();
        let pcs = list_pcs(&desktop.db).unwrap();
        assert_eq!(pcs.len(), 2);
        let laptop_row = pcs.iter().find(|pc| pc.device_id == "laptop").unwrap();
        assert!(laptop_row.merged_before && !laptop_row.newer);
        assert!(pcs.iter().any(|pc| pc.device_id == "desktop" && pc.this_pc));
    }

    #[test]
    fn newer_backups_of_a_merged_pc_come_in_by_themselves() {
        let (laptop, desktop, _) = two_pcs();
        let game = laptop.game("Hades", Some("1145360"));
        laptop.play(&game, 30);
        publish(&laptop.db, &back_up(&laptop)).unwrap();

        desktop.act();
        let due = || merge_due(&desktop.db, &desktop.assets, &desktop.context).unwrap();
        assert!(
            due().is_empty(),
            "the first backup of a PC waits for the player"
        );
        preview_merge(&desktop.db, &desktop.context, "laptop", false).unwrap();
        merge_backup(
            &desktop.db,
            &desktop.assets,
            &desktop.context,
            "laptop",
            &[link(&game)],
            false,
        )
        .unwrap();

        let second = laptop.play(&game, 45);
        publish(&laptop.db, &back_up(&laptop)).unwrap();
        desktop.act();
        let done = due();
        assert_eq!(done.len(), 1);
        assert_eq!((done[0].outcome, done[0].sessions_added), ("merged", 1));
        assert_eq!(desktop.events(&second), laptop.events(&second));
        assert!(due().is_empty(), "a merged backup is not merged again");

        laptop.play(&game, 20);
        publish(&laptop.db, &back_up(&laptop)).unwrap();
        desktop.act();
        preview_merge(&desktop.db, &desktop.context, "laptop", false).unwrap();
        assert!(due().is_empty(), "an open preview keeps its backup");
        discard(&desktop.context, "laptop");
        assert_eq!(due().len(), 1);
    }

    #[test]
    fn a_new_pc_restores_another_ones_backup_from_the_folder() {
        let (laptop, desktop, _) = two_pcs();
        let game = laptop.game("Hades", None);
        let first = laptop.play(&game, 30);
        publish(&laptop.db, &back_up(&laptop)).unwrap();

        desktop.act();
        let seen = inspect(&desktop.db, &desktop.context, "laptop").unwrap();
        assert_eq!((seen.sessions_count, seen.games_count), (1, 1));
        restore(
            &desktop.db,
            &desktop.assets,
            &desktop.context,
            "laptop",
            false,
        )
        .unwrap();
        let restored = sessions::list_all_sessions(&desktop.db).unwrap();
        assert!(restored.iter().any(|session| session.id == first));
        assert!(!staging_folder(&desktop.context, "laptop").unwrap().exists());
    }

    #[test]
    fn a_backup_still_syncing_waits() {
        let (laptop, desktop, root) = two_pcs();
        let backup = back_up(&laptop);
        publish(&laptop.db, &backup).unwrap();
        let copied = root.join("laptop").join(backup.file_name().unwrap());
        fs::write(copied.join(DATABASE_FILE), b"half of a database").unwrap();

        desktop.act();
        let waiting = preview_merge(&desktop.db, &desktop.context, "laptop", false).unwrap_err();
        assert!(waiting.to_string().contains("still be syncing"));
        assert!(!staging_folder(&desktop.context, "laptop").unwrap().exists());

        // A backup of another backup version says so instead.
        let manifest = copied.join(BACKUP_MANIFEST_FILE);
        let text = fs::read_to_string(&manifest).unwrap();
        let newer = text.replace(
            &format!("\"backup_version\": {BACKUP_VERSION}"),
            "\"backup_version\": 99",
        );
        assert_ne!(newer, text);
        fs::write(&manifest, newer).unwrap();
        let other = preview_merge(&desktop.db, &desktop.context, "laptop", false).unwrap_err();
        assert!(other.to_string().contains("Update Vaultime on both PCs"));
    }

    #[test]
    fn a_folder_counts_only_with_backups_of_its_own_pc() {
        let (laptop, desktop, root) = two_pcs();
        let backup = back_up(&laptop);
        publish(&laptop.db, &backup).unwrap();
        fs::rename(root.join("laptop"), root.join("tablet")).unwrap();

        desktop.act();
        assert!(list_pcs(&desktop.db).unwrap().is_empty());
        assert!(preview_merge(&desktop.db, &desktop.context, "../laptop", false).is_err());
    }

    #[test]
    fn picking_a_sync_folder_copies_a_backup_right_away() {
        let laptop = Pc::new("laptop");
        let root = sync_folder(&laptop, &laptop.context.app_dir.join("Dropbox"));
        laptop.act();
        auto::back_up_to_sync_folder(&laptop.db, &laptop.assets, &laptop.context).unwrap();
        assert_eq!(backups_in(&root.join("laptop")).len(), 1);
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
