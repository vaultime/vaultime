// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Row types shared by the repositories and the IPC layer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub executable_path: Option<String>,
    pub install_folder: Option<String>,
    pub launcher_source: Option<String>,
    pub metadata_json: String,
    pub is_hidden: bool,
    pub created_at: String,
    pub updated_at: String,
    /// The PC this game came from with merged sessions. None for the games
    /// of this PC, the only ones the tracker watches.
    pub origin_device_id: Option<String>,
}

/// A game's status from a moment on: `backlog`, `playing`, `finished`,
/// `dropped`, or `none` when it was cleared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStatusChange {
    pub id: String,
    pub game_id: String,
    pub status: String,
    pub changed_at: String,
}

/// A short note the player wrote on a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionNote {
    pub session_id: String,
    pub note: String,
    pub updated_at: String,
}

/// Playtime from before Vaultime, imported from a launcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlierPlaytime {
    pub game_id: String,
    /// Where it came from, `steam`.
    pub source: String,
    /// The launcher's total at the import.
    pub launcher_minutes: i64,
    /// Runtime Vaultime had tracked for the game before the import.
    pub tracked_before_ms: i64,
    /// The launcher's total without the part Vaultime had tracked.
    pub earlier_ms: i64,
    pub last_played_at: Option<String>,
    pub imported_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameMetadata {
    pub preferred_cover_asset_id: Option<String>,
    /// How the player cut each cover from its original file, by asset id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cover_crops: BTreeMap<String, CropRect>,
    /// Counts only while no other game runs, like a launcher or a game's
    /// client. Its time beside another game is set aside.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub steps_aside: bool,
    /// The launcher's own id for the game, like the Steam app id, so the same
    /// game can be told apart on another PC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher_id: Option<String>,
    /// Fields this version does not know, as a newer one wrote them, kept so
    /// that a write here does not drop them.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// The part of an image that becomes a cover, in fractions of the image width
/// and height. It reaches past the edges when the image was zoomed out to fit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CropRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Deserialize)]
pub struct CreateGame {
    pub title: String,
    pub executable_path: Option<String>,
    pub install_folder: Option<String>,
    pub launcher_source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGame {
    pub title: Option<String>,
    pub executable_path: Option<String>,
    pub install_folder: Option<String>,
    pub launcher_source: Option<String>,
    pub is_hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAsset {
    pub id: String,
    pub game_id: String,
    pub asset_type: String,
    pub source: String,
    pub file_path: String,
    pub cache_path: Option<String>,
    pub hash: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub game_id: String,
    pub device_id: String,
    pub started_at_wall: String,
    pub ended_at_wall: Option<String>,
    pub elapsed_monotonic_ms: i64,
    pub active_ms: i64,
    pub idle_ms: i64,
    pub runtime_ms: i64,
    pub integrity_status: String,
    pub closed_cleanly: bool,
    /// Runtime, active and idle time set aside because the game steps aside
    /// for others. Worked out from the play slices, never stored.
    #[serde(default)]
    pub set_aside_ms: i64,
    #[serde(default)]
    pub set_aside_active_ms: i64,
    #[serde(default)]
    pub set_aside_idle_ms: i64,
    /// Runtime, active and idle time that another copy of the same game ran
    /// through as well, like a linked install or the game on another PC. One
    /// game cannot be played twice at once, so that time counts once, shared
    /// between the copies. Worked out from the play slices, never stored.
    #[serde(default, skip_serializing)]
    pub copy_overlap_ms: i64,
    #[serde(default, skip_serializing)]
    pub copy_overlap_active_ms: i64,
    #[serde(default, skip_serializing)]
    pub copy_overlap_idle_ms: i64,
    /// Runtime, active and idle time the session adds to totals: without the
    /// time set aside, and with the time it shares with another copy of its
    /// game counted once. Worked out for the pages, never stored.
    #[serde(default)]
    pub counted_runtime_ms: i64,
    #[serde(default)]
    pub counted_active_ms: i64,
    #[serde(default)]
    pub counted_idle_ms: i64,
    /// When the time a closed session counts ended, for a game that steps
    /// aside, and its start when none of it counts. Worked out from the play
    /// spans, never stored.
    #[serde(default, skip_serializing)]
    pub counted_until: Option<String>,
    /// The game the session was recorded with, when it is shown as the game
    /// that one is linked to. Never stored, the session keeps its own game.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_game_id: Option<String>,
    /// The PC whose session was still running when the backup it came with
    /// was made, for a session shown up to its last event. Never stored, the
    /// session stays open until a merge brings its end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still_running_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    pub id: String,
    pub session_id: String,
    pub sequence: i64,
    pub event_type: String,
    pub event_time_wall: String,
    pub event_time_monotonic: Option<i64>,
    pub payload_json: String,
    pub hash_prev: Option<String>,
    pub hash_self: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub platform: String,
    pub app_version: String,
    /// The public key the PC signs its ledger with, in hex.
    pub key_id: Option<String>,
    pub registered_at: String,
    /// What the player calls the PC. It starts as the PC's own name.
    pub name: Option<String>,
    /// When sessions of this PC were last merged here.
    pub merged_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSnapshot {
    pub id: String,
    pub created_at: String,
    pub source_device_id: Option<String>,
    pub checksum: String,
    pub remote_path: Option<String>,
    pub restore_point_label: Option<String>,
}
