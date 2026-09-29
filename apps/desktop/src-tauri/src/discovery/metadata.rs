// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Title inference and metadata extraction from file paths and folder names.

use std::path::Path;

/// Infers a human-readable game title from an executable path.
///
/// Prefers the parent folder name, for example "Counter-Strike 2", and falls
/// back to the file name. Strips build suffixes and separators.
pub fn infer_title(executable_path: &str) -> String {
    let path = Path::new(executable_path);

    // The parent folder is usually the install directory and the best title.
    let from_parent = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .and_then(|name| {
            let cleaned = clean_title(&name);
            if is_generic_folder(&cleaned) {
                None
            } else {
                Some(cleaned)
            }
        });

    if let Some(title) = from_parent {
        return title;
    }

    let filename = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    clean_title(&filename)
}

/// Build and launcher suffixes, in lowercase. Each comes before any shorter suffix it ends with.
const TITLE_SUFFIXES: &[&str] = &[
    "-win64-shipping",
    "-win32-shipping",
    "_win64_shipping",
    "-shipping",
    "_shipping",
    "_x64",
    "_x86",
    "-x64",
    "-x86",
    ".x86_64",
    ".x86",
    "_64bit",
    "_32bit",
    "-launcher",
    "_launcher",
    " launcher",
];

fn clean_title(raw: &str) -> String {
    let title = strip_title_suffixes(raw).replace(['_', '-', '.'], " ");

    let parts: Vec<&str> = title.split_whitespace().collect();
    let result = parts.join(" ");

    if result.is_empty() {
        raw.to_string()
    } else {
        result
    }
}

/// Strips suffixes from the end only, repeatedly, as in `Game_x64-launcher`.
/// ASCII lowercasing keeps byte positions, so the cut lands on a char boundary.
fn strip_title_suffixes(mut title: &str) -> &str {
    loop {
        let lower = title.to_ascii_lowercase();
        let Some(suffix) = TITLE_SUFFIXES
            .iter()
            .find(|suffix| lower.ends_with(*suffix))
        else {
            return title;
        };
        title = &title[..title.len() - suffix.len()];
    }
}

fn is_generic_folder(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.as_str(),
        "bin"
            | "binaries"
            | "game"
            | "games"
            | "x64"
            | "x86"
            | "win64"
            | "win32"
            | "windows"
            | "linux"
            | "macos"
            | "shipping"
            | "release"
            | "debug"
            | "build"
            | "dist"
            | "steamapps"
            | "common"
            | "program files"
            | "program files (x86)"
    )
}

/// Filters out helper programs that ship in game folders, such as crash
/// reporters, redistributables and uninstallers.
pub fn is_likely_game_executable(filename: &str) -> bool {
    let lower = filename.to_lowercase();

    let reject_patterns = [
        "unins",
        "uninst",
        "setup",
        "install",
        "crash",
        "reporter",
        "redist",
        "vcredist",
        "dxsetup",
        "directx",
        "dotnet",
        "ue4prereq",
        "ue4-prereq",
        "launcher",
        "updater",
        "update",
        "helper",
        "eac_",
        "easyanticheat",
        "battleye",
        "beclient",
        "beservice",
        "steam_api",
        "steamclient",
        // Crash reporters and embedded browsers.
        "crs-handler",
        "crs-uploader",
        "cefsubprocess",
        "prereq",
        // Dedicated servers.
        "srcds",
        "dedicated",
    ];

    // Source engine tools that ship next to the game. Whole names only, a
    // part like "hammer" would also hit games such as Warhammer.
    let reject_names = [
        "qc_eyes",
        "elementviewer",
        "studiomdl",
        "hlfaceposer",
        "dmxedit",
        "makescenesimage",
        "vconsole2",
        "hammer",
        "captioncompiler",
        "shadercompile",
        "resourcecompiler",
        "vbsp",
        "vvis",
        "vrad",
    ];

    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    let stem = stem.strip_suffix("_win64").unwrap_or(stem);
    !reject_patterns
        .iter()
        .any(|pattern| lower.contains(pattern))
        && !reject_names.contains(&stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_from_parent_folder() {
        assert_eq!(
            infer_title("/home/user/.steam/steamapps/common/Counter-Strike 2/cs2"),
            "Counter Strike 2"
        );
    }

    #[test]
    fn title_from_exe_name() {
        assert_eq!(infer_title("/opt/games/bin/Factorio.x86_64"), "Factorio");
    }

    #[test]
    fn strips_shipping_suffix() {
        assert_eq!(
            infer_title("/games/MyGame/Binaries/Win64/MyGame-Win64-Shipping.exe"),
            "MyGame"
        );
    }

    #[test]
    fn strips_suffixes_after_letters_that_change_length_when_lowercased() {
        // The Kelvin sign takes three bytes and its lowercase "k" one.
        assert_eq!(clean_title("\u{212A}ゲーム_x64"), "\u{212A}ゲーム");
        assert_eq!(
            clean_title("İstanbul Racer-Win64-Shipping"),
            "İstanbul Racer"
        );
    }

    #[test]
    fn strips_stacked_suffixes_but_keeps_inner_words() {
        assert_eq!(clean_title("Game_x64-launcher"), "Game");
        assert_eq!(
            clean_title("Space Launcher Tycoon"),
            "Space Launcher Tycoon"
        );
    }

    #[test]
    fn generic_folder_falls_back_to_exe() {
        assert_eq!(infer_title("/opt/games/bin/hollow_knight"), "hollow knight");
    }

    #[test]
    fn rejects_non_game_executables() {
        assert!(!is_likely_game_executable("UnityCrashHandler64.exe"));
        assert!(!is_likely_game_executable("unins000.exe"));
        assert!(!is_likely_game_executable("vcredist_x64.exe"));
        assert!(is_likely_game_executable("Cyberpunk2077.exe"));
        assert!(is_likely_game_executable("factorio"));
        assert!(!is_likely_game_executable("crs-handler.exe"));
        assert!(!is_likely_game_executable("srcds_win64.exe"));
        assert!(!is_likely_game_executable("hammer.exe"));
        assert!(!is_likely_game_executable("vconsole2.exe"));
        assert!(!is_likely_game_executable("studiomdl_win64.exe"));
        assert!(is_likely_game_executable("Warhammer3.exe"));
        assert!(is_likely_game_executable("cstrike_win64.exe"));
    }
}
