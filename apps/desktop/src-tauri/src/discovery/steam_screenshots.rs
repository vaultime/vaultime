// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Screenshots taken with Steam. Each account lists them in
//! `userdata/<account>/760/screenshots.vdf`, with the app, the file and when
//! it was taken, and keeps them below `760/remote/<app id>/screenshots`.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::vdf::{self, Vdf};

/// The app id Steam files screenshots under, fixed by its folder layout.
const SCREENSHOTS_APP: &str = "760";

/// A screenshot as Steam lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamScreenshot {
    pub app_id: String,
    /// File name in the app's screenshot folder.
    pub file: String,
    /// Unix seconds.
    pub taken_at: i64,
    pub width: u32,
    pub height: u32,
}

/// Every screenshot the accounts of the Steam install at `root` list.
pub fn list(root: &Path) -> Vec<SteamScreenshot> {
    accounts(root)
        .filter_map(|account| fs::read_to_string(account.join("screenshots.vdf")).ok())
        .flat_map(|text| parse(&text))
        .collect()
}

/// The file of a screenshot `list` named, or of Steam's small copy of it.
pub fn picture(root: &Path, app_id: &str, file: &str, thumbnail: bool) -> Option<PathBuf> {
    if !is_app_id(app_id) || !is_file_name(file) {
        return None;
    }
    accounts(root).find_map(|account| {
        let folder = account.join("remote").join(app_id).join("screenshots");
        let folder = if thumbnail {
            folder.join("thumbnails")
        } else {
            folder
        };
        Some(folder.join(file)).filter(|path| path.is_file())
    })
}

/// The screenshot folder of each account.
fn accounts(root: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(root.join("userdata"))
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path().join(SCREENSHOTS_APP))
}

fn parse(text: &str) -> Vec<SteamScreenshot> {
    let listing = vdf::parse(text);
    let Some(apps) = listing.get("screenshots") else {
        return Vec::new();
    };
    apps.entries()
        .iter()
        .filter(|(app_id, _)| is_app_id(app_id))
        .flat_map(|(app_id, shots)| {
            shots.entries().iter().filter_map(move |(_, shot)| {
                // Steam names the file within its app's folder, never elsewhere.
                let file = shot
                    .get("filename")?
                    .text()?
                    .strip_prefix(app_id.as_str())?
                    .strip_prefix("/screenshots/")?;
                is_file_name(file).then(|| SteamScreenshot {
                    app_id: app_id.clone(),
                    file: file.to_owned(),
                    // Steam writes 32-bit times, so a longer number is damage.
                    taken_at: number::<u32>(shot, "creation")
                        .map(i64::from)
                        .unwrap_or_default(),
                    width: number(shot, "width").unwrap_or_default(),
                    height: number(shot, "height").unwrap_or_default(),
                })
            })
        })
        .filter(|shot| shot.taken_at > 0)
        .collect()
}

fn number<T: FromStr>(shot: &Vdf, key: &str) -> Option<T> {
    shot.get(key)?.text()?.trim().parse().ok()
}

fn is_app_id(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Steam names screenshots like `20260726012003_1.jpg`. Anything else, above
/// all a name that leads out of the folder, is never opened.
fn is_file_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        && [".jpg", ".jpeg", ".png"]
            .iter()
            .any(|extension| lower.ends_with(extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = r#"
"screenshots"
{
	"632360"
	{
		"0"
		{
			"type"		"1"
			"filename"		"632360/screenshots/20250224222253_1.jpg"
			"thumbnail"		"632360/screenshots/thumbnails/20250224222253_1.jpg"
			"width"		"2560"
			"height"		"1440"
			"creation"		"1740432173"
		}
		"1"
		{
			"filename"		"632360/screenshots/../../secret.jpg"
			"creation"		"1740432174"
		}
		"2"
		{
			"filename"		"632360/screenshots/20250224222254_1.jpg"
			"creation"		"99999999999999999"
		}
	}
	"not-an-app"
	{
		"0"
		{
			"filename"		"x/screenshots/20250224222253_1.jpg"
			"creation"		"1740432173"
		}
	}
}
"#;

    fn steam_root() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("vaultime-shots-test-{}", uuid::Uuid::new_v4()));
        let account = root.join("userdata").join("101").join(SCREENSHOTS_APP);
        let thumbnails = account.join("remote/632360/screenshots/thumbnails");
        fs::create_dir_all(&thumbnails).unwrap();
        fs::write(account.join("screenshots.vdf"), LISTING).unwrap();
        fs::write(
            account.join("remote/632360/screenshots/20250224222253_1.jpg"),
            b"full",
        )
        .unwrap();
        fs::write(thumbnails.join("20250224222253_1.jpg"), b"small").unwrap();
        root
    }

    #[test]
    fn lists_the_screenshots_of_every_account_with_when_they_were_taken() {
        let root = steam_root();
        assert_eq!(
            list(&root),
            vec![SteamScreenshot {
                app_id: "632360".into(),
                file: "20250224222253_1.jpg".into(),
                taken_at: 1_740_432_173,
                width: 2560,
                height: 1440,
            }]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn opens_only_screenshot_files_in_the_folder_of_their_app() {
        let root = steam_root();
        let full = picture(&root, "632360", "20250224222253_1.jpg", false).unwrap();
        assert_eq!(fs::read(full).unwrap(), b"full");
        let small = picture(&root, "632360", "20250224222253_1.jpg", true).unwrap();
        assert_eq!(fs::read(small).unwrap(), b"small");
        assert_eq!(picture(&root, "632360", "missing_1.jpg", false), None);
        assert_eq!(picture(&root, "632360", "../screenshots.vdf", false), None);
        assert_eq!(
            picture(&root, "../760", "20250224222253_1.jpg", false),
            None
        );
        assert_eq!(picture(&root, "632360", "screenshots.vdf", false), None);
        fs::remove_dir_all(root).unwrap();
    }
}
