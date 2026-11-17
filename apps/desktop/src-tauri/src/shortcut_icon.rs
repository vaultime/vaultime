// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The icon of this program's shortcuts. Windows shows an app started from
//! the Start menu or a pin on the taskbar with the icon of its shortcut, not
//! the one its window sets. So the shortcuts that start this program get the
//! logo the tray shows, in the accent, from an icon file in the appearance
//! folder. Shortcuts of other copies,
//! like a development build, stay as they are, and so does everything else
//! about a shortcut.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use image::ExtendedColorType;
use image::codecs::ico::{IcoEncoder, IcoFrame};
use log::{info, warn};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IPersistFile, STGM_READWRITE,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_RoamingAppData, IShellLinkW, KF_FLAG_DEFAULT,
    SHCNE_UPDATEITEM, SHCNF_PATHW, SHChangeNotify, SHGetKnownFolderPath, ShellLink,
};
use windows::core::{GUID, HSTRING, Interface, PCWSTR};

use crate::constants::{
    SHORTCUT_ICON_DELAY, SHORTCUT_ICON_SIZES_PX, SHORTCUT_PATH_UNITS, SHORTCUT_SEARCH_DEPTH,
};
use crate::platform::process::{file_name, path_key, resolved_key};
use crate::window_look::draw_logo;

const ICON_PREFIX: &str = "shortcut-icon-";
/// Names the drawing of the logo, so icon files of an earlier drawing are
/// made again instead of taken as they are.
const ICON_DRAWING: &str = "-framed";
const ICON_EXTENSION: &str = "ico";
/// The pinned taskbar buttons, in the roaming app data folder.
const TASKBAR_PINS: &str = r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar";

/// The accent the shortcuts should show next, with the appearance folder.
static WANTED: Mutex<Option<(PathBuf, String)>> = Mutex::new(None);
/// Held while shortcuts change, so two changes never overlap.
static CHANGING: Mutex<()> = Mutex::new(());

/// Gives this program's shortcuts the logo in `accent`, `#rrggbb`, off the
/// calling thread. It waits a moment first, so of quick changes, like those
/// of an accent tried out in the settings, only the last one is drawn.
pub fn follow(appearance_dir: PathBuf, accent: String) {
    *WANTED.lock().unwrap_or_else(PoisonError::into_inner) = Some((appearance_dir, accent));
    let spawned = std::thread::Builder::new()
        .name("shortcut-icon".into())
        .spawn(|| {
            std::thread::sleep(SHORTCUT_ICON_DELAY);
            let _changing = CHANGING.lock().unwrap_or_else(PoisonError::into_inner);
            let wanted = WANTED.lock().unwrap_or_else(PoisonError::into_inner).take();
            let Some((appearance_dir, accent)) = wanted else {
                return;
            };
            let Ok(program) = std::env::current_exe() else {
                return;
            };
            let _com = Com::start();
            let folders = shortcut_folders();
            point_shortcuts(&folders, &program, &appearance_dir, &accent);
        });
    if let Err(error) = spawned {
        warn!("could not start changing the shortcut icons: {error}");
    }
}

/// The Start menu, the desktop and the pinned taskbar buttons of this user,
/// each with how deep shortcuts sit in it.
fn shortcut_folders() -> Vec<(PathBuf, usize)> {
    let mut folders = Vec::new();
    if let Some(programs) = known_folder(&FOLDERID_Programs) {
        folders.push((programs, SHORTCUT_SEARCH_DEPTH));
    }
    if let Some(desktop) = known_folder(&FOLDERID_Desktop) {
        folders.push((desktop, 1));
    }
    if let Some(roaming) = known_folder(&FOLDERID_RoamingAppData) {
        folders.push((roaming.join(TASKBAR_PINS), 1));
    }
    folders
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    // SAFETY: the id is a known folder id, and the path Windows hands back is
    // read once and then freed with the allocator that made it.
    unsafe {
        let path = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let folder = path.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(path.0.cast_const().cast()));
        folder
    }
}

/// Points every shortcut in `folders` that starts `program` at the icon of
/// the logo in `accent`, made once a shortcut needs it. The icons of earlier
/// accents go, except one a shortcut that could not be changed still points
/// at. Returns the shortcuts changed.
fn point_shortcuts(
    folders: &[(PathBuf, usize)],
    program: &Path,
    appearance_dir: &Path,
    accent: &str,
) -> Vec<PathBuf> {
    // A shortcut gives its target as a long path, the program may run from
    // a short one like `C:\PROGRA~1`.
    let program_key = resolved_key(&program.to_string_lossy());
    let program_name = file_name(&program_key).unwrap_or_default().to_owned();
    let mut icon: Option<PathBuf> = None;
    let mut changed = Vec::new();
    let mut still_used = Vec::new();
    let shortcuts = folders.iter().flat_map(|(folder, depth)| {
        walkdir::WalkDir::new(folder)
            .max_depth(*depth)
            .follow_links(false)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
            })
            .map(walkdir::DirEntry::into_path)
    });
    for shortcut in shortcuts {
        let Some(link) = Shortcut::open(&shortcut) else {
            continue;
        };
        if link
            .target()
            .is_none_or(|target| !starts_program(&target, &program_name, &program_key))
        {
            continue;
        }
        if icon.is_none() {
            icon = write_icon(appearance_dir, accent);
        }
        let Some(icon) = &icon else {
            return changed;
        };
        let icon_text = icon.to_string_lossy();
        let current = link.icon();
        if current
            .as_deref()
            .is_some_and(|current| path_key(current) == path_key(&icon_text))
        {
            continue;
        }
        match link.point_at(&shortcut, icon) {
            Ok(()) => {
                info!("{} shows the logo in {accent} now", shortcut.display());
                changed.push(shortcut);
            }
            Err(error) => {
                still_used.extend(current);
                warn!(
                    "could not change the icon of {}: {error}",
                    shortcut.display()
                );
            }
        }
    }
    if let Some(icon) = &icon {
        remove_other_icons(appearance_dir, icon, &still_used);
    }
    changed
}

/// Whether a shortcut's `target` is the program. Only a target of the same
/// file name is resolved, so other shortcuts cost no file system call.
fn starts_program(target: &str, program_name: &str, program_key: &str) -> bool {
    file_name(&path_key(target)) == Some(program_name) && resolved_key(target) == program_key
}

/// The icon file of the logo in `accent`, written when it is not there yet.
fn write_icon(appearance_dir: &Path, accent: &str) -> Option<PathBuf> {
    let name = format!(
        "{ICON_PREFIX}{}{ICON_DRAWING}.{ICON_EXTENSION}",
        accent.trim_start_matches('#').to_ascii_lowercase()
    );
    let path = appearance_dir.join(name);
    if path.is_file() {
        return Some(path);
    }
    let frames: Vec<IcoFrame<'_>> = SHORTCUT_ICON_SIZES_PX
        .iter()
        .filter_map(|&size| {
            let rgba = draw_logo(accent, size)?;
            IcoFrame::as_png(&rgba, size, size, ExtendedColorType::Rgba8).ok()
        })
        .collect();
    if frames.len() != SHORTCUT_ICON_SIZES_PX.len() {
        warn!("could not draw the shortcut icon in {accent}");
        return None;
    }
    let mut bytes = Vec::new();
    if let Err(error) = IcoEncoder::new(&mut bytes).encode_images(&frames) {
        warn!("could not write the shortcut icon: {error}");
        return None;
    }
    // Written beside it first, so a shortcut never points at half a file.
    let partial = path.with_extension("partial");
    let written = fs::create_dir_all(appearance_dir)
        .and_then(|()| fs::write(&partial, &bytes))
        .and_then(|()| fs::rename(&partial, &path));
    match written {
        Ok(()) => Some(path),
        Err(error) => {
            warn!("could not save the shortcut icon: {error}");
            let _ = fs::remove_file(&partial);
            None
        }
    }
}

/// Removes the icon files of earlier accents and drawings, but `keep` and
/// those shortcuts still point at.
fn remove_other_icons(appearance_dir: &Path, keep: &Path, still_used: &[String]) {
    let Ok(entries) = fs::read_dir(appearance_dir) else {
        return;
    };
    for entry in entries.filter_map(std::result::Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let key = path_key(&path.to_string_lossy());
        if path != keep
            && !still_used.iter().any(|used| path_key(used) == key)
            && name.starts_with(ICON_PREFIX)
            && path
                .extension()
                .is_some_and(|extension| extension == ICON_EXTENSION)
            && let Err(error) = fs::remove_file(&path)
        {
            warn!("could not remove {}: {error}", path.display());
        }
    }
}

/// A shortcut file, loaded for reading and writing.
struct Shortcut {
    link: IShellLinkW,
    file: IPersistFile,
}

impl Shortcut {
    fn open(path: &Path) -> Option<Self> {
        // SAFETY: COM is started on this thread, and the interfaces live as
        // long as this value.
        unsafe {
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            let file: IPersistFile = link.cast().ok()?;
            file.Load(&HSTRING::from(path.as_os_str()), STGM_READWRITE)
                .ok()?;
            Some(Self { link, file })
        }
    }

    /// The program the shortcut starts.
    fn target(&self) -> Option<String> {
        let mut buffer = [0_u16; SHORTCUT_PATH_UNITS];
        // SAFETY: the buffer is passed with its length, and no find data is asked for.
        unsafe { self.link.GetPath(&mut buffer, std::ptr::null_mut(), 0) }.ok()?;
        text_of(&buffer)
    }

    /// The file its icon comes from.
    fn icon(&self) -> Option<String> {
        let mut buffer = [0_u16; SHORTCUT_PATH_UNITS];
        let mut index = 0;
        // SAFETY: the buffer is passed with its length, the index is a live i32.
        unsafe { self.link.GetIconLocation(&mut buffer, &raw mut index) }.ok()?;
        text_of(&buffer)
    }

    /// Sets the icon to the first one in `icon`, saves the shortcut and lets
    /// Explorer know it changed.
    fn point_at(&self, path: &Path, icon: &Path) -> windows::core::Result<()> {
        // SAFETY: COM is started on this thread, the strings outlive the calls.
        unsafe {
            self.link
                .SetIconLocation(&HSTRING::from(icon.as_os_str()), 0)?;
            self.file.Save(PCWSTR::null(), true)?;
            let wide = HSTRING::from(path.as_os_str());
            SHChangeNotify(
                SHCNE_UPDATEITEM,
                SHCNF_PATHW,
                Some(wide.as_ptr().cast()),
                None,
            );
        }
        Ok(())
    }
}

fn text_of(buffer: &[u16]) -> Option<String> {
    let end = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    (end > 0).then(|| String::from_utf16_lossy(&buffer[..end]))
}

/// COM for this thread, as long as the value lives.
struct Com(bool);

impl Com {
    fn start() -> Self {
        // SAFETY: called once on this thread, and ended by the drop below
        // only when it started.
        Self(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: matches the successful start on this thread.
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::Storage::FileSystem::GetShortPathNameW;

    use super::*;

    const TEAL: &str = "#2BB5A0";

    fn folder() -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("vaultime-shortcut-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// A shortcut to `target` with the target's own icon, as installers make them.
    fn make_shortcut(path: &Path, target: &Path) {
        // SAFETY: COM is started by the caller, the strings outlive the calls.
        unsafe {
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            link.SetPath(&HSTRING::from(target.as_os_str())).unwrap();
            link.SetIconLocation(&HSTRING::from(target.as_os_str()), 0)
                .unwrap();
            let file: IPersistFile = link.cast().unwrap();
            file.Save(&HSTRING::from(path.as_os_str()), true).unwrap();
        }
    }

    #[test]
    fn the_icon_file_holds_every_size() {
        let appearance = folder();
        let icon = write_icon(&appearance, TEAL).unwrap();
        assert_eq!(icon.file_name().unwrap(), "shortcut-icon-2bb5a0-framed.ico");
        let decoded = image::ImageReader::open(&icon)
            .unwrap()
            .with_guessed_format()
            .unwrap();
        assert_eq!(decoded.format(), Some(image::ImageFormat::Ico));
        // The decoder gives the largest image.
        assert_eq!(decoded.decode().unwrap().width(), 256);
        assert!(write_icon(&appearance, "violet").is_none());
        fs::remove_dir_all(appearance).unwrap();
    }

    /// The short form Windows gives `path`, when its volume keeps short names.
    fn short_path(path: &Path) -> Option<PathBuf> {
        let mut buffer = [0_u16; SHORTCUT_PATH_UNITS];
        // SAFETY: the buffer outlives the call, which writes inside it only.
        let length =
            unsafe { GetShortPathNameW(&HSTRING::from(path.as_os_str()), Some(&mut buffer)) }
                as usize;
        (length > 0 && length < buffer.len())
            .then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
    }

    #[test]
    fn a_program_started_through_a_short_path_finds_its_shortcuts() {
        let _com = Com::start();
        let root = folder();
        let long = root.join("a long folder name");
        fs::create_dir_all(&long).unwrap();
        let ours = long.join("vaultime.exe");
        fs::write(&ours, b"MZ").unwrap();
        let programs = root.join("Programs");
        fs::create_dir_all(&programs).unwrap();
        make_shortcut(&programs.join("Vaultime.lnk"), &ours);

        let short = short_path(&ours).unwrap();
        if short != ours {
            let folders = [(programs.clone(), SHORTCUT_SEARCH_DEPTH)];
            let changed = point_shortcuts(&folders, &short, &root.join("appearance"), TEAL);
            assert_eq!(changed, [programs.join("Vaultime.lnk")]);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_shortcuts_of_this_program_change() {
        let _com = Com::start();
        let root = folder();
        let (programs, appearance) = (root.join("Programs"), root.join("appearance"));
        fs::create_dir_all(programs.join("Games")).unwrap();
        let ours = root.join("vaultime.exe");
        let other = root.join("other.exe");
        fs::write(&ours, b"MZ").unwrap();
        fs::write(&other, b"MZ").unwrap();
        make_shortcut(&programs.join("Vaultime.lnk"), &ours);
        make_shortcut(&programs.join(r"Games\Other.lnk"), &other);
        // Icons of an earlier accent and of the earlier drawing go once the
        // new one is in place.
        fs::create_dir_all(&appearance).unwrap();
        fs::write(appearance.join("shortcut-icon-ff0000-framed.ico"), b"old").unwrap();
        fs::write(appearance.join("shortcut-icon-2bb5a0.ico"), b"old").unwrap();

        let folders = [(programs.clone(), SHORTCUT_SEARCH_DEPTH)];
        let changed = point_shortcuts(&folders, &ours, &appearance, TEAL);
        assert_eq!(changed, [programs.join("Vaultime.lnk")]);
        let icon = appearance.join("shortcut-icon-2bb5a0-framed.ico");
        let ours_icon = Shortcut::open(&programs.join("Vaultime.lnk"))
            .unwrap()
            .icon()
            .unwrap();
        assert_eq!(path_key(&ours_icon), path_key(&icon.to_string_lossy()));
        let others_icon = Shortcut::open(&programs.join(r"Games\Other.lnk"))
            .unwrap()
            .icon()
            .unwrap();
        assert_eq!(path_key(&others_icon), path_key(&other.to_string_lossy()));
        assert!(!appearance.join("shortcut-icon-ff0000-framed.ico").exists());
        assert!(!appearance.join("shortcut-icon-2bb5a0.ico").exists());

        // Nothing to do the second time.
        assert!(point_shortcuts(&folders, &ours, &appearance, TEAL).is_empty());
        // Icons a shortcut still points at stay, the others go.
        fs::write(appearance.join("shortcut-icon-00ff00-framed.ico"), b"old").unwrap();
        fs::write(appearance.join("shortcut-icon-0000ff-framed.ico"), b"old").unwrap();
        let used = appearance.join("shortcut-icon-00ff00-framed.ico");
        remove_other_icons(&appearance, &icon, &[used.to_string_lossy().into_owned()]);
        assert!(used.exists() && icon.exists());
        assert!(!appearance.join("shortcut-icon-0000ff-framed.ico").exists());
        fs::remove_file(used).unwrap();
        // Another accent gets its own icon, and the earlier one goes.
        assert_eq!(
            point_shortcuts(&folders, &ours, &appearance, "#F08A2C"),
            [programs.join("Vaultime.lnk")]
        );
        assert!(!icon.exists());
        assert!(appearance.join("shortcut-icon-f08a2c-framed.ico").exists());
        // Another program's shortcuts alone make no icon file.
        assert!(
            point_shortcuts(
                &folders,
                &root.join("elsewhere.exe"),
                &appearance,
                "#123456"
            )
            .is_empty()
        );
        assert!(!appearance.join("shortcut-icon-123456-framed.ico").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
