// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cached images as the window loads them: through the app's own scheme,
//! with a thumbnail for tiles and lists and the colors of a cover worked out
//! once and kept in files next to the image. A cached image never changes
//! under its name, so its address and those files stay true until the rule
//! changes.

use std::fs;
use std::path::{Path, PathBuf};

use image::DynamicImage;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use tauri::http::{Response, StatusCode, header};

use super::crop::decode_artwork;
use super::read_artwork;
use super::tint::{ArtTint, tint_of_image};
use crate::constants::{
    ART_CACHE_MAX_AGE_SECS, ART_RULES_VERSION, ART_SCHEME, CACHED_JPEG_QUALITY,
    THUMBNAIL_HEIGHT_PX, THUMBNAIL_WIDTH_PX,
};
use crate::error::{Result, VaultimeError};

/// Follows the name of a cached image in the name of its thumbnail.
const THUMBNAIL_FILE: &str = "thumb.jpg";
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
    /// The thumbnail, or the image itself where it is small enough.
    pub thumb_url: String,
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

    fn is_jpeg(&self) -> bool {
        Path::new(&self.file)
            .extension()
            .is_some_and(|extension| extension == "jpg")
    }

    fn derived(&self, name: &str) -> PathBuf {
        self.path.with_file_name(format!("{}.{name}", self.stem))
    }

    /// The art of the image. The thumbnail counts only with the colors, which
    /// are written after it, and its address names the rules it was made by,
    /// as a webview keeps what it was once served.
    fn art(&self, tint: Option<ArtTint>) -> Art {
        let image_url = art_url(&self.folder, &self.file);
        let thumb_url = if tint.is_some() && self.derived(THUMBNAIL_FILE).is_file() {
            let file = format!("{}.{THUMBNAIL_FILE}", self.stem);
            format!("{}?rules={ART_RULES_VERSION}", art_url(&self.folder, &file))
        } else {
            image_url.clone()
        };
        Art {
            image_url,
            thumb_url,
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
    cached
        .path
        .is_file()
        .then(|| cached.art(read_tint(&cached.derived(TINT_FILE))))
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
    /// The image is there and its thumbnail and colors still have to be made.
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

/// A cover with its thumbnail and colors, made from the image and kept when
/// they are not known yet.
pub fn cover_art(cache_dir: &Path, cache_path: &str) -> Result<Art> {
    let cached = Cached::find(cache_dir, cache_path)
        .ok_or_else(|| VaultimeError::Asset(format!("{cache_path} is not a cached image")))?;
    if let Some(tint) = read_tint(&cached.derived(TINT_FILE)) {
        return Ok(cached.art(Some(tint)));
    }
    let image = decode_artwork(&read_artwork(&cached.path)?, &cached.path)?;
    let thumbnail = thumbnail_of(&cached, &image);
    let tint = tint_of_image(thumbnail.as_ref().unwrap_or(&image));
    // A cover that can be read shows where these cannot be kept, as on a
    // full disk, with its image in place of the thumbnail.
    if let Err(error) = keep(&cached, thumbnail.as_ref(), &tint) {
        log::warn!("the thumbnail and colors of {cache_path} were not kept: {error}");
        return Ok(Art {
            tint: Some(tint),
            ..cached.art(None)
        });
    }
    Ok(cached.art(Some(tint)))
}

/// The thumbnail of a JPEG larger than one. Other images are icons, small
/// and see-through, and are shown as they are.
fn thumbnail_of(cached: &Cached, image: &DynamicImage) -> Option<DynamicImage> {
    (cached.is_jpeg()
        && (image.width() > THUMBNAIL_WIDTH_PX || image.height() > THUMBNAIL_HEIGHT_PX))
        .then(|| {
            image.resize(
                THUMBNAIL_WIDTH_PX,
                THUMBNAIL_HEIGHT_PX,
                FilterType::Lanczos3,
            )
        })
}

/// Writes the thumbnail, then the colors, which vouch for it.
fn keep(cached: &Cached, thumbnail: Option<&DynamicImage>, tint: &ArtTint) -> Result<()> {
    if let Some(thumbnail) = thumbnail {
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, CACHED_JPEG_QUALITY)
            .encode_image(&thumbnail.to_rgb8())
            .map_err(|error| VaultimeError::Asset(format!("thumbnail not made: {error}")))?;
        write_atomically(&cached.derived(THUMBNAIL_FILE), &bytes)?;
    }
    let file = TintFile {
        rules: ART_RULES_VERSION,
        tint: tint.clone(),
    };
    let bytes = serde_json::to_vec(&file)
        .map_err(|error| VaultimeError::Asset(format!("colors not written: {error}")))?;
    write_atomically(&cached.derived(TINT_FILE), &bytes)
}

/// Whether a file of the cache was made from an image, a thumbnail or its
/// colors, or is still being written. Backups leave these out, as the app
/// makes them again.
pub fn is_derived(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "tmp")
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.split_once('.'))
            .is_some_and(|(_, kind)| [THUMBNAIL_FILE, TINT_FILE].contains(&kind))
}

/// Answers a request of the art scheme with a cached image or its thumbnail.
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

/// The file a request names: an image or thumbnail in a game folder of the
/// cache, never a path outside it.
fn requested_file(cache_dir: &Path, path: &str) -> Option<(PathBuf, &'static str)> {
    let (folder, file) = path.strip_prefix('/')?.split_once('/')?;
    let (stem, kind) = file.split_once('.')?;
    let mime = match kind {
        "jpg" | THUMBNAIL_FILE => "image/jpeg",
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
    [THUMBNAIL_FILE, TINT_FILE]
        .iter()
        .map(|name| cache_path.with_file_name(format!("{stem}.{name}")))
        .collect()
}

/// The UUID of the cached image a file in a game folder was made for, for
/// files that are not cached images themselves.
pub fn derived_from(file: &str) -> Option<&str> {
    let (stem, rest) = file.split_once('.')?;
    (is_uuid(stem) && (rest == THUMBNAIL_FILE || rest == TINT_FILE || is_passing(file)))
        .then_some(stem)
}

/// A file written under a passing name, which a crash may leave behind.
pub fn is_passing(file: &str) -> bool {
    Path::new(file)
        .extension()
        .is_some_and(|extension| extension == "tmp")
}

fn read_tint(path: &Path) -> Option<ArtTint> {
    let file: TintFile = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    (file.rules == ART_RULES_VERSION).then_some(file.tint)
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
    fn a_cover_gets_its_thumbnail_and_colors_once_and_keeps_them_next_to_it() {
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
        assert!(
            art.thumb_url
                .ends_with(&format!(".thumb.jpg?rules={ART_RULES_VERSION}"))
        );
        let [thumbnail, kept] = &derived_files(&cover)[..] else {
            panic!("a thumbnail and colors are kept");
        };
        let small = image::open(thumbnail).unwrap();
        assert_eq!((small.width(), small.height()), (360, 480));
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
    fn small_images_and_icons_are_their_own_thumbnail() {
        let dir = cache_dir();
        for cover in [
            write_cover(&dir, 60, 80, "jpg"),
            write_cover(&dir, 720, 960, "png"),
        ] {
            let art = cover_art(&dir, &cover.to_string_lossy()).unwrap();
            assert_eq!(art.thumb_url, art.image_url);
            assert!(!derived_files(&cover)[0].exists());
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_thumbnail_counts_only_with_its_colors() {
        let dir = cache_dir();
        let cover = write_cover(&dir, 720, 960, "jpg");
        let stored = cover.to_string_lossy().into_owned();
        let art = cover_art(&dir, &stored).unwrap();
        assert_eq!(image_art(&dir, &stored), Some(art));
        // Without them, as before they are made again under new rules.
        let [_, colors] = &derived_files(&cover)[..] else {
            panic!("a thumbnail and colors are kept");
        };
        fs::remove_file(colors).unwrap();
        let unread = image_art(&dir, &stored).unwrap();
        assert_eq!(unread.thumb_url, unread.image_url);
    }

    #[test]
    fn a_cover_shows_where_its_thumbnail_and_colors_cannot_be_kept() {
        let dir = cache_dir();
        let cover = write_cover(&dir, 720, 960, "jpg");
        // A folder in the place of the colors keeps them from being written.
        let stem = cover.file_stem().unwrap().to_string_lossy().into_owned();
        fs::create_dir(cover.with_file_name(format!("{stem}.{TINT_FILE}"))).unwrap();
        let art = cover_art(&dir, &cover.to_string_lossy()).unwrap();
        assert!(art.tint.is_some());
        assert_eq!(art.thumb_url, art.image_url);
    }

    #[test]
    fn the_scheme_serves_images_and_thumbnails_of_the_cache_and_nothing_else() {
        let dir = cache_dir();
        let cover = write_cover(&dir, 720, 960, "jpg");
        let art = cover_art(&dir, &cover.to_string_lossy()).unwrap();
        // The path of a request, as the webview hands it over without its query.
        let path_of = |url: &str| {
            let path = url.splitn(4, '/').nth(3).unwrap();
            format!("/{}", path.split('?').next().unwrap())
        };

        let image = respond(Some(&dir), &path_of(&art.image_url));
        assert_eq!(image.status(), StatusCode::OK);
        assert_eq!(image.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(image.body(), &fs::read(&cover).unwrap());
        let thumbnail = respond(Some(&dir), &path_of(&art.thumb_url));
        assert_eq!(thumbnail.status(), StatusCode::OK);
        assert!(thumbnail.body().len() < image.body().len());

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
        for derived in ["thumb.jpg", "tint.json", "tint.0f3a.tmp"] {
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
