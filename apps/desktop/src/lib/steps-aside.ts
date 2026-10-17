// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Games that count only while no other game runs, like a launcher or a
// game's client. The core sets their time beside other games aside.

import type { Game } from "@/lib/types";

/** Whether a game counts only while no other game runs. */
export function stepsAside(game: Game): boolean {
  try {
    const metadata: unknown = JSON.parse(game.metadata_json || "{}");
    return typeof metadata === "object" && metadata !== null && "steps_aside" in metadata && metadata.steps_aside === true;
  } catch {
    return false;
  }
}
