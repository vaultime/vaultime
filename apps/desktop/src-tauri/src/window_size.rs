// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed window sizes, like a game client, or a free window.
//!
//! A preset locks the window: it cannot be resized or maximized. Windows then
//! also leaves out Snap and the maximize on a title bar double click. GTK sizes
//! a window that cannot be resized itself and asks the window manager for
//! exactly that size, so it cannot be maximized either.

use std::sync::Arc;

use log::warn;
use serde::Serialize;
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, Runtime, WebviewWindow};

use crate::constants::{
    DEFAULT_WINDOW_PRESET, FREE_WINDOW, WINDOW_MIN_HEIGHT_PX, WINDOW_MIN_WIDTH_PX, WINDOW_PRESETS,
    WINDOW_SIZE_SETTING, WINDOW_TITLE_BAR_MIN_PX,
};
use crate::db::connection::Database;
use crate::db::repo::settings;

/// A width and height in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    pub width: f64,
    pub height: f64,
}

impl Extent {
    /// Within a pixel of each other, as sizes round at fractional scales.
    fn matches(self, other: Extent) -> bool {
        (self.width - other.width).abs() < 1.0 && (self.height - other.height).abs() < 1.0
    }
}

/// The room a window has on its monitor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    /// The monitor without taskbars and panels.
    pub area: Extent,
    /// The title bar and borders around the page.
    pub frame: Extent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowPreset {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
}

impl WindowPreset {
    fn extent(self) -> Extent {
        Extent {
            width: f64::from(self.width),
            height: f64::from(self.height),
        }
    }
}

/// Every preset, smallest first.
pub fn presets() -> impl Iterator<Item = WindowPreset> {
    WINDOW_PRESETS
        .iter()
        .map(|&(name, width, height)| WindowPreset {
            name,
            width,
            height,
        })
}

fn preset(name: &str) -> Option<WindowPreset> {
    presets().find(|preset| preset.name == name)
}

/// Whether the page may ask for this window size.
pub fn is_choice(value: &str) -> bool {
    value == FREE_WINDOW || preset(value).is_some()
}

/// The saved choice, or the default when nothing valid was saved.
pub fn parse_choice(saved: Option<&str>) -> &'static str {
    match saved {
        Some(FREE_WINDOW) => FREE_WINDOW,
        Some(name) => preset(name).map_or(DEFAULT_WINDOW_PRESET, |preset| preset.name),
        None => DEFAULT_WINDOW_PRESET,
    }
}

/// Whether a window with this page and the frame around it fits the screen.
pub fn fits(preset: WindowPreset, screen: Screen) -> bool {
    let page = preset.extent();
    page.width + screen.frame.width <= screen.area.width
        && page.height + screen.frame.height <= screen.area.height
}

/// The preset the window takes for `choice`: the choice when it fits the
/// screen, otherwise the largest smaller one that does. `None` is a free
/// window, also when no preset fits. Without a screen to measure, the
/// choice is taken as it is.
pub fn fitting_preset(choice: &str, screen: Option<Screen>) -> Option<WindowPreset> {
    let chosen = preset(choice)?;
    let Some(screen) = screen else {
        return Some(chosen);
    };
    presets()
        .filter(|preset| preset.width <= chosen.width && fits(*preset, screen))
        .last()
}

/// A free window page of `wanted` size, made smaller where it would not fit
/// the screen, but never below the smallest free window.
pub fn free_page(wanted: Extent, screen: Screen) -> Extent {
    let room = Extent {
        width: screen.area.width - screen.frame.width,
        height: screen.area.height - screen.frame.height,
    };
    Extent {
        width: wanted.width.min(room.width).max(WINDOW_MIN_WIDTH_PX),
        height: wanted.height.min(room.height).max(WINDOW_MIN_HEIGHT_PX),
    }
}

/// A preset for Settings.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WindowPresetView {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    /// Whether the window fits the screen it is on at this size.
    pub fits: bool,
}

/// The window size choice and what the window does with it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WindowSizeState {
    /// The saved choice, a preset name or `free`.
    pub choice: &'static str,
    /// What the window uses. A smaller preset or `free` when the chosen
    /// preset does not fit this screen.
    pub applied: &'static str,
    pub presets: Vec<WindowPresetView>,
}

pub fn build_state(choice: &str, screen: Option<Screen>) -> WindowSizeState {
    let choice = parse_choice(Some(choice));
    WindowSizeState {
        choice,
        applied: fitting_preset(choice, screen).map_or(FREE_WINDOW, |preset| preset.name),
        presets: presets()
            .map(|preset| WindowPresetView {
                name: preset.name,
                width: preset.width,
                height: preset.height,
                fits: screen.is_none_or(|screen| fits(preset, screen)),
            })
            .collect(),
    }
}

/// The choice saved on this PC.
pub fn saved_choice(db: &Database) -> &'static str {
    let saved = settings::get_setting(db, WINDOW_SIZE_SETTING)
        .inspect_err(|error| warn!("failed to read the window size: {error}"))
        .ok()
        .flatten();
    parse_choice(saved.as_deref())
}

/// Where a window is and what surrounds it.
struct Placement {
    screen: Screen,
    /// Top left corner of the work area, in physical pixels.
    origin: PhysicalPosition<i32>,
    /// The frame as the system reports it, for centering.
    frame: Extent,
    /// The page as it is now.
    page: Extent,
    scale: f64,
}

fn placement<R: Runtime>(window: &WebviewWindow<R>) -> Option<Placement> {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())?;
    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();
    let area: LogicalSize<f64> = work_area.size.to_logical(scale);
    let page: LogicalSize<f64> = window.inner_size().ok()?.to_logical(scale);
    let outer: LogicalSize<f64> = window.outer_size().ok()?.to_logical(scale);
    let frame = Extent {
        width: (outer.width - page.width).max(0.0),
        height: (outer.height - page.height).max(0.0),
    };
    // Windows counts its invisible resize borders as frame, at the bottom as
    // wide as on each side. Before the window is first shown, systems may
    // report no frame at all.
    let hidden_bottom = if cfg!(windows) {
        frame.width / 2.0
    } else {
        0.0
    };
    Some(Placement {
        screen: Screen {
            area: Extent {
                width: area.width,
                height: area.height,
            },
            frame: Extent {
                width: frame.width,
                height: (frame.height - hidden_bottom).max(WINDOW_TITLE_BAR_MIN_PX),
            },
        },
        origin: work_area.position,
        frame,
        page: Extent {
            width: page.width,
            height: page.height,
        },
        scale,
    })
}

/// What the window and its screen allow for `choice`, without changing anything.
pub fn state<R: Runtime>(window: &WebviewWindow<R>, choice: &str) -> WindowSizeState {
    build_state(choice, placement(window).map(|placement| placement.screen))
}

/// Gives the window the size of `choice`, or of the largest preset that fits
/// its screen. `opening` is for the hidden window at start: it then gets its
/// size and goes to the middle of the screen. Later calls leave a window
/// where it is unless its size changes.
pub fn apply<R: Runtime>(
    window: &WebviewWindow<R>,
    choice: &str,
    opening: bool,
) -> WindowSizeState {
    let placement = placement(window);
    let screen = placement.as_ref().map(|placement| placement.screen);
    let page = placement.as_ref().map(|placement| placement.page);
    let was_resizable = window.is_resizable().unwrap_or(true);
    let target = fitting_preset(choice, screen);
    let locked = target.is_some();
    let state = build_state(choice, screen);

    if locked && window.is_maximized().unwrap_or(false) {
        report(window.unmaximize(), "leave the maximized window");
    }
    // The style first: Windows keeps the outer size when it changes, so the
    // page size is set after it.
    report(window.set_maximizable(!locked), "change maximizing");
    report(window.set_resizable(!locked), "change resizing");
    if !locked && window.is_maximized().unwrap_or(false) {
        return state;
    }

    let size = if let Some(preset) = target {
        preset.extent()
    } else {
        let default_page = preset(DEFAULT_WINDOW_PRESET).map(WindowPreset::extent);
        let Some(wanted) = (if opening { default_page } else { page }) else {
            return state;
        };
        screen.map_or(wanted, |screen| free_page(wanted, screen))
    };
    let changed = opening || page.is_none_or(|page| !page.matches(size));
    if changed || was_resizable == locked {
        report(
            window.set_size(LogicalSize::new(size.width, size.height)),
            "resize the window",
        );
    }
    if changed {
        center(window, placement.as_ref(), size);
    }
    state
}

/// Puts a window with a page of `size` in the middle of its work area. On
/// Wayland the compositor places windows and this does nothing.
fn center<R: Runtime>(window: &WebviewWindow<R>, placement: Option<&Placement>, size: Extent) {
    let Some(placement) = placement else {
        return;
    };
    let room = placement.screen.area;
    let outer = Extent {
        width: size.width + placement.frame.width,
        height: size.height + placement.frame.height,
    };
    // Never above the top of the work area, so the title bar stays in reach.
    let offset =
        |room: f64, outer: f64| (((room - outer) / 2.0).max(0.0) * placement.scale).round();
    let position = PhysicalPosition::new(
        placement.origin.x + offset(room.width, outer.width) as i32,
        placement.origin.y + offset(room.height, outer.height) as i32,
    );
    report(window.set_position(position), "center the window");
}

fn report(result: tauri::Result<()>, action: &str) {
    if let Err(error) = result {
        warn!("failed to {action}: {error}");
    }
}

/// Sizes the main window for the choice saved on this PC.
pub fn fit_main_window<R: Runtime>(app: &AppHandle<R>, opening: bool) {
    let (Some(window), Some(db)) = (
        app.get_webview_window("main"),
        app.try_state::<Arc<Database>>(),
    ) else {
        return;
    };
    apply(&window, saved_choice(&db), opening);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent(width: f64, height: f64) -> Extent {
        Extent { width, height }
    }

    /// A monitor at the given logical size with a 48 pixel taskbar and the
    /// frame of Windows 11.
    fn screen(width: f64, height: f64) -> Screen {
        Screen {
            area: extent(width, height - 48.0),
            frame: extent(16.0, 32.0),
        }
    }

    fn fitting(choice: &str, screen: Screen) -> Option<&'static str> {
        fitting_preset(choice, Some(screen)).map(|preset| preset.name)
    }

    #[test]
    fn presets_go_from_small_to_large_in_one_shape() {
        let all: Vec<_> = presets().collect();
        assert!(all.windows(2).all(|pair| pair[0].width < pair[1].width));
        for preset in &all {
            assert_eq!(preset.width * 10, preset.height * 16, "{}", preset.name);
            assert!(f64::from(preset.width) >= WINDOW_MIN_WIDTH_PX);
            assert!(f64::from(preset.height) >= WINDOW_MIN_HEIGHT_PX);
        }
        assert!(preset(DEFAULT_WINDOW_PRESET).is_some());
        assert!(preset(FREE_WINDOW).is_none());
    }

    #[test]
    fn the_window_is_created_at_the_smallest_preset() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let window = &config["app"]["windows"][0];
        let smallest = presets().next().unwrap();
        assert_eq!(window["width"], smallest.width);
        assert_eq!(window["height"], smallest.height);
        assert_eq!(window["minWidth"].as_f64(), Some(WINDOW_MIN_WIDTH_PX));
        assert_eq!(window["minHeight"].as_f64(), Some(WINDOW_MIN_HEIGHT_PX));
    }

    #[test]
    fn unknown_choices_fall_back_to_the_default() {
        assert_eq!(parse_choice(None), DEFAULT_WINDOW_PRESET);
        assert_eq!(parse_choice(Some("huge")), DEFAULT_WINDOW_PRESET);
        assert_eq!(parse_choice(Some("")), DEFAULT_WINDOW_PRESET);
        assert_eq!(parse_choice(Some(FREE_WINDOW)), FREE_WINDOW);
        assert_eq!(parse_choice(Some("large")), "large");
        assert!(is_choice("compact") && is_choice(FREE_WINDOW));
        assert!(!is_choice("Compact") && !is_choice("1280x800"));
    }

    #[test]
    fn a_preset_that_fits_is_kept() {
        assert_eq!(
            fitting("standard", screen(1920.0, 1080.0)),
            Some("standard")
        );
        assert_eq!(fitting("large", screen(1920.0, 1080.0)), Some("large"));
        assert_eq!(
            fitting("extra_large", screen(2560.0, 1440.0)),
            Some("extra_large")
        );
    }

    #[test]
    fn a_preset_that_does_not_fit_steps_down() {
        // 1920 by 1080 at 125 %.
        assert_eq!(fitting("standard", screen(1536.0, 864.0)), Some("compact"));
        assert_eq!(
            fitting("extra_large", screen(1920.0, 1080.0)),
            Some("large")
        );
        // 1366 by 768 at 100 %.
        assert_eq!(fitting("large", screen(1366.0, 768.0)), Some("compact"));
    }

    #[test]
    fn a_window_that_fits_exactly_still_fits() {
        // 1920 by 1080 at 150 %, 720 minus the taskbar leaves 672, the
        // compact page with its frame.
        let tight = screen(1280.0, 720.0);
        assert_eq!(fitting("compact", tight), Some("compact"));
        let lower = Screen {
            area: extent(tight.area.width, tight.area.height - 1.0),
            ..tight
        };
        assert_eq!(fitting("compact", lower), None);
    }

    #[test]
    fn without_a_fitting_preset_the_window_is_free() {
        assert_eq!(fitting("standard", screen(1024.0, 600.0)), None);
        assert_eq!(fitting(FREE_WINDOW, screen(2560.0, 1440.0)), None);
    }

    #[test]
    fn without_a_screen_the_choice_stands() {
        assert_eq!(
            fitting_preset("extra_large", None).map(|preset| preset.name),
            Some("extra_large")
        );
        assert_eq!(fitting_preset(FREE_WINDOW, None), None);
    }

    #[test]
    fn a_free_window_shrinks_to_the_screen_but_not_below_its_minimum() {
        assert_eq!(
            free_page(extent(1280.0, 800.0), screen(1920.0, 1080.0)),
            extent(1280.0, 800.0)
        );
        assert_eq!(
            free_page(extent(1280.0, 800.0), screen(1280.0, 720.0)),
            extent(1264.0, 640.0)
        );
        assert_eq!(
            free_page(extent(1280.0, 800.0), screen(800.0, 600.0)),
            extent(WINDOW_MIN_WIDTH_PX, WINDOW_MIN_HEIGHT_PX)
        );
    }

    #[test]
    fn the_state_names_presets_that_do_not_fit() {
        let state = build_state("large", Some(screen(1536.0, 864.0)));
        assert_eq!(state.choice, "large");
        assert_eq!(state.applied, "compact");
        let fitting: Vec<_> = state.presets.iter().map(|preset| preset.fits).collect();
        assert_eq!(fitting, [true, false, false, false]);

        let unknown = build_state("tiny", None);
        assert_eq!(unknown.choice, DEFAULT_WINDOW_PRESET);
        assert_eq!(unknown.applied, DEFAULT_WINDOW_PRESET);
        assert!(unknown.presets.iter().all(|preset| preset.fits));
    }
}
