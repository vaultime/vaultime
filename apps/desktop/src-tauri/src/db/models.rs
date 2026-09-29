// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Row types shared by the repositories and the IPC layer.

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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameMetadata {
    pub preferred_cover_asset_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub key_id: Option<String>,
    pub registered_at: String,
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
