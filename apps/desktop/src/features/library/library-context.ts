// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext, useMemo, useSyncExternalStore } from "react";
import type { GameTint } from "@/lib/game-tint";
import { stepsAside } from "@/lib/game-metadata";
import type { Device, EarlierPlaytime, Game, GameLink, GameStatus, GameStatusChange, Session } from "@/lib/types";

export interface GameSummary {
  game: Game;
  /** Other titles of the game, those of installs and games of other PCs that count as it, for search. */
  otherTitles: string[];
  /** One of the player's favorites. */
  favorite: boolean;
  /** Where the game and the games that count as it came from (`lib/launchers.ts`). */
  launchers: string[];
  /** Tracked runtime by the launcher of the copy that recorded it. */
  launcherMs: Record<string, number>;
  /** The thumbnail of its cover, for tiles and lists. */
  cover: string | null;
  /** Its cover at full size, for the game page and the art behind the window. */
  art: string | null;
  /** From the cover art when there is some, otherwise from the title. */
  tint: GameTint;
  /** Tracked by Vaultime. */
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
  /** The runtime of the longest session. */
  longestSessionMs: number;
  firstPlayedAt: string | null;
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
  /** When the newest play ended, now while the game runs here. */
  lastPlayedAt: string | null;
}

interface LibraryState {
  games: Game[];
  sessions: Session[];
  /** Sessions whose chain is not checked yet, as after a start. Their trust label waits for it. */
  unchecked: ReadonlySet<string>;
  /** Cover thumbnails by game. */
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
  /** Whether the colors of the covers are read. Until then every game has the color of its title. */
  artRead: boolean;
  /** Why the last load failed, null when it worked. */
  error: string | null;
  refresh: () => Promise<void>;
  setStatus: (gameId: string, status: GameStatus | "none") => Promise<void>;
  setFavorite: (gameId: string, favorite: boolean) => Promise<void>;
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
/** The running sessions as they stand, kept while their times move. */
export const RunningContext = createContext<Session[]>([]);
export const PlayingContext = createContext<ReadonlySet<string>>(new Set());
export const CountedAtContext = createContext<Record<string, number>>({});
export const MinuteContext = createContext(0);
export const PlayedTodayContext = createContext(0);

/** This PC's running sessions with their times as of the last poll, a new list with each poll while one runs. */
export function useActiveSessions(): Session[] {
  return useContext(ActiveContext);
}

/**
 * This PC's running sessions, the same list until one starts, ends or gets
 * another trust label. Their times are those of when that happened, for
 * pages that only ask what runs.
 */
export function useRunningSessions(): Session[] {
  return useContext(RunningContext);
}

/**
 * The running sessions of the last poll by id, which each line of a session
 * asks for its own, so a poll draws only the lines of running sessions again.
 */
export class RunningStore {
  private sessions = new Map<string, Session>();
  private readonly listeners = new Set<() => void>();

  set(next: Session[]) {
    this.sessions = new Map(next.map((session) => [session.id, session]));
    for (const listener of this.listeners) listener();
  }

  get(id: string): Session | null {
    return this.sessions.get(id) ?? null;
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
}

export const RunningStoreContext = createContext(new RunningStore());

/** A session with its times as of the last poll while it runs, null once it ended or for one that never ran. */
export function useRunningSession(id: string): Session | null {
  const store = useContext(RunningStoreContext);
  return useSyncExternalStore(store.subscribe, () => store.get(id));
}

/** Ids of the games running now. The same set while the same games run. */
export function usePlaying(): ReadonlySet<string> {
  return useContext(PlayingContext);
}

/** When the tracker counted each running session's times, by session id, for ticking its timer on from there. */
export function useCountedAt(): Record<string, number> {
  return useContext(CountedAtContext);
}

/** The minute of the last poll, so words like "5 min ago" move on. */
export function useMinute(): number {
  return useContext(MinuteContext);
}

/**
 * The session of the game being played rather than a launcher that steps
 * aside for it, whose clock stands still while the game runs.
 */
export function useCurrentSession(): Session | null {
  return useCurrentOf(useActiveSessions());
}

/** The game of `useCurrentSession`, the same while it runs. */
export function useTrackedGameId(): string | null {
  return useCurrentOf(useRunningSessions())?.game_id ?? null;
}

function useCurrentOf(sessions: Session[]): Session | null {
  const { games } = useLibrary();
  const stepping = useMemo(() => new Set(games.filter(stepsAside).map((game) => game.id)), [games]);
  return sessions.find((session) => !stepping.has(session.game_id)) ?? sessions[0] ?? null;
}

/** Clock time with a game running today, as the core counted it at the last poll. */
export function usePlayedToday(): number {
  return useContext(PlayedTodayContext);
}
