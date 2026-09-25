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
  cloud_verified: boolean;
  cloud_verified_at: string | null;
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
  synced_at: string | null;
  server_ack_at: string | null;
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

// ---------------------------------------------------------------------------
// Cloud
// ---------------------------------------------------------------------------

/** Mirrors the Rust `CloudUser` struct. */
export interface CloudUser {
  id: string;
  email: string;
  created_at: string | null;
}

/** Mirrors the Rust `CloudSession` struct. */
export interface CloudSession {
  user: CloudUser;
  device_registered: boolean;
  expires_at: number;
}

/** Mirrors the Rust `CloudConfig` struct. */
export interface CloudConfig {
  configured: boolean;
  billing_enabled: boolean;
}

/** Subscription tier. */
export type SubscriptionTier = "free" | "pro";

/** Subscription status. */
export type SubscriptionStatus =
  | "none"
  | "active"
  | "past_due"
  | "canceled"
  | "expired";

/** Mirrors the Rust `Subscription` struct. */
export interface Subscription {
  tier: SubscriptionTier;
  status: SubscriptionStatus;
  current_period_end: string | null;
  cancel_at_period_end: boolean;
}

/** Result of a sync pass. */
export interface SyncResult {
  uploaded: number;
  remaining: number;
  verified_sessions: number;
  conflicted_events: number;
  last_sync_at: string | null;
}

/** Snapshot of current cloud sync status. */
export interface CloudSyncStatus {
  connected: boolean;
  last_sync_at: string | null;
  last_backup_at: string | null;
  pending_events: number;
}

/** Metadata row returned for a cloud backup snapshot. */
export interface CloudBackupRecord {
  id: string;
  device_id: string;
  created_at: string;
  checksum: string;
  storage_path: string;
  size_bytes: number | null;
  label: string | null;
}

/** Summary of a cloud backup payload. */
export interface CloudBackupSummary {
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
}

/** Result returned after creating and uploading a cloud backup. */
export interface CloudBackupUploadResult {
  backup: CloudBackupRecord;
  summary: CloudBackupSummary;
  uploaded_files: number;
}

/** Preflight details shown before a cloud restore. */
export interface CloudBackupRestorePreview {
  backup: CloudBackupRecord;
  summary: CloudBackupSummary;
  has_active_sessions: boolean;
  unsynced_events: number;
  newer_local_sessions: number;
  requires_force: boolean;
}

/** Result returned after restoring a cloud backup locally. */
export interface CloudBackupRestoreResult {
  backup: CloudBackupRecord;
  restart_required: boolean;
}

/** Input for sign-up / sign-in commands. */
export interface AuthCredentials {
  email: string;
  password: string;
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

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
