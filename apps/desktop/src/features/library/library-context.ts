// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { createContext, useContext } from "react";
import type { GameTint } from "@/lib/game-tint";
import type { Game, Session } from "@/lib/types";

export interface GameSummary {
  game: Game;
  cover: string | null;
  /** From the cover art when there is some, otherwise from the title. */
  tint: GameTint;
  runtimeMs: number;
  activeMs: number;
  sessionsCount: number;
  suspiciousCount: number;
  recoveredCount: number;
  lastPlayedAt: string | null;
}

export interface LibraryState {
  games: Game[];
  sessions: Session[];
  covers: Record<string, string>;
  active: Session[];
  /** When `active` was last polled, for ticking the live timer between polls. */
  activePolledAt: number;
  idleThresholdSeconds: number;
  /** Every game with its totals, most recently played first. */
  summaries: GameSummary[];
  loaded: boolean;
  /** Why the last load failed, null when it worked. */
  error: string | null;
  refresh: () => Promise<void>;
}

export const LibraryContext = createContext<LibraryState | null>(null);

export function useLibrary(): LibraryState {
  const value = useContext(LibraryContext);
  if (!value) throw new Error("useLibrary needs a LibraryProvider");
  return value;
}
