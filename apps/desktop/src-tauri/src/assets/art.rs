// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cached images as the window loads them: through the app's own scheme,
//! with the colors of a cover worked out once and kept in a file next to the
//! image. A cached image never changes under its name, so its address and
//! that file stay true until the rule changes.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::http::{Response, StatusCode, header};

use super::crop::decode_artwork;
use super::read_artwork;
use super::tint::{ArtTint, tint_of_image};
use crate::constants::{ART_CACHE_MAX_AGE_SECS, ART_SCHEME, ART_TINT_RULES_VERSION};
use crate::error::{Result, VaultimeError};

/// Follows the name of a cached image in the name of the file with its colors.
const TINT_FILE: &str = "tint.json";

#[derive(Serialize, Deserialize)]
struct TintFile {
    rules: u32,
    tint: ArtTint,
}

/// A cached image as the window loads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Art {
    pub image_url: String,
    pub tint: Option<ArtTint>,
}

/// A cached image in a game folder of the cache, named by a UUID.
struct Cached {
    folder: String,
    stem: String,
    file: String,
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
                folder: folder.to_owned(),
                stem: stem.to_owned(),
                file: file.to_owned(),
                path: cache_dir.join(folder).join(file),
            }
        })
    }

    fn derived(&self, name: &str) -> PathBuf {
        self.path.with_file_name(format!("{}.{name}", self.stem))
    }

    fn art(&self, tint: Option<ArtTint>) -> Art {
        Art {
            image_url: art_url(&self.folder, &self.file),
            tint,
        }
    }
}

/// The address of a file of the art scheme. The webview of Windows serves
/// custom schemes over http on a host of their name, the others under the
/// scheme itself.
fn art_url(folder: &str, file: &str) -> String {
    if cfg!(windows) {
        format!("http://{ART_SCHEME}.localhost/{folder}/{file}")
    } else {
        format!("{ART_SCHEME}://localhost/{folder}/{file}")
    }
}

/// A cached image without its colors, None when the file is not there.
pub fn image_art(cache_dir: &Path, cache_path: &str) -> Option<Art> {
    let cached = Cached::find(cache_dir, cache_path)?;
    cached.path.is_file().then(|| cached.art(None))
}

/// Where a cached image is in this cache folder, when it is there.
pub fn image_path(cache_dir: &Path, cache_path: &str) -> Option<PathBuf> {
    Cached::find(cache_dir, cache_path)
        .map(|cached| cached.path)
        .filter(|path| path.is_file())
}

/// What is known of a cover without opening the image.
pub enum Known {
    Art(Art),
    /// The image is there and its colors still have to be worked out.
    Unread,
    /// No cached image.
    Missing,
}

pub fn known_cover(cache_dir: &Path, cache_path: &str) -> Known {
    match Cached::find(cache_dir, cache_path) {
        Some(cached) if cached.path.is_file() => read_tint(&cached.derived(TINT_FILE))
            .map_or(Known::Unread, |tint| Known::Art(cached.art(Some(tint)))),
        _ => Known::Missing,
    }
}

/// A cover with its colors, worked out from the image and kept when they
/// are not known yet.
pub fn cover_art(cache_dir: &Path, cache_path: &str) -> Result<Art> {
    let cached = Cached::find(cache_dir, cache_path)
        .ok_or_else(|| VaultimeError::Asset(format!("{cache_path} is not a cached image")))?;
    if let Some(tint) = read_tint(&cached.derived(TINT_FILE)) {
        return Ok(cached.art(Some(tint)));
    }
    let image = decode_artwork(&read_artwork(&cached.path)?, &cached.path)?;
    let file = TintFile {
        rules: ART_TINT_RULES_VERSION,
        tint: tint_of_image(&image),
    };
    let bytes = serde_json::to_vec(&file)
        .map_err(|error| VaultimeError::Asset(format!("colors not written: {error}")))?;
    write_atomically(&cached.derived(TINT_FILE), &bytes)?;
    Ok(cached.art(Some(file.tint)))
}

/// Answers a request of the art scheme with a cached image.
pub fn respond(cache_dir: Option<&Path>, path: &str) -> Response<Vec<u8>> {
    let found = cache_dir
        .and_then(|dir| requested_file(dir, path))
        .and_then(|(file, mime)| Some((read_artwork(&file).ok()?, mime)));
    let response = match found {
        Some((bytes, mime)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .header(
                header::CACHE_CONTROL,
                format!("max-age={ART_CACHE_MAX_AGE_SECS}, immutable"),
            )
            .body(bytes),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new()),
    };
    response.unwrap_or_default()
}

/// The file a request names: an image in a game folder of the cache, never
/// a path outside it.
fn requested_file(cache_dir: &Path, path: &str) -> Option<(PathBuf, &'static str)> {
    let (folder, file) = path.strip_prefix('/')?.split_once('/')?;
    let (stem, kind) = file.split_once('.')?;
    let mime = match kind {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        _ => return None,
    };
    (is_folder_name(folder) && is_uuid(stem)).then(|| (cache_dir.join(folder).join(file), mime))
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

/// The UUID of the cached image a file in a game folder was made for, for
/// files that are not cached images themselves.
pub fn derived_from(file: &str) -> Option<&str> {
    let (stem, rest) = file.split_once('.')?;
    (is_uuid(stem) && (rest == TINT_FILE || is_passing(file))).then_some(stem)
}

/// A file written under a passing name, which a crash may leave behind.
pub fn is_passing(file: &str) -> bool {
    Path::new(file)
        .extension()
        .is_some_and(|extension| extension == "tmp")
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

    fn write_cover(dir: &Path, width: u32, height: u32, extension: &str) -> PathBuf {
        let path = dir
            .join("game-1")
            .join(format!("{}.{extension}", uuid::Uuid::new_v4()));
        image::RgbImage::from_pixel(width, height, image::Rgb([220, 30, 30]))
            .save(&path)
            .unwrap();
        path
    }

    fn name_of(path: &Path) -> String {
        path.file_name().unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn a_cover_gets_its_colors_once_and_keeps_them_next_to_it() {
        let dir = cache_dir();
        let cover = write_cover(&dir, 720, 960, "jpg");
        let stored = cover.to_string_lossy().into_owned();
        assert!(matches!(known_cover(&dir, &stored), Known::Unread));

        let art = cover_art(&dir, &stored).unwrap();
        assert_eq!(art.tint.as_ref().unwrap().colors.len(), 1);
        assert!(
            art.image_url
                .ends_with(&format!("/game-1/{}", name_of(&cover)))
        );
        let [kept] = &derived_files(&cover)[..] else {
            panic!("the colors are kept");
        };
        let Known::Art(known) = known_cover(&dir, &stored) else {
            panic!("the colors were not kept");
        };
        assert_eq!(known, art);

        // A cache that moved finds the image by its folder and name.
        let moved = Path::new("/elsewhere/cache/game-1").join(name_of(&cover));
        assert!(matches!(
            known_cover(&dir, &moved.to_string_lossy()),
            Known::Art(_)
        ));

        // Colors of an older rule are worked out again.
        fs::write(
            kept,
            br#"{"rules":0,"tint":{"colors":[],"strength":0,"lightness":null}}"#,
        )
        .unwrap();
        assert!(matches!(known_cover(&dir, &stored), Known::Unread));
        assert_eq!(cover_art(&dir, &stored).unwrap(), art);

        // Kept colors never stand in for an image that is gone.
        fs::remove_file(&cover).unwrap();
        assert!(matches!(known_cover(&dir, &stored), Known::Missing));
        assert!(image_art(&dir, &stored).is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_scheme_serves_images_of_the_cache_and_nothing_else() {
        let dir = cache_dir();
        let cover = write_cover(&dir, 720, 960, "jpg");
        let art = cover_art(&dir, &cover.to_string_lossy()).unwrap();
        let path_of = |url: &str| format!("/{}", url.splitn(4, '/').nth(3).unwrap());

        let image = respond(Some(&dir), &path_of(&art.image_url));
        assert_eq!(image.status(), StatusCode::OK);
        assert_eq!(image.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(image.body(), &fs::read(&cover).unwrap());

        let id = uuid::Uuid::new_v4();
        let outside = dir.parent().unwrap().join(format!("{id}.jpg"));
        fs::copy(&cover, &outside).unwrap();
        for path in [
            format!("/game-1/{id}.jpg"),
            format!("/game-1/{id}.tint.json"),
            format!("/../{id}.jpg"),
            format!("/game-1/../{id}.jpg"),
            format!("/game-1/..%2F{id}.jpg"),
            format!("/game-1/x/../../{id}.jpg"),
            format!("/{id}.jpg"),
            "/game-1/cover.jpg".to_owned(),
        ] {
            assert_eq!(
                respond(Some(&dir), &path).status(),
                StatusCode::NOT_FOUND,
                "{path}"
            );
        }
        assert_eq!(
            respond(None, &path_of(&art.image_url)).status(),
            StatusCode::NOT_FOUND
        );
        fs::remove_file(outside).unwrap();
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
                matches!(known_cover(&dir, &stored), Known::Missing),
                "{stored}"
            );
            assert!(cover_art(&dir, &stored).is_err(), "{stored}");
        }
        let id_text = id.to_string();
        for derived in ["tint.json", "tint.0f3a.tmp"] {
            assert_eq!(
                derived_from(&format!("{id}.{derived}")),
                Some(id_text.as_str())
            );
        }
        assert_eq!(derived_from(&format!("{id}.jpg")), None);
        assert_eq!(derived_from("notes.tint.json"), None);
        fs::remove_dir_all(dir).unwrap();
    }
}
