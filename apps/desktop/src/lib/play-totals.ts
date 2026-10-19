// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Play totals from the core, grouped for charts and hover cards.

import { HOURS_PER_DAY } from "@/lib/constants";
import type { PlayTotal } from "@/lib/types";

/** A game's time in one bucket. */
export interface GamePlay {
  gameId: string;
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
}

/** Everything played in one bucket, the most active game first. */
export interface BucketPlay {
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
  games: GamePlay[];
}

/** The totals by bucket, each with its games. */
export function groupByBucket(totals: PlayTotal[]): Map<string, BucketPlay> {
  const buckets = new Map<string, BucketPlay>();
  for (const total of totals) {
    const bucket = buckets.get(total.bucket) ?? { runtimeMs: 0, activeMs: 0, idleMs: 0, games: [] };
    bucket.runtimeMs += total.runtime_ms;
    bucket.activeMs += total.active_ms;
    bucket.idleMs += total.idle_ms;
    bucket.games.push({
      gameId: total.game_id,
      runtimeMs: total.runtime_ms,
      activeMs: total.active_ms,
      idleMs: total.idle_ms,
    });
    buckets.set(total.bucket, bucket);
  }
  for (const bucket of buckets.values()) {
    bucket.games.sort((a, b) => b.activeMs - a.activeMs || b.runtimeMs - a.runtimeMs);
  }
  return buckets;
}

/** Runtime by hour of the day, from hour of the week totals ("4-21"), every weekday added up. */
export function runtimeByHour(totals: PlayTotal[]): number[] {
  const hours = Array<number>(HOURS_PER_DAY).fill(0);
  for (const total of totals) {
    const hour = Number(total.bucket.split("-")[1]);
    if (hours[hour] !== undefined) hours[hour] += total.runtime_ms;
  }
  return hours;
}

/** Each game's time over all the totals, the most runtime first. */
export function sumByGame(totals: PlayTotal[]): GamePlay[] {
  const games = new Map<string, GamePlay>();
  for (const total of totals) {
    const game = games.get(total.game_id) ?? { gameId: total.game_id, runtimeMs: 0, activeMs: 0, idleMs: 0 };
    game.runtimeMs += total.runtime_ms;
    game.activeMs += total.active_ms;
    game.idleMs += total.idle_ms;
    games.set(total.game_id, game);
  }
  return [...games.values()].sort((a, b) => b.runtimeMs - a.runtimeMs);
}
