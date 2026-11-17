// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the library shows of a cached cover besides the image: its colors,
//! worked out once and kept in a file next to it. A cached image never
//! changes under its name, so that file stays true until the rule changes.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::crop::decode_artwork;
use super::read_artwork;
use super::tint::{ArtTint, tint_of_image};
use crate::constants::ART_TINT_RULES_VERSION;
use crate::error::{Result, VaultimeError};

/// Follows the name of a cached image in the name of the file with its colors.
const TINT_FILE: &str = "tint.json";

#[derive(Serialize, Deserialize)]
struct TintFile {
    rules: u32,
    tint: ArtTint,
}

/// A cached image in a game folder of the cache, named by a UUID.
struct Cached {
    stem: String,
    path: PathBuf,
}

impl Cached {
    /// The cached image a stored cache path names, looked up by its game
    /// folder and file name, so a cache that moved still counts.
    fn find(cache_dir: &Path, cache_path: &str) -> Option<Self> {
        let path = Path::new(cache_path);
        let file = path.file_name()?.to_str()?;
        let folder = path.parent()?.file_name()?.to_str()?;
        let (stem, extension) = file.split_once('.')?;
        (is_folder_name(folder) && is_uuid(stem) && matches!(extension, "jpg" | "png")).then(|| {
            Self {
                stem: stem.to_owned(),
                path: cache_dir.join(folder).join(file),
            }
        })
    }

    fn tint_file(&self) -> PathBuf {
        self.path
            .with_file_name(format!("{}.{TINT_FILE}", self.stem))
    }
}

/// What is known of a cover's colors without opening the image.
pub enum Known {
    Tint(ArtTint),
    /// The image is there and its colors still have to be worked out.
    Unread,
    /// No cached image to read them from.
    Missing,
}

pub fn known_tint(cache_dir: &Path, cache_path: &str) -> Known {
    match Cached::find(cache_dir, cache_path) {
        Some(cached) if cached.path.is_file() => {
            read_tint(&cached.tint_file()).map_or(Known::Unread, Known::Tint)
        }
        _ => Known::Missing,
    }
}

/// The colors of a cover, worked out from the image and kept when they are
/// not known yet.
pub fn cover_tint(cache_dir: &Path, cache_path: &str) -> Result<ArtTint> {
    let cached = Cached::find(cache_dir, cache_path)
        .ok_or_else(|| VaultimeError::Asset(format!("{cache_path} is not a cached image")))?;
    if let Some(tint) = read_tint(&cached.tint_file()) {
        return Ok(tint);
    }
    let image = decode_artwork(&read_artwork(&cached.path)?, &cached.path)?;
    let tint = tint_of_image(&image);
    let file = TintFile {
        rules: ART_TINT_RULES_VERSION,
        tint,
    };
    let bytes = serde_json::to_vec(&file)
        .map_err(|error| VaultimeError::Asset(format!("colors not written: {error}")))?;
    write_atomically(&cached.tint_file(), &bytes)?;
    Ok(file.tint)
}

/// The files kept for a cached image, to go with it.
pub fn derived_files(cache_path: &Path) -> Vec<PathBuf> {
    let Some(stem) = cache_path
        .file_name()
        .and_then(|file| file.to_str())
        .and_then(|file| file.split_once('.'))
        .map(|(stem, _)| stem)
    else {
        return Vec::new();
    };
    vec![cache_path.with_file_name(format!("{stem}.{TINT_FILE}"))]
}

fn read_tint(path: &Path) -> Option<ArtTint> {
    let file: TintFile = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    (file.rules == ART_TINT_RULES_VERSION).then_some(file.tint)
}

/// Writes under a passing name first, so a reader never finds half a file.
/// Two writers of the same file write the same bytes, so the second one may
/// find it taken.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let passing = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let written = fs::write(&passing, bytes).and_then(|()| fs::rename(&passing, path));
    if let Err(error) = written {
        let _ = fs::remove_file(&passing);
        if !path.is_file() {
            return Err(VaultimeError::Asset(format!(
                "failed to write {}: {error}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// A game folder name that is safe in a path and an address.
fn is_folder_name(name: &str) -> bool {
    super::is_plain_name(name)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn is_uuid(name: &str) -> bool {
    name.bytes()
        .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
        && uuid::Uuid::parse_str(name).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaultime-art-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(dir.join("game-1")).unwrap();
        dir
    }

    fn write_red_cover(dir: &Path) -> PathBuf {
        let path = dir
            .join("game-1")
            .join(format!("{}.jpg", uuid::Uuid::new_v4()));
        image::RgbImage::from_pixel(60, 80, image::Rgb([220, 30, 30]))
            .save(&path)
            .unwrap();
        path
    }

    #[test]
    fn the_colors_of_a_cover_are_worked_out_once_and_kept_next_to_it() {
        let dir = cache_dir();
        let cover = write_red_cover(&dir);
        let stored = cover.to_string_lossy().into_owned();
        assert!(matches!(known_tint(&dir, &stored), Known::Unread));

        let tint = cover_tint(&dir, &stored).unwrap();
        assert_eq!(tint.colors.len(), 1);
        let Known::Tint(known) = known_tint(&dir, &stored) else {
            panic!("the colors were not kept");
        };
        assert_eq!(known, tint);

        // A cache that moved finds the image by its folder and name.
        let moved = Path::new("/elsewhere/cache/game-1").join(cover.file_name().unwrap());
        assert!(matches!(
            known_tint(&dir, &moved.to_string_lossy()),
            Known::Tint(_)
        ));

        // Colors of an older rule are worked out again.
        let kept = derived_files(&cover).remove(0);
        fs::write(
            &kept,
            br#"{"rules":0,"tint":{"colors":[],"strength":0,"lightness":null}}"#,
        )
        .unwrap();
        assert!(matches!(known_tint(&dir, &stored), Known::Unread));
        assert_eq!(cover_tint(&dir, &stored).unwrap(), tint);

        // Kept colors never stand in for an image that is gone.
        fs::remove_file(&cover).unwrap();
        assert!(matches!(known_tint(&dir, &stored), Known::Missing));
        fs::remove_file(&kept).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn only_images_in_game_folders_named_by_a_uuid_count() {
        let dir = cache_dir();
        let id = uuid::Uuid::new_v4();
        for stored in [
            format!("/cache/game-1/{id}.gif"),
            "/cache/game-1/cover.jpg".to_owned(),
            format!("/cache/{{{id}}}/{id}.jpg"),
            format!("/cache/game 1/{id}.jpg"),
            format!("{id}.jpg"),
        ] {
            assert!(
                matches!(known_tint(&dir, &stored), Known::Missing),
                "{stored}"
            );
            assert!(cover_tint(&dir, &stored).is_err(), "{stored}");
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
