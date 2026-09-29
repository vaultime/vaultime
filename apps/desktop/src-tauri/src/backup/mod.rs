// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Local backup export, inspection and restore.

pub mod auto;
mod crypto;
pub mod remote;

use std::fmt::Write;
use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

use log::warn;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::AppContext;
use crate::assets::{AssetManager, is_plain_name};
use crate::constants::{AUTO_BACKUP_FOLDER_SETTING, BACKUP_VERSION, HASH_BUFFER_BYTES};
use crate::db::connection::Database;
use crate::db::migrate::known_migrations;
use crate::db::repo::{devices, sessions};
use crate::error::{Result, VaultimeError};
use crate::integrity;
use crate::platform::process::file_name;

const BACKUP_DIR_PREFIX: &str = "vaultime-backup";
const BACKUP_DB_FILE: &str = "vaultime.db";
pub(crate) const BACKUP_MANIFEST_FILE: &str = "manifest.json";
pub(crate) const BACKUP_ASSET_DIR: &str = "asset-cache";
/// Settings that belong to this PC. A restore keeps the local values, so a
/// backup cannot send the daily backups to a folder of its choosing.
const DEVICE_SETTINGS: &[&str] = &[AUTO_BACKUP_FOLDER_SETTING];
/// Why sessions that a backup caught while they ran are closed after a restore.
const RESTORED_OPEN_SESSION_REASON: &str = "restored_while_running";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalBackupSummary {
    pub backup_id: String,
    pub backup_version: u32,
    pub created_at: String,
    pub app_version: String,
    pub source_device_id: String,
    pub schema_migrations: Vec<String>,
    pub overall_checksum: String,
    pub games_count: i64,
    pub sessions_count: i64,
    pub assets_count: i64,
    pub asset_file_count: usize,
    pub backup_path: String,
    pub manifest_path: String,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalBackupManifest {
    backup_id: String,
    backup_version: u32,
    created_at: String,
    app_version: String,
    source_device_id: String,
    schema_migrations: Vec<String>,
    overall_checksum: String,
    games_count: i64,
    sessions_count: i64,
    assets_count: i64,
    files: Vec<BackupFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupFileEntry {
    path: String,
    bytes: u64,
    sha256: String,
}

pub fn export_local_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    destination_dir: &Path,
) -> Result<LocalBackupSummary> {
    export_backup_to(
        db,
        asset_manager,
        app_context,
        destination_dir,
        BACKUP_DIR_PREFIX,
        None,
    )
}

/// Writes a backup folder named `<prefix>-<timestamp>` into `destination_dir`.
/// Artwork that `previous_backup` holds unchanged is hard linked from there,
/// so it is stored once however many backups share it.
pub(crate) fn export_backup_to(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    destination_dir: &Path,
    prefix: &str,
    previous_backup: Option<&Path>,
) -> Result<LocalBackupSummary> {
    if !destination_dir.exists() || !destination_dir.is_dir() {
        return Err(VaultimeError::Backup(
            "backup destination must be an existing directory".into(),
        ));
    }

    let backup_id = uuid::Uuid::new_v4().to_string();
    let created_at = integrity::now_timestamp();
    let backup_dir = destination_dir.join(format!("{prefix}-{}", compact_timestamp(&created_at)));

    fs::create_dir_all(&backup_dir).map_err(|error| {
        VaultimeError::Backup(format!(
            "failed to create backup directory {}: {error}",
            backup_dir.display()
        ))
    })?;

    let backup_db_path = backup_dir.join(BACKUP_DB_FILE);
    export_database_snapshot(db, &backup_db_path)?;
    copy_directory_contents(
        asset_manager.cache_dir(),
        &backup_dir.join(BACKUP_ASSET_DIR),
        previous_backup
            .map(|previous| previous.join(BACKUP_ASSET_DIR))
            .as_deref(),
    )?;

    let schema_migrations = list_applied_migrations(db)?;
    let (games_count, sessions_count, assets_count) = summarize_database(&backup_db_path)?;
    let files = collect_backup_files(&backup_dir)?;
    let overall_checksum = compute_overall_checksum(
        &backup_id,
        &created_at,
        &app_context.app_version,
        &app_context.device_id,
        &schema_migrations,
        &files,
    );

    let manifest = LocalBackupManifest {
        backup_id: backup_id.clone(),
        backup_version: BACKUP_VERSION,
        created_at: created_at.clone(),
        app_version: app_context.app_version.clone(),
        source_device_id: app_context.device_id.clone(),
        schema_migrations,
        overall_checksum,
        games_count,
        sessions_count,
        assets_count,
        files,
    };

    write_manifest(&backup_dir, &manifest)?;
    Ok(summary_from_manifest(&manifest, &backup_dir, false))
}

pub fn inspect_local_backup(backup_path: &Path) -> Result<LocalBackupSummary> {
    let backup_dir = resolve_backup_dir(backup_path)?;
    let (manifest, _) = load_and_validate_manifest(&backup_dir)?;
    Ok(summary_from_manifest(&manifest, &backup_dir, false))
}

pub fn import_local_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    backup_path: &Path,
) -> Result<LocalBackupSummary> {
    let backup_dir = resolve_backup_dir(backup_path)?;
    let (manifest, artwork) = load_and_validate_manifest(&backup_dir)?;
    ensure_schema_supported(&manifest.schema_migrations)?;

    // A fresh name, never one from the backup, so the folder that is cleared
    // afterwards is always our own.
    let staging_dir = app_context
        .app_dir
        .join(format!(".restore-{}", uuid::Uuid::new_v4()));

    let result = restore_from_staging(
        db,
        asset_manager,
        app_context,
        &backup_dir,
        &staging_dir,
        &artwork,
    );
    cleanup_staging_dir(&staging_dir);
    result?;

    Ok(summary_from_manifest(&manifest, &backup_dir, true))
}

/// Removes a staging folder. It can hold a decrypted backup, so a failure is logged.
fn cleanup_staging_dir(staging_dir: &Path) {
    match fs::remove_dir_all(staging_dir) {
        Err(error) if error.kind() != ErrorKind::NotFound => warn!(
            "failed to remove staging directory {}: {error}",
            staging_dir.display()
        ),
        _ => {}
    }
}

/// Stages the backup, checks it, then swaps the artwork cache and the
/// database. A failed database swap puts the old cache back.
fn restore_from_staging(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    backup_dir: &Path,
    staging_dir: &Path,
    artwork: &[String],
) -> Result<()> {
    let staged_assets = staging_dir.join(BACKUP_ASSET_DIR);
    create_dir(&staged_assets)?;
    for relative_path in artwork {
        let target = staging_dir.join(relative_path);
        if let Some(parent) = target.parent() {
            create_dir(parent)?;
        }
        fs::copy(backup_dir.join(relative_path), &target).map_err(|error| {
            VaultimeError::Backup(format!("failed to stage {relative_path}: {error}"))
        })?;
    }

    // Older backups are migrated on a copy first so their columns match ours.
    let staged_db = staging_dir.join(BACKUP_DB_FILE);
    fs::copy(backup_dir.join(BACKUP_DB_FILE), &staged_db).map_err(|error| {
        VaultimeError::Backup(format!("failed to stage backup database: {error}"))
    })?;
    drop(Database::open(&staged_db)?);
    check_restored_ids(&staged_db)?;

    let cache_dir = asset_manager.cache_dir();
    let previous_cache = staging_dir.join("previous-asset-cache");
    let had_cache = cache_dir.exists();
    if had_cache {
        fs::rename(cache_dir, &previous_cache).map_err(|error| {
            VaultimeError::Backup(format!("failed to set the current artwork aside: {error}"))
        })?;
    }
    let put_back = || {
        let _ = fs::remove_dir_all(cache_dir);
        if had_cache {
            let _ = fs::rename(&previous_cache, cache_dir);
        }
    };
    if let Err(error) = fs::rename(&staged_assets, cache_dir) {
        put_back();
        return Err(VaultimeError::Backup(format!(
            "failed to move the restored artwork in place: {error}"
        )));
    }
    if let Err(error) = restore_database_snapshot(db, &staged_db) {
        put_back();
        return Err(error);
    }
    rewrite_asset_cache_paths(db, cache_dir)?;

    // A backup made during play holds sessions that were still running.
    // Nothing tracks them here, so they are closed like after a crash.
    for session in sessions::get_active_sessions(db)? {
        sessions::recover_session(db, &session.id, RESTORED_OPEN_SESSION_REASON)?;
    }

    // The restored device list may not contain this machine yet.
    devices::ensure_device(
        db,
        &app_context.device_id,
        std::env::consts::OS,
        &app_context.app_version,
    )?;
    Ok(())
}

fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|error| {
        VaultimeError::Backup(format!(
            "failed to create directory {}: {error}",
            path.display()
        ))
    })
}

/// Game ids become folder names in the artwork cache, so a backup with an id
/// that is not a plain name is refused before anything changes.
fn check_restored_ids(database_path: &Path) -> Result<()> {
    let refused = |error: rusqlite::Error| {
        VaultimeError::Backup(format!("failed to check the backup database: {error}"))
    };
    let conn = Connection::open(database_path).map_err(refused)?;
    let mut stmt = conn
        .prepare("SELECT id FROM games UNION SELECT game_id FROM game_assets")
        .map_err(refused)?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
        .map_err(refused)?;
    if ids.iter().any(|id| !is_plain_name(id)) {
        return Err(VaultimeError::Backup(
            "this backup holds a game id Vaultime cannot use, so it was not restored".into(),
        ));
    }
    Ok(())
}

/// Rejects backups made by a newer app version with migrations we do not know.
fn ensure_schema_supported(backup_migrations: &[String]) -> Result<()> {
    let known: Vec<&str> = known_migrations().collect();
    match backup_migrations
        .iter()
        .find(|name| !known.contains(&name.as_str()))
    {
        Some(unknown) => Err(VaultimeError::Backup(format!(
            "this backup was made by a newer Vaultime version (unknown migration {unknown}). Update Vaultime and try again."
        ))),
        None => Ok(()),
    }
}

/// Tables in foreign key order, parents first.
const RESTORE_TABLES: &[&str] = &[
    "devices",
    "games",
    "earlier_playtime",
    "game_assets",
    "game_status_changes",
    "sessions",
    "session_events",
    "session_notes",
    "settings",
    "backup_snapshots",
];

fn export_database_snapshot(db: &Database, destination_path: &Path) -> Result<()> {
    let destination = destination_path.to_string_lossy().to_string();

    db.with_conn(|conn| {
        conn.execute_batch(&format!(
            "PRAGMA wal_checkpoint(FULL); VACUUM INTO '{}';",
            sqlite_string_literal(&destination)
        ))
        .map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to export SQLite snapshot to {}: {error}",
                destination_path.display()
            ))
        })?;

        Ok(())
    })
}

fn restore_database_snapshot(db: &Database, backup_db_path: &Path) -> Result<()> {
    let backup_db = backup_db_path.to_string_lossy().to_string();

    db.with_conn(|conn| {
        conn.execute_batch(&format!(
            "ATTACH DATABASE '{}' AS backup_restore;",
            sqlite_string_literal(&backup_db)
        ))
        .map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to attach backup database {}: {error}",
                backup_db_path.display()
            ))
        })?;

        let restore_result = restore_statements(conn).and_then(|statements| {
            conn.execute_batch(&statements).map_err(|error| {
                VaultimeError::Backup(format!(
                    "failed to restore database tables from backup: {error}"
                ))
            })
        });
        if restore_result.is_err() {
            // Apart, so a failed rollback cannot leave foreign keys off.
            let _ = conn.execute_batch("ROLLBACK;");
            let _ = conn.execute_batch("PRAGMA foreign_keys=ON;");
        }
        let detach_result = conn.execute_batch("DETACH DATABASE backup_restore;");

        restore_result?;
        detach_result.map_err(|error| {
            VaultimeError::Backup(format!("failed to detach backup database: {error}"))
        })
    })
}

/// Replaces every restored table with the backup's rows. Tables and columns
/// that a backup predates stay empty or take their defaults, so backups of
/// older versions restore too.
fn restore_statements(conn: &Connection) -> Result<String> {
    let mut statements = String::from("PRAGMA foreign_keys=OFF;\nBEGIN IMMEDIATE;\n");
    let kept_settings = DEVICE_SETTINGS
        .iter()
        .map(|key| format!("'{}'", sqlite_string_literal(key)))
        .collect::<Vec<_>>()
        .join(", ");
    for table in RESTORE_TABLES.iter().rev() {
        let keep = if *table == "settings" {
            format!(" WHERE key NOT IN ({kept_settings})")
        } else {
            String::new()
        };
        let _ = writeln!(statements, "DELETE FROM {table}{keep};");
    }
    for table in RESTORE_TABLES {
        let in_backup = table_columns(conn, "backup_restore", table)?;
        let columns: Vec<String> = table_columns(conn, "main", table)?
            .into_iter()
            .filter(|column| in_backup.contains(column))
            .collect();
        if columns.is_empty() {
            continue;
        }
        let columns = columns.join(", ");
        let skip = if *table == "settings" {
            format!(" WHERE key NOT IN ({kept_settings})")
        } else {
            String::new()
        };
        let _ = writeln!(
            statements,
            "INSERT INTO {table} ({columns}) SELECT {columns} FROM backup_restore.{table}{skip};"
        );
    }
    statements.push_str("COMMIT;\nPRAGMA foreign_keys=ON;");
    Ok(statements)
}

fn table_columns(conn: &Connection, schema: &str, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT name FROM pragma_table_info(?1, ?2)")
        .map_err(|error| VaultimeError::Backup(format!("failed to read columns: {error}")))?;
    let columns = stmt
        .query_map([table, schema], |row| row.get::<_, String>(0))
        .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
        .map_err(|error| VaultimeError::Backup(format!("failed to read columns: {error}")))?;
    Ok(columns)
}

/// Points cached artwork at the restored cache folder, keeping each file name.
fn rewrite_asset_cache_paths(db: &Database, cache_dir: &Path) -> Result<()> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, game_id, cache_path FROM game_assets WHERE cache_path IS NOT NULL")
            .map_err(|error| {
                VaultimeError::Backup(format!("failed to query cached assets: {error}"))
            })?;
        let assets = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
            .map_err(|error| {
                VaultimeError::Backup(format!("failed to query cached assets: {error}"))
            })?;

        for (asset_id, game_id, old_path) in assets {
            // The backup may come from another OS. A path that does not end in
            // a plain file name is dropped, so no cached path leaves the cache.
            let next_path = file_name(&old_path)
                .filter(|name| is_plain_name(name) && is_plain_name(&game_id))
                .map(|name| {
                    cache_dir
                        .join(&game_id)
                        .join(name)
                        .to_string_lossy()
                        .to_string()
                });

            conn.execute(
                "UPDATE game_assets SET cache_path = ?1 WHERE id = ?2",
                rusqlite::params![next_path, asset_id],
            )
            .map_err(|error| {
                VaultimeError::Backup(format!(
                    "failed to rewrite cache path for asset {asset_id}: {error}"
                ))
            })?;
        }

        Ok(())
    })
}

fn list_applied_migrations(db: &Database) -> Result<Vec<String>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT name FROM _migrations ORDER BY name ASC")
            .map_err(|error| {
                VaultimeError::Backup(format!("failed to prepare migration query: {error}"))
            })?;

        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| {
                VaultimeError::Backup(format!("failed to query migrations: {error}"))
            })?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|error| {
            VaultimeError::Backup(format!("failed to collect migrations: {error}"))
        })
    })
}

fn summarize_database(database_path: &Path) -> Result<(i64, i64, i64)> {
    let conn = Connection::open(database_path).map_err(|error| {
        VaultimeError::Backup(format!(
            "failed to open backup database {}: {error}",
            database_path.display()
        ))
    })?;

    let games_count = count_table(&conn, "games")?;
    let sessions_count = count_table(&conn, "sessions")?;
    let assets_count = count_table(&conn, "game_assets")?;
    Ok((games_count, sessions_count, assets_count))
}

fn count_table(conn: &Connection, table: &str) -> Result<i64> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .map_err(|error| VaultimeError::Backup(format!("failed to count rows in {table}: {error}")))
}

fn write_manifest(backup_dir: &Path, manifest: &LocalBackupManifest) -> Result<()> {
    let manifest_path = backup_dir.join(BACKUP_MANIFEST_FILE);
    let manifest_json = serde_json::to_string_pretty(manifest).map_err(|error| {
        VaultimeError::Backup(format!("failed to serialize backup manifest: {error}"))
    })?;

    fs::write(&manifest_path, manifest_json).map_err(|error| {
        VaultimeError::Backup(format!(
            "failed to write backup manifest {}: {error}",
            manifest_path.display()
        ))
    })
}

/// Reads and checks the manifest. Returns it with the artwork files that
/// passed their check.
fn load_and_validate_manifest(backup_dir: &Path) -> Result<(LocalBackupManifest, Vec<String>)> {
    let manifest_path = backup_dir.join(BACKUP_MANIFEST_FILE);
    let manifest_json = fs::read_to_string(&manifest_path).map_err(|error| {
        VaultimeError::Backup(format!(
            "failed to read backup manifest {}: {error}",
            manifest_path.display()
        ))
    })?;

    let manifest: LocalBackupManifest = serde_json::from_str(&manifest_json).map_err(|error| {
        VaultimeError::Backup(format!(
            "failed to parse backup manifest {}: {error}",
            manifest_path.display()
        ))
    })?;

    if manifest.backup_version != BACKUP_VERSION {
        return Err(VaultimeError::Backup(format!(
            "unsupported backup version {}",
            manifest.backup_version
        )));
    }
    if uuid::Uuid::parse_str(&manifest.backup_id).is_err() {
        return Err(VaultimeError::Backup(
            "backup manifest has an invalid backup id".into(),
        ));
    }

    let artwork = validate_manifest_files(backup_dir, &manifest)?;
    let expected_checksum = compute_overall_checksum(
        &manifest.backup_id,
        &manifest.created_at,
        &manifest.app_version,
        &manifest.source_device_id,
        &manifest.schema_migrations,
        &manifest.files,
    );

    if manifest.overall_checksum != expected_checksum {
        return Err(VaultimeError::Backup(
            "backup manifest checksum does not match the file inventory".into(),
        ));
    }

    Ok((manifest, artwork))
}

/// Checks the listed files. The database must be listed and intact. Artwork
/// that fails its check is left out with a warning, so one damaged cover
/// does not block a restore. Only regular files count, never links, and
/// anything the manifest does not list is ignored.
fn validate_manifest_files(
    backup_dir: &Path,
    manifest: &LocalBackupManifest,
) -> Result<Vec<String>> {
    if !manifest
        .files
        .iter()
        .any(|file| file.path == BACKUP_DB_FILE)
    {
        return Err(VaultimeError::Backup(
            "backup manifest does not list the database".into(),
        ));
    }
    let mut artwork = Vec::new();
    for file in &manifest.files {
        let resolved_path = resolve_manifest_file_path(backup_dir, &file.path)?;
        let intact = fs::symlink_metadata(&resolved_path).is_ok_and(|meta| meta.is_file())
            && hash_file(&resolved_path)
                .is_ok_and(|(bytes, sha256)| bytes == file.bytes && sha256 == file.sha256);
        if file.path == BACKUP_DB_FILE {
            if !intact {
                return Err(VaultimeError::Backup(
                    "the backup database does not match its checksum".into(),
                ));
            }
        } else if !intact {
            warn!("left out backup file {} that failed its check", file.path);
        } else if is_artwork_path(&file.path) {
            artwork.push(file.path.clone());
        }
    }
    Ok(artwork)
}

/// `asset-cache/<game id>/<file>`, the only shape cached artwork has.
fn is_artwork_path(relative_path: &str) -> bool {
    let mut parts = relative_path.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(BACKUP_ASSET_DIR), Some(game_id), Some(name), None)
            if is_plain_name(game_id) && is_plain_name(name)
    )
}

fn collect_backup_files(backup_dir: &Path) -> Result<Vec<BackupFileEntry>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(backup_dir).min_depth(1) {
        let entry = entry.map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to walk backup directory {}: {error}",
                backup_dir.display()
            ))
        })?;

        if !entry.file_type().is_file() {
            continue;
        }

        let relative_path = entry
            .path()
            .strip_prefix(backup_dir)
            .map_err(|error| {
                VaultimeError::Backup(format!(
                    "failed to normalize backup path {}: {error}",
                    entry.path().display()
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");

        let (bytes, sha256) = hash_file(entry.path())?;
        files.push(BackupFileEntry {
            path: relative_path,
            bytes,
            sha256,
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

/// Copies a directory tree. A file that `link_from` holds at the same place
/// with the same content is hard linked from there instead. Where links do not
/// work, for example across drives or on FAT, the file is copied.
fn copy_directory_contents(
    source_dir: &Path,
    destination_dir: &Path,
    link_from: Option<&Path>,
) -> Result<()> {
    fs::create_dir_all(destination_dir).map_err(|error| {
        VaultimeError::Backup(format!(
            "failed to create directory {}: {error}",
            destination_dir.display()
        ))
    })?;

    if !source_dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(source_dir).min_depth(1) {
        let entry = entry.map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to walk source directory {}: {error}",
                source_dir.display()
            ))
        })?;

        let relative_path = entry.path().strip_prefix(source_dir).map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to normalize source path {}: {error}",
                entry.path().display()
            ))
        })?;
        let target_path = destination_dir.join(relative_path);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&target_path).map_err(|error| {
                VaultimeError::Backup(format!(
                    "failed to create directory {}: {error}",
                    target_path.display()
                ))
            })?;
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                VaultimeError::Backup(format!(
                    "failed to create directory {}: {error}",
                    parent.display()
                ))
            })?;
        }

        if let Some(earlier) = link_from.map(|dir| dir.join(relative_path))
            && same_content(entry.path(), &earlier)
            && fs::hard_link(&earlier, &target_path).is_ok()
        {
            continue;
        }

        fs::copy(entry.path(), &target_path).map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to copy {} to {}: {error}",
                entry.path().display(),
                target_path.display()
            ))
        })?;
    }

    Ok(())
}

fn resolve_backup_dir(backup_path: &Path) -> Result<PathBuf> {
    if backup_path.is_dir() {
        return Ok(backup_path.to_path_buf());
    }

    if backup_path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        == Some(BACKUP_MANIFEST_FILE)
    {
        return backup_path.parent().map(Path::to_path_buf).ok_or_else(|| {
            VaultimeError::Backup("backup manifest path has no parent directory".into())
        });
    }

    Err(VaultimeError::Backup(
        "backup import expects a backup directory or manifest.json file".into(),
    ))
}

fn resolve_manifest_file_path(backup_dir: &Path, relative_path: &str) -> Result<PathBuf> {
    let relative = Path::new(relative_path);
    // Plain names only: no root, no drive like `C:` and no way up. Backups
    // write `/` between names on every system, so `\` and `:` never belong.
    if relative_path.is_empty()
        || relative_path.contains(['\\', ':'])
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(VaultimeError::Backup(format!(
            "backup manifest contains an unsafe path: {relative_path}"
        )));
    }

    Ok(backup_dir.join(relative))
}

fn compute_overall_checksum(
    backup_id: &str,
    created_at: &str,
    app_version: &str,
    source_device_id: &str,
    schema_migrations: &[String],
    files: &[BackupFileEntry],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(backup_id.as_bytes());
    hasher.update(created_at.as_bytes());
    hasher.update(app_version.as_bytes());
    hasher.update(source_device_id.as_bytes());

    for migration in schema_migrations {
        hasher.update(migration.as_bytes());
        hasher.update(b"\n");
    }

    for file in files {
        hasher.update(file.path.as_bytes());
        hasher.update(b"\n");
        hasher.update(file.bytes.to_string().as_bytes());
        hasher.update(b"\n");
        hasher.update(file.sha256.as_bytes());
        hasher.update(b"\n");
    }

    crate::hex::encode(&hasher.finalize())
}

/// Whether two files hold the same bytes. Sizes are compared first, so most
/// changed files are told apart without reading them.
fn same_content(left: &Path, right: &Path) -> bool {
    let size = |path: &Path| fs::metadata(path).map(|metadata| metadata.len()).ok();
    size(left).is_some_and(|bytes| size(right) == Some(bytes))
        && matches!((hash_file(left), hash_file(right)), (Ok(a), Ok(b)) if a == b)
}

/// Size and SHA-256 hex digest of a file, read in chunks.
fn hash_file(path: &Path) -> Result<(u64, String)> {
    let read_error = |error: std::io::Error| {
        VaultimeError::Backup(format!("failed to read {}: {error}", path.display()))
    };
    let mut file = File::open(path).map_err(read_error)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];

    loop {
        let read = file.read(&mut buffer).map_err(read_error)?;
        if read == 0 {
            break;
        }

        bytes += u64::try_from(read)
            .map_err(|_| VaultimeError::Backup("backup file length overflow".into()))?;
        hasher.update(&buffer[..read]);
    }

    Ok((bytes, crate::hex::encode(&hasher.finalize())))
}

fn summary_from_manifest(
    manifest: &LocalBackupManifest,
    backup_dir: &Path,
    restart_required: bool,
) -> LocalBackupSummary {
    let asset_file_count = manifest
        .files
        .iter()
        .filter(|file| file.path.starts_with(&format!("{BACKUP_ASSET_DIR}/")))
        .count();

    LocalBackupSummary {
        backup_id: manifest.backup_id.clone(),
        backup_version: manifest.backup_version,
        created_at: manifest.created_at.clone(),
        app_version: manifest.app_version.clone(),
        source_device_id: manifest.source_device_id.clone(),
        schema_migrations: manifest.schema_migrations.clone(),
        overall_checksum: manifest.overall_checksum.clone(),
        games_count: manifest.games_count,
        sessions_count: manifest.sessions_count,
        assets_count: manifest.assets_count,
        asset_file_count,
        backup_path: backup_dir.to_string_lossy().to_string(),
        manifest_path: backup_dir
            .join(BACKUP_MANIFEST_FILE)
            .to_string_lossy()
            .to_string(),
        restart_required,
    }
}

fn compact_timestamp(value: &str) -> String {
    value.chars().filter(char::is_ascii_alphanumeric).collect()
}

fn sqlite_string_literal(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::{devices, game_assets, games, sessions};

    #[test]
    fn restores_a_backup_that_predates_a_table() {
        let context = test_paths();
        let db = Database::open(&context.app_dir.join("vaultime.db")).unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Old Game".into(),
                executable_path: Some("/games/old.exe".into()),
                install_folder: None,
                launcher_source: Some("steam".into()),
            },
        )
        .unwrap();
        crate::db::repo::earlier_playtime::replace_earlier_playtime(
            &db,
            &[crate::db::models::EarlierPlaytime {
                game_id: game.id.clone(),
                source: "steam".into(),
                launcher_minutes: 60,
                tracked_before_ms: 0,
                earlier_ms: 0,
                last_played_at: None,
                imported_at: "2026-09-30T10:00:00Z".into(),
            }],
        )
        .unwrap();

        // A backup from before the table existed.
        let old_path = context.app_dir.join("old.db");
        export_database_snapshot(&db, &old_path).unwrap();
        Connection::open(&old_path)
            .unwrap()
            .execute_batch("DROP TABLE earlier_playtime;")
            .unwrap();

        restore_database_snapshot(&db, &old_path).unwrap();
        assert_eq!(games::list_all_games(&db).unwrap().len(), 1);
        assert!(
            crate::db::repo::earlier_playtime::list_earlier_playtime(&db)
                .unwrap()
                .is_empty()
        );
        drop(db);
        fs::remove_dir_all(&context.app_dir).ok();
    }

    fn test_paths() -> AppContext {
        let root =
            std::env::temp_dir().join(format!("vaultime-backup-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        AppContext {
            app_dir: root,
            device_id: "test-device".into(),
            app_version: "0.1.0".into(),
        }
    }

    #[test]
    fn checksum_changes_when_file_inventory_changes() {
        let files = vec![BackupFileEntry {
            path: "vaultime.db".into(),
            bytes: 42,
            sha256: "abc".into(),
        }];
        let first = compute_overall_checksum(
            "backup",
            "2026-01-01T00:00:00.000Z",
            "0.1.0",
            "device",
            &["0001_initial_schema".into()],
            &files,
        );
        let second = compute_overall_checksum(
            "backup",
            "2026-01-01T00:00:00.000Z",
            "0.1.0",
            "device",
            &["0001_initial_schema".into()],
            &[BackupFileEntry {
                path: "vaultime.db".into(),
                bytes: 43,
                sha256: "abc".into(),
            }],
        );
        assert_ne!(first, second);
    }

    #[test]
    fn schema_check_rejects_unknown_migrations() {
        assert!(ensure_schema_supported(&["0001_initial_schema".into()]).is_ok());
        assert!(ensure_schema_supported(&["9999_from_the_future".into()]).is_err());
    }

    #[test]
    fn export_and_import_roundtrip_restores_data() {
        let context = test_paths();
        let asset_cache_dir = context.app_dir.join("asset-cache");
        let db = Database::open(&context.app_dir.join("vaultime.db")).unwrap();
        devices::ensure_device(&db, &context.device_id, "linux", &context.app_version).unwrap();

        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Roundtrip Game".into(),
                executable_path: Some("/games/roundtrip.exe".into()),
                install_folder: Some("/games".into()),
                launcher_source: None,
            },
        )
        .unwrap();
        let session = sessions::create_session(&db, &game.id, &context.device_id).unwrap();
        sessions::end_session(
            &db,
            &session.id,
            60_000,
            45_000,
            15_000,
            integrity::STATUS_LOCAL,
        )
        .unwrap();

        let cached_asset_path = asset_cache_dir.join(&game.id).join("asset-1.png");
        fs::create_dir_all(cached_asset_path.parent().unwrap()).unwrap();
        fs::write(&cached_asset_path, b"vaultime").unwrap();
        game_assets::create_asset(
            &db,
            &game.id,
            "cover",
            "user_picked",
            "/games/cover.png",
            Some(&cached_asset_path.to_string_lossy()),
            Some("asset-hash"),
        )
        .unwrap();

        let exports_root = context.app_dir.join("exports");
        fs::create_dir_all(&exports_root).unwrap();
        let asset_manager = AssetManager::new(asset_cache_dir.clone());
        let backup = export_local_backup(&db, &asset_manager, &context, &exports_root).unwrap();

        games::delete_game(&db, &game.id).unwrap();
        assert!(games::list_all_games(&db).unwrap().is_empty());

        import_local_backup(
            &db,
            &asset_manager,
            &context,
            Path::new(&backup.backup_path),
        )
        .unwrap();

        let restored_games = games::list_all_games(&db).unwrap();
        assert_eq!(restored_games.len(), 1);
        let restored_assets = game_assets::list_assets_for_game(&db, &game.id).unwrap();
        assert_eq!(restored_assets.len(), 1);
        let restored_path = restored_assets[0].cache_path.as_deref().unwrap();
        assert!(restored_path.starts_with(&*asset_cache_dir.to_string_lossy()));
        assert!(Path::new(restored_path).is_file());
        assert!(fs::read_dir(&context.app_dir).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".restore-")
        }));

        // Windows cannot delete the folder while the database is open.
        drop(db);
        fs::remove_dir_all(&context.app_dir).unwrap();
    }

    /// A library with one game, one cached cover and a backup of it.
    struct Fixture {
        context: AppContext,
        db: Database,
        assets: AssetManager,
        game_id: String,
        backup_dir: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let context = test_paths();
            let db = Database::open(&context.app_dir.join("vaultime.db")).unwrap();
            devices::ensure_device(&db, &context.device_id, "linux", &context.app_version).unwrap();
            let game = games::create_game(
                &db,
                &CreateGame {
                    title: "Fixture Game".into(),
                    executable_path: Some("/games/fixture.exe".into()),
                    install_folder: None,
                    launcher_source: None,
                },
            )
            .unwrap();
            let cache_dir = context.app_dir.join("asset-cache");
            let cover = cache_dir.join(&game.id).join("cover.png");
            fs::create_dir_all(cover.parent().unwrap()).unwrap();
            fs::write(&cover, b"cover").unwrap();
            game_assets::create_asset(
                &db,
                &game.id,
                "cover",
                "user_picked",
                "/games/cover.png",
                Some(&cover.to_string_lossy()),
                Some("cover-hash"),
            )
            .unwrap();
            Self {
                context,
                db,
                assets: AssetManager::new(cache_dir),
                game_id: game.id,
                backup_dir: PathBuf::new(),
            }
        }

        fn back_up(mut self) -> Self {
            let exports = self.context.app_dir.join("exports");
            fs::create_dir_all(&exports).unwrap();
            let backup =
                export_local_backup(&self.db, &self.assets, &self.context, &exports).unwrap();
            self.backup_dir = PathBuf::from(backup.backup_path);
            self
        }

        /// Changes the backup the way someone crafting one could, checksums included.
        fn tamper(&self, change: impl FnOnce(&mut LocalBackupManifest, &Connection)) {
            let manifest_path = self.backup_dir.join(BACKUP_MANIFEST_FILE);
            let mut manifest: LocalBackupManifest =
                serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
            let conn = Connection::open(self.backup_dir.join(BACKUP_DB_FILE)).unwrap();
            change(&mut manifest, &conn);
            drop(conn);
            manifest.files = collect_backup_files(&self.backup_dir)
                .unwrap()
                .into_iter()
                .filter(|file| file.path != BACKUP_MANIFEST_FILE)
                .collect();
            manifest.overall_checksum = compute_overall_checksum(
                &manifest.backup_id,
                &manifest.created_at,
                &manifest.app_version,
                &manifest.source_device_id,
                &manifest.schema_migrations,
                &manifest.files,
            );
            write_manifest(&self.backup_dir, &manifest).unwrap();
        }

        fn restore(&self) -> Result<LocalBackupSummary> {
            import_local_backup(&self.db, &self.assets, &self.context, &self.backup_dir)
        }

        fn finish(self) {
            drop(self.db);
            fs::remove_dir_all(&self.context.app_dir).ok();
        }
    }

    #[test]
    fn a_crafted_backup_id_cannot_reach_outside_the_app_folder() {
        let fixture = Fixture::new().back_up();
        // Where `.restore-<id>` pointed before, one level above the app folder.
        let victim = fixture.context.app_dir.join("victim");
        fs::create_dir_all(&victim).unwrap();
        fs::write(victim.join("keep.txt"), b"keep").unwrap();
        fixture.tamper(|manifest, _| manifest.backup_id = "x/../victim".into());

        assert!(fixture.restore().is_err());
        assert!(victim.join("keep.txt").is_file());
        fixture.finish();
    }

    #[test]
    fn refuses_a_game_id_that_is_a_path_and_changes_nothing() {
        let fixture = Fixture::new().back_up();
        fixture.tamper(|_, conn| {
            conn.execute_batch(
                "PRAGMA foreign_keys=OFF; UPDATE game_assets SET game_id = '../../elsewhere';",
            )
            .unwrap();
        });
        games::update_game(
            &fixture.db,
            &fixture.game_id,
            &crate::db::models::UpdateGame {
                title: Some("Renamed here".into()),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
                is_hidden: None,
            },
        )
        .unwrap();

        assert!(fixture.restore().is_err());
        let game = games::get_game(&fixture.db, &fixture.game_id).unwrap();
        assert_eq!(game.title, "Renamed here");
        assert!(
            fixture
                .assets
                .cache_dir()
                .join(&fixture.game_id)
                .join("cover.png")
                .is_file()
        );
        fixture.finish();
    }

    #[test]
    fn keeps_the_backup_folder_of_this_pc() {
        let fixture = Fixture::new();
        crate::db::repo::settings::set_setting(
            &fixture.db,
            AUTO_BACKUP_FOLDER_SETTING,
            "\\\\elsewhere\\share",
        )
        .unwrap();
        crate::db::repo::settings::set_setting(&fixture.db, "idle_threshold_seconds", "600")
            .unwrap();
        let fixture = fixture.back_up();
        crate::db::repo::settings::set_setting(&fixture.db, AUTO_BACKUP_FOLDER_SETTING, "D:/Mine")
            .unwrap();

        fixture.restore().unwrap();
        let setting = |key| crate::db::repo::settings::get_setting(&fixture.db, key).unwrap();
        assert_eq!(
            setting(AUTO_BACKUP_FOLDER_SETTING).as_deref(),
            Some("D:/Mine")
        );
        assert_eq!(setting("idle_threshold_seconds").as_deref(), Some("600"));
        fixture.finish();
    }

    #[test]
    fn leaves_out_damaged_artwork_but_restores_the_rest() {
        let fixture = Fixture::new().back_up();
        let cover = fixture
            .backup_dir
            .join(BACKUP_ASSET_DIR)
            .join(&fixture.game_id)
            .join("cover.png");
        fs::write(&cover, b"damaged").unwrap();
        games::delete_game(&fixture.db, &fixture.game_id).unwrap();

        fixture.restore().unwrap();
        assert_eq!(games::list_all_games(&fixture.db).unwrap().len(), 1);
        assert!(
            !fixture
                .assets
                .cache_dir()
                .join(&fixture.game_id)
                .join("cover.png")
                .exists()
        );
        fixture.finish();
    }

    #[test]
    fn closes_sessions_the_backup_caught_while_running() {
        let fixture = Fixture::new();
        let running =
            sessions::create_session(&fixture.db, &fixture.game_id, &fixture.context.device_id)
                .unwrap();
        let fixture = fixture.back_up();
        sessions::end_session(
            &fixture.db,
            &running.id,
            1_000,
            1_000,
            0,
            integrity::STATUS_LOCAL,
        )
        .unwrap();

        fixture.restore().unwrap();
        assert!(
            sessions::get_active_sessions(&fixture.db)
                .unwrap()
                .is_empty()
        );
        let restored = sessions::list_all_sessions(&fixture.db).unwrap();
        assert_eq!(restored[0].integrity_status, integrity::STATUS_RECOVERED);
        fixture.finish();
    }

    #[test]
    fn manifest_paths_must_be_plain_names() {
        let root = Path::new("backup");
        assert!(resolve_manifest_file_path(root, "asset-cache/game/cover.png").is_ok());
        for unsafe_path in [
            "",
            "../x",
            "/etc/passwd",
            "a/../../b",
            "C:evil",
            "C:/evil",
            "\\\\host\\share",
        ] {
            assert!(
                resolve_manifest_file_path(root, unsafe_path).is_err(),
                "{unsafe_path}"
            );
        }
        assert!(is_artwork_path("asset-cache/game/cover.png"));
        assert!(!is_artwork_path("asset-cache/cover.png"));
        assert!(!is_artwork_path("asset-cache/a/b/cover.png"));
        assert!(!is_artwork_path("elsewhere/game/cover.png"));
    }
}
