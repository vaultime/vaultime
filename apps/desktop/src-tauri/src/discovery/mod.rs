// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Game auto-discovery — folder scanning and launcher adapters.

pub mod metadata;
pub mod scanner;
pub mod steam;

use serde::{Deserialize, Serialize};

/// A game candidate found during auto-discovery.
///
/// Not yet added to the library — the user selects which discoveries
/// to import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredGame {
    /// Inferred title.
    pub title: String,
    /// Path to the main executable.
    pub executable_path: String,
    /// Install folder (parent of the executable or launcher-reported).
    pub install_folder: Option<String>,
    /// Where this candidate was found (e.g. "steam", "`folder_scan`").
    pub source: String,
    /// Launcher-specific app ID if applicable (e.g. Steam app ID).
    pub source_id: Option<String>,
    /// Whether a game with this executable path already exists in the library.
    pub already_added: bool,
}
