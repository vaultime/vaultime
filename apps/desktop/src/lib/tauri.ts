// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { invoke } from "@tauri-apps/api/core";
import type { Game, CreateGameInput, UpdateGameInput } from "@/lib/types";

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
