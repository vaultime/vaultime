// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

/** Mirrors the Rust `Game` struct. */
export interface Game {
  id: string;
  title: string;
  executable_path: string | null;
  install_folder: string | null;
  launcher_source: string | null;
  metadata_json: string;
  is_hidden: boolean;
  created_at: string;
  updated_at: string;
}

/** Mirrors the Rust `CreateGame` struct. */
export interface CreateGameInput {
  title: string;
  executable_path?: string | null;
  install_folder?: string | null;
  launcher_source?: string | null;
}

/** Mirrors the Rust `UpdateGame` struct. */
export interface UpdateGameInput {
  title?: string | null;
  executable_path?: string | null;
  install_folder?: string | null;
  launcher_source?: string | null;
  is_hidden?: boolean | null;
}

/** Mirrors the Rust `Session` struct. */
export interface Session {
  id: string;
  game_id: string;
  device_id: string;
  started_at_wall: string;
  ended_at_wall: string | null;
  elapsed_monotonic_ms: number;
  active_ms: number;
  idle_ms: number;
  runtime_ms: number;
  integrity_status: string;
  closed_cleanly: boolean;
}

/** Mirrors the Rust `SessionEvent` struct. */
export interface SessionEvent {
  id: string;
  session_id: string;
  sequence: number;
  event_type: string;
  event_time_wall: string;
  event_time_monotonic: number | null;
  payload_json: string;
  hash_prev: string | null;
  hash_self: string | null;
  signature: string | null;
}

/** Mirrors the Rust `BackupSnapshot` struct. */
export interface BackupSnapshot {
  id: string;
  created_at: string;
  source_device_id: string | null;
  checksum: string;
  remote_path: string | null;
  restore_point_label: string | null;
}

/** Mirrors the Rust `Setting` struct. */
export interface Setting {
  key: string;
  value: string;
  updated_at: string;
}

/** Runtime tracking capabilities reported by the backend. */
export interface TrackingDiagnostics {
  platform: string;
  running: boolean;
  foreground_detection: string;
  idle_detection: string;
  poll_interval_seconds: number;
}

/** Summary returned by local backup export/import/inspection commands. */
export interface LocalBackupSummary {
  backup_id: string;
  backup_version: number;
  created_at: string;
  app_version: string;
  source_device_id: string;
  schema_migrations: string[];
  overall_checksum: string;
  games_count: number;
  sessions_count: number;
  assets_count: number;
  asset_file_count: number;
  backup_path: string;
  manifest_path: string;
  restart_required: boolean;
}

/** A game candidate found during auto-discovery. */
export interface DiscoveredGame {
  title: string;
  executable_path: string;
  install_folder: string | null;
  source: string;
  source_id: string | null;
  already_added: boolean;
}

/** Artwork entry plus an inline preview payload returned by the backend. */
export interface GameAssetView {
  id: string;
  game_id: string;
  asset_type: string;
  source: string;
  file_path: string;
  cache_path: string | null;
  hash: string | null;
  created_at: string;
  preview_data_url: string | null;
  is_preferred: boolean;
}

export interface CloudAuthUser {
  id: string;
  email: string;
  role: string;
}

export interface CloudAuthSession {
  access_token: string;
  refresh_token: string;
  expires_at: string;
  refresh_expires_at: string;
  user: CloudAuthUser;
}

export interface CloudDevice {
  id: string;
  client_device_id: string;
  device_name: string;
  platform: string;
  app_version: string;
  registered_at: string;
  last_seen_at: string;
}

export interface CloudBackupPayloadSummary {
  local_backup_id: string;
  backup_version: number;
  created_at: string;
  app_version: string;
  source_device_id: string;
  overall_checksum: string;
  games_count: number;
  sessions_count: number;
  assets_count: number;
  asset_file_count: number;
  archive_format: string;
  encryption: string;
  archive_checksum: string;
  archive_size_bytes: number;
}

export interface CloudBackupRecord {
  id: string;
  label: string | null;
  storage_key: string;
  checksum: string;
  size_bytes: number;
  backup_created_at: string;
  uploaded_at: string;
  status: string;
  client_device_id: string | null;
  metadata_json: CloudBackupPayloadSummary | null;
}

export interface CloudCreateAdminInviteInput {
  prefix?: string | null;
  max_redemptions?: number | null;
  expires_at?: string | null;
  note?: string | null;
}

export interface CloudAdminInvite {
  code: string;
  lookup_key: string;
  salt: string;
  code_hash: string;
  max_redemptions: number;
  expires_at: string | null;
  note: string | null;
  created_at: string;
}

export interface CloudBackupUploadResult {
  backup: CloudBackupRecord;
  payload_summary: CloudBackupPayloadSummary;
}

export interface CloudBackupRestoreSummary {
  created_at: string;
  source_device_id: string;
  overall_checksum: string;
  games_count: number;
  sessions_count: number;
  assets_count: number;
  asset_file_count: number;
  restart_required: boolean;
}

export interface CloudBackupRestoreResult {
  backup: CloudBackupRecord;
  restored_summary: CloudBackupRestoreSummary;
}
