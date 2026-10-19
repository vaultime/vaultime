// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! What happens when Vaultime cannot start: the database of a newer version,
//! a damaged database file or a migration that failed. The player decides
//! before anything changes, and a database set aside stays next to the new
//! one, so nothing is lost.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use log::{error, info, warn};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
use rusqlite::{Connection, OpenFlags};

use crate::AppContext;
use crate::assets::AssetManager;
use crate::backup::{self, BACKUP_MANIFEST_FILE, auto};
use crate::constants::{AUTO_BACKUP_DIR, AUTO_BACKUP_FOLDER_SETTING, CLOUD_API_BASE_URL};
use crate::db::connection::Database;
use crate::db::migrate;
use crate::error::VaultimeError;

const TITLE: &str = "Vaultime";
const RESTORE: &str = "Restore the backup";
const QUIT: &str = "Quit";
/// The files of a database in WAL mode, besides the database itself.
const DATABASE_SIDE_FILES: [&str; 2] = ["-wal", "-shm"];

/// Says why Vaultime cannot start and quits. Nothing runs yet at this point,
/// so quitting leaves nothing half done.
pub fn fatal(message: &str) -> ! {
    error!("{message}");
    MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title(TITLE)
        .set_description(message)
        .set_buttons(MessageButtons::Ok)
        .show();
    std::process::exit(1);
}

/// Opens the database. When it does not open, the player may set it aside
/// and start from the newest automatic backup. Quits when there is none or
/// the player declines.
pub fn open_database(context: &AppContext, assets: &AssetManager, db_path: &Path) -> Database {
    match open_or_restore(context, assets, db_path, ask_to_restore) {
        Ok((database, None)) => database,
        Ok((database, Some(note))) => {
            MessageDialog::new()
                .set_level(MessageLevel::Info)
                .set_title(TITLE)
                .set_description(note)
                .set_buttons(MessageButtons::Ok)
                .show();
            database
        }
        Err(Stop::Declined) => std::process::exit(1),
        Err(Stop::Fatal(message)) => fatal(&message),
    }
}

/// Why Vaultime does not start after all.
#[derive(Debug, PartialEq, Eq)]
enum Stop {
    /// The player chose to quit.
    Declined,
    /// What to tell the player before quitting.
    Fatal(String),
}

/// The steps of `open_database`, with `ask` deciding on the restore. Returns
/// the database and, after a restore, what to tell the player.
fn open_or_restore(
    context: &AppContext,
    assets: &AssetManager,
    db_path: &Path,
    ask: impl FnOnce(&str, &Path) -> bool,
) -> Result<(Database, Option<String>), Stop> {
    let failure = match Database::open(db_path) {
        Ok(database) => return Ok((database, None)),
        Err(failure) => failure,
    };
    error!("the database did not open: {failure}");
    // Nothing is wrong with a database another program holds, so it is not
    // set aside for an older backup.
    if matches!(failure, VaultimeError::Busy(_)) {
        return Err(Stop::Fatal(format!(
            "Another program is using Vaultime's database, like a sync tool or a virus scanner. Close it or wait a moment, then start Vaultime again.\n\nThe database is {}.",
            db_path.display()
        )));
    }

    let readable = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok();
    if let Some(newer) = readable
        .as_ref()
        .and_then(|conn| migrate::newer_migration(conn).ok().flatten())
    {
        return Err(Stop::Fatal(format!(
            "This Vaultime is older than the one that last used its data (it added {newer}). Install the newest Vaultime from {CLOUD_API_BASE_URL} and start it again."
        )));
    }
    let folder = backup_folder(readable.as_ref(), &context.app_dir);
    // Closed before the file is set aside, which Windows refuses while open.
    drop(readable);

    let mut reason = failure.user_message();
    if !reason.ends_with('.') {
        reason.push('.');
    }
    let Some(newest) = auto::newest_complete(&folder) else {
        return Err(Stop::Fatal(format!(
            "Vaultime could not open its database. {reason}\n\nThere is no automatic backup in {} to start from. The database is {}.",
            folder.display(),
            db_path.display()
        )));
    };
    if !ask(&reason, &newest) {
        return Err(Stop::Declined);
    }

    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let aside = set_aside(db_path, &stamp).map_err(|problem| {
        Stop::Fatal(format!(
            "Vaultime could not set its database aside. {problem}\n\nNothing changed. The database is {}.",
            db_path.display()
        ))
    })?;
    match restore_into_new(context, assets, db_path, &newest) {
        Ok(database) => {
            info!(
                "started from {} after the database did not open, the old one is {}",
                newest.display(),
                aside.display()
            );
            let note = format!(
                "Vaultime started from the automatic backup of {}.\n\nThe database it could not open is kept as {}.",
                saved_at(&newest),
                aside.display()
            );
            Ok((database, Some(note)))
        }
        Err(problem) => {
            put_back(&aside, db_path);
            Err(Stop::Fatal(format!(
                "The automatic backup of {} did not restore. {}\n\nNothing changed. The database is {}.",
                saved_at(&newest),
                problem.user_message(),
                db_path.display()
            )))
        }
    }
}

/// Asks whether to start from `backup`. True when the player agreed.
fn ask_to_restore(reason: &str, backup: &Path) -> bool {
    let answer = MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title(TITLE)
        .set_description(format!(
            "Vaultime could not open its database. {reason}\n\nIt can set the database aside and start from the newest automatic backup, saved {}. Play recorded after that stays only in the database set aside.\n\nWhen another program may be using the file, quit and start Vaultime again later.",
            saved_at(backup)
        ))
        .set_buttons(MessageButtons::OkCancelCustom(RESTORE.into(), QUIT.into()))
        .show();
    // Linux answers Ok for the first of two custom buttons.
    match answer {
        MessageDialogResult::Ok => true,
        MessageDialogResult::Custom(label) => label == RESTORE,
        _ => false,
    }
}

/// Where automatic backups go, read from the database that did not open
/// where it still answers.
fn backup_folder(readable: Option<&Connection>, app_dir: &Path) -> PathBuf {
    readable
        .and_then(|conn| {
            conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [AUTO_BACKUP_FOLDER_SETTING],
                |row| row.get::<_, String>(0),
            )
            .ok()
        })
        .filter(|folder| !folder.trim().is_empty())
        .map_or_else(|| app_dir.join(AUTO_BACKUP_DIR), PathBuf::from)
}

/// When a backup finished, from its manifest, which is written last.
fn saved_at(backup: &Path) -> String {
    fs::metadata(backup.join(BACKUP_MANIFEST_FILE))
        .and_then(|metadata| metadata.modified())
        .map_or_else(
            |_| "an unknown time".into(),
            |modified| {
                DateTime::<Local>::from(modified)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            },
        )
}

/// Renames the database with `stamp`, and its side files to match, so the
/// copy set aside opens with everything it had. Returns its new name. When
/// one of them cannot be renamed, the ones renamed already go back, so
/// nothing changed.
fn set_aside(db_path: &Path, stamp: &str) -> std::io::Result<PathBuf> {
    let aside = PathBuf::from(format!("{}.damaged-{stamp}", db_path.display()));
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for suffix in std::iter::once("").chain(DATABASE_SIDE_FILES) {
        let (from, to) = (with_suffix(db_path, suffix), with_suffix(&aside, suffix));
        if !suffix.is_empty() && !from.exists() {
            continue;
        }
        if let Err(problem) = fs::rename(&from, &to) {
            for (from, to) in moved.iter().rev() {
                if let Err(undo) = fs::rename(to, from) {
                    warn!("could not put {} back: {undo}", from.display());
                }
            }
            return Err(problem);
        }
        moved.push((from, to));
    }
    Ok(aside)
}

/// Undoes `set_aside` once the new database is closed: its files go and the
/// ones set aside come back.
fn put_back(aside: &Path, db_path: &Path) {
    for suffix in std::iter::once("").chain(DATABASE_SIDE_FILES) {
        let new = with_suffix(db_path, suffix);
        if let Err(problem) = fs::remove_file(&new)
            && problem.kind() != std::io::ErrorKind::NotFound
        {
            warn!("could not remove {}: {problem}", new.display());
        }
        let old = with_suffix(aside, suffix);
        if old.exists()
            && let Err(problem) = fs::rename(&old, &new)
        {
            warn!(
                "could not put {} back as {}: {problem}",
                old.display(),
                new.display()
            );
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", path.display()))
}

/// Restores `backup` into a new database at `db_path`. The new database is
/// closed again when the restore fails.
fn restore_into_new(
    context: &AppContext,
    assets: &AssetManager,
    db_path: &Path,
    backup: &Path,
) -> crate::error::Result<Database> {
    let database = Database::open(db_path)?;
    // A restore keeps the settings that stay with a PC from the database it
    // replaces, here a new one without any. A backup of this PC has them.
    if backup::inspect_local_backup(&database, backup)?.source_device_id == context.device_id {
        backup::take_device_settings(&database, backup)?;
    }
    backup::import_local_backup(&database, assets, context, backup, false)?;
    Ok(database)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> PathBuf {
        let root = std::env::temp_dir().join(format!("vaultime-recovery-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// A data folder whose database holds one finished session and has an
    /// automatic backup of it.
    fn played_and_backed_up() -> (AppContext, AssetManager, PathBuf) {
        crate::integrity::ledger::act_as(Some(&crate::integrity::ledger::test_pc("test-device")));
        let context = AppContext {
            app_dir: folder(),
            device_id: "test-device".into(),
            app_version: "0.1.0".into(),
        };
        let assets = AssetManager::new(context.app_dir.join(crate::constants::ASSET_CACHE_DIR));
        let db_path = context.app_dir.join(crate::constants::DATABASE_FILE);
        let db = Database::open(&db_path).unwrap();
        crate::db::repo::devices::ensure_device(&db, &context.device_id, "test", "0.1.0").unwrap();
        let game = crate::db::repo::games::create_game(
            &db,
            &crate::db::models::CreateGame {
                title: "Rescued".into(),
                executable_path: Some("rescued.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        let session =
            crate::db::repo::sessions::create_session(&db, &game.id, &context.device_id).unwrap();
        crate::db::repo::sessions::end_session(
            &db,
            &session.id,
            60_000,
            60_000,
            0,
            crate::integrity::STATUS_LOCAL,
        )
        .unwrap();
        crate::db::repo::settings::set_setting(&db, crate::constants::WINDOW_SIZE_SETTING, "large")
            .unwrap();
        auto::back_up_now(&db, &assets, &context).unwrap();
        (context, assets, db_path)
    }

    fn damage(db_path: &Path) {
        fs::write(db_path, "this is no database").unwrap();
        for suffix in DATABASE_SIDE_FILES {
            let _ = fs::remove_file(with_suffix(db_path, suffix));
        }
    }

    #[test]
    fn a_damaged_database_is_set_aside_for_the_newest_automatic_backup() {
        let (context, assets, db_path) = played_and_backed_up();
        damage(&db_path);

        let mut asked = None;
        let (database, note) = open_or_restore(&context, &assets, &db_path, |reason, backup| {
            asked = Some((reason.to_owned(), backup.to_path_buf()));
            true
        })
        .unwrap();
        let (reason, backup) = asked.unwrap();
        assert!(reason.contains("not a database"), "{reason}");
        assert!(backup.starts_with(context.app_dir.join(AUTO_BACKUP_DIR)));
        let restored = crate::db::repo::sessions::list_all_sessions(&database).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].runtime_ms, 60_000);
        assert_eq!(restored[0].integrity_status, crate::integrity::STATUS_LOCAL);
        // The window size stays with this PC, as in the backup.
        assert_eq!(
            crate::db::repo::settings::get_setting(
                &database,
                crate::constants::WINDOW_SIZE_SETTING
            )
            .unwrap()
            .as_deref(),
            Some("large")
        );
        let note = note.unwrap();
        let aside: Vec<_> = fs::read_dir(&context.app_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".damaged-"))
            .collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(
            fs::read_to_string(aside[0].path()).unwrap(),
            "this is no database"
        );
        assert!(note.contains(&*aside[0].path().to_string_lossy()), "{note}");
        drop(database);
        fs::remove_dir_all(&context.app_dir).unwrap();
    }

    #[test]
    fn declining_or_a_failed_restore_changes_nothing() {
        let (context, assets, db_path) = played_and_backed_up();
        damage(&db_path);
        let files = |dir: &Path| {
            let mut names: Vec<_> = fs::read_dir(dir)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.file_name())
                .collect();
            names.sort();
            names
        };
        let before = files(&context.app_dir);

        let declined = open_or_restore(&context, &assets, &db_path, |_, _| false).err();
        assert_eq!(declined, Some(Stop::Declined));
        assert_eq!(files(&context.app_dir), before);

        // A backup whose database was damaged as well does not restore.
        let backup = auto::newest_complete(&context.app_dir.join(AUTO_BACKUP_DIR)).unwrap();
        fs::write(backup.join(crate::constants::DATABASE_FILE), "damaged too").unwrap();
        let failed = open_or_restore(&context, &assets, &db_path, |_, _| true).err();
        assert!(
            matches!(&failed, Some(Stop::Fatal(message)) if message.contains("did not restore")),
            "{failed:?}"
        );
        assert_eq!(files(&context.app_dir), before);
        assert_eq!(fs::read_to_string(&db_path).unwrap(), "this is no database");
        fs::remove_dir_all(&context.app_dir).unwrap();
    }

    #[test]
    fn a_database_of_a_newer_version_is_never_replaced() {
        let (context, assets, db_path) = played_and_backed_up();
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute(
                "INSERT INTO _migrations (name) VALUES ('9999_from_a_newer_version')",
                [],
            )
            .unwrap();
        }
        let refused = open_or_restore(&context, &assets, &db_path, |_, _| {
            panic!("a newer database must not be offered for a restore")
        })
        .err();
        assert!(
            matches!(&refused, Some(Stop::Fatal(message)) if message.contains("older than the one")),
            "{refused:?}"
        );
        fs::remove_dir_all(&context.app_dir).unwrap();
    }

    #[test]
    fn without_an_automatic_backup_it_only_says_where_the_database_is() {
        let context = AppContext {
            app_dir: folder(),
            device_id: "test-device".into(),
            app_version: "0.1.0".into(),
        };
        let assets = AssetManager::new(context.app_dir.join(crate::constants::ASSET_CACHE_DIR));
        let db_path = context.app_dir.join(crate::constants::DATABASE_FILE);
        damage(&db_path);
        let stopped = open_or_restore(&context, &assets, &db_path, |_, _| true).err();
        assert!(
            matches!(&stopped, Some(Stop::Fatal(message)) if message.contains("no automatic backup")),
            "{stopped:?}"
        );
        assert_eq!(fs::read_to_string(&db_path).unwrap(), "this is no database");
        fs::remove_dir_all(&context.app_dir).unwrap();
    }

    #[test]
    fn a_database_set_aside_comes_back_whole() {
        let root = folder();
        let db_path = root.join("vaultime.db");
        fs::write(&db_path, "damaged").unwrap();
        fs::write(with_suffix(&db_path, "-wal"), "log").unwrap();

        let aside = set_aside(&db_path, "20261020T000000Z").unwrap();
        assert!(!db_path.exists());
        assert_eq!(fs::read_to_string(&aside).unwrap(), "damaged");
        assert_eq!(
            fs::read_to_string(with_suffix(&aside, "-wal")).unwrap(),
            "log"
        );

        // A new database that failed to restore goes, the old one returns.
        fs::write(&db_path, "new").unwrap();
        fs::write(with_suffix(&db_path, "-shm"), "new").unwrap();
        put_back(&aside, &db_path);
        assert_eq!(fs::read_to_string(&db_path).unwrap(), "damaged");
        assert_eq!(
            fs::read_to_string(with_suffix(&db_path, "-wal")).unwrap(),
            "log"
        );
        assert!(!with_suffix(&db_path, "-shm").exists());
        assert!(!aside.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_database_that_cannot_be_set_aside_whole_stays_as_it_was() {
        let root = folder();
        let db_path = root.join("vaultime.db");
        fs::write(&db_path, "damaged").unwrap();
        fs::write(with_suffix(&db_path, "-wal"), "log").unwrap();
        // Something in the way of the log's new name.
        let stamp = "20261020T000000Z";
        fs::create_dir_all(root.join(format!("vaultime.db.damaged-{stamp}-wal/inside"))).unwrap();

        assert!(set_aside(&db_path, stamp).is_err());
        assert_eq!(fs::read_to_string(&db_path).unwrap(), "damaged");
        assert_eq!(
            fs::read_to_string(with_suffix(&db_path, "-wal")).unwrap(),
            "log"
        );
        assert!(!root.join(format!("vaultime.db.damaged-{stamp}")).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_database_another_program_holds_is_not_offered_for_a_restore() {
        let (context, assets, db_path) = played_and_backed_up();
        // A program that keeps the file to itself, so not even reading works.
        let holder = Connection::open(&db_path).unwrap();
        holder
            .execute_batch("PRAGMA locking_mode=EXCLUSIVE; BEGIN EXCLUSIVE; CREATE TABLE held (x);")
            .unwrap();
        let refused = open_or_restore(&context, &assets, &db_path, |_, _| {
            panic!("a busy database must not be offered for a restore")
        })
        .err();
        assert!(
            matches!(&refused, Some(Stop::Fatal(message)) if message.contains("Another program")),
            "{refused:?}"
        );
        holder.execute_batch("ROLLBACK;").unwrap();
        drop(holder);
        fs::remove_dir_all(&context.app_dir).unwrap();
    }

    #[test]
    fn the_backup_folder_comes_from_the_database_where_it_answers() {
        let root = folder();
        let db_path = root.join("vaultime.db");
        assert_eq!(backup_folder(None, &root), root.join(AUTO_BACKUP_DIR));
        {
            let db = Database::open(&db_path).unwrap();
            crate::db::repo::settings::set_setting(&db, AUTO_BACKUP_FOLDER_SETTING, "D:\\Backups")
                .unwrap();
        }
        let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(
            backup_folder(Some(&conn), &root),
            PathBuf::from("D:\\Backups")
        );
        drop(conn);
        fs::remove_dir_all(root).unwrap();
    }
}
