// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The icon of this program's launcher on Linux. The app menu, the dock and,
//! on Wayland, the taskbar show the icon the desktop entry names, not the one
//! the window sets. An icon of that name in the user's own icon folder comes
//! before the installed one, so the logo the tray shows, in the accent, goes
//! there at the sizes desktops look for. A copy no package installed, so
//! without a desktop entry, leaves the folder alone.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use image::ExtendedColorType;
use image::ImageEncoder;
use image::codecs::png::PngEncoder;
use log::{info, warn};

use crate::constants::{
    DESKTOP_ENTRY_FILE, LAUNCHER_ICON_NAME, LAUNCHER_ICON_SIZES, SHORTCUT_ICON_DELAY,
};
use crate::window_look::draw_logo;

/// The system's data folders when `XDG_DATA_DIRS` names none.
const DEFAULT_DATA_DIRS: &str = "/usr/local/share:/usr/share";

/// The accent the launcher should show next.
static WANTED: Mutex<Option<String>> = Mutex::new(None);
/// Held while the icons change, so two changes never overlap.
static CHANGING: Mutex<()> = Mutex::new(());

/// Gives this program's launcher the logo in `accent`, `#rrggbb`, off the
/// calling thread. It waits a moment first, so of quick changes only the last
/// one is drawn.
pub fn follow(accent: String) {
    *WANTED.lock().unwrap_or_else(PoisonError::into_inner) = Some(accent);
    let spawned = std::thread::Builder::new()
        .name("launcher-icon".into())
        .spawn(|| {
            std::thread::sleep(SHORTCUT_ICON_DELAY);
            let _changing = CHANGING.lock().unwrap_or_else(PoisonError::into_inner);
            let wanted = WANTED.lock().unwrap_or_else(PoisonError::into_inner).take();
            let (Some(accent), Some(user)) = (wanted, dirs::data_dir()) else {
                return;
            };
            if installed(
                &user,
                &system_data_dirs(std::env::var("XDG_DATA_DIRS").ok()),
            ) {
                write_icons(&user.join("icons").join("hicolor"), &accent);
            }
        });
    if let Err(error) = spawned {
        warn!("could not start changing the launcher icon: {error}");
    }
}

/// The system's data folders, from `XDG_DATA_DIRS` or its default.
fn system_data_dirs(named: Option<String>) -> Vec<PathBuf> {
    named
        .filter(|dirs| !dirs.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_DATA_DIRS.into())
        .split(':')
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .collect()
}

/// Whether a package installed this program's desktop entry, for this user or
/// for everyone.
fn installed(user: &Path, system: &[PathBuf]) -> bool {
    std::iter::once(user)
        .chain(system.iter().map(PathBuf::as_path))
        .any(|dir| dir.join("applications").join(DESKTOP_ENTRY_FILE).is_file())
}

/// Writes the logo in `accent` into the hicolor folders under `theme` at every
/// size, each file only when it changed. Returns whether one did.
fn write_icons(theme: &Path, accent: &str) -> bool {
    let mut changed = false;
    for (size, scale) in LAUNCHER_ICON_SIZES {
        let pixels = size * scale;
        let Some(png) = draw_logo(accent, pixels).and_then(|rgba| encode_png(&rgba, pixels)) else {
            warn!("could not draw the launcher icon in {accent}");
            break;
        };
        let folder = if scale == 1 {
            format!("{size}x{size}")
        } else {
            format!("{size}x{size}@{scale}")
        };
        let folder = theme.join(folder).join("apps");
        let path = folder.join(format!("{LAUNCHER_ICON_NAME}.png"));
        if fs::read(&path).is_ok_and(|current| current == png) {
            continue;
        }
        // Written beside it first, so a desktop never reads half a file.
        let partial = path.with_extension("partial");
        let written = fs::create_dir_all(&folder)
            .and_then(|()| fs::write(&partial, &png))
            .and_then(|()| fs::rename(&partial, &path));
        if let Err(error) = written {
            warn!("could not save {}: {error}", path.display());
            let _ = fs::remove_file(&partial);
            break;
        }
        changed = true;
    }
    if changed {
        // Desktops read the icons of a theme again once its folder changed.
        let touched =
            fs::File::open(theme).and_then(|folder| folder.set_modified(SystemTime::now()));
        if let Err(error) = touched {
            warn!("could not mark {} as changed: {error}", theme.display());
        }
        info!("the launcher shows the logo in {accent} now");
    }
    changed
}

fn encode_png(rgba: &[u8], size: u32) -> Option<Vec<u8>> {
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(rgba, size, size, ExtendedColorType::Rgba8)
        .inspect_err(|error| warn!("could not encode the launcher icon: {error}"))
        .ok()?;
    Some(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("vaultime-launcher-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn reads_the_system_data_folders() {
        assert_eq!(
            system_data_dirs(None),
            [
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share")
            ]
        );
        assert_eq!(system_data_dirs(Some(" ".into())), system_data_dirs(None));
        assert_eq!(
            system_data_dirs(Some("/opt/share:relative:/usr/share".into())),
            [PathBuf::from("/opt/share"), PathBuf::from("/usr/share")]
        );
    }

    #[test]
    fn only_an_installed_desktop_entry_counts() {
        let root = folder();
        let (user, system) = (root.join("home"), root.join("usr"));
        assert!(!installed(&user, std::slice::from_ref(&system)));
        fs::create_dir_all(system.join("applications")).unwrap();
        fs::write(system.join("applications").join("Other.desktop"), "").unwrap();
        assert!(!installed(&user, std::slice::from_ref(&system)));
        fs::write(system.join("applications").join(DESKTOP_ENTRY_FILE), "").unwrap();
        assert!(installed(&user, &[system]));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn writes_every_size_once_and_again_for_another_accent() {
        let theme = folder().join("hicolor");
        assert!(write_icons(&theme, "#2BB5A0"));
        for (size, scale) in LAUNCHER_ICON_SIZES {
            let folder = if scale == 1 {
                format!("{size}x{size}")
            } else {
                format!("{size}x{size}@{scale}")
            };
            let path = theme.join(folder).join("apps").join("vaultime.png");
            let icon = image::open(&path).unwrap();
            assert_eq!((icon.width(), icon.height()), (size * scale, size * scale));
        }
        let small = theme.join("32x32").join("apps").join("vaultime.png");
        let teal = fs::read(&small).unwrap();
        assert!(!write_icons(&theme, "#2BB5A0"));
        assert!(write_icons(&theme, "#F08A2C"));
        assert_ne!(fs::read(&small).unwrap(), teal);
        assert!(
            !theme
                .join("32x32")
                .join("apps")
                .join("vaultime.partial")
                .exists()
        );
        assert!(!write_icons(&theme, "orange"));
        fs::remove_dir_all(theme.parent().unwrap()).unwrap();
    }
}
