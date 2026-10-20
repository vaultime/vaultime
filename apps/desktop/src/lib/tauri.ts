// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { invoke } from "@tauri-apps/api/core";
import type {
  ActiveSessions,
  ArtworkSource,
  AutoMerge,
  BackupSnapshot,
  CloudBackupRestoreResult,
  CloudBackupUploadResult,
  CloudPc,
  CreateGameInput,
  CropRect,
  Device,
  DiscoveredGame,
  EarlierPlaytime,
  ExportFormat,
  Game,
  GameAssetView,
  GameChoice,
  GameLink,
  GameStatus,
  GameStatusChange,
  IgnoredProgram,
  JournalWeekPlay,
  LocalBackupSummary,
  MergePreview,
  MergeSummary,
  NewGames,
  PlayBucket,
  PlayTotal,
  RecentPlay,
  Session,
  SessionEvent,
  SessionList,
  SessionNote,
  Setting,
  SteamPlaytimePreview,
  StepsAsideHint,
  ThisPc,
  TrackingDiagnostics,
  TrimPreview,
  UpdateGameInput,
  WindowLook,
  WindowSizeChoice,
  WindowSizeState,
  YearPlay,
} from "@/lib/types";

/** Whether the core is a development build, with a cloud sign-in of its own. */
export async function isDevelopmentBuild(): Promise<boolean> {
  return invoke<boolean>("is_development_build");
}

/** The id of this PC, stored with its sessions and backups. */
export async function getDeviceId(): Promise<string> {
  return invoke<string>("get_device_id");
}

/** This PC's name and what its ledger says. */
export async function getThisPc(): Promise<ThisPc> {
  return invoke<ThisPc>("get_this_pc");
}

export async function renameThisPc(name: string): Promise<ThisPc> {
  return invoke<ThisPc>("rename_this_pc", { name });
}

/** Every PC this database knows, this one included. */
export async function listDevices(): Promise<Device[]> {
  return invoke<Device[]>("list_devices");
}

export async function listGameLinks(): Promise<GameLink[]> {
  return invoke<GameLink[]>("list_game_links");
}

/** Makes a game of another PC count as a game of this PC, or as itself again with null. */
export async function linkGame(gameId: string, linkedGameId: string | null): Promise<void> {
  return invoke<void>("link_game", { gameId, linkedGameId });
}

/** With `trustNewKeys`, the player vouches that new keys claiming the backup's PC are its own. */
export async function previewMerge(path: string, trustNewKeys = false): Promise<MergePreview> {
  return invoke<MergePreview>("preview_merge", { path, trustNewKeys });
}

export async function mergeBackup(path: string, choices: GameChoice[], trustNewKeys = false): Promise<MergeSummary> {
  return invoke<MergeSummary>("merge_backup", { path, choices, trustNewKeys });
}

/** The newest cloud backup of every other PC of the account. */
export async function listCloudPcs(apiBaseUrl: string, accessToken: string, accountId: string): Promise<CloudPc[]> {
  return invoke<CloudPc[]>("list_cloud_pcs", { apiBaseUrl, accessToken, accountId });
}

/** What merging another PC's cloud backup would bring in. The backup stays downloaded until merged or discarded. */
export async function previewCloudMerge(
  apiBaseUrl: string,
  accessToken: string,
  accountId: string,
  backupId: string,
  trustNewKeys: boolean,
): Promise<MergePreview> {
  return invoke<MergePreview>("preview_cloud_merge", { apiBaseUrl, accessToken, accountId, backupId, trustNewKeys });
}

export async function mergeCloudBackup(
  apiBaseUrl: string,
  accessToken: string,
  accountId: string,
  backupId: string,
  choices: GameChoice[],
  trustNewKeys: boolean,
): Promise<MergeSummary> {
  return invoke<MergeSummary>("merge_cloud_backup", {
    apiBaseUrl,
    accessToken,
    accountId,
    backupId,
    choices,
    trustNewKeys,
  });
}

/** Lets go of a cloud backup downloaded for a merge that did not happen. */
export async function discardCloudMerge(backupId: string): Promise<void> {
  return invoke<void>("discard_cloud_merge", { backupId });
}

/** Merges the newer backups of the PCs merged here before, as far as they need no choice of the player. */
export async function mergeDueCloudBackups(apiBaseUrl: string, accessToken: string, accountId: string): Promise<AutoMerge[]> {
  return invoke<AutoMerge[]>("merge_due_cloud_backups", { apiBaseUrl, accessToken, accountId });
}

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

export async function openArtworkFile(sourcePath: string): Promise<ArtworkSource> {
  return invoke<ArtworkSource>("open_artwork_file", { sourcePath });
}

export async function openGameAssetSource(gameId: string, assetId: string): Promise<ArtworkSource> {
  return invoke<ArtworkSource>("open_game_asset_source", { gameId, assetId });
}

export async function importGameAsset(
  gameId: string,
  sourcePath: string,
  crop: CropRect,
): Promise<GameAssetView[]> {
  return invoke<GameAssetView[]>("import_game_asset", {
    gameId,
    sourcePath,
    crop,
  });
}

/** Deletes one of a game's images, never the file it came from. */
export async function deleteGameAsset(gameId: string, assetId: string): Promise<GameAssetView[]> {
  return invoke<GameAssetView[]>("delete_game_asset", { gameId, assetId });
}

export async function cropGameAsset(gameId: string, assetId: string, crop: CropRect): Promise<GameAssetView[]> {
  return invoke<GameAssetView[]>("crop_game_asset", { gameId, assetId, crop });
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

/** Every session and each game's totals. Trust labels of the last `recentDays` days count as recent. */
export async function listSessions(recentDays: number): Promise<SessionList> {
  return invoke<SessionList>("list_sessions", { recentDays });
}

/**
 * Each game's time per bucket from the local day `from` up to but not
 * including `to`, both as `toDayKey` gives them. Running sessions count up
 * to the tracker's latest tick.
 */
export async function getPlayTotals(
  from: string,
  to: string,
  bucket: PlayBucket,
  gameId?: string,
): Promise<PlayTotal[]> {
  return invoke<PlayTotal[]>("get_play_totals", { from, to, bucket, gameId: gameId ?? null });
}

/** The journal's week from the local Monday `from`, "2026-10-05", worked out by the core. */
export async function getJournalWeek(from: string): Promise<JournalWeekPlay> {
  return invoke<JournalWeekPlay>("get_journal_week", { from });
}

/** A year of play for the stats page. */
export async function getYearPlay(year: number): Promise<YearPlay> {
  return invoke<YearPlay>("get_year_play", { year });
}

/** Every day with play up to today, for the stats page's all time view. */
export async function getAllTimePlay(): Promise<YearPlay> {
  return invoke<YearPlay>("get_all_time_play");
}

/** The last `days` local days of play, today included. */
export async function getRecentPlay(days: number): Promise<RecentPlay> {
  return invoke<RecentPlay>("get_recent_play", { days });
}

/** Why a game's page should suggest stepping aside, or null. */
export async function getStepsAsideHint(gameId: string): Promise<StepsAsideHint | null> {
  return invoke<StepsAsideHint | null>("get_steps_aside_hint", { gameId });
}

/** This PC's running sessions as of the tracker's latest tick, and today's play. */
export async function getActiveSessions(): Promise<ActiveSessions> {
  return invoke<ActiveSessions>("get_active_sessions");
}

/** The events of a game's sessions without the checkpoints, newest first. */
export async function getNotableEventsForGame(gameId: string): Promise<SessionEvent[]> {
  return invoke<SessionEvent[]>("get_notable_events_for_game", { gameId });
}

/** Saves every finished session to `path` and returns how many. */
export async function exportSessions(path: string, format: ExportFormat): Promise<number> {
  return invoke<number>("export_sessions", { path, format });
}

/** Saves the year card, a PNG image as base64, to a `.png` file. */
export async function saveShareImage(path: string, pngBase64: string): Promise<void> {
  return invoke<void>("save_share_image", { path, pngBase64 });
}

/** Counts a closed session only up to `endedAt`, with a reason. */
export async function trimSession(sessionId: string, endedAt: string, reason: string): Promise<Session> {
  return invoke<Session>("trim_session", { sessionId, endedAt, reason });
}

/** Programs the player said are no game, newest first. */
export async function listIgnoredPrograms(): Promise<IgnoredProgram[]> {
  return invoke<IgnoredProgram[]>("list_ignored_programs");
}

/** Never offers this program again and never counts it. */
export async function ignoreProgram(path: string, title: string): Promise<void> {
  await invoke<void>("ignore_program", { path, title });
}

/** Lets discovery offer an ignored program again and the tracker count it. */
export async function allowProgram(pathKey: string): Promise<boolean> {
  return invoke<boolean>("allow_program", { pathKey });
}

/** Makes a game count only while no other game runs, like a launcher, or always again. */
export async function setGameStepsAside(gameId: string, stepsAside: boolean): Promise<Game> {
  return invoke<Game>("set_game_steps_aside", { gameId, stepsAside });
}

/** What a closed session would keep when it counted only up to `endedAt`. */
export async function previewTrim(sessionId: string, endedAt: string): Promise<TrimPreview> {
  return invoke<TrimPreview>("preview_trim", { sessionId, endedAt });
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
  return invoke<void>("set_cloud_signed_in", { signedIn });
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

/** Restores a backup. With `takeOver`, a fresh install continues as the PC that made it. */
export async function importLocalBackup(
  path: string,
  takeOver: boolean,
): Promise<LocalBackupSummary> {
  return invoke<LocalBackupSummary>("import_local_backup", { path, takeOver });
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
  takeOver: boolean,
): Promise<CloudBackupRestoreResult> {
  return invoke<CloudBackupRestoreResult>("restore_remote_backup", {
    apiBaseUrl,
    accessToken,
    accountId,
    backupId,
    takeOver,
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

/** The window size picked on this PC and which presets fit its screen. */
export async function getWindowSize(): Promise<WindowSizeState> {
  return invoke<WindowSizeState>("get_window_size");
}

/** Saves a window size for this PC, the window takes it right away. */
export async function setWindowSize(choice: WindowSizeChoice): Promise<WindowSizeState> {
  return invoke<WindowSizeState>("set_window_size", { choice });
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

/** The games the look at start found that are still new. */
export async function listFoundGames(): Promise<NewGames> {
  return invoke<NewGames>("list_found_games");
}

/** Remembers that discovery showed these programs, so the look at start never offers them again. */
export async function rememberOfferedGames(paths: string[]): Promise<number> {
  return invoke<number>("remember_offered_games", { paths });
}

export async function importDiscoveredGames(
  discoveries: DiscoveredGame[],
): Promise<Game[]> {
  return invoke<Game[]>("import_discovered_games", { discoveries });
}

/** The background picture as a data URL, null without one. */
export async function getBackgroundImage(): Promise<string | null> {
  return invoke<string | null>("get_background_image");
}

/** Stores a scaled down copy of a picture as the background and returns it. */
export async function setBackgroundImage(sourcePath: string): Promise<string> {
  return invoke<string>("set_background_image", { sourcePath });
}

/** Uses a game's artwork as the background and returns it. */
export async function setBackgroundFromGame(gameId: string): Promise<string> {
  return invoke<string>("set_background_from_game", { gameId });
}

export async function clearBackgroundImage(): Promise<boolean> {
  return invoke<boolean>("clear_background_image");
}

/** Colors the icons and the title bar like the page. False when nothing changed. */
export async function setWindowLook(look: WindowLook): Promise<boolean> {
  return invoke<boolean>("set_window_look", { look });
}
