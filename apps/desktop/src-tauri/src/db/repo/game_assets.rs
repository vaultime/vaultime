// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Queries for the `game_assets` table.

use rusqlite::{Row, params};

use crate::db::connection::Database;
use crate::db::models::GameAsset;
use crate::error::{Result, VaultimeError};

fn row_to_asset(row: &Row) -> rusqlite::Result<GameAsset> {
    Ok(GameAsset {
        id: row.get("id")?,
        game_id: row.get("game_id")?,
        asset_type: row.get("asset_type")?,
        source: row.get("source")?,
        file_path: row.get("file_path")?,
        cache_path: row.get("cache_path")?,
        hash: row.get("hash")?,
        created_at: row.get("created_at")?,
    })
}

fn map_db(e: rusqlite::Error) -> VaultimeError {
    VaultimeError::Database(format!("{e}"))
}

pub fn create_asset(
    db: &Database,
    game_id: &str,
    asset_type: &str,
    source: &str,
    file_path: &str,
    cache_path: Option<&str>,
    hash: Option<&str>,
) -> Result<GameAsset> {
    let id = uuid::Uuid::new_v4().to_string();

    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO game_assets
                (id, game_id, asset_type, source, file_path, cache_path, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, game_id, asset_type, source, file_path, cache_path, hash],
        )
        .map_err(map_db)?;

        conn.query_row(
            "SELECT * FROM game_assets WHERE id = ?1",
            [&id],
            row_to_asset,
        )
        .map_err(map_db)
    })
}

pub fn list_assets_for_game(db: &Database, game_id: &str) -> Result<Vec<GameAsset>> {
    db.with_conn(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM game_assets
                 WHERE game_id = ?1
                 ORDER BY created_at DESC, asset_type ASC",
            )
            .map_err(map_db)?;

        let rows = stmt.query_map([game_id], row_to_asset).map_err(map_db)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db)
    })
}

pub fn get_asset(db: &Database, asset_id: &str) -> Result<GameAsset> {
    db.with_conn(|conn| {
        conn.query_row(
            "SELECT * FROM game_assets WHERE id = ?1",
            [asset_id],
            row_to_asset,
        )
        .map_err(map_db)
    })
}

pub fn delete_non_user_assets_for_game(db: &Database, game_id: &str) -> Result<Vec<GameAsset>> {
    let assets = list_assets_for_game(db, game_id)?;
    let removable: Vec<GameAsset> = assets
        .into_iter()
        .filter(|asset| asset.source != "user_picked")
        .collect();

    db.with_conn(|conn| {
        for asset in &removable {
            conn.execute("DELETE FROM game_assets WHERE id = ?1", [&asset.id])
                .map_err(map_db)?;
        }
        Ok(removable)
    })
}
