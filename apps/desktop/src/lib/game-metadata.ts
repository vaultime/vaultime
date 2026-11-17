// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Switches the core keeps in a game's metadata.

import type { Game } from "@/lib/types";

/** Whether a switch in a game's metadata is on, off when it is missing or the metadata does not read. */
function metadataFlag(game: Game, flag: "steps_aside" | "favorite"): boolean {
  try {
    const metadata: unknown = JSON.parse(game.metadata_json || "{}");
    return typeof metadata === "object" && metadata !== null && flag in metadata && (metadata as Record<string, unknown>)[flag] === true;
  } catch {
    return false;
  }
}

/** Whether a game counts only while no other game runs, like a launcher or a game's client. The core sets its time beside other games aside. */
export function stepsAside(game: Game): boolean {
  return metadataFlag(game, "steps_aside");
}

/** Whether a game is one of the player's favorites. */
export function isFavorite(game: Game): boolean {
  return metadataFlag(game, "favorite");
}
