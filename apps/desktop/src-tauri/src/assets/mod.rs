// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Artwork scanning, thumbnail caching and cover selection.

pub mod crop;

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use base64::Engine;
use image::ImageFormat;
use image::imageops::FilterType;
use log::warn;
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::constants::{
    ARTWORK_BACKFILL_SETTING, ARTWORK_BACKFILL_VERSION, ARTWORK_PENALTY_SCREENSHOT_PATH,
    ARTWORK_SCORE_COVER, ARTWORK_SCORE_HERO, ARTWORK_SCORE_LOGO, ARTWORK_SCORE_POSTER,
    ARTWORK_SCORE_SCREENSHOT, ARTWORK_SCORE_STEAM_COVER, ASSET_SCAN_DEPTH, BANNER_HEIGHT_PX,
    BANNER_WIDTH_PX, CACHED_JPEG_QUALITY, COVER_HEIGHT_PX, COVER_WIDTH_PX, ICON_MAX_SIZE_PX,
    MAX_ARTWORK_SOURCE_BYTES, MAX_LIBRARY_PREVIEWS, MAX_SCANNED_ASSETS, SCREENSHOT_MAX_HEIGHT_PX,
    SCREENSHOT_MAX_WIDTH_PX,
};
use crate::db::connection::Database;
use crate::db::models::{CropRect, Game, GameAsset, GameMetadata};
use crate::db::repo::{game_assets, games, settings};
use crate::discovery::steam;
use crate::error::{Result, VaultimeError};
use crop::{ArtworkSource, decode_artwork, fill_crop, render_cover};

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

    for game in games::list_all_games(db)? {
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

/// A single folder or file name, never a path, so an id or name from a
/// restored backup cannot point outside the folder it is joined to. Names
/// Windows changes or reserves are refused too: it drops trailing dots and
/// spaces, so `...` would name the folder itself.
pub(crate) fn is_plain_name(value: &str) -> bool {
    const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
    let mut components = Path::new(value).components();
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let numbered_device = (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.len() == 4
        && stem.ends_with(|last: char| last.is_ascii_digit());
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    ) && !value.contains(['/', '\\', ':'])
        && !value.ends_with(['.', ' '])
        && !RESERVED.contains(&stem.as_str())
        && !numbered_device
}

/// A failure only leaves unused files behind, so it is logged and not returned.
fn remove_game_cache(asset_manager: &AssetManager, game_id: &str) {
    if !is_plain_name(game_id) {
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
    cleanup_assets(asset_manager, &removed_assets);
    // A file the player already cropped into a cover is not offered again.
    let picked_hashes: HashSet<String> = game_assets::list_assets_for_game(db, game_id)?
        .into_iter()
        .filter_map(|asset| asset.hash)
        .collect();

    // Images the player deleted stay deleted.
    let dismissed = parse_game_metadata(&game).dismissed_artwork;
    let candidates = find_candidates(&game)?
        .into_iter()
        .filter(|candidate| !dismissed.contains(candidate.path.to_string_lossy().as_ref()));
    let mut inserted_assets = Vec::new();

    for candidate in candidates.take(MAX_SCANNED_ASSETS) {
        match cache_candidate(asset_manager, &game, &candidate, false) {
            Ok(cached) if picked_hashes.contains(&cached.hash) => {
                let _ = fs::remove_file(&cached.cache_path);
            }
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

/// Opens an image file for the crop dialog.
pub fn open_artwork_file(source_path: &str) -> Result<ArtworkSource> {
    let path = Path::new(source_path);
    let image = decode_artwork(&read_artwork(path)?, path)?;
    ArtworkSource::new(&image, true, None)
}

/// Opens the image behind a cover for the crop dialog: the original file
/// while it still holds the same image, otherwise the cached cover.
pub fn open_game_asset_source(
    db: &Database,
    game_id: &str,
    asset_id: &str,
) -> Result<ArtworkSource> {
    let (game, asset) = game_and_asset(db, game_id, asset_id)?;
    let (image, from_original) = asset_source(&asset)?;
    let crop = from_original
        .then(|| {
            parse_game_metadata(&game)
                .cover_crops
                .get(&asset.id)
                .copied()
        })
        .flatten();
    ArtworkSource::new(&image, from_original, crop)
}

/// Adds an image file as the game's cover, cut by `crop` or from the middle.
/// A file that is already one of the game's images is cut again instead of
/// added twice.
pub fn import_game_asset(
    db: &Database,
    asset_manager: &AssetManager,
    game_id: &str,
    source_path: &str,
    crop: Option<CropRect>,
) -> Result<Vec<GameAssetView>> {
    let game = games::get_game(db, game_id)?;
    let path = Path::new(source_path);
    let bytes = read_artwork(path)?;
    let hash = crate::hex::encode(&Sha256::digest(&bytes));
    let image = decode_artwork(&bytes, path)?;
    let crop = crop.unwrap_or_else(|| fill_crop(image.width(), image.height()));
    let cache_path = cache_cover(asset_manager, &game, &image, crop)?;

    let existing = game_assets::list_assets_for_game(db, game_id)?
        .into_iter()
        .find(|asset| asset.hash.as_deref() == Some(hash.as_str()));
    let asset = match existing {
        Some(asset) => replace_cover(db, asset_manager, &asset, &cache_path)?,
        None => game_assets::create_asset(
            db,
            game_id,
            "cover",
            "user_picked",
            source_path,
            Some(&cache_path),
            Some(&hash),
        )?,
    };

    use_cover(db, &game, &asset.id, Some(crop))?;
    // A file the player deleted before may come back by hand.
    let game = games::get_game(db, game_id)?;
    let mut metadata = parse_game_metadata(&game);
    if metadata.dismissed_artwork.remove(source_path) {
        games::set_metadata(db, game_id, &metadata)?;
    }
    list_game_assets(db, game_id)
}

/// Deletes one of the game's images with its cached copy, never the file it
/// came from. A scan does not bring it back. When it was the cover, the next
/// image takes over, the player's own first.
pub fn delete_game_asset(
    db: &Database,
    asset_manager: &AssetManager,
    game_id: &str,
    asset_id: &str,
) -> Result<Vec<GameAssetView>> {
    let (game, asset) = game_and_asset(db, game_id, asset_id)?;
    game_assets::delete_asset(db, &asset.id)?;
    cleanup_assets(asset_manager, std::slice::from_ref(&asset));

    let mut metadata = parse_game_metadata(&game);
    let was_cover = metadata.preferred_cover_asset_id.as_deref() == Some(asset.id.as_str());
    metadata.cover_crops.remove(&asset.id);
    metadata.dismissed_artwork.insert(asset.file_path.clone());
    let game = games::set_metadata(db, game_id, &metadata)?;
    if was_cover {
        let remaining = game_assets::list_assets_for_game(db, game_id)?;
        ensure_preferred_asset(db, &game, &remaining, None, None)?;
    }
    list_game_assets(db, game_id)
}

/// Cuts one of the game's images again and makes it the cover. It stays one
/// asset, now marked as picked by the player so a folder scan keeps it.
pub fn crop_game_asset(
    db: &Database,
    asset_manager: &AssetManager,
    game_id: &str,
    asset_id: &str,
    crop: CropRect,
) -> Result<Vec<GameAssetView>> {
    let (game, asset) = game_and_asset(db, game_id, asset_id)?;
    let (image, from_original) = asset_source(&asset)?;
    let cache_path = cache_cover(asset_manager, &game, &image, crop)?;
    let asset = replace_cover(db, asset_manager, &asset, &cache_path)?;
    // A crop of the cached cover says nothing about the original file.
    use_cover(db, &game, &asset.id, from_original.then_some(crop))?;
    list_game_assets(db, game_id)
}

fn game_and_asset(db: &Database, game_id: &str, asset_id: &str) -> Result<(Game, GameAsset)> {
    let game = games::get_game(db, game_id)?;
    let asset = game_assets::get_asset(db, asset_id)?;
    if asset.game_id != game.id {
        return Err(VaultimeError::Asset(
            "asset does not belong to the requested game".into(),
        ));
    }
    Ok((game, asset))
}

/// The original file when it still matches the hash taken when it was added,
/// so a moved or changed file, or a path from another PC, is never used.
/// Otherwise the cached copy.
fn asset_source(asset: &GameAsset) -> Result<(image::DynamicImage, bool)> {
    let original = Path::new(&asset.file_path);
    if let Some(expected) = asset.hash.as_deref()
        && let Ok(bytes) = read_artwork(original)
        && crate::hex::encode(&Sha256::digest(&bytes)) == expected
        && let Ok(image) = decode_artwork(&bytes, original)
    {
        return Ok((image, true));
    }

    let cached = Path::new(
        asset
            .cache_path
            .as_deref()
            .ok_or_else(|| VaultimeError::Asset("artwork has no cached copy".into()))?,
    );
    Ok((decode_artwork(&read_artwork(cached)?, cached)?, false))
}

/// Writes a cropped cover into the game's cache folder and returns its path.
fn cache_cover(
    asset_manager: &AssetManager,
    game: &Game,
    image: &image::DynamicImage,
    crop: CropRect,
) -> Result<String> {
    let cover = image::DynamicImage::ImageRgba8(render_cover(image, crop)?);
    let cache_path = write_cached_image(asset_manager, game, &cover, true)?;
    Ok(cache_path.to_string_lossy().to_string())
}

/// Points an asset at a new cover and removes its old cached file.
fn replace_cover(
    db: &Database,
    asset_manager: &AssetManager,
    asset: &GameAsset,
    cache_path: &str,
) -> Result<GameAsset> {
    let replaced =
        game_assets::replace_asset_image(db, &asset.id, "cover", "user_picked", cache_path)?;
    if asset.cache_path.as_deref() != Some(cache_path) {
        cleanup_assets(asset_manager, std::slice::from_ref(asset));
    }
    Ok(replaced)
}

/// Makes an asset the cover and remembers its crop. Crops of assets that are
/// gone are dropped on the way.
fn use_cover(db: &Database, game: &Game, asset_id: &str, crop: Option<CropRect>) -> Result<()> {
    let assets: HashSet<String> = game_assets::list_assets_for_game(db, &game.id)?
        .into_iter()
        .map(|asset| asset.id)
        .collect();
    let mut metadata = parse_game_metadata(game);
    metadata.preferred_cover_asset_id = Some(asset_id.to_string());
    metadata.cover_crops.retain(|id, _| assets.contains(id));
    match crop {
        Some(crop) => metadata.cover_crops.insert(asset_id.to_string(), crop),
        None => metadata.cover_crops.remove(asset_id),
    };
    games::set_metadata(db, &game.id, &metadata)?;
    Ok(())
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

/// Only the cached copy is shown. The original path can come from another PC
/// through a restored backup, so it is never read here.
fn build_preview_data_url(asset: &GameAsset) -> Result<String> {
    let data_path = PathBuf::from(
        asset
            .cache_path
            .as_deref()
            .ok_or_else(|| VaultimeError::Asset("artwork has no cached copy".into()))?,
    );
    let bytes = read_artwork(&data_path)?;

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
    let source_bytes = read_artwork(&candidate.path)?;
    let source_hash = crate::hex::encode(&Sha256::digest(&source_bytes));
    let image = decode_artwork(&source_bytes, &candidate.path)?;

    let asset_type = if prefer_cover {
        "cover"
    } else {
        &candidate.asset_type
    };
    let processed = process_image(image, asset_type);
    // Artwork is opaque and much smaller as JPEG, icons keep their transparency.
    let cache_path = write_cached_image(asset_manager, game, &processed, asset_type != "icon")?;

    Ok(CachedAsset {
        file_path: candidate.path.to_string_lossy().to_string(),
        cache_path: cache_path.to_string_lossy().to_string(),
        asset_type: asset_type.into(),
        source: candidate.source.clone(),
        hash: source_hash,
    })
}

/// Writes an image under a new name in the game's cache folder.
fn write_cached_image(
    asset_manager: &AssetManager,
    game: &Game,
    image: &image::DynamicImage,
    as_jpeg: bool,
) -> Result<PathBuf> {
    let game_cache_dir = asset_manager.cache_dir().join(&game.id);
    fs::create_dir_all(&game_cache_dir).map_err(|error| {
        VaultimeError::Asset(format!(
            "failed to create game asset cache {}: {error}",
            game_cache_dir.display()
        ))
    })?;

    let asset_id = uuid::Uuid::new_v4().to_string();
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
                encoder.encode_image(&image.to_rgb8())
            })
    } else {
        image.save_with_format(&cache_path, ImageFormat::Png)
    };
    written.map_err(|error| {
        VaultimeError::Asset(format!(
            "failed to write cached image {}: {error}",
            cache_path.display()
        ))
    })?;
    Ok(cache_path)
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

/// Reads an image file unless it is too large to be artwork.
fn read_artwork(path: &Path) -> Result<Vec<u8>> {
    let failed = |error: std::io::Error| {
        VaultimeError::Asset(format!(
            "failed to read artwork {}: {error}",
            path.display()
        ))
    };
    let size = fs::metadata(path).map_err(failed)?.len();
    if size > MAX_ARTWORK_SOURCE_BYTES {
        return Err(VaultimeError::Asset(format!(
            "{} is too large to be artwork",
            path.display()
        )));
    }
    fs::read(path).map_err(failed)
}

/// Deletes cached copies, never a file outside the cache.
fn cleanup_assets(asset_manager: &AssetManager, assets: &[GameAsset]) {
    for asset in assets {
        if let Some(cache_path) = asset.cache_path.as_deref().map(Path::new)
            && cache_path.starts_with(asset_manager.cache_dir())
            && !cache_path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
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

    #[test]
    fn only_plain_names_pass() {
        for name in ["2f0e9c3a-game-id", "cover.png", "Elden Ring"] {
            assert!(is_plain_name(name), "{name}");
        }
        for name in [
            "", ".", "..", "...", " ", "cover.", "cover ", "a/b", "a\\b", "C:x", "/etc", "CON",
            "nul.png", "com1", "LPT9.txt",
        ] {
            assert!(!is_plain_name(name), "{name}");
        }
    }

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

    /// Writes a wide PNG, red on the left and blue on the right.
    fn write_logo(path: &Path) {
        image::RgbImage::from_fn(400, 200, |x, _| {
            if x < 200 {
                image::Rgb([220, 30, 30])
            } else {
                image::Rgb([30, 60, 230])
            }
        })
        .save_with_format(path, ImageFormat::Png)
        .unwrap();
    }

    fn crop_of(db: &Database, game_id: &str, asset_id: &str) -> Option<CropRect> {
        parse_game_metadata(&games::get_game(db, game_id).unwrap())
            .cover_crops
            .get(asset_id)
            .copied()
    }

    #[test]
    fn importing_the_same_file_again_recuts_it() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let game = create_game(&db);
        let logo = asset_manager.cache_dir().join("logo.png");
        write_logo(&logo);
        let fit = CropRect {
            x: 0.0,
            y: -0.83,
            width: 1.0,
            height: 2.67,
        };

        let first = import_game_asset(&db, &asset_manager, &game.id, &logo.to_string_lossy(), None)
            .unwrap();
        assert_eq!(first.len(), 1);
        let first_cache = first[0].cache_path.clone().unwrap();
        let second = import_game_asset(
            &db,
            &asset_manager,
            &game.id,
            &logo.to_string_lossy(),
            Some(fit),
        )
        .unwrap();

        assert_eq!(second.len(), 1);
        assert_eq!(second[0].id, first[0].id);
        assert!(second[0].is_preferred);
        assert!(!Path::new(&first_cache).exists());
        assert_eq!(crop_of(&db, &game.id, &second[0].id), Some(fit));
        let source = open_game_asset_source(&db, &game.id, &second[0].id).unwrap();
        assert!(source.from_original);
        assert_eq!(source.crop, Some(fit));
        assert_eq!((source.width, source.height), (400, 200));

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

    #[test]
    fn recropping_uses_the_cached_cover_once_the_original_changed() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let game = create_game(&db);
        let logo = asset_manager.cache_dir().join("logo.png");
        write_logo(&logo);
        let asset_id =
            import_game_asset(&db, &asset_manager, &game.id, &logo.to_string_lossy(), None)
                .unwrap()[0]
                .id
                .clone();

        fs::write(&logo, b"something else").unwrap();
        let source = open_game_asset_source(&db, &game.id, &asset_id).unwrap();
        assert!(!source.from_original);
        assert_eq!(source.crop, None);
        assert_eq!(
            (source.width, source.height),
            (COVER_WIDTH_PX, COVER_HEIGHT_PX)
        );

        let whole = CropRect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        let assets = crop_game_asset(&db, &asset_manager, &game.id, &asset_id, whole).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(crop_of(&db, &game.id, &asset_id), None);

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

    #[test]
    fn recropping_a_scanned_image_keeps_it_through_the_next_scan() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let folder = asset_manager.cache_dir().join("Game Folder");
        fs::create_dir_all(&folder).unwrap();
        write_logo(&folder.join("cover.png"));
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Scanned Game".into(),
                executable_path: None,
                install_folder: Some(folder.to_string_lossy().to_string()),
                launcher_source: None,
            },
        )
        .unwrap();

        let scanned = scan_game_assets(&db, &asset_manager, &game.id).unwrap();
        assert_eq!(scanned.len(), 1);
        let crop = fill_crop(400, 200);
        let cropped = crop_game_asset(&db, &asset_manager, &game.id, &scanned[0].id, crop).unwrap();
        assert_eq!(cropped[0].id, scanned[0].id);
        assert_eq!(cropped[0].source, "user_picked");

        let rescanned = scan_game_assets(&db, &asset_manager, &game.id).unwrap();
        assert_eq!(rescanned.len(), 1);
        assert_eq!(rescanned[0].id, scanned[0].id);
        assert!(rescanned[0].is_preferred);
        assert_eq!(crop_of(&db, &game.id, &scanned[0].id), Some(crop));

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

    #[test]
    fn crops_only_apply_to_images_of_the_game() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let game = create_game(&db);
        let other = create_game(&db);
        let logo = asset_manager.cache_dir().join("logo.png");
        write_logo(&logo);
        let asset_id = import_game_asset(
            &db,
            &asset_manager,
            &other.id,
            &logo.to_string_lossy(),
            None,
        )
        .unwrap()[0]
            .id
            .clone();

        assert!(open_game_asset_source(&db, &game.id, &asset_id).is_err());
        assert!(
            crop_game_asset(
                &db,
                &asset_manager,
                &game.id,
                &asset_id,
                fill_crop(400, 200)
            )
            .is_err()
        );

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

    /// A game whose folder holds `cover.png`, scanned once.
    fn scanned_game(db: &Database, asset_manager: &AssetManager) -> (Game, PathBuf) {
        let folder = asset_manager.cache_dir().join("Game Folder");
        fs::create_dir_all(&folder).unwrap();
        let cover = folder.join("cover.png");
        write_logo(&cover);
        let game = games::create_game(
            db,
            &CreateGame {
                title: "Scanned Game".into(),
                executable_path: None,
                install_folder: Some(folder.to_string_lossy().to_string()),
                launcher_source: None,
            },
        )
        .unwrap();
        scan_game_assets(db, asset_manager, &game.id).unwrap();
        (game, cover)
    }

    #[test]
    fn deleting_an_image_removes_its_row_and_cached_copy_only() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let (game, cover) = scanned_game(&db, &asset_manager);
        let scanned = list_game_assets(&db, &game.id).unwrap().remove(0);
        let cached = PathBuf::from(scanned.cache_path.clone().unwrap());
        assert!(cached.exists());

        let left = delete_game_asset(&db, &asset_manager, &game.id, &scanned.id).unwrap();
        assert!(left.is_empty());
        assert!(game_assets::get_asset(&db, &scanned.id).is_err());
        assert!(!cached.exists());
        assert!(cover.exists(), "the original file stays");

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

    #[test]
    fn deleting_the_cover_in_use_hands_over_to_the_next_image() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let (game, _) = scanned_game(&db, &asset_manager);
        let logo = asset_manager.cache_dir().join("logo.png");
        image::RgbImage::from_pixel(300, 400, image::Rgb([40, 200, 90]))
            .save_with_format(&logo, ImageFormat::Png)
            .unwrap();
        let assets =
            import_game_asset(&db, &asset_manager, &game.id, &logo.to_string_lossy(), None)
                .unwrap();
        let picked = assets.iter().find(|asset| asset.is_preferred).unwrap();
        assert_eq!(picked.source, "user_picked");

        let left = delete_game_asset(&db, &asset_manager, &game.id, &picked.id).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].source, "scanned_local");
        assert!(left[0].is_preferred);
        assert_eq!(
            preferred_asset_id(&games::get_game(&db, &game.id).unwrap()).as_deref(),
            Some(left[0].id.as_str())
        );

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

    #[test]
    fn a_deleted_image_stays_out_of_scans_until_added_by_hand() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let (game, cover) = scanned_game(&db, &asset_manager);
        let scanned = list_game_assets(&db, &game.id).unwrap().remove(0);
        delete_game_asset(&db, &asset_manager, &game.id, &scanned.id).unwrap();

        assert!(
            scan_game_assets(&db, &asset_manager, &game.id)
                .unwrap()
                .is_empty()
        );

        let added = import_game_asset(
            &db,
            &asset_manager,
            &game.id,
            &cover.to_string_lossy(),
            None,
        )
        .unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].source, "user_picked");
        assert!(
            parse_game_metadata(&games::get_game(&db, &game.id).unwrap())
                .dismissed_artwork
                .is_empty()
        );
        // The scan now finds the file again, but it is the player's image already.
        assert_eq!(
            scan_game_assets(&db, &asset_manager, &game.id)
                .unwrap()
                .len(),
            1
        );

        fs::remove_dir_all(asset_manager.cache_dir()).unwrap();
    }

    #[test]
    fn deleting_never_touches_a_file_outside_the_cache() {
        let db = Database::open_in_memory().unwrap();
        let root = test_cache();
        let asset_manager = AssetManager::new(root.cache_dir().join("cache"));
        fs::create_dir_all(asset_manager.cache_dir()).unwrap();
        let outside = root.cache_dir().join("outside.png");
        fs::write(&outside, b"png").unwrap();
        let game = create_game(&db);
        let asset = game_assets::create_asset(
            &db,
            &game.id,
            "cover",
            "scanned_local",
            "/games/cover.png",
            Some(&outside.to_string_lossy()),
            None,
        )
        .unwrap();

        delete_game_asset(&db, &asset_manager, &game.id, &asset.id).unwrap();
        assert!(game_assets::get_asset(&db, &asset.id).is_err());
        assert!(outside.exists());

        fs::remove_dir_all(root.cache_dir()).unwrap();
    }

    #[test]
    fn deleting_needs_an_image_of_the_game() {
        let db = Database::open_in_memory().unwrap();
        let asset_manager = test_cache();
        let (game, _) = scanned_game(&db, &asset_manager);
        let other = create_game(&db);
        let scanned = list_game_assets(&db, &game.id).unwrap().remove(0);

        assert!(delete_game_asset(&db, &asset_manager, &other.id, &scanned.id).is_err());
        assert!(game_assets::get_asset(&db, &scanned.id).is_ok());

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
