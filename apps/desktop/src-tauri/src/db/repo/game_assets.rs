// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Queries for the `game_assets` table.

use rusqlite::{Row, params};

use crate::db::connection::Database;
use crate::db::models::GameAsset;
use crate::db::repo::map_db;
use crate::error::Result;

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

/// Removes one asset row. False when there was none.
pub fn delete_asset(db: &Database, asset_id: &str) -> Result<bool> {
    db.with_conn(|conn| {
        let deleted = conn
            .execute("DELETE FROM game_assets WHERE id = ?1", [asset_id])
            .map_err(map_db)?;
        Ok(deleted > 0)
    })
}

/// Points an asset at a new cached image, as after the player cropped it.
pub fn replace_asset_image(
    db: &Database,
    asset_id: &str,
    asset_type: &str,
    source: &str,
    cache_path: &str,
) -> Result<GameAsset> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE game_assets SET asset_type = ?2, source = ?3, cache_path = ?4 WHERE id = ?1",
            params![asset_id, asset_type, source, cache_path],
        )
        .map_err(map_db)?;
        conn.query_row(
            "SELECT * FROM game_assets WHERE id = ?1",
            [asset_id],
            row_to_asset,
        )
        .map_err(map_db)
    })
}
