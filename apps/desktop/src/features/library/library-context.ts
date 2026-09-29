// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";
import type { GameTint } from "@/lib/game-tint";
import type { EarlierPlaytime, Game, GameStatus, GameStatusChange, Session } from "@/lib/types";

export interface GameSummary {
  game: Game;
  cover: string | null;
  /** From the cover art when there is some, otherwise from the title. */
  tint: GameTint;
  /** Tracked by Vaultime. */
  runtimeMs: number;
  activeMs: number;
  /** Playtime from before Vaultime, when it was imported. */
  earlier: EarlierPlaytime | null;
  /** Tracked and earlier playtime together. */
  totalMs: number;
  status: GameStatus | null;
  /** When the current status was set. */
  statusSince: string | null;
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
  /** The games the library shows, without hidden ones. Hidden games are still
   * tracked, and their sessions count in the journal and the totals. */
  visible: GameSummary[];
  /** Every status change, oldest first. */
  statusChanges: GameStatusChange[];
  /** Session notes by session id. */
  notes: Record<string, string>;
  loaded: boolean;
  /** Why the last load failed, null when it worked. */
  error: string | null;
  refresh: () => Promise<void>;
  setStatus: (gameId: string, status: GameStatus | "none") => Promise<void>;
  /** An empty note removes it. */
  saveNote: (sessionId: string, note: string) => Promise<void>;
}

export const LibraryContext = createContext<LibraryState | null>(null);

export function useLibrary(): LibraryState {
  const value = useContext(LibraryContext);
  if (!value) throw new Error("useLibrary needs a LibraryProvider");
  return value;
}
