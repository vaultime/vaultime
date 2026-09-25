// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { invoke } from "@tauri-apps/api/core";
import type {
  Game,
  CreateGameInput,
  UpdateGameInput,
  Session,
  SessionEvent,
  Setting,
  TrackingDiagnostics,
  GameAssetView,
} from "@/lib/types";

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
