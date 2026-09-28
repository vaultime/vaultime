// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Local backup export, inspection and restore.

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
use crate::assets::AssetManager;
use crate::constants::{BACKUP_VERSION, HASH_BUFFER_BYTES};
use crate::db::connection::Database;
use crate::db::migrate::known_migrations;
use crate::db::repo::devices;
use crate::error::{Result, VaultimeError};
use crate::integrity;
use crate::platform::process::file_name;

const BACKUP_DIR_PREFIX: &str = "vaultime-backup";
const BACKUP_DB_FILE: &str = "vaultime.db";
const BACKUP_MANIFEST_FILE: &str = "manifest.json";
const BACKUP_ASSET_DIR: &str = "asset-cache";

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
    if !destination_dir.exists() || !destination_dir.is_dir() {
        return Err(VaultimeError::Backup(
            "backup destination must be an existing directory".into(),
        ));
    }

    let backup_id = uuid::Uuid::new_v4().to_string();
    let created_at = integrity::now_timestamp();
    let backup_dir = destination_dir.join(format!(
        "{BACKUP_DIR_PREFIX}-{}",
        compact_timestamp(&created_at)
    ));

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
    let manifest = load_and_validate_manifest(&backup_dir)?;
    Ok(summary_from_manifest(&manifest, &backup_dir, false))
}

pub fn import_local_backup(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    backup_path: &Path,
) -> Result<LocalBackupSummary> {
    let backup_dir = resolve_backup_dir(backup_path)?;
    let manifest = load_and_validate_manifest(&backup_dir)?;
    ensure_schema_supported(&manifest.schema_migrations)?;

    let staging_dir = app_context
        .app_dir
        .join(format!(".restore-{}", manifest.backup_id));
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to clear staging directory {}: {error}",
                staging_dir.display()
            ))
        })?;
    }

    let result = restore_from_staging(db, asset_manager, app_context, &backup_dir, &staging_dir);
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

fn restore_from_staging(
    db: &Database,
    asset_manager: &AssetManager,
    app_context: &AppContext,
    backup_dir: &Path,
    staging_dir: &Path,
) -> Result<()> {
    let staged_assets = staging_dir.join(BACKUP_ASSET_DIR);
    copy_directory_contents(&backup_dir.join(BACKUP_ASSET_DIR), &staged_assets)?;

    // Older backups are migrated on a copy first so their columns match ours.
    let staged_db = staging_dir.join(BACKUP_DB_FILE);
    fs::copy(backup_dir.join(BACKUP_DB_FILE), &staged_db).map_err(|error| {
        VaultimeError::Backup(format!("failed to stage backup database: {error}"))
    })?;
    drop(Database::open(&staged_db)?);

    restore_database_snapshot(db, &staged_db)?;
    replace_directory(&staged_assets, asset_manager.cache_dir())?;
    rewrite_asset_cache_paths(db, asset_manager.cache_dir())?;

    // The restored device list may not contain this machine yet.
    devices::ensure_device(
        db,
        &app_context.device_id,
        std::env::consts::OS,
        &app_context.app_version,
    )?;
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
    "game_assets",
    "sessions",
    "session_events",
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
        let mut statements = String::from("PRAGMA foreign_keys=OFF;\nBEGIN IMMEDIATE;\n");
        for table in RESTORE_TABLES.iter().rev() {
            let _ = writeln!(statements, "DELETE FROM {table};");
        }
        for table in RESTORE_TABLES {
            let columns = table_columns(conn, table)?.join(", ");
            let _ = writeln!(
                statements,
                "INSERT INTO {table} ({columns}) SELECT {columns} FROM backup_restore.{table};"
            );
        }
        statements.push_str("COMMIT;\nPRAGMA foreign_keys=ON;");

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

        let restore_result = conn.execute_batch(&statements);
        if restore_result.is_err() {
            let _ = conn.execute_batch("ROLLBACK; PRAGMA foreign_keys=ON;");
        }
        let detach_result = conn.execute_batch("DETACH DATABASE backup_restore;");

        restore_result.map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to restore database tables from backup: {error}"
            ))
        })?;
        detach_result.map_err(|error| {
            VaultimeError::Backup(format!("failed to detach backup database: {error}"))
        })
    })
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT name FROM pragma_table_info(?1)")
        .map_err(|error| VaultimeError::Backup(format!("failed to read columns: {error}")))?;
    let columns = stmt
        .query_map([table], |row| row.get::<_, String>(0))
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
            // The backup may come from another OS.
            let Some(file_name) = file_name(&old_path) else {
                continue;
            };
            let next_path = cache_dir
                .join(&game_id)
                .join(file_name)
                .to_string_lossy()
                .to_string();

            conn.execute(
                "UPDATE game_assets SET cache_path = ?1 WHERE id = ?2",
                [&next_path, &asset_id],
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

fn replace_directory(staged_dir: &Path, destination_dir: &Path) -> Result<()> {
    if destination_dir.exists() {
        fs::remove_dir_all(destination_dir).map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to clear destination directory {}: {error}",
                destination_dir.display()
            ))
        })?;
    }

    if let Some(parent) = destination_dir.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            VaultimeError::Backup(format!(
                "failed to create parent directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    fs::rename(staged_dir, destination_dir).map_err(|error| {
        VaultimeError::Backup(format!(
            "failed to move staged assets into {}: {error}",
            destination_dir.display()
        ))
    })?;

    Ok(())
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

fn load_and_validate_manifest(backup_dir: &Path) -> Result<LocalBackupManifest> {
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

    validate_manifest_files(backup_dir, &manifest)?;
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

    Ok(manifest)
}

fn validate_manifest_files(backup_dir: &Path, manifest: &LocalBackupManifest) -> Result<()> {
    for file in &manifest.files {
        let resolved_path = resolve_manifest_file_path(backup_dir, &file.path)?;
        let (bytes, sha256) = hash_file(&resolved_path)?;

        if bytes != file.bytes {
            return Err(VaultimeError::Backup(format!(
                "backup file size mismatch for {}",
                resolved_path.display()
            )));
        }

        if sha256 != file.sha256 {
            return Err(VaultimeError::Backup(format!(
                "backup checksum mismatch for {}",
                resolved_path.display()
            )));
        }
    }

    Ok(())
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

fn copy_directory_contents(source_dir: &Path, destination_dir: &Path) -> Result<()> {
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
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
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
        let staging_dir = context
            .app_dir
            .join(format!(".restore-{}", backup.backup_id));
        assert!(!staging_dir.exists());

        // Windows cannot delete the folder while the database is open.
        drop(db);
        fs::remove_dir_all(&context.app_dir).unwrap();
    }
}
