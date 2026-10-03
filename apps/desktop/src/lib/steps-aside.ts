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
  // Where games that do not step aside ran on each PC, joined and sorted
  // once, so each session that steps aside finds its overlaps by a binary
  // search. A launcher steps aside only for games on its own PC.
  const coveredByPc = new Map<string, [number, number][]>();
  const others = sessions
    .filter((session) => !stepping.has(session.game_id) && countsAsPlay(session))
    .map((session) => ({ device: session.device_id, ...spanOf(session) }))
    .filter(({ start, end }) => end > start)
    .sort((a, b) => a.start - b.start);
  for (const { device, start, end } of others) {
    const covered = coveredByPc.get(device) ?? [];
    coveredByPc.set(device, covered);
    const last = covered.at(-1);
    if (last && start <= last[1]) last[1] = Math.max(last[1], end);
    else covered.push([start, end]);
  }
  const firstEndingAfter = (covered: [number, number][], moment: number) => {
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
    // The parts of the span no other game on the same PC covered.
    const covered = coveredByPc.get(session.device_id) ?? [];
    const parts: [number, number][] = [];
    let cursor = start;
    for (let index = firstEndingAfter(covered, start); index < covered.length && covered[index][0] < end; index += 1) {
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
 * How long other games ran beside a game, which share of its time on the
 * clock that was, and which share of that came from games started while it
 * was already open. A launcher left open through matches shows up with both
 * high, while a game played inside its client starts after the client.
 */
export function besideOthers(
  gameId: string,
  sessions: Session[],
  stepping: Set<string>,
  now = new Date(),
): { besideMs: number; share: number; openedFirstShare: number } {
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

  // Time beside games that started during one of its sessions, each session
  // looking only at those, found by a binary search over their starts.
  const spanOf = (session: Session) => ({
    start: parseVaultimeDate(session.started_at_wall).getTime(),
    end: session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall).getTime() : now.getTime(),
  });
  const laterByPc = new Map<string, { start: number; end: number }[]>();
  for (const session of sessions) {
    if (session.game_id === gameId || stepping.has(session.game_id) || !countsAsPlay(session)) continue;
    const list = laterByPc.get(session.device_id) ?? [];
    laterByPc.set(session.device_id, list);
    list.push(spanOf(session));
  }
  for (const list of laterByPc.values()) list.sort((a, b) => a.start - b.start);
  let openedFirstMs = 0;
  for (const session of own) {
    const { start, end } = spanOf(session);
    const later = laterByPc.get(session.device_id) ?? [];
    let [low, high] = [0, later.length];
    while (low < high) {
      const middle = (low + high) >> 1;
      if (later[middle].start < start) low = middle + 1;
      else high = middle;
    }
    let cursor = start;
    for (let index = low; index < later.length && later[index].start < end; index += 1) {
      const to = Math.min(later[index].end, end);
      openedFirstMs += Math.max(0, to - Math.max(cursor, later[index].start));
      cursor = Math.max(cursor, to);
    }
  }
  return {
    besideMs,
    share: ownMs > 0 ? besideMs / ownMs : 0,
    openedFirstShare: besideMs > 0 ? Math.min(1, openedFirstMs / besideMs) : 0,
  };
}

/**
 * `stepAside` for the sessions with time between `from` and `to`, leaving a
 * long history out of the work. A session that began earlier keeps the games
 * that ran beside it then, since they decide how its time spreads, so the
 * parts come out as they would from the whole history. Parts outside the
 * window may come along.
 */
export function stepAsideWithin(
  sessions: Session[],
  stepping: Set<string>,
  from: Date,
  to: Date,
  now = new Date(),
): Session[] {
  const touching = sessions.filter((session) => clipToWindow(session, from, to, now) !== null);
  const earliest = touching.reduce(
    (first, session) => Math.min(first, parseVaultimeDate(session.started_at_wall).getTime()),
    from.getTime(),
  );
  const nearby = sessions.filter((session) => clipToWindow(session, new Date(earliest), to, now) !== null);
  return stepAside(nearby, stepping, now);
}

/**
 * Time with a game running on the local day that holds `now`, the way the
 * journal counts a day: split at midnight, games side by side once, and a
 * game that steps aside only where it ran alone.
 */
export function playedToday(sessions: Session[], stepping: Set<string>, now = new Date()): number {
  const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  const today = stepAsideWithin(sessions, stepping, midnight, tomorrow, now).flatMap(
    (session) => clipToWindow(session, midnight, tomorrow, now) ?? [],
  );
  return playedMs(today, now);
}
