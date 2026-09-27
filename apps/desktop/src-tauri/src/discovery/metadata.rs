// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Title inference and metadata extraction from file paths and folder names.

use std::path::Path;

/// Infer a human-readable game title from an executable path.
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

fn clean_title(raw: &str) -> String {
    let mut title = raw.to_string();

    for suffix in &[
        "-Win64-Shipping",
        "-Win32-Shipping",
        "_Win64_Shipping",
        "-Shipping",
        "_Shipping",
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
        " Launcher",
    ] {
        if let Some(pos) = title.to_lowercase().rfind(&suffix.to_lowercase()) {
            title.truncate(pos);
        }
    }

    title = title.replace(['_', '-', '.'], " ");

    let parts: Vec<&str> = title.split_whitespace().collect();
    let result = parts.join(" ");

    if result.is_empty() {
        raw.to_string()
    } else {
        result
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
    ];

    for pattern in &reject_patterns {
        if lower.contains(pattern) {
            return false;
        }
    }

    true
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
    }
}
