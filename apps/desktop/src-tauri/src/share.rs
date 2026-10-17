// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The year card the page draws, saved where the player picked. Only PNG
//! images go to `.png` files, so the page cannot write anything else.

use std::fs;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::constants::SHARE_IMAGE_MAX_BYTES;
use crate::error::{Result, VaultimeError};

/// The first bytes of every PNG file.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// Saves a PNG image, given as base64, to `path`, which must end in `.png`.
pub fn save_share_image(path: &Path, png_base64: &str) -> Result<()> {
    let is_png_path = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
    if !is_png_path {
        return Err(VaultimeError::Invalid(
            "the card can only be saved as a .png file".into(),
        ));
    }
    // Base64 takes four characters for every three bytes.
    if png_base64.len() / 4 * 3 > SHARE_IMAGE_MAX_BYTES {
        return Err(VaultimeError::Invalid(
            "the card is too large to save".into(),
        ));
    }
    let bytes = STANDARD
        .decode(png_base64)
        .map_err(|_| VaultimeError::Invalid("the card could not be read".into()))?;
    if !bytes.starts_with(&PNG_SIGNATURE) {
        return Err(VaultimeError::Invalid("the card is not a PNG image".into()));
    }
    fs::write(path, bytes)
        .map_err(|error| VaultimeError::Invalid(format!("could not save the card: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_base64() -> String {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.extend_from_slice(b"rest of the image");
        STANDARD.encode(bytes)
    }

    fn folder() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vaultime-share-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saves_a_png_to_a_png_path() {
        let dir = folder();
        let path = dir.join("vaultime-2026.PNG");
        save_share_image(&path, &png_base64()).unwrap();
        assert!(fs::read(&path).unwrap().starts_with(&PNG_SIGNATURE));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_other_paths_and_other_bytes() {
        let dir = folder();
        let not_png = dir.join("vaultime-2026.exe");
        assert!(save_share_image(&not_png, &png_base64()).is_err());
        assert!(!not_png.exists());

        let path = dir.join("vaultime-2026.png");
        assert!(save_share_image(&path, &STANDARD.encode(b"MZ not an image")).is_err());
        assert!(save_share_image(&path, "not base64 at all!").is_err());
        assert!(!path.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
