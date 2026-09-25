// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Game discovery through folder scans and the Steam library.

pub mod metadata;
pub mod scanner;
pub mod steam;

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::db::connection::Database;
use crate::db::repo::games;
use crate::error::Result;
use crate::platform::process::path_key;

/// A game candidate found during discovery. The user picks which ones to import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredGame {
    pub title: String,
    pub executable_path: String,
    /// Install folder, either reported by the launcher or the executable's parent.
    pub install_folder: Option<String>,
    /// Where the candidate came from, `steam` or `folder_scan`.
    pub source: String,
    /// Launcher app id, for example the Steam app id.
    pub source_id: Option<String>,
    /// True when the library already has a game with this executable.
    pub already_added: bool,
}

/// Path keys of every executable already in the library.
fn library_executables(db: &Database) -> Result<HashSet<String>> {
    Ok(games::list_all_games(db)?
        .into_iter()
        .filter_map(|game| game.executable_path)
        .map(|path| path_key(&path))
        .collect())
}

/// Whether a file can be launched as a program on this platform.
fn is_executable(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    #[cfg(windows)]
    {
        ext == "exe"
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Shared libraries often carry the execute bit but are never games.
        let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase());
        if ext == "so" || name.is_some_and(|n| n.contains(".so.")) {
            return false;
        }

        matches!(ext.as_str(), "sh" | "x86_64" | "x86")
            || path
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    }

    #[cfg(not(any(windows, unix)))]
    {
        let _ = ext;
        false
    }
}
