// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { GameStatus } from "@/lib/types";

/** The order statuses appear in, from a game waiting to one that is done. */
export const GAME_STATUSES: GameStatus[] = ["backlog", "playing", "finished", "dropped"];

export const GAME_STATUS_LABELS: Record<GameStatus, string> = {
  backlog: "Backlog",
  playing: "Playing",
  finished: "Finished",
  dropped: "Dropped",
};
