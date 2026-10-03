// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Games that count only while no other game runs, like a launcher or a
// game's client. The core sets their time beside other games aside and says
// how much per session. This file presents that.

import { clipToWindow, countsAsPlay, playedMs } from "@/lib/session-stats";
import { parseVaultimeDate } from "@/lib/time";
import type { Game, Session } from "@/lib/types";

/** Whether a game counts only while no other game runs. */
export function stepsAside(game: Game): boolean {
  try {
    return JSON.parse(game.metadata_json || "{}")?.steps_aside === true;
  } catch {
    return false;
  }
}

/** A session with the time set aside taken out of its runtime, active and idle time. */
export function countedSession(session: Session): Session {
  const aside = session.set_aside_ms ?? 0;
  if (aside <= 0) return session;
  return {
    ...session,
    runtime_ms: Math.max(0, session.runtime_ms - aside),
    active_ms: Math.max(0, session.active_ms - (session.set_aside_active_ms ?? 0)),
    idle_ms: Math.max(0, session.idle_ms - (session.set_aside_idle_ms ?? 0)),
  };
}

/**
 * Sessions as the day strip draws them: a session of a game that steps aside
 * keeps only the parts of its span in which no other game ran, its counted
 * time spread over them. Other sessions stay as they are.
 */
export function stepAside(sessions: Session[], stepping: Set<string>, now = new Date()): Session[] {
  if (stepping.size === 0) return sessions;
  const spanOf = (session: Session) => {
    const start = parseVaultimeDate(session.started_at_wall).getTime();
    const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall).getTime() : now.getTime();
    return { start, end };
  };
  // Where games that do not step aside ran, joined and sorted once, so each
  // session that steps aside finds its overlaps by a binary search.
  const covered: [number, number][] = [];
  const others = sessions
    .filter((session) => !stepping.has(session.game_id) && countsAsPlay(session))
    .map(spanOf)
    .filter(({ start, end }) => end > start)
    .sort((a, b) => a.start - b.start);
  for (const { start, end } of others) {
    const last = covered.at(-1);
    if (last && start <= last[1]) last[1] = Math.max(last[1], end);
    else covered.push([start, end]);
  }
  const firstEndingAfter = (moment: number) => {
    let [low, high] = [0, covered.length];
    while (low < high) {
      const middle = (low + high) >> 1;
      if (covered[middle][1] <= moment) low = middle + 1;
      else high = middle;
    }
    return low;
  };
  return sessions.flatMap((session) => {
    if (!stepping.has(session.game_id)) return [session];
    const { start, end } = spanOf(session);
    if (end <= start) return [session];
    // The parts of the span no other game covered.
    const parts: [number, number][] = [];
    let cursor = start;
    for (let index = firstEndingAfter(start); index < covered.length && covered[index][0] < end; index += 1) {
      const [from, to] = covered[index];
      if (from > cursor) parts.push([cursor, from]);
      cursor = Math.max(cursor, to);
    }
    if (cursor < end) parts.push([cursor, end]);
    const length = parts.reduce((sum, [from, to]) => sum + to - from, 0);
    if (length === 0) return [];
    return parts.map(([from, to]) => {
      const share = (to - from) / length;
      return {
        ...session,
        started_at_wall: new Date(from).toISOString(),
        ended_at_wall: new Date(to).toISOString(),
        runtime_ms: Math.round(session.runtime_ms * share),
        active_ms: Math.round(session.active_ms * share),
        idle_ms: Math.round(session.idle_ms * share),
      };
    });
  });
}

/**
 * How long other games ran beside a game, and which share of its time on the
 * clock that was. A launcher left open through matches shows up this way.
 */
export function besideOthers(
  gameId: string,
  sessions: Session[],
  stepping: Set<string>,
  now = new Date(),
): { besideMs: number; share: number } {
  const own = sessions.filter((session) => session.game_id === gameId && countsAsPlay(session));
  const ownMs = own.reduce((sum, session) => {
    const start = parseVaultimeDate(session.started_at_wall).getTime();
    const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall).getTime() : now.getTime();
    return sum + Math.max(0, end - start);
  }, 0);
  const alone = stepAside(
    [...own, ...sessions.filter((session) => session.game_id !== gameId)],
    new Set([...stepping, gameId]),
    now,
  )
    .filter((session) => session.game_id === gameId)
    .reduce(
      (sum, session) =>
        sum +
        parseVaultimeDate(session.ended_at_wall ?? session.started_at_wall).getTime() -
        parseVaultimeDate(session.started_at_wall).getTime(),
      0,
    );
  const besideMs = Math.max(0, ownMs - alone);
  return { besideMs, share: ownMs > 0 ? besideMs / ownMs : 0 };
}

/**
 * Time with a game running on the local day that holds `now`, the way the
 * journal counts a day: split at midnight, games side by side once, and a
 * game that steps aside only where it ran alone.
 */
export function playedToday(sessions: Session[], stepping: Set<string>, now = new Date()): number {
  const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  // Only sessions that can change today's part, so a long history stays out
  // of the slower steps. A session that began yesterday keeps the games that
  // ran beside it then, since they decide how its time spreads.
  const touching = sessions.filter((session) => clipToWindow(session, midnight, tomorrow, now) !== null);
  const earliest = touching.reduce(
    (first, session) => Math.min(first, parseVaultimeDate(session.started_at_wall).getTime()),
    midnight.getTime(),
  );
  const nearby = sessions.filter((session) => clipToWindow(session, new Date(earliest), tomorrow, now) !== null);
  const today = stepAside(nearby, stepping, now).flatMap(
    (session) => clipToWindow(session, midnight, tomorrow, now) ?? [],
  );
  return playedMs(today, now);
}
