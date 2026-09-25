// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { invoke } from "@tauri-apps/api/core";
import type {
  Game,
  CreateGameInput,
  UpdateGameInput,
  BackupSnapshot,
  LocalBackupSummary,
  Session,
  SessionEvent,
  Setting,
  TrackingDiagnostics,
  GameAssetView,
  DiscoveredGame,
  CloudBackupRestoreResult,
  CloudBackupUploadResult,
} from "@/lib/types";

// ---------------------------------------------------------------------------
// App commands
// ---------------------------------------------------------------------------

export async function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
}

export async function loadCloudSessionSecure(): Promise<string | null> {
  return invoke<string | null>("load_cloud_session_secure");
}

export async function storeCloudSessionSecure(
  sessionJson: string,
): Promise<boolean> {
  return invoke<boolean>("store_cloud_session_secure", { sessionJson });
}

export async function clearCloudSessionSecure(): Promise<boolean> {
  return invoke<boolean>("clear_cloud_session_secure");
}

export async function storeCloudBackupKeySecure(
  accountId: string,
  email: string,
  password: string,
): Promise<boolean> {
  return invoke<boolean>("store_cloud_backup_key_secure", {
    accountId,
    email,
    password,
  });
}

export async function clearCloudBackupKeySecure(): Promise<boolean> {
  return invoke<boolean>("clear_cloud_backup_key_secure");
}

// ---------------------------------------------------------------------------
// Game commands
// ---------------------------------------------------------------------------

export async function listGames(): Promise<Game[]> {
  return invoke<Game[]>("list_games");
}

export async function getGame(id: string): Promise<Game> {
  return invoke<Game>("get_game", { id });
}

export async function createGame(input: CreateGameInput): Promise<Game> {
  return invoke<Game>("create_game", { input });
}

export async function updateGame(
  id: string,
  input: UpdateGameInput,
): Promise<Game> {
  return invoke<Game>("update_game", { id, input });
}

export async function deleteGame(id: string): Promise<boolean> {
  return invoke<boolean>("delete_game", { id });
}

// ---------------------------------------------------------------------------
// Asset commands
// ---------------------------------------------------------------------------

export async function listGameAssets(gameId: string): Promise<GameAssetView[]> {
  return invoke<GameAssetView[]>("list_game_assets", { gameId });
}

export async function listPreferredGameAssets(): Promise<GameAssetView[]> {
  return invoke<GameAssetView[]>("list_preferred_game_assets");
}

export async function scanGameAssets(gameId: string): Promise<GameAssetView[]> {
  return invoke<GameAssetView[]>("scan_game_assets", { gameId });
}

export async function importGameAsset(
  gameId: string,
  sourcePath: string,
): Promise<GameAssetView[]> {
  return invoke<GameAssetView[]>("import_game_asset", {
    gameId,
    sourcePath,
  });
}

export async function setPreferredGameAsset(
  gameId: string,
  assetId: string,
): Promise<boolean> {
  return invoke<boolean>("set_preferred_game_asset", {
    gameId,
    assetId,
  });
}

// ---------------------------------------------------------------------------
// Session commands
// ---------------------------------------------------------------------------

export async function listSessions(): Promise<Session[]> {
  return invoke<Session[]>("list_sessions");
}

export async function getSessionsForGame(gameId: string): Promise<Session[]> {
  return invoke<Session[]>("get_sessions_for_game", { gameId });
}

export async function getActiveSessions(): Promise<Session[]> {
  return invoke<Session[]>("get_active_sessions");
}

export async function getSessionEventsForGame(
  gameId: string,
): Promise<SessionEvent[]> {
  return invoke<SessionEvent[]>("get_session_events_for_game", { gameId });
}

export async function listBackupSnapshots(): Promise<BackupSnapshot[]> {
  return invoke<BackupSnapshot[]>("list_backup_snapshots");
}

export async function exportLocalBackup(
  destinationDir: string,
): Promise<LocalBackupSummary> {
  return invoke<LocalBackupSummary>("export_local_backup", { destinationDir });
}

export async function inspectLocalBackup(
  path: string,
): Promise<LocalBackupSummary> {
  return invoke<LocalBackupSummary>("inspect_local_backup", { path });
}

export async function importLocalBackup(
  path: string,
): Promise<LocalBackupSummary> {
  return invoke<LocalBackupSummary>("import_local_backup", { path });
}

export async function uploadRemoteBackup(
  apiBaseUrl: string,
  accessToken: string,
  clientDeviceId?: string | null,
  label?: string | null,
): Promise<CloudBackupUploadResult> {
  return invoke<CloudBackupUploadResult>("upload_remote_backup", {
    apiBaseUrl,
    accessToken,
    clientDeviceId,
    label,
  });
}

export async function restoreRemoteBackup(
  apiBaseUrl: string,
  accessToken: string,
  backupId: string,
): Promise<CloudBackupRestoreResult> {
  return invoke<CloudBackupRestoreResult>("restore_remote_backup", {
    apiBaseUrl,
    accessToken,
    backupId,
  });
}

// ---------------------------------------------------------------------------
// Settings commands
// ---------------------------------------------------------------------------

export async function listSettings(): Promise<Setting[]> {
  return invoke<Setting[]>("list_settings");
}

export async function setSetting(
  key: string,
  value: string,
): Promise<boolean> {
  return invoke<boolean>("set_setting", { key, value });
}

// ---------------------------------------------------------------------------
// Tracking commands
// ---------------------------------------------------------------------------

export async function getTrackingStatus(): Promise<boolean> {
  return invoke<boolean>("get_tracking_status");
}

export async function getTrackingDiagnostics(): Promise<TrackingDiagnostics> {
  return invoke<TrackingDiagnostics>("get_tracking_diagnostics");
}

// ---------------------------------------------------------------------------
// Discovery commands
// ---------------------------------------------------------------------------

export async function discoverGames(
  paths: string[],
): Promise<DiscoveredGame[]> {
  return invoke<DiscoveredGame[]>("discover_games", { paths });
}

export async function discoverSteamGames(): Promise<DiscoveredGame[]> {
  return invoke<DiscoveredGame[]>("discover_steam_games");
}

export async function getDefaultScanPaths(): Promise<string[]> {
  return invoke<string[]>("get_default_scan_paths");
}

export async function importDiscoveredGames(
  discoveries: DiscoveredGame[],
): Promise<Game[]> {
  return invoke<Game[]>("import_discovered_games", { discoveries });
}
