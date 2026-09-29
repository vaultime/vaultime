// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { RECENT_DAYS } from "@/lib/constants";
import { parseVaultimeDate, UI_LOCALE } from "@/lib/time";
import type { Session } from "@/lib/types";

export interface ActivityPoint {
  key: string;
  label: string;
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
}

export function buildDailyActivity(
  sessions: Session[],
  days: number,
): ActivityPoint[] {
  const today = new Date();
  today.setHours(0, 0, 0, 0);

  const points = new Map<string, ActivityPoint>();

  for (let offset = days - 1; offset >= 0; offset -= 1) {
    const day = new Date(today);
    day.setDate(today.getDate() - offset);
    const key = toDayKey(day);
    points.set(key, {
      key,
      label: day.toLocaleDateString(UI_LOCALE, {
        month: "short",
        day: "numeric",
      }),
      runtimeMs: 0,
      activeMs: 0,
      idleMs: 0,
    });
  }

  for (const session of sessions) {
    const key = toDayKey(parseVaultimeDate(session.started_at_wall));
    const point = points.get(key);
    if (!point) {
      continue;
    }

    point.runtimeMs += session.runtime_ms;
    point.activeMs += session.active_ms;
    point.idleMs += session.idle_ms;
  }

  return [...points.values()];
}

/** The local calendar day, "2026-09-29". */
export function toDayKey(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** A session placed on the clock. */
interface Span {
  session: Session;
  start: number;
  end: number;
  /** Runtime per millisecond on the clock. Below 1 when the PC slept in between. */
  density: number;
}

/** A stretch of clock time and the sessions that ran through all of it. */
interface Stretch {
  start: number;
  end: number;
  running: Span[];
}

function toSpans(sessions: Session[], now: Date): Span[] {
  return sessions.map((session) => {
    const start = parseVaultimeDate(session.started_at_wall).getTime();
    const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall).getTime() : now.getTime();
    return { session, start, end, density: end > start ? session.runtime_ms / (end - start) : 0 };
  });
}

function toStretches(spans: Span[]): Stretch[] {
  const timed = spans.filter((span) => span.end > span.start);
  const bounds = [...new Set(timed.flatMap((span) => [span.start, span.end]))].sort((a, b) => a - b);
  const stretches: Stretch[] = [];
  for (let index = 0; index + 1 < bounds.length; index += 1) {
    const [start, end] = [bounds[index], bounds[index + 1]];
    const running = timed
      .filter((span) => span.start <= start && span.end >= end)
      .sort((a, b) => a.start - b.start);
    if (running.length > 0) stretches.push({ start, end, running });
  }
  return stretches;
}

/**
 * Time with at least one game running, so games that run side by side count
 * once. Each session spreads its runtime evenly over its time on the clock,
 * which keeps time the PC slept out, as runtime does. A session on its own
 * adds exactly its runtime.
 */
export function playedMs(sessions: Session[], now = new Date()): number {
  const spans = toSpans(sessions, now);
  const untimed = spans
    .filter((span) => span.end <= span.start)
    .reduce((sum, span) => sum + span.session.runtime_ms, 0);
  const timed = toStretches(spans).reduce(
    (sum, stretch) => sum + (stretch.end - stretch.start) * Math.max(...stretch.running.map((span) => span.density)),
    0,
  );
  return untimed + Math.round(timed);
}

/** A stretch of clock time in which more than one game ran. */
export interface SideBySide {
  start: Date;
  end: Date;
  /** In the order the games started. */
  gameIds: string[];
}

/** The stretches in which games ran side by side, joined while the same games run. */
export function sideBySide(sessions: Session[], now = new Date()): SideBySide[] {
  const result: SideBySide[] = [];
  for (const stretch of toStretches(toSpans(sessions, now))) {
    const gameIds = [...new Set(stretch.running.map((span) => span.session.game_id))];
    if (gameIds.length < 2) continue;
    const last = result.at(-1);
    if (last && last.end.getTime() === stretch.start && last.gameIds.join() === gameIds.join()) {
      last.end = new Date(stretch.end);
    } else {
      result.push({ start: new Date(stretch.start), end: new Date(stretch.end), gameIds });
    }
  }
  return result;
}

export interface RecentPlay {
  /** Time with a game running, games side by side counted once. */
  playedMs: number;
  runtimeMs: number;
  activeMs: number;
  sessionsCount: number;
  /** Days with at least one session. */
  daysCount: number;
  runtimeByGame: Map<string, number>;
  longest: Session | null;
}

/** Totals for the last `days` calendar days, today included. */
export function summarizeRecentPlay(sessions: Session[], days = RECENT_DAYS, now = new Date()): RecentPlay {
  const cutoff = new Date(now.getFullYear(), now.getMonth(), now.getDate() - (days - 1));
  const summary: RecentPlay = {
    playedMs: 0,
    runtimeMs: 0,
    activeMs: 0,
    sessionsCount: 0,
    daysCount: 0,
    runtimeByGame: new Map(),
    longest: null,
  };
  const playedDays = new Set<string>();
  const recent = sessions.filter((session) => parseVaultimeDate(session.started_at_wall) >= cutoff);

  for (const session of recent) {
    const started = parseVaultimeDate(session.started_at_wall);
    summary.runtimeMs += session.runtime_ms;
    summary.activeMs += session.active_ms;
    summary.sessionsCount += 1;
    summary.runtimeByGame.set(
      session.game_id,
      (summary.runtimeByGame.get(session.game_id) ?? 0) + session.runtime_ms,
    );
    playedDays.add(toDayKey(started));
    if (!summary.longest || session.runtime_ms > summary.longest.runtime_ms) {
      summary.longest = session;
    }
  }

  summary.daysCount = playedDays.size;
  summary.playedMs = playedMs(recent, now);
  return summary;
}
