// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a backup would hold, as one hash. An automatic backup of a database
//! and artwork that did not change since the last one would add nothing, so
//! it is left out.

use std::path::Path;
use std::time::UNIX_EPOCH;

use rusqlite::Connection;
use rusqlite::types::ValueRef;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::db::connection::Database;
use crate::error::{Result, VaultimeError};
use crate::hex;

/// Tables read otherwise or not at all: session events and ledger entries,
/// which only grow and count by their newest rows, sessions, whose rows
/// change only with one of those and count by their number, the quarter
/// hours and play spans, which are a cache, the programs seen running, which
/// no backup carries, and the history of backups, which each backup adds to.
const READ_OTHERWISE: [&str; 7] = [
    "session_events",
    "ledger_entries",
    "sessions",
    "play_slices",
    "play_spans",
    "seen_programs",
    "backup_snapshots",
];

/// What a backup of `db` and the artwork in `artwork_dir` would hold, as a
/// hash: the newest session event and ledger entry, the number of sessions,
/// every row of the other tables, settings without the time they were
/// written, and the path, size and change time of every artwork file. Read
/// before a copy, it never covers more than the copy holds.
pub(crate) fn fingerprint(db: &Database, artwork_dir: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    db.with_conn(|conn| hash_tables(conn, &mut hasher))?;
    hash_files(artwork_dir, &mut hasher)?;
    Ok(hex::encode(&hasher.finalize()))
}

fn hash_tables(conn: &Connection, hasher: &mut Sha256) -> Result<()> {
    let failed = |error: rusqlite::Error| {
        VaultimeError::Backup(format!(
            "failed to read the database for its fingerprint: {error}"
        ))
    };
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(failed)?;
    let tables = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(failed)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(failed)?;
    let mut queries = vec![
        "SELECT id, hash_self FROM session_events ORDER BY rowid DESC LIMIT 1".to_owned(),
        "SELECT key_id, sequence, hash_self FROM ledger_entries ORDER BY rowid DESC LIMIT 1"
            .to_owned(),
        "SELECT COUNT(*) FROM sessions".to_owned(),
    ];
    for table in tables
        .iter()
        .filter(|table| !READ_OTHERWISE.contains(&table.as_str()))
    {
        // A setting written again with the same value is no change.
        // A merge notes the newest backup it took of each PC, also one that
        // brought nothing, and merging that one again would bring nothing.
        queries.push(if table == "settings" {
            format!(
                "SELECT key, value FROM settings WHERE key <> '{}'",
                crate::constants::MERGED_BACKUPS_SETTING
            )
        } else {
            format!("SELECT * FROM \"{table}\"")
        });
    }
    for query in &queries {
        hasher.update(query.as_bytes());
        let mut stmt = conn.prepare(query).map_err(failed)?;
        let width = stmt.column_count();
        let mut rows = stmt.query([]).map_err(failed)?;
        while let Some(row) = rows.next().map_err(failed)? {
            for column in 0..width {
                hash_value(hasher, row.get_ref(column).map_err(failed)?);
            }
        }
    }
    Ok(())
}

/// Hashes a value with its kind, so values of other kinds never read alike.
fn hash_value(hasher: &mut Sha256, value: ValueRef<'_>) {
    match value {
        ValueRef::Null => hasher.update(b"n"),
        ValueRef::Integer(number) => {
            hasher.update(b"i");
            hasher.update(number.to_le_bytes());
        }
        ValueRef::Real(number) => {
            hasher.update(b"r");
            hasher.update(number.to_le_bytes());
        }
        ValueRef::Text(bytes) => hash_bytes(hasher, b"t", bytes),
        ValueRef::Blob(bytes) => hash_bytes(hasher, b"b", bytes),
    }
}

/// Hashes bytes after their kind and length, so where they end is known.
fn hash_bytes(hasher: &mut Sha256, kind: &[u8], bytes: &[u8]) {
    hasher.update(kind);
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

/// Hashes the path, size and change time of every file below `dir`.
fn hash_files(dir: &Path, hasher: &mut Sha256) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let failed = |error: walkdir::Error| {
        VaultimeError::Backup(format!(
            "failed to list the artwork for its fingerprint: {error}"
        ))
    };
    for entry in WalkDir::new(dir).min_depth(1).sort_by_file_name() {
        let entry = entry.map_err(failed)?;
        if crate::assets::art::is_derived(entry.path()) {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            // Gone since the folder was listed, as the backup will find it.
            Err(error)
                if error.io_error().map(std::io::Error::kind)
                    == Some(std::io::ErrorKind::NotFound) =>
            {
                continue;
            }
            Err(error) => return Err(failed(error)),
        };
        if !meta.is_file() {
            continue;
        }
        let changed = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_nanos());
        let path = entry.path().strip_prefix(dir).unwrap_or(entry.path());
        hash_bytes(hasher, b"f", path.to_string_lossy().as_bytes());
        hasher.update(meta.len().to_le_bytes());
        hasher.update(changed.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::db::models::CreateGame;
    use crate::db::repo::{
        annotations, backup_snapshots, devices, game_assets, games, map_db, seen, sessions,
        settings,
    };
    use crate::integrity::STATUS_LOCAL;

    #[test]
    fn follows_what_a_backup_holds_and_nothing_else() {
        let db = Database::open_in_memory().unwrap();
        devices::ensure_device(&db, "pc", "linux", "0.1.0").unwrap();
        let artwork =
            std::env::temp_dir().join(format!("vaultime-fingerprint-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&artwork).unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Hades".into(),
                executable_path: Some("/games/hades.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        settings::set_setting(&db, "look", "dark").unwrap();
        let print = || fingerprint(&db, &artwork).unwrap();
        let first = print();
        assert_eq!(print(), first);

        // A setting written again, a program seen running, a backup noted.
        db.with_conn(|conn| {
            conn.execute("UPDATE settings SET updated_at = '2000-01-01 00:00:00'", [])
                .map_err(map_db)
        })
        .unwrap();
        settings::set_setting(&db, "look", "dark").unwrap();
        settings::set_setting(&db, crate::constants::MERGED_BACKUPS_SETTING, "{}").unwrap();
        seen::add_front_time(&db, "/games/other.exe", "Other", 60).unwrap();
        for derived in ["cover.thumb.jpg", "cover.tint.json", "cover.tint.0f3a.tmp"] {
            fs::write(artwork.join(derived), b"made from the cover").unwrap();
        }
        backup_snapshots::create_snapshot(&db, Some("pc"), "sum", Some("/backups/1"), None)
            .unwrap();
        assert_eq!(print(), first);

        let mut prints = vec![first];
        settings::set_setting(&db, "look", "light").unwrap();
        prints.push(print());
        games::set_favorite(&db, &game.id, true).unwrap();
        prints.push(print());
        let session = sessions::create_session(&db, &game.id, "pc").unwrap();
        prints.push(print());
        sessions::end_session(&db, &session.id, 60_000, 60_000, 0, STATUS_LOCAL).unwrap();
        prints.push(print());
        annotations::set_session_note(&db, &session.id, "A long evening").unwrap();
        prints.push(print());
        annotations::set_game_status(&db, &game.id, "finished").unwrap();
        prints.push(print());
        let copy = games::create_game(
            &db,
            &CreateGame {
                title: "Hades, second copy".into(),
                executable_path: Some("/games/copy/hades.exe".into()),
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        prints.push(print());
        games::link_game(&db, &copy.id, Some(&game.id)).unwrap();
        prints.push(print());
        game_assets::create_asset(&db, &game.id, "cover", "scan", "/art/cover.jpg", None, None)
            .unwrap();
        prints.push(print());
        // Only a ledger entry, as when the key of another PC is trusted.
        db.with_conn(|conn| {
            crate::integrity::ledger::trust_keys(
                conn,
                "laptop",
                &std::collections::HashSet::from(["ab".repeat(32)]),
            )
        })
        .unwrap();
        prints.push(print());
        fs::write(artwork.join("cover.jpg"), b"cover").unwrap();
        prints.push(print());
        fs::write(artwork.join("cover.jpg"), b"other cover").unwrap();
        prints.push(print());
        let changed: std::collections::HashSet<_> = prints.iter().collect();
        assert_eq!(changed.len(), prints.len(), "each change shows");
        fs::remove_dir_all(&artwork).ok();
    }
}
