// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";
import type { GameTint } from "@/lib/game-tint";
import type { Device, EarlierPlaytime, Game, GameLink, GameStatus, GameStatusChange, Session } from "@/lib/types";

export interface GameSummary {
  game: Game;
  /** Other titles of the game, those of installs and games of other PCs that count as it, for search. */
  otherTitles: string[];
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
  /** Suspicious sessions of the last `TRUST_BADGE_RECENT_DAYS` days. */
  recentSuspiciousCount: number;
  /** Recovered sessions of the last `TRUST_BADGE_RECENT_DAYS` days. */
  recentRecoveredCount: number;
  lastPlayedAt: string | null;
}

interface LibraryState {
  games: Game[];
  sessions: Session[];
  covers: Record<string, string>;
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
  /** Every PC whose sessions are here, this one included. */
  devices: Device[];
  /** This PC's id, null until it is known. */
  thisDeviceId: string | null;
  /** Games of other PCs that count as games of this PC. */
  links: GameLink[];
  /** The local day at the last poll, so pages that say today or yesterday turn over at midnight. */
  today: string;
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

// The live session has contexts of its own, so a poll renders only what shows it.
export const ActiveContext = createContext<Session[]>([]);
export const PlayingContext = createContext<ReadonlySet<string>>(new Set());
export const LastPollContext = createContext(0);

/** This PC's running sessions as of the last poll. The same list until they change. */
export function useActiveSessions(): Session[] {
  return useContext(ActiveContext);
}

/** Ids of the games running now. The same set while the same games run. */
export function usePlaying(): ReadonlySet<string> {
  return useContext(PlayingContext);
}

/** When the running sessions were last polled, for ticking the live timer between polls. */
export function useLastPoll(): number {
  return useContext(LastPollContext);
}
