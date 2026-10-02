// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The background picture behind the app. It has its own folder in the data
//! folder, outside the artwork cache, so backups leave it out.

use std::fs;
use std::io::BufWriter;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use tauri::State;

use crate::assets::crop::decode_artwork;
use crate::assets::{AssetManager, read_file_limited};
use crate::constants::{
    BACKGROUND_FILE, BACKGROUND_JPEG_QUALITY, BACKGROUND_MAX_SIDE_PX, BACKGROUND_SOURCE_MAX_BYTES,
};
use crate::db::connection::Database;
use crate::db::models::{GameAsset, GameMetadata};
use crate::db::repo::{game_assets, games};
use crate::error::{Result, VaultimeError};

const BYTES_PER_MEGABYTE: u64 = 1024 * 1024;

/// The folder that holds the background picture.
pub struct BackgroundStore {
    dir: PathBuf,
}

impl BackgroundStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn file(&self) -> PathBuf {
        self.dir.join(BACKGROUND_FILE)
    }

    /// The picture as a data URL, or `None` without one.
    pub fn read(&self) -> Result<Option<String>> {
        match fs::read(self.file()) {
            Ok(bytes) => Ok(Some(data_url(&bytes))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(failed("read the background picture", error)),
        }
    }

    /// Stores a scaled down copy of an image as the picture and returns it as
    /// a data URL. A file that is no image leaves the current picture alone.
    pub fn import(&self, source: &Path) -> Result<String> {
        let bytes = read_file_limited(source, BACKGROUND_SOURCE_MAX_BYTES).map_err(|error| {
            if error.kind() == std::io::ErrorKind::FileTooLarge {
                VaultimeError::Invalid(format!(
                    "This picture is too large. Pick one under {} MB.",
                    BACKGROUND_SOURCE_MAX_BYTES / BYTES_PER_MEGABYTE
                ))
            } else {
                failed("read the picture", error)
            }
        })?;
        let image = decode_artwork(&bytes, source).map_err(|_| {
            VaultimeError::Invalid("This file is not a picture Vaultime can read.".into())
        })?;
        let image =
            if image.width() > BACKGROUND_MAX_SIDE_PX || image.height() > BACKGROUND_MAX_SIDE_PX {
                image.resize(
                    BACKGROUND_MAX_SIDE_PX,
                    BACKGROUND_MAX_SIDE_PX,
                    FilterType::Lanczos3,
                )
            } else {
                image
            };

        let mut encoded = Vec::new();
        JpegEncoder::new_with_quality(BufWriter::new(&mut encoded), BACKGROUND_JPEG_QUALITY)
            .encode_image(&image.to_rgb8())
            .map_err(|error| failed("store the picture", error))?;

        // Written next to the old picture first, so a failed write keeps it.
        fs::create_dir_all(&self.dir).map_err(|error| failed("store the picture", error))?;
        let staged = self.dir.join(format!("{BACKGROUND_FILE}.new"));
        fs::write(&staged, &encoded).map_err(|error| failed("store the picture", error))?;
        fs::rename(&staged, self.file()).map_err(|error| failed("store the picture", error))?;
        Ok(data_url(&encoded))
    }

    /// Uses a game's artwork as the picture. Wide art fills a window best, so
    /// a banner or screenshot wins over the cover.
    pub fn import_from_game(
        &self,
        db: &Database,
        asset_manager: &AssetManager,
        game_id: &str,
    ) -> Result<String> {
        let game = games::get_game(db, game_id)?;
        let preferred = serde_json::from_str::<GameMetadata>(&game.metadata_json)
            .ok()
            .and_then(|metadata| metadata.preferred_cover_asset_id);
        let assets = game_assets::list_assets_for_game(db, game_id)?;
        let cached = |asset: &&GameAsset| {
            asset
                .cache_path
                .as_deref()
                .is_some_and(|path| inside(asset_manager.cache_dir(), Path::new(path)))
        };
        let picked = ["banner", "screenshot"]
            .iter()
            .find_map(|kind| {
                assets
                    .iter()
                    .filter(cached)
                    .find(|asset| asset.asset_type == *kind)
            })
            .or_else(|| {
                assets
                    .iter()
                    .filter(cached)
                    .find(|asset| Some(&asset.id) == preferred.as_ref())
            })
            .or_else(|| assets.iter().find(cached))
            .ok_or_else(|| VaultimeError::Invalid("This game has no artwork yet.".into()))?;
        self.import(Path::new(picked.cache_path.as_deref().unwrap_or_default()))
    }

    /// Removes the picture. Nothing to remove is fine.
    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(self.file()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(failed("remove the background picture", error)),
        }
    }
}

/// Whether a path lies inside a folder, without steps out of it.
fn inside(folder: &Path, path: &Path) -> bool {
    path.starts_with(folder)
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

fn data_url(jpeg: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    )
}

fn failed(action: &str, error: impl std::fmt::Display) -> VaultimeError {
    VaultimeError::Asset(format!("failed to {action}: {error}"))
}

#[tauri::command(async)]
pub fn get_background_image(
    store: State<'_, BackgroundStore>,
) -> std::result::Result<Option<String>, VaultimeError> {
    store.read()
}

#[tauri::command(async)]
pub fn set_background_image(
    store: State<'_, BackgroundStore>,
    source_path: String,
) -> std::result::Result<String, VaultimeError> {
    store.import(Path::new(&source_path))
}

#[tauri::command(async)]
pub fn set_background_from_game(
    store: State<'_, BackgroundStore>,
    db: State<'_, Arc<Database>>,
    asset_manager: State<'_, AssetManager>,
    game_id: String,
) -> std::result::Result<String, VaultimeError> {
    store.import_from_game(&db, &asset_manager, &game_id)
}

#[tauri::command]
pub fn clear_background_image(
    store: State<'_, BackgroundStore>,
) -> std::result::Result<bool, VaultimeError> {
    store.clear()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateGame;
    use image::{ImageFormat, RgbImage};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaultime-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_png(path: &Path, width: u32, height: u32) {
        RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 90])
        })
        .save_with_format(path, ImageFormat::Png)
        .unwrap();
    }

    fn stored_size(store: &BackgroundStore) -> (u32, u32) {
        let image = image::open(store.file()).unwrap();
        (image.width(), image.height())
    }

    #[test]
    fn starts_without_a_picture() {
        let store = BackgroundStore::new(temp_dir("background").join("appearance"));
        assert_eq!(store.read().unwrap(), None);
        store.clear().unwrap();
    }

    #[test]
    fn scales_a_large_picture_down_and_keeps_its_shape() {
        let root = temp_dir("background");
        let source = root.join("wide.png");
        write_png(&source, BACKGROUND_MAX_SIDE_PX * 2, BACKGROUND_MAX_SIDE_PX);
        let store = BackgroundStore::new(root.join("appearance"));

        let url = store.import(&source).unwrap();
        assert!(url.starts_with("data:image/jpeg;base64,"));
        assert_eq!(store.read().unwrap(), Some(url));
        assert_eq!(
            stored_size(&store),
            (BACKGROUND_MAX_SIDE_PX, BACKGROUND_MAX_SIDE_PX / 2)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn keeps_a_small_picture_at_its_size() {
        let root = temp_dir("background");
        let source = root.join("small.png");
        write_png(&source, 300, 200);
        let store = BackgroundStore::new(root.join("appearance"));
        store.import(&source).unwrap();
        assert_eq!(stored_size(&store), (300, 200));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_file_that_is_no_picture_keeps_the_old_one() {
        let root = temp_dir("background");
        let source = root.join("picture.png");
        write_png(&source, 40, 30);
        let store = BackgroundStore::new(root.join("appearance"));
        let before = store.import(&source).unwrap();

        let text = root.join("notes.png");
        fs::write(&text, b"not a picture").unwrap();
        assert!(matches!(
            store.import(&text),
            Err(VaultimeError::Invalid(_))
        ));
        assert_eq!(store.read().unwrap(), Some(before));

        store.clear().unwrap();
        assert_eq!(store.read().unwrap(), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn takes_wide_game_art_over_the_cover() {
        let root = temp_dir("background");
        let cache = AssetManager::new(root.join("asset-cache"));
        let db = Database::open_in_memory().unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Wallpaper Game".into(),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        let game_dir = cache.cache_dir().join(&game.id);
        fs::create_dir_all(&game_dir).unwrap();
        let cover = game_dir.join("cover.png");
        let banner = game_dir.join("banner.png");
        write_png(&cover, 30, 40);
        write_png(&banner, 64, 36);
        for (kind, path) in [("cover", &cover), ("banner", &banner)] {
            game_assets::create_asset(
                &db,
                &game.id,
                kind,
                "scan",
                &path.to_string_lossy(),
                Some(&path.to_string_lossy()),
                None,
            )
            .unwrap();
        }

        let store = BackgroundStore::new(root.join("appearance"));
        store.import_from_game(&db, &cache, &game.id).unwrap();
        assert_eq!(stored_size(&store), (64, 36));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn never_reads_game_art_outside_the_cache() {
        let root = temp_dir("background");
        let cache = AssetManager::new(root.join("asset-cache"));
        fs::create_dir_all(cache.cache_dir()).unwrap();
        let db = Database::open_in_memory().unwrap();
        let game = games::create_game(
            &db,
            &CreateGame {
                title: "Restored Game".into(),
                executable_path: None,
                install_folder: None,
                launcher_source: None,
            },
        )
        .unwrap();
        let outside = root.join("elsewhere.png");
        write_png(&outside, 20, 20);
        let sneaky = cache.cache_dir().join("..").join("elsewhere.png");
        for path in [&outside, &sneaky] {
            game_assets::create_asset(
                &db,
                &game.id,
                "banner",
                "scan",
                &path.to_string_lossy(),
                Some(&path.to_string_lossy()),
                None,
            )
            .unwrap();
        }

        let store = BackgroundStore::new(root.join("appearance"));
        assert!(matches!(
            store.import_from_game(&db, &cache, &game.id),
            Err(VaultimeError::Invalid(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
