// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window around the page in the look the player picked. The icon of the
//! tray, the taskbar and the title bar takes the accent, signed in to cloud
//! backup or not. On Windows the title bar follows dark or light mode, and on
//! Windows 11 it takes the ground of the page and the window border the accent.
//! Linux leaves the title bar to the desktop.

use std::sync::{Arc, Mutex, PoisonError};

use log::warn;
use resvg::{tiny_skia, usvg};
use serde::{Deserialize, Serialize};
use tauri::image::Image;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::constants::{WINDOW_ICON_PX, WINDOW_LOOK_SETTING};
use crate::db::connection::Database;
use crate::db::repo::settings;
use crate::error::VaultimeError;

/// The logo as `scripts/build-brand.py` writes it, with a violet pivot.
const ICON_SVG: &str = include_str!("../../../../assets/vaultime-icon-source.svg");
/// The logo while signed in to cloud backup, with a violet frame and hands.
const SIGNED_IN_ICON_SVG: &str = include_str!("../../../../assets/vaultime-icon-signed-in.svg");
/// The violet of the logo files, which the accent replaces.
const LOGO_VIOLET: &str = "#9D7CFF";

/// Dark or light mode of the title bar, or the system's.
#[cfg_attr(not(windows), allow(dead_code, reason = "only Windows sets the mode"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowTheme {
    Dark,
    Light,
    #[default]
    System,
}

/// Colors of the window around the page, each as `#rrggbb`, and its mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowLook {
    /// Accent of the logo. The logo has its own dark tile, so this is the
    /// accent of dark mode in either mode.
    pub icon_accent: String,
    /// The title bar, in the ground of the page.
    pub title_bar: String,
    /// The title, in the text color of the page.
    pub title_text: String,
    /// The window border, in the accent of the page.
    pub border: String,
    /// The mode of the title bar. Looks stored before it had one follow the
    /// system.
    #[serde(default)]
    #[cfg_attr(not(windows), allow(dead_code, reason = "only Windows sets the mode"))]
    pub theme: WindowTheme,
}

impl WindowLook {
    fn colors(&self) -> [&str; 4] {
        [
            &self.icon_accent,
            &self.title_bar,
            &self.title_text,
            &self.border,
        ]
    }
}

/// What the window should show.
#[derive(Default)]
struct Wanted {
    look: Option<WindowLook>,
    signed_in: bool,
}

/// What the window shows. Held while it is redrawn, so redraws never overlap.
#[derive(Default)]
struct Drawn {
    look: Option<WindowLook>,
    /// The accent of the icon, none for the bundled one, and the cloud state.
    icon: Option<(Option<String>, bool)>,
}

/// The look and whether this PC is signed in, the two things the icon shows.
pub struct WindowLookState {
    wanted: Mutex<Wanted>,
    drawn: Mutex<Drawn>,
}

impl WindowLookState {
    /// Starts with the look of the last run, so the window has it before the
    /// page loads.
    pub fn load(db: &Database) -> Self {
        let look = settings::get_setting(db, WINDOW_LOOK_SETTING)
            .ok()
            .flatten()
            .and_then(|stored| serde_json::from_str::<WindowLook>(&stored).ok())
            .filter(|look| look.colors().into_iter().all(|color| rgb(color).is_some()));
        Self {
            wanted: Mutex::new(Wanted {
                look,
                signed_in: false,
            }),
            drawn: Mutex::new(Drawn::default()),
        }
    }

    fn wanted(&self) -> std::sync::MutexGuard<'_, Wanted> {
        self.wanted.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Notes whether this PC is signed in to cloud backup.
    pub fn set_signed_in(&self, signed_in: bool) {
        self.wanted().signed_in = signed_in;
    }
}

/// Brings the icons and the title bar to what is wanted now. Only what
/// changed is drawn, and two redraws never overlap, so the last one always
/// leaves the latest look and cloud state behind. Never call it on the main
/// thread while another thread may redraw: setting the tray icon waits for
/// the main thread.
pub fn redraw<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<WindowLookState>() else {
        return;
    };
    let mut drawn = state.drawn.lock().unwrap_or_else(PoisonError::into_inner);
    let (look, signed_in) = {
        let wanted = state.wanted();
        (wanted.look.clone(), wanted.signed_in)
    };
    if let Some(look) = &look
        && drawn.look.as_ref() != Some(look)
    {
        paint_title_bar(app, look);
        drawn.look = Some(look.clone());
    }
    let icon = (look.map(|look| look.icon_accent), signed_in);
    if drawn.icon.as_ref() != Some(&icon) {
        crate::tray::show_icon(app, icon.0.as_deref(), signed_in);
        drawn.icon = Some(icon);
    }
}

/// `#rrggbb` as red, green and blue. Anything else is no color.
fn rgb(color: &str) -> Option<[u8; 3]> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |start: usize| u8::from_str_radix(&hex[start..start + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// The logo in an accent, signed in to cloud backup or not.
pub fn draw_icon(accent: &str, signed_in: bool) -> Option<Image<'static>> {
    // Only a plain color goes into the SVG.
    rgb(accent)?;
    let svg = if signed_in {
        SIGNED_IN_ICON_SVG
    } else {
        ICON_SVG
    }
    .replace(LOGO_VIOLET, accent);
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())
        .inspect_err(|error| warn!("failed to read the logo: {error}"))
        .ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(WINDOW_ICON_PX, WINDOW_ICON_PX)?;
    let scale = f64::from(WINDOW_ICON_PX) / f64::from(tree.size().width());
    #[allow(clippy::cast_possible_truncation, reason = "a scale near 1 fits f32")]
    let scale = scale as f32;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(Image::new_owned(
        pixmap.take_demultiplied(),
        WINDOW_ICON_PX,
        WINDOW_ICON_PX,
    ))
}

/// Sets the big icon of a window, the one the taskbar shows. Tauri sets only
/// the small one, and the taskbar keeps showing an old icon until the big one
/// changes.
#[cfg(windows)]
pub fn set_taskbar_icon<R: Runtime>(window: &tauri::WebviewWindow<R>, icon: &Image<'_>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateIcon, DestroyIcon, ICON_BIG, SendMessageW, WM_SETICON,
    };
    /// Icons from RGBA have one color plane of 32 bits per pixel.
    const BITS_PER_PIXEL: u8 = 32;

    let (Ok(hwnd), Ok(width), Ok(height)) = (
        window.hwnd(),
        i32::try_from(icon.width()),
        i32::try_from(icon.height()),
    ) else {
        return;
    };
    // Windows wants blue first. The alpha of each pixel makes the icon see
    // through, so the mask, one bit a pixel in rows of whole 16 bit words, is
    // all clear.
    let mut bgra = icon.rgba().to_vec();
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let mask_row_bytes = icon.width().div_ceil(16) * 2;
    let mask = vec![0_u8; usize::try_from(mask_row_bytes * icon.height()).unwrap_or_default()];
    // SAFETY: the color buffer holds four bytes and the mask one bit for each
    // of the width times height pixels, and Windows copies both.
    let handle = unsafe {
        CreateIcon(
            std::ptr::null_mut(),
            width,
            height,
            1,
            BITS_PER_PIXEL,
            mask.as_ptr(),
            bgra.as_ptr(),
        )
    };
    if handle.is_null() {
        return;
    }
    // SAFETY: the handle belongs to a live window. The icon it returns was set
    // here before, as Tauri never sets the big one, so it is ours to destroy.
    unsafe {
        let previous = SendMessageW(hwnd.0, WM_SETICON, ICON_BIG as usize, handle as isize);
        if previous != 0 {
            DestroyIcon(previous as _);
        }
    }
}

/// Paints the title bar and the border of the main window. Only Windows lets
/// an app pick their mode, and only Windows 11 their colors. Linux is left
/// alone: tao has no way back to the desktop's mode once one is set, it
/// writes GTK's dark preference off, and System mode then shows light on a
/// dark desktop.
fn paint_title_bar<R: Runtime>(app: &AppHandle<R>, look: &WindowLook) {
    #[cfg(windows)]
    if let Some(window) = app.get_webview_window("main") {
        paint_windows_title_bar(&window, look);
    }
    #[cfg(not(windows))]
    let _ = (app, look);
}

#[cfg(windows)]
fn paint_windows_title_bar<R: Runtime>(window: &tauri::WebviewWindow<R>, look: &WindowLook) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute,
    };
    let theme = match look.theme {
        WindowTheme::Dark => Some(tauri::Theme::Dark),
        WindowTheme::Light => Some(tauri::Theme::Light),
        WindowTheme::System => None,
    };
    if let Err(error) = window.set_theme(theme) {
        warn!("failed to set the title bar mode: {error}");
    }
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    for (attribute, color) in [
        (DWMWA_CAPTION_COLOR, &look.title_bar),
        (DWMWA_TEXT_COLOR, &look.title_text),
        (DWMWA_BORDER_COLOR, &look.border),
    ] {
        let Some([red, green, blue]) = rgb(color) else {
            continue;
        };
        let colorref = u32::from(red) | (u32::from(green) << 8) | (u32::from(blue) << 16);
        // Windows 10 refuses these attributes and keeps its own title bar.
        // SAFETY: the handle belongs to a live window and the value is a
        // COLORREF of the size passed.
        unsafe {
            DwmSetWindowAttribute(
                hwnd.0,
                attribute.cast_unsigned(),
                (&raw const colorref).cast(),
                u32::try_from(size_of_val(&colorref)).unwrap_or_default(),
            );
        }
    }
}

/// Puts the look of the last run on the icons and the title bar, before the
/// window shows.
pub fn show_stored<R: Runtime>(app: &AppHandle<R>) {
    redraw(app);
}

/// Takes the colors and mode of the page for the icons and the title bar,
/// and keeps them for the next start. Runs off the main thread, so drawing
/// the icons never holds up the window.
#[tauri::command(async)]
pub fn set_window_look(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    state: State<'_, WindowLookState>,
    look: WindowLook,
) -> Result<bool, VaultimeError> {
    if look.colors().into_iter().any(|color| rgb(color).is_none()) {
        return Err(VaultimeError::Invalid(
            "window colors must be given as #rrggbb".into(),
        ));
    }
    {
        let mut wanted = state.wanted();
        if wanted.look.as_ref() == Some(&look) {
            return Ok(false);
        }
        wanted.look = Some(look.clone());
    }
    redraw(&app);
    let stored = serde_json::to_string(&look).map_err(|error| {
        VaultimeError::Invalid(format!("failed to store the window colors: {error}"))
    })?;
    settings::set_setting(&db, WINDOW_LOOK_SETTING, &stored)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pixel at the logo's pivot, in the middle of the icon.
    fn pivot(icon: &Image<'_>) -> [u8; 4] {
        let size = WINDOW_ICON_PX as usize;
        // The pivot sits at 512, 520 of the 1024 wide logo file.
        let (x, y) = (size / 2, size * 520 / 1024);
        let start = (y * size + x) * 4;
        icon.rgba()[start..start + 4].try_into().unwrap()
    }

    #[test]
    fn reads_plain_colors_only() {
        assert_eq!(rgb("#9d7cff"), Some([0x9d, 0x7c, 0xff]));
        assert_eq!(rgb("#9D7CFF"), Some([0x9d, 0x7c, 0xff]));
        for wrong in [
            "9d7cff",
            "#9d7cf",
            "#9d7cffa",
            "#9d7cfg",
            "red",
            "#9d7cff\"/>",
            "",
        ] {
            assert_eq!(rgb(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn draws_the_pivot_in_the_accent() {
        let icon = draw_icon("#f08a2c", false).unwrap();
        assert_eq!(
            (icon.width(), icon.height()),
            (WINDOW_ICON_PX, WINDOW_ICON_PX)
        );
        assert_eq!(pivot(&icon), [0xf0, 0x8a, 0x2c, 0xff]);
    }

    #[test]
    fn signed_in_keeps_the_pivot_light_and_colors_the_frame() {
        let icon = draw_icon("#f08a2c", true).unwrap();
        assert_eq!(pivot(&icon), [0xf1, 0xeb, 0xfa, 0xff]);
        let rgba = icon.rgba();
        let accent_pixels = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| **pixel == [0xf0, 0x8a, 0x2c, 0xff])
            .count();
        assert!(accent_pixels > rgba.len() / 4 / 20, "{accent_pixels}");
    }

    #[test]
    fn refuses_anything_but_a_color() {
        assert!(draw_icon("#f08a2c\"/><script/>", false).is_none());
        assert!(draw_icon("orange", true).is_none());
    }

    #[test]
    fn keeps_the_look_of_the_last_run() {
        let db = Database::open_in_memory().unwrap();
        assert!(WindowLookState::load(&db).wanted().look.is_none());

        let look = WindowLook {
            icon_accent: "#f08a2c".into(),
            title_bar: "#140f0b".into(),
            title_text: "#f3ece6".into(),
            border: "#f08a2c".into(),
            theme: WindowTheme::Dark,
        };
        settings::set_setting(
            &db,
            WINDOW_LOOK_SETTING,
            &serde_json::to_string(&look).unwrap(),
        )
        .unwrap();
        let state = WindowLookState::load(&db);
        assert_eq!(state.wanted().look.as_ref(), Some(&look));
        state.set_signed_in(true);
        assert!(state.wanted().signed_in);

        settings::set_setting(&db, WINDOW_LOOK_SETTING, r#"{"iconAccent":"red"}"#).unwrap();
        assert!(WindowLookState::load(&db).wanted().look.is_none());
    }

    #[test]
    fn reads_the_mode_and_looks_stored_without_one() {
        let colors = r##""iconAccent":"#f08a2c","titleBar":"#140f0b","titleText":"#f3ece6","border":"#f08a2c""##;
        let read = |json: String| serde_json::from_str::<WindowLook>(&json);
        assert_eq!(
            read(format!("{{{colors}}}")).unwrap().theme,
            WindowTheme::System
        );
        for (name, theme) in [
            ("dark", WindowTheme::Dark),
            ("light", WindowTheme::Light),
            ("system", WindowTheme::System),
        ] {
            let json = format!(r#"{{{colors},"theme":"{name}"}}"#);
            assert_eq!(read(json).unwrap().theme, theme);
        }
        assert!(read(format!(r#"{{{colors},"theme":"blue"}}"#)).is_err());
        let written = serde_json::to_value(read(format!("{{{colors}}}")).unwrap()).unwrap();
        assert_eq!(written["theme"], "system");
    }
}
