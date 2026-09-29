// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

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
  EarlierPlaytime,
  ExportFormat,
  GameStatus,
  GameStatusChange,
  SessionNote,
  SteamPlaytimePreview,
} from "@/lib/types";

export async function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
}

export async function loadCloudSessionSecure(): Promise<string | null> {
  return invoke<string | null>("load_cloud_session_secure");
}

export async function hasCloudBackupKeySecure(
  accountId: string,
): Promise<boolean> {
  return invoke<boolean>("has_cloud_backup_key_secure", { accountId });
}

export async function storeCloudSessionSecure(
  sessionJson: string,
): Promise<boolean> {
  return invoke<boolean>("store_cloud_session_secure", { sessionJson });
}

export async function clearCloudSessionSecure(): Promise<boolean> {
  return invoke<boolean>("clear_cloud_session_secure");
}

/** Refuses a passphrase whose key does not match `expectedKeyCheck`, the key check of the newest cloud backup. */
export async function storeCloudBackupKeySecure(
  accountId: string,
  passphrase: string,
  expectedKeyCheck: string | null,
): Promise<boolean> {
  return invoke<boolean>("store_cloud_backup_key_secure", {
    accountId,
    passphrase,
    expectedKeyCheck,
  });
}

export async function clearCloudBackupKeySecure(
  accountId: string,
): Promise<boolean> {
  return invoke<boolean>("clear_cloud_backup_key_secure", { accountId });
}

export async function listGames(): Promise<Game[]> {
  return invoke<Game[]>("list_games");
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

export async function listSessions(): Promise<Session[]> {
  return invoke<Session[]>("list_sessions");
}

export async function getActiveSessions(): Promise<Session[]> {
  return invoke<Session[]>("get_active_sessions");
}

export async function getSessionEventsForGame(
  gameId: string,
): Promise<SessionEvent[]> {
  return invoke<SessionEvent[]>("get_session_events_for_game", { gameId });
}

/** Saves every finished session to `path` and returns how many. */
export async function exportSessions(path: string, format: ExportFormat): Promise<number> {
  return invoke<number>("export_sessions", { path, format });
}

/** Counts a closed session only up to `endedAt`, with a reason. */
export async function trimSession(sessionId: string, endedAt: string, reason: string): Promise<Session> {
  return invoke<Session>("trim_session", { sessionId, endedAt, reason });
}

/** Takes all time out of a closed session, with a reason. */
export async function discardSession(sessionId: string, reason: string): Promise<Session> {
  return invoke<Session>("discard_session", { sessionId, reason });
}

/** Adds play Vaultime did not see, labeled Manual. */
export async function addManualSession(
  gameId: string,
  startedAt: string,
  runtimeMs: number,
  reason: string,
  launcher: string | null,
): Promise<Session> {
  return invoke<Session>("add_manual_session", { gameId, startedAt, runtimeMs, reason, launcher });
}

/** Every status change of every game, oldest first. */
export async function listStatusChanges(): Promise<GameStatusChange[]> {
  return invoke<GameStatusChange[]>("list_status_changes");
}

export async function setGameStatus(gameId: string, status: GameStatus | "none"): Promise<GameStatusChange | null> {
  return invoke<GameStatusChange | null>("set_game_status", { gameId, status });
}

export async function listSessionNotes(): Promise<SessionNote[]> {
  return invoke<SessionNote[]>("list_session_notes");
}

/** An empty note removes it. */
export async function setSessionNote(sessionId: string, note: string): Promise<SessionNote | null> {
  return invoke<SessionNote | null>("set_session_note", { sessionId, note });
}

export async function listEarlierPlaytime(): Promise<EarlierPlaytime[]> {
  return invoke<EarlierPlaytime[]>("list_earlier_playtime");
}

/** What an import from Steam would add, without storing anything. */
export async function previewSteamPlaytime(): Promise<SteamPlaytimePreview> {
  return invoke<SteamPlaytimePreview>("preview_steam_playtime");
}

export async function importSteamPlaytime(): Promise<SteamPlaytimePreview> {
  return invoke<SteamPlaytimePreview>("import_steam_playtime");
}

/** Removes all playtime imported from Steam. Returns how many games had some. */
export async function removeSteamPlaytime(): Promise<number> {
  return invoke<number>("remove_steam_playtime");
}

/** Turns the tray and taskbar icon violet while signed in to cloud backup. */
export async function setCloudSignedIn(signedIn: boolean): Promise<void> {
  return invoke("set_cloud_signed_in", { signedIn });
}

export async function getAutoBackupFolder(): Promise<string> {
  return invoke<string>("get_auto_backup_folder");
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
  accountId: string,
  clientDeviceId?: string | null,
  label?: string | null,
): Promise<CloudBackupUploadResult> {
  return invoke<CloudBackupUploadResult>("upload_remote_backup", {
    apiBaseUrl,
    accessToken,
    accountId,
    clientDeviceId,
    label,
  });
}

export async function restoreRemoteBackup(
  apiBaseUrl: string,
  accessToken: string,
  accountId: string,
  backupId: string,
): Promise<CloudBackupRestoreResult> {
  return invoke<CloudBackupRestoreResult>("restore_remote_backup", {
    apiBaseUrl,
    accessToken,
    accountId,
    backupId,
  });
}

export async function listSettings(): Promise<Setting[]> {
  return invoke<Setting[]>("list_settings");
}

export async function setSetting(
  key: string,
  value: string,
): Promise<boolean> {
  return invoke<boolean>("set_setting", { key, value });
}

export async function getTrackingDiagnostics(): Promise<TrackingDiagnostics> {
  return invoke<TrackingDiagnostics>("get_tracking_diagnostics");
}

/** Whether this system shows a tray icon, so closing the window can keep tracking. */
export async function trayAvailable(): Promise<boolean> {
  return invoke<boolean>("tray_available");
}

export async function discoverGames(
  paths: string[],
): Promise<DiscoveredGame[]> {
  return invoke<DiscoveredGame[]>("discover_games", { paths });
}

/** Games from every launcher other than Steam that are installed on this PC. */
export async function discoverLauncherGames(): Promise<DiscoveredGame[]> {
  return invoke<DiscoveredGame[]>("discover_launcher_games");
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
