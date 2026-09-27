// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Artwork scanning, thumbnail caching and cover selection.

use std::collections::HashSet;
use std::fs;
use std::io::{Cursor, ErrorKind};
use std::path::{Component, Path, PathBuf};

use base64::Engine;
use image::ImageFormat;
use image::ImageReader;
use image::imageops::FilterType;
use log::warn;
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::constants::{
    ARTWORK_BACKFILL_SETTING, ARTWORK_BACKFILL_VERSION, ARTWORK_PENALTY_SCREENSHOT_PATH,
    ARTWORK_SCORE_COVER, ARTWORK_SCORE_HERO, ARTWORK_SCORE_LOGO, ARTWORK_SCORE_POSTER,
    ARTWORK_SCORE_SCREENSHOT, ARTWORK_SCORE_STEAM_COVER, ARTWORK_SCORE_USER_PICKED,
    ASSET_SCAN_DEPTH, BANNER_HEIGHT_PX, BANNER_WIDTH_PX, CACHED_JPEG_QUALITY, COVER_HEIGHT_PX,
    COVER_WIDTH_PX, ICON_MAX_SIZE_PX, MAX_LIBRARY_PREVIEWS, MAX_SCANNED_ASSETS,
    SCREENSHOT_MAX_HEIGHT_PX, SCREENSHOT_MAX_WIDTH_PX,
};
use crate::db::connection::Database;
use crate::db::models::{Game, GameAsset, GameMetadata};
use crate::db::repo::{game_assets, games, settings};
use crate::discovery::steam;
use crate::error::{Result, VaultimeError};

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

pub fn list_game_assets(db: &Database, game_id: &str) -> Result<Vec<GameAssetView>> {
    let game = games::get_game(db, game_id)?;
    let assets = game_assets::list_assets_for_game(db, game_id)?;
    let preferred_asset_id = preferred_asset_id(&game);
    Ok(build_asset_views(&assets, preferred_asset_id.as_deref()))
}

/// One cover per game with a preview. When the preferred cover cannot be read,
/// the next readable asset stands in.
pub fn list_preferred_game_assets(db: &Database) -> Result<Vec<GameAssetView>> {
    let mut views = Vec::new();

    for game in games::list_games(db)? {
        let assets = game_assets::list_assets_for_game(db, &game.id)?;
        let preferred = resolve_preferred(&assets, preferred_asset_id(&game).as_deref());
        let is_preferred = |asset: &GameAsset| preferred.is_some_and(|p| p.id == asset.id);
        let cover = preferred
            .into_iter()
            .chain(assets.iter().filter(|asset| !is_preferred(asset)))
            .find_map(|asset| {
                let preview = build_preview_data_url(asset).ok()?;
                Some(asset_view(asset, Some(preview), is_preferred(asset)))
            });

        views.extend(cover);
    }

    Ok(views)
}

/// Deletes a game and then its cached artwork.
pub fn delete_game(db: &Database, asset_manager: &AssetManager, game_id: &str) -> Result<bool> {
    let deleted = games::delete_game(db, game_id)?;
    if deleted {
        remove_game_cache(asset_manager, game_id);
    }
    Ok(deleted)
}

/// A failure only leaves unused files behind, so it is logged and not returned.
fn remove_game_cache(asset_manager: &AssetManager, game_id: &str) {
    // Only a plain folder name, so an odd id from a restored backup cannot
    // point outside the cache.
    let mut components = Path::new(game_id).components();
    if !matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    ) {
        return;
    }

    let game_cache_dir = asset_manager.cache_dir().join(game_id);
    match fs::remove_dir_all(&game_cache_dir) {
        Err(error) if error.kind() != ErrorKind::NotFound => warn!(
            "failed to remove cached artwork {}: {error}",
            game_cache_dir.display()
        ),
        _ => {}
    }
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
    // Candidates were cached best first, so the first new one is the best.
    ensure_preferred_asset(
        db,
        &game,
        &all_assets,
        previous_preferred_asset_id.as_deref(),
        inserted_assets.first().map(|asset| asset.id.as_str()),
    )?;

    list_game_assets(db, game_id)
}

/// Scans every Steam game once for the covers Steam keeps, skipping games
/// that already have one or an image the user added. Returns how many games
/// were scanned. Runs again only when `ARTWORK_BACKFILL_VERSION` changes.
pub fn backfill_steam_covers(db: &Database, asset_manager: &AssetManager) -> Result<usize> {
    if settings::get_setting(db, ARTWORK_BACKFILL_SETTING)?.as_deref()
        == Some(ARTWORK_BACKFILL_VERSION)
    {
        return Ok(0);
    }

    let mut scanned = 0;
    for game in games::list_all_games(db)? {
        if game.launcher_source.as_deref() != Some("steam") {
            continue;
        }
        let assets = game_assets::list_assets_for_game(db, &game.id)?;
        if assets
            .iter()
            .any(|asset| asset.source == "steam_cache" || asset.source == "user_picked")
        {
            continue;
        }
        match scan_game_assets(db, asset_manager, &game.id) {
            Ok(_) => scanned += 1,
            Err(error) => warn!("artwork backfill failed for {}: {error}", game.title),
        }
    }

    settings::set_setting(db, ARTWORK_BACKFILL_SETTING, ARTWORK_BACKFILL_VERSION)?;
    Ok(scanned)
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
        score: ARTWORK_SCORE_USER_PICKED,
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
    list_game_assets(db, game_id)
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
    best_scanned_asset_id: Option<&str>,
) -> Result<()> {
    let find = |asset_id: &str| assets.iter().find(|asset| asset.id == asset_id);
    let next_preferred = previous_preferred_asset_id
        .and_then(find)
        .or_else(|| assets.iter().find(|asset| asset.source == "user_picked"))
        .or_else(|| best_scanned_asset_id.and_then(find))
        .or_else(|| assets.first())
        .map(|asset| asset.id.clone());

    let mut metadata = parse_game_metadata(game);
    metadata.preferred_cover_asset_id = next_preferred;
    games::set_metadata(db, &game.id, &metadata)?;
    Ok(())
}

/// The preferred asset while it still exists, otherwise the first one.
fn resolve_preferred<'a>(
    assets: &'a [GameAsset],
    preferred_asset_id: Option<&str>,
) -> Option<&'a GameAsset> {
    preferred_asset_id
        .and_then(|asset_id| assets.iter().find(|asset| asset.id == asset_id))
        .or_else(|| assets.first())
}

fn build_asset_views(assets: &[GameAsset], preferred_asset_id: Option<&str>) -> Vec<GameAssetView> {
    let preferred = resolve_preferred(assets, preferred_asset_id);
    assets
        .iter()
        .take(MAX_LIBRARY_PREVIEWS)
        .map(|asset| {
            asset_view(
                asset,
                build_preview_data_url(asset).ok(),
                preferred.is_some_and(|p| p.id == asset.id),
            )
        })
        .collect()
}

fn asset_view(
    asset: &GameAsset,
    preview_data_url: Option<String>,
    is_preferred: bool,
) -> GameAssetView {
    GameAssetView {
        id: asset.id.clone(),
        game_id: asset.game_id.clone(),
        asset_type: asset.asset_type.clone(),
        source: asset.source.clone(),
        file_path: asset.file_path.clone(),
        cache_path: asset.cache_path.clone(),
        hash: asset.hash.clone(),
        created_at: asset.created_at.clone(),
        preview_data_url,
        is_preferred,
    }
}

fn build_preview_data_url(asset: &GameAsset) -> Result<String> {
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

    let mime = mime_for_path(&data_path);

    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

fn find_candidates(game: &Game) -> Result<Vec<AssetCandidate>> {
    let roots = scan_roots(game);
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    // Steam keeps a portrait cover for every game it installs.
    if game.launcher_source.as_deref() == Some("steam")
        && let Some(folder) = &game.install_folder
        && let Some(cover) = steam::cached_cover(Path::new(folder))
    {
        seen.insert(cover.to_string_lossy().into_owned());
        candidates.push(AssetCandidate {
            path: cover,
            asset_type: "cover".into(),
            source: "steam_cache".into(),
            score: ARTWORK_SCORE_STEAM_COVER,
        });
    }

    for root in roots {
        for entry in WalkDir::new(root)
            .max_depth(ASSET_SCAN_DEPTH)
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
        score += ARTWORK_SCORE_COVER;
    }
    if file_name.contains("poster") || file_name.contains("banner") {
        score += ARTWORK_SCORE_POSTER;
    }
    if file_name.contains("hero") || file_name.contains("art") {
        score += ARTWORK_SCORE_HERO;
    }
    if file_name.contains("logo") || file_name.contains("icon") {
        score += ARTWORK_SCORE_LOGO;
    }
    if file_name.contains("screenshot") || file_name.contains("screen") {
        score += ARTWORK_SCORE_SCREENSHOT;
    }
    if path_text.contains("screenshot") {
        score -= ARTWORK_PENALTY_SCREENSHOT_PATH;
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
    let reader = ImageReader::new(Cursor::new(&source_bytes))
        .with_guessed_format()
        .map_err(|e| {
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

    // Artwork is opaque and much smaller as JPEG, icons keep their transparency.
    let asset_id = uuid::Uuid::new_v4().to_string();
    let as_jpeg = asset_type != "icon";
    let cache_path = game_cache_dir.join(format!(
        "{asset_id}.{}",
        if as_jpeg { "jpg" } else { "png" }
    ));
    let written = if as_jpeg {
        fs::File::create(&cache_path)
            .map_err(image::ImageError::IoError)
            .and_then(|file| {
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    std::io::BufWriter::new(file),
                    CACHED_JPEG_QUALITY,
                );
                encoder.encode_image(&processed.to_rgb8())
            })
    } else {
        processed.save_with_format(&cache_path, ImageFormat::Png)
    };
    written.map_err(|e| {
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
        "banner" => image.resize_to_fill(BANNER_WIDTH_PX, BANNER_HEIGHT_PX, FilterType::Lanczos3),
        "icon" => image.thumbnail(ICON_MAX_SIZE_PX, ICON_MAX_SIZE_PX),
        "screenshot" => image.resize(
            SCREENSHOT_MAX_WIDTH_PX,
            SCREENSHOT_MAX_HEIGHT_PX,
            FilterType::Lanczos3,
        ),
        _ => image.resize_to_fill(COVER_WIDTH_PX, COVER_HEIGHT_PX, FilterType::Lanczos3),
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
    use crate::db::models::CreateGame;

    fn create_game(db: &Database) -> Game {
        games::create_game(
            db,
            &CreateGame {
                title: "Cached Game".into(),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap()
    }

    /// Scans a real Steam game and prints what it picked as the cover. Set
    /// `VAULTIME_GAME_FOLDER` to a Steam install folder and run
    /// `cargo test -- --ignored --nocapture real_steam_cover`.
    #[test]
    #[ignore = "reads a local Steam install"]
    fn picks_a_real_steam_cover() {
        let folder = std::env::var("VAULTIME_GAME_FOLDER").unwrap();
        let db = Database::open_in_memory().unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Real Game".into(),
                executable_path: None,
                install_folder: Some(folder),
                launcher_source: Some("steam".into()),
            },
        )
        .unwrap();
        let cache = test_cache();
        let assets = scan_game_assets(&db, &cache, &game.id).unwrap();
        for asset in &assets {
            println!(
                "asset {} {} preferred={}",
                asset.source, asset.file_path, asset.is_preferred
            );
        }
        let preferred = assets.iter().find(|asset| asset.is_preferred).unwrap();
        println!(
            "{} assets, cover {} from {}, preview {} bytes",
            assets.len(),
            preferred.file_path,
            preferred.source,
            preferred.preview_data_url.as_ref().map_or(0, String::len)
        );
        assert_eq!(preferred.source, "steam_cache");
        fs::remove_dir_all(cache.cache_dir()).unwrap();
    }

    fn test_cache() -> AssetManager {
        let dir =
            std::env::temp_dir().join(format!("vaultime-asset-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        AssetManager::new(dir)
    }

    #[test]
    fn backfill_runs_once_and_skips_other_games() {
        let db = Database::open_in_memory().unwrap();
        create_game(&db);
        let cache = test_cache();
        assert_eq!(backfill_steam_covers(&db, &cache).unwrap(), 0);
        assert_eq!(
            settings::get_setting(&db, ARTWORK_BACKFILL_SETTING)
                .unwrap()
                .as_deref(),
            Some(ARTWORK_BACKFILL_VERSION)
        );

        games::create_game(
            &db,
            &CreateGame {
                title: "Steam Game".into(),
                executable_path: None,
                install_folder: None,
                launcher_source: Some("steam".into()),
            },
        )
        .unwrap();
        // Already ran for this version, so nothing is scanned.
        assert_eq!(backfill_steam_covers(&db, &cache).unwrap(), 0);
        settings::set_setting(&db, ARTWORK_BACKFILL_SETTING, "0").unwrap();
        assert_eq!(backfill_steam_covers(&db, &cache).unwrap(), 1);
        fs::remove_dir_all(cache.cache_dir()).unwrap();
    }

    #[test]
    fn deleting_a_game_removes_its_cached_artwork() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let game = create_game(&db);
        let other = create_game(&db);
        for id in [&game.id, &other.id] {
            let dir = asset_manager.cache_dir().join(id);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("cover.png"), b"png").unwrap();
        }

        assert!(delete_game(&db, &asset_manager, &game.id).unwrap());
        assert!(!asset_manager.cache_dir().join(&game.id).exists());
        assert!(asset_manager.cache_dir().join(&other.id).exists());

        // A game without cached artwork deletes cleanly.
        fs::remove_dir_all(asset_manager.cache_dir().join(&other.id)).unwrap();
        assert!(delete_game(&db, &asset_manager, &other.id).unwrap());

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

    #[test]
    fn cover_falls_back_when_the_preferred_file_is_missing() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let game = create_game(&db);
        let readable = asset_manager.cache_dir().join("readable.png");
        fs::write(&readable, b"png").unwrap();
        let add_asset = |cache_path: &Path| {
            game_assets::create_asset(
                &db,
                &game.id,
                "cover",
                "scanned_local",
                "/games/cover.png",
                Some(&cache_path.to_string_lossy()),
                None,
            )
            .unwrap()
        };
        let missing = add_asset(&asset_manager.cache_dir().join("missing.png"));
        let fallback = add_asset(&readable);
        set_preferred_game_asset(&db, &game.id, &missing.id).unwrap();

        let covers = list_preferred_game_assets(&db).unwrap();
        assert_eq!(covers.len(), 1);
        assert_eq!(covers[0].id, fallback.id);
        assert!(covers[0].preview_data_url.is_some());

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

    #[test]
    fn cache_removal_ignores_ids_that_are_not_a_folder_name() {
        let asset_manager = test_cache();
        let outside = asset_manager.cache_dir().join("outside");
        let cache = AssetManager::new(asset_manager.cache_dir().join("cache"));
        fs::create_dir_all(&outside).unwrap();
        fs::create_dir_all(cache.cache_dir()).unwrap();

        remove_game_cache(&cache, "../outside");
        assert!(outside.exists());

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

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
