// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Artwork scanning, thumbnail caching and cover selection.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use image::ImageFormat;
use image::ImageReader;
use image::imageops::FilterType;
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::db::connection::Database;
use crate::db::models::{Game, GameAsset, GameMetadata};
use crate::db::repo::{game_assets, games};
use crate::error::{Result, VaultimeError};

const MAX_SCAN_DEPTH: usize = 3;
const MAX_SCANNED_ASSETS: usize = 10;
const MAX_LIBRARY_PREVIEWS: usize = 20;
const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "ico"];

#[derive(Debug)]
pub struct AssetManager {
    cache_dir: PathBuf,
}

impl AssetManager {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GameAssetView {
    pub id: String,
    pub game_id: String,
    pub asset_type: String,
    pub source: String,
    pub file_path: String,
    pub cache_path: Option<String>,
    pub hash: Option<String>,
    pub created_at: String,
    pub preview_data_url: Option<String>,
    pub is_preferred: bool,
}

pub fn list_game_assets(
    db: &Database,
    asset_manager: &AssetManager,
    game_id: &str,
) -> Result<Vec<GameAssetView>> {
    let game = games::get_game(db, game_id)?;
    let assets = game_assets::list_assets_for_game(db, game_id)?;
    let preferred_asset_id = preferred_asset_id(&game);
    build_asset_views(&assets, preferred_asset_id.as_deref(), asset_manager)
}

pub fn list_preferred_game_assets(
    db: &Database,
    asset_manager: &AssetManager,
) -> Result<Vec<GameAssetView>> {
    let games = games::list_games(db)?;
    let mut views = Vec::new();

    for game in games {
        let assets = game_assets::list_assets_for_game(db, &game.id)?;
        let preferred_asset_id = preferred_asset_id(&game);
        let maybe_view = build_asset_views(&assets, preferred_asset_id.as_deref(), asset_manager)?
            .into_iter()
            .find(|asset| asset.is_preferred)
            .or_else(|| {
                build_asset_views(&assets, None, asset_manager)
                    .ok()
                    .and_then(|mut asset_views| asset_views.drain(..).next())
            });

        if let Some(view) = maybe_view {
            views.push(view);
        }
    }

    Ok(views)
}

pub fn scan_game_assets(
    db: &Database,
    asset_manager: &AssetManager,
    game_id: &str,
) -> Result<Vec<GameAssetView>> {
    let game = games::get_game(db, game_id)?;
    let previous_preferred_asset_id = preferred_asset_id(&game);

    let removed_assets = game_assets::delete_non_user_assets_for_game(db, game_id)?;
    cleanup_assets(&removed_assets);

    let candidates = find_candidates(&game)?;
    let mut inserted_assets = Vec::new();

    for candidate in candidates.into_iter().take(MAX_SCANNED_ASSETS) {
        match cache_candidate(asset_manager, &game, &candidate, false) {
            Ok(cached) => {
                let inserted = game_assets::create_asset(
                    db,
                    game_id,
                    &cached.asset_type,
                    &cached.source,
                    &cached.file_path,
                    Some(&cached.cache_path),
                    Some(&cached.hash),
                )?;
                inserted_assets.push(inserted);
            }
            Err(error) => {
                log::warn!(
                    "failed to cache artwork candidate {} for game {}: {}",
                    candidate.path.display(),
                    game.title,
                    error
                );
            }
        }
    }

    let all_assets = game_assets::list_assets_for_game(db, game_id)?;
    ensure_preferred_asset(
        db,
        &game,
        &all_assets,
        previous_preferred_asset_id.as_deref(),
    )?;

    list_game_assets(db, asset_manager, game_id)
}

pub fn import_game_asset(
    db: &Database,
    asset_manager: &AssetManager,
    game_id: &str,
    source_path: &str,
) -> Result<Vec<GameAssetView>> {
    let game = games::get_game(db, game_id)?;
    let candidate = AssetCandidate {
        path: PathBuf::from(source_path),
        asset_type: classify_asset_type(Path::new(source_path)),
        source: "user_picked".into(),
        score: 10_000,
    };

    let cached = cache_candidate(asset_manager, &game, &candidate, true)?;
    let inserted = game_assets::create_asset(
        db,
        game_id,
        &cached.asset_type,
        &cached.source,
        &cached.file_path,
        Some(&cached.cache_path),
        Some(&cached.hash),
    )?;

    set_preferred_game_asset(db, game_id, &inserted.id)?;
    list_game_assets(db, asset_manager, game_id)
}

pub fn set_preferred_game_asset(db: &Database, game_id: &str, asset_id: &str) -> Result<bool> {
    let game = games::get_game(db, game_id)?;
    let asset = game_assets::get_asset(db, asset_id)?;
    if asset.game_id != game.id {
        return Err(VaultimeError::Asset(
            "asset does not belong to the requested game".into(),
        ));
    }

    let mut metadata = parse_game_metadata(&game);
    metadata.preferred_cover_asset_id = Some(asset.id);
    games::set_metadata(db, &game.id, &metadata)?;
    Ok(true)
}

fn ensure_preferred_asset(
    db: &Database,
    game: &Game,
    assets: &[GameAsset],
    previous_preferred_asset_id: Option<&str>,
) -> Result<()> {
    let next_preferred = previous_preferred_asset_id
        .and_then(|asset_id| assets.iter().find(|asset| asset.id == asset_id))
        .or_else(|| assets.iter().find(|asset| asset.source == "user_picked"))
        .or_else(|| assets.first())
        .map(|asset| asset.id.clone());

    let mut metadata = parse_game_metadata(game);
    metadata.preferred_cover_asset_id = next_preferred;
    games::set_metadata(db, &game.id, &metadata)?;
    Ok(())
}

fn build_asset_views(
    assets: &[GameAsset],
    preferred_asset_id: Option<&str>,
    asset_manager: &AssetManager,
) -> Result<Vec<GameAssetView>> {
    let resolved_preferred = preferred_asset_id
        .and_then(|asset_id| assets.iter().find(|asset| asset.id == asset_id))
        .map(|asset| asset.id.clone())
        .or_else(|| assets.first().map(|asset| asset.id.clone()));

    let mut views = Vec::new();
    for asset in assets.iter().take(MAX_LIBRARY_PREVIEWS) {
        views.push(GameAssetView {
            id: asset.id.clone(),
            game_id: asset.game_id.clone(),
            asset_type: asset.asset_type.clone(),
            source: asset.source.clone(),
            file_path: asset.file_path.clone(),
            cache_path: asset.cache_path.clone(),
            hash: asset.hash.clone(),
            created_at: asset.created_at.clone(),
            preview_data_url: build_preview_data_url(asset, asset_manager).ok(),
            is_preferred: resolved_preferred
                .as_deref()
                .is_some_and(|preferred_id| preferred_id == asset.id),
        });
    }

    Ok(views)
}

fn build_preview_data_url(asset: &GameAsset, asset_manager: &AssetManager) -> Result<String> {
    let data_path = asset
        .cache_path
        .as_deref()
        .map_or_else(|| PathBuf::from(&asset.file_path), PathBuf::from);

    let bytes = fs::read(&data_path).map_err(|e| {
        VaultimeError::Asset(format!(
            "failed to read cached asset {}: {e}",
            data_path.display()
        ))
    })?;

    let mime = if asset.cache_path.is_some() {
        "image/png"
    } else {
        mime_for_path(&data_path)
    };

    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let _ = asset_manager;
    Ok(format!("data:{mime};base64,{encoded}"))
}

fn find_candidates(game: &Game) -> Result<Vec<AssetCandidate>> {
    let roots = scan_roots(game);
    if roots.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    for root in roots {
        for entry in WalkDir::new(root)
            .max_depth(MAX_SCAN_DEPTH)
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if !is_supported_image(path) {
                continue;
            }

            let canonical = path.to_string_lossy().to_string();
            if !seen.insert(canonical.clone()) {
                continue;
            }

            candidates.push(AssetCandidate {
                path: PathBuf::from(canonical),
                asset_type: classify_asset_type(path),
                source: "scanned_local".into(),
                score: score_candidate(path),
            });
        }
    }

    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(candidates)
}

fn scan_roots(game: &Game) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(folder) = &game.install_folder {
        let path = PathBuf::from(folder);
        if path.is_dir() {
            roots.push(path);
        }
    }

    if let Some(executable) = &game.executable_path {
        let executable_path = PathBuf::from(executable);
        if let Some(parent) = executable_path.parent() {
            let path = parent.to_path_buf();
            if path.is_dir() && !roots.contains(&path) {
                roots.push(path);
            }
        }
    }

    roots
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            SUPPORTED_IMAGE_EXTENSIONS
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(extension))
        })
}

fn classify_asset_type(path: &Path) -> String {
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if name.contains("banner") || name.contains("hero") {
        "banner".into()
    } else if name.contains("icon") || name.contains("logo") {
        "icon".into()
    } else if name.contains("shot") || name.contains("screen") {
        "screenshot".into()
    } else {
        "cover".into()
    }
}

fn score_candidate(path: &Path) -> i32 {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let path_text = path.to_string_lossy().to_ascii_lowercase();

    let mut score = 0;
    if file_name.contains("cover") || file_name.contains("capsule") {
        score += 120;
    }
    if file_name.contains("poster") || file_name.contains("banner") {
        score += 100;
    }
    if file_name.contains("hero") || file_name.contains("art") {
        score += 80;
    }
    if file_name.contains("logo") || file_name.contains("icon") {
        score += 60;
    }
    if file_name.contains("screenshot") || file_name.contains("screen") {
        score += 20;
    }
    if path_text.contains("screenshot") {
        score -= 20;
    }

    let depth_penalty = path.components().count() as i32;
    score - depth_penalty
}

fn cache_candidate(
    asset_manager: &AssetManager,
    game: &Game,
    candidate: &AssetCandidate,
    prefer_cover: bool,
) -> Result<CachedAsset> {
    let source_bytes = fs::read(&candidate.path).map_err(|e| {
        VaultimeError::Asset(format!(
            "failed to read asset source {}: {e}",
            candidate.path.display()
        ))
    })?;

    let source_hash = crate::hex::encode(&Sha256::digest(&source_bytes));
    let mut reader = ImageReader::open(&candidate.path).map_err(|e| {
        VaultimeError::Asset(format!(
            "failed to open image {}: {e}",
            candidate.path.display()
        ))
    })?;
    reader = reader.with_guessed_format().map_err(|e| {
        VaultimeError::Asset(format!(
            "failed to detect image format {}: {e}",
            candidate.path.display()
        ))
    })?;
    let image = reader.decode().map_err(|e| {
        VaultimeError::Asset(format!(
            "failed to decode image {}: {e}",
            candidate.path.display()
        ))
    })?;

    let asset_type = if prefer_cover {
        "cover"
    } else {
        &candidate.asset_type
    };
    let processed = process_image(image, asset_type);

    let game_cache_dir = asset_manager.cache_dir().join(&game.id);
    fs::create_dir_all(&game_cache_dir).map_err(|e| {
        VaultimeError::Asset(format!(
            "failed to create game asset cache {}: {e}",
            game_cache_dir.display()
        ))
    })?;

    let asset_id = uuid::Uuid::new_v4().to_string();
    let cache_path = game_cache_dir.join(format!("{asset_id}.png"));
    processed
        .save_with_format(&cache_path, ImageFormat::Png)
        .map_err(|e| {
            VaultimeError::Asset(format!(
                "failed to write cached image {}: {e}",
                cache_path.display()
            ))
        })?;

    Ok(CachedAsset {
        file_path: candidate.path.to_string_lossy().to_string(),
        cache_path: cache_path.to_string_lossy().to_string(),
        asset_type: asset_type.into(),
        source: candidate.source.clone(),
        hash: source_hash,
    })
}

fn process_image(image: image::DynamicImage, asset_type: &str) -> image::DynamicImage {
    match asset_type {
        "banner" => image.resize_to_fill(1280, 720, FilterType::Lanczos3),
        "icon" => image.thumbnail(512, 512),
        "screenshot" => image.resize(1280, 720, FilterType::Lanczos3),
        _ => image.resize_to_fill(720, 960, FilterType::Lanczos3),
    }
}

fn cleanup_assets(assets: &[GameAsset]) {
    for asset in assets {
        if let Some(cache_path) = &asset.cache_path {
            let _ = fs::remove_file(cache_path);
        }
    }
}

fn parse_game_metadata(game: &Game) -> GameMetadata {
    serde_json::from_str(&game.metadata_json).unwrap_or_default()
}

fn preferred_asset_id(game: &Game) -> Option<String> {
    parse_game_metadata(game).preferred_cover_asset_id
}

fn mime_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        _ => "image/png",
    }
}

#[derive(Debug)]
struct AssetCandidate {
    path: PathBuf,
    asset_type: String,
    source: String,
    score: i32,
}

#[derive(Debug)]
struct CachedAsset {
    file_path: String,
    cache_path: String,
    asset_type: String,
    source: String,
    hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_cover_asset() {
        assert_eq!(
            classify_asset_type(Path::new("/games/CoolGame/cover.png")),
            "cover"
        );
    }

    #[test]
    fn scores_cover_above_screenshot() {
        assert!(
            score_candidate(Path::new("/games/cover.png"))
                > score_candidate(Path::new("/games/screenshots/shot01.png"))
        );
    }
}
