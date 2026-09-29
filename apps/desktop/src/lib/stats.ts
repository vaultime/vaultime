// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The numbers of the stats page, worked out from the sessions on this PC.

import {
  DAYS_PER_WEEK,
  HOUR_MS,
  HOURS_PER_DAY,
  MONTHS_PER_YEAR,
  SESSION_LONG_MAX_MS,
  SESSION_PLAIN_MAX_MS,
  SESSION_QUICK_MAX_MS,
  SESSION_SHORT_MAX_MS,
} from "@/lib/constants";
import { countsAsPlay, playedMs, toDayKey } from "@/lib/session-stats";
import { parseVaultimeDate } from "@/lib/time";
import type { Session } from "@/lib/types";

/** Session lengths from a quick look to a marathon, shortest first. */
export const SESSION_SHAPES = ["quick", "short", "plain", "long", "marathon"] as const;
export type SessionShape = (typeof SESSION_SHAPES)[number];

export interface GameYear {
  gameId: string;
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
  sessionsCount: number;
}

export interface Streak {
  days: number;
  start: Date;
  end: Date;
}

interface YearStats {
  year: number;
  /** Time with a game running, games side by side counted once. */
  playedMs: number;
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
  sessionsCount: number;
  /** Active time per local calendar day, by `toDayKey`. */
  activeByDay: Map<string, number>;
  daysPlayed: number;
  longestStreak: Streak | null;
  /** Days in a row with play up to today, or up to yesterday while today has none yet. */
  currentStreak: number;
  /** Active time by weekday, Monday first, then by hour of the day. */
  weekClock: number[][];
  /** Active and idle time per month, January first. */
  months: { activeMs: number; idleMs: number }[];
  /** Most runtime first. */
  games: GameYear[];
  shapes: Record<SessionShape, number>;
  longest: Session | null;
}

export function shapeOf(session: Session): SessionShape {
  const ms = session.runtime_ms;
  if (ms < SESSION_QUICK_MAX_MS) return "quick";
  if (ms < SESSION_SHORT_MAX_MS) return "short";
  if (ms < SESSION_PLAIN_MAX_MS) return "plain";
  if (ms < SESSION_LONG_MAX_MS) return "long";
  return "marathon";
}

/** Monday is 0. */
export function weekdayOf(date: Date): number {
  return (date.getDay() + DAYS_PER_WEEK - 1) % DAYS_PER_WEEK;
}

function nextDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1);
}

function previousDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() - 1);
}

function endOf(session: Session, now: Date): Date {
  return session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : now;
}

/** Spreads a session's active time evenly over the hours it covered on the clock. */
function spreadOverHours(session: Session, now: Date, add: (weekday: number, hour: number, ms: number) => void) {
  const start = parseVaultimeDate(session.started_at_wall);
  const end = endOf(session, now);
  const span = end.getTime() - start.getTime();
  if (span <= 0) {
    add(weekdayOf(start), start.getHours(), session.active_ms);
    return;
  }
  let cursor = start;
  while (cursor < end) {
    let next = new Date(cursor.getFullYear(), cursor.getMonth(), cursor.getDate(), cursor.getHours() + 1);
    // Clocks turned back repeat an hour, which must not stop the walk.
    if (next <= cursor) next = new Date(cursor.getTime() + HOUR_MS);
    const stop = next < end ? next : end;
    add(weekdayOf(cursor), cursor.getHours(), (session.active_ms * (stop.getTime() - cursor.getTime())) / span);
    cursor = stop;
  }
}

/** The longest run of days in a row among `days`, the earliest when runs tie. */
function longestRun(days: Date[]): Streak | null {
  const sorted = [...days].sort((a, b) => a.getTime() - b.getTime());
  let best: Streak | null = null;
  let current: Streak | null = null;
  for (const day of sorted) {
    if (current && toDayKey(nextDay(current.end)) === toDayKey(day)) {
      current = { days: current.days + 1, start: current.start, end: day };
    } else {
      current = { days: 1, start: day, end: day };
    }
    if (!best || current.days > best.days) best = current;
  }
  return best;
}

/** Everything the stats page shows for `year`. The current streak looks at all sessions. */
export function yearStats(allSessions: Session[], year: number, now = new Date()): YearStats {
  const sessions = allSessions.filter(countsAsPlay);
  const inYear = sessions.filter((session) => parseVaultimeDate(session.started_at_wall).getFullYear() === year);
  const activeByDay = new Map<string, number>();
  const playedDays = new Map<string, Date>();
  const weekClock = Array.from({ length: DAYS_PER_WEEK }, () => Array<number>(HOURS_PER_DAY).fill(0));
  const months = Array.from({ length: MONTHS_PER_YEAR }, () => ({ activeMs: 0, idleMs: 0 }));
  const games = new Map<string, GameYear>();
  const shapes = Object.fromEntries(SESSION_SHAPES.map((shape) => [shape, 0])) as Record<SessionShape, number>;
  let longest: Session | null = null;
  let runtimeMs = 0;
  let activeMs = 0;
  let idleMs = 0;

  for (const session of inYear) {
    const started = parseVaultimeDate(session.started_at_wall);
    const key = toDayKey(started);
    activeByDay.set(key, (activeByDay.get(key) ?? 0) + session.active_ms);
    playedDays.set(key, new Date(started.getFullYear(), started.getMonth(), started.getDate()));
    spreadOverHours(session, now, (weekday, hour, ms) => {
      weekClock[weekday][hour] += ms;
    });
    months[started.getMonth()].activeMs += session.active_ms;
    months[started.getMonth()].idleMs += session.idle_ms;
    const game = games.get(session.game_id) ?? {
      gameId: session.game_id,
      runtimeMs: 0,
      activeMs: 0,
      idleMs: 0,
      sessionsCount: 0,
    };
    game.runtimeMs += session.runtime_ms;
    game.activeMs += session.active_ms;
    game.idleMs += session.idle_ms;
    game.sessionsCount += 1;
    games.set(session.game_id, game);
    shapes[shapeOf(session)] += 1;
    if (!longest || session.runtime_ms > longest.runtime_ms) longest = session;
    runtimeMs += session.runtime_ms;
    activeMs += session.active_ms;
    idleMs += session.idle_ms;
  }

  const allDays = new Set(sessions.map((session) => toDayKey(parseVaultimeDate(session.started_at_wall))));
  let day = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  if (!allDays.has(toDayKey(day))) day = previousDay(day);
  let currentStreak = 0;
  while (allDays.has(toDayKey(day))) {
    currentStreak += 1;
    day = previousDay(day);
  }

  return {
    year,
    playedMs: playedMs(inYear, now),
    runtimeMs,
    activeMs,
    idleMs,
    sessionsCount: inYear.length,
    activeByDay,
    daysPlayed: playedDays.size,
    longestStreak: longestRun([...playedDays.values()]),
    currentStreak,
    weekClock,
    months,
    games: [...games.values()].sort((a, b) => b.runtimeMs - a.runtimeMs),
    shapes,
    longest,
  };
}

/** Days of the year up to and including today, or all of them for a past year. */
export function daysSoFar(year: number, now = new Date()): number {
  const first = new Date(year, 0, 1);
  const end = year < now.getFullYear() ? new Date(year + 1, 0, 1) : nextDay(new Date(now.getFullYear(), now.getMonth(), now.getDate()));
  let days = 0;
  for (let day = first; day < end; day = nextDay(day)) days += 1;
  return days;
}
