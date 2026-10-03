// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

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
  /** The PC this game came from with merged sessions. Null for this PC's games, the only ones tracked here. */
  origin_device_id: string | null;
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

/** A program the player said is no game. Mirrors the Rust `IgnoredProgram`. */
export interface IgnoredProgram {
  path_key: string;
  path: string;
  title: string;
  ignored_at: string;
}

/** What the ledgers say about the history as a whole. */
export interface LedgerReport {
  /** This PC's public key in hex, which names its ledger. */
  key_id: string;
  /** When this PC's ledger began, if it has. */
  began_at: string | null;
  /** Entries in this PC's ledger. */
  entries: number;
  /** Sessions that a ledger covers. */
  covered_sessions: number;
  /** Sessions a ledger names that are gone without a note that the player removed them. */
  missing_sessions: number;
  /** Whether any ledger fails its check. */
  broken: boolean;
}

/** A PC this database knows. */
export interface Device {
  id: string;
  platform: string;
  app_version: string;
  key_id: string | null;
  registered_at: string;
  name: string | null;
  /** When sessions of this PC were last merged here. Such sessions can only be corrected there. */
  merged_at: string | null;
}

/** A game of another PC that counts as a game of this PC. */
export interface GameLink {
  game_id: string;
  /** Its title on the other PC. */
  title: string;
  origin_device_id: string | null;
  linked_game_id: string;
}

/** A game that comes with merged sessions. */
export interface MergeGame {
  game_id: string;
  title: string;
  launcher_source: string | null;
  sessions: number;
  runtime_ms: number;
  /** The game of this PC it most likely is. */
  suggested_game_id: string | null;
  /** `launcher` for the same launcher id, `title` for the same title. */
  suggested_because: "launcher" | "title" | null;
}

/** A session that stays as it is here. */
export interface MergeConflict {
  session_id: string;
  game_title: string;
  started_at_wall: string;
  reason: "changed_on_both" | "update_fails_check";
}

/** What merging another PC's backup would bring in. */
export interface MergePreview {
  backup_path: string;
  backup_created_at: string;
  device_id: string;
  device_name: string | null;
  new_sessions: number;
  grown_sessions: number;
  already_here: number;
  newer_here: number;
  removed_here: number;
  running_there: number;
  /** Sessions that come in unvouched, as they fail their check there too. */
  failing: number;
  /** A ledger in the backup claims to be that PC with a key this PC does not trust for it. */
  unknown_keys: boolean;
  /** This PC has never seen that PC, so its ledgers are taken at their word. */
  first_merge: boolean;
  /** Sessions here that came in unvouched and pass their check now. */
  vouched_now: number;
  /** Sessions added by hand here that overlap sessions that come in. */
  overlapping_manual: number;
  conflicts: MergeConflict[];
  games: MergeGame[];
}

/** The player's choice for a game that comes along. */
export interface GameChoice {
  game_id: string;
  linked_game_id: string | null;
}

export interface MergeSummary {
  device_id: string;
  device_name: string | null;
  sessions_added: number;
  sessions_grown: number;
  /** Sessions here this PC vouches for now. */
  sessions_vouched: number;
  failing: number;
  games_added: number;
  games_linked: number;
  /** The backup saved just before, which a restore can go back to. */
  safety_backup_path: string | null;
}

/** This PC: its name and what the ledgers say about the history. */
export interface ThisPc {
  device_id: string;
  name: string;
  ledger: LedgerReport;
}

/** What one play total covers, in local time. */
export type PlayBucket = "day" | "month" | "year" | "hour_of_week";

/** A game's time in one bucket, worked out by the core from quarter hour slices. */
export interface PlayTotal {
  /**
   * "2026-10-02" for a day, "2026-10" for a month, "2026" for a year, and
   * "weekday-hour" with Monday as 0 for an hour of the week.
   */
  bucket: string;
  game_id: string;
  runtime_ms: number;
  active_ms: number;
  idle_ms: number;
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
  /** Runtime set aside because the game steps aside for other games. The core works it out. */
  set_aside_ms?: number;
  set_aside_active_ms?: number;
  set_aside_idle_ms?: number;
}

/** Mirrors the Rust `ExportFormat` enum. */
export type ExportFormat = "csv" | "json";

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

/** A window size preset or `free`, see `WINDOW_PRESETS` in constants.rs. */
export type WindowSizeChoice = "compact" | "standard" | "large" | "extra_large" | "free";

/** Mirrors the Rust `WindowPresetView` struct. */
export interface WindowPresetView {
  name: Exclude<WindowSizeChoice, "free">;
  width: number;
  height: number;
  /** Whether the window fits the screen it is on at this size. */
  fits: boolean;
}

/** Mirrors the Rust `WindowLook` struct. Each color is `#rrggbb`. */
export interface WindowLook {
  /** The mode the player chose, which the title bar follows. */
  theme: "dark" | "light" | "system";
  /** Accent of the logo in the tray, the taskbar and the title bar. */
  iconAccent: string;
  titleBar: string;
  titleText: string;
  border: string;
}

/** Mirrors the Rust `WindowSizeState` struct. */
export interface WindowSizeState {
  choice: WindowSizeChoice;
  /** What the window uses, a smaller preset or `free` when the choice does not fit this screen. */
  applied: WindowSizeChoice;
  presets: WindowPresetView[];
}

/** Runtime tracking capabilities reported by the core. */
export interface TrackingDiagnostics {
  platform: string;
  running: boolean;
  foreground_detection: string;
  idle_detection: string;
  controller_detection: string;
  controllers_connected: number;
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

/** Part of an image that becomes a cover, in shares of its width and height. */
export interface CropRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** An image opened in the crop dialog. */
export interface ArtworkSource {
  /** The image, scaled down for the dialog. */
  preview_data_url: string;
  /** What fills the cover where the image does not reach. */
  backdrop_data_url: string;
  /** Size of the full image in pixels. */
  width: number;
  height: number;
  /** False when the original file is gone or changed and the cover in use stands in. */
  from_original: boolean;
  /** The crop in use, when it was cut from this same image. */
  crop: CropRect | null;
}

/** Artwork entry plus an inline preview payload returned by the core. */
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

interface CloudAuthUser {
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

interface CloudBackupPayloadSummary {
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
  /** Tells whether a backup key opens this backup. Missing on older backups. */
  key_check?: string;
  /** Artwork the backup restores, stored once for all backups. Missing on older backups. */
  artwork_bytes?: number;
}

/** Where a game stands for the player. */
export type GameStatus = "backlog" | "playing" | "finished" | "dropped";

/** A game's status from a moment on. "none" clears it. */
export interface GameStatusChange {
  id: string;
  game_id: string;
  status: GameStatus | "none";
  changed_at: string;
}

/** A short note the player wrote on a session. */
export interface SessionNote {
  session_id: string;
  note: string;
  updated_at: string;
}

/** Playtime a game had before Vaultime, read once from a launcher. */
export interface EarlierPlaytime {
  game_id: string;
  /** "steam". */
  source: string;
  /** The launcher's total at the import. */
  launcher_minutes: number;
  /** Runtime Vaultime had tracked for the game before the import. */
  tracked_before_ms: number;
  /** The launcher's total without the part Vaultime had tracked. */
  earlier_ms: number;
  last_played_at: string | null;
  imported_at: string;
}

/** A library game and the playtime Steam counted for it. */
export interface EarlierCandidate {
  game_id: string;
  title: string;
  launcher_minutes: number;
  tracked_before_ms: number;
  earlier_ms: number;
  last_played_at: string | null;
}

export interface SteamPlaytimePreview {
  /** Whether a Steam install with playtime was found at all. */
  found: boolean;
  account: string | null;
  games: EarlierCandidate[];
}

/** An application for cloud beta access from the website. */
export interface BetaApplication {
  id: string;
  email: string;
  /** "windows", "linux" or "both". */
  platform: string;
  note: string | null;
  created_at: string;
}

/** Cloud space of the signed in account. */
export interface CloudStorage {
  backup_bytes: number;
  artwork_bytes: number;
  limit_bytes: number;
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
  /** Artwork files that were new to the server. */
  artwork_uploaded: number;
}

interface CloudBackupRestoreSummary {
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
