// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The numbers of the stats page, from the core's play totals and the
// sessions on this PC.

import {
  DAYS_PER_WEEK,
  HOURS_PER_DAY,
  MONTHS_PER_YEAR,
  SESSION_LONG_MAX_MS,
  SESSION_PLAIN_MAX_MS,
  SESSION_QUICK_MAX_MS,
  SESSION_SHORT_MAX_MS,
} from "@/lib/constants";
import { groupByBucket, sumByGame, type BucketPlay, type GamePlay } from "@/lib/play-totals";
import { clipToWindow, countsAsPlay, daysTouched, playedMs, toDayKey } from "@/lib/session-stats";
import { parseVaultimeDate } from "@/lib/time";
import type { PlayTotal, Session } from "@/lib/types";

/** Session lengths from a quick look to a marathon, shortest first. */
export const SESSION_SHAPES = ["quick", "short", "plain", "long", "marathon"] as const;
export type SessionShape = (typeof SESSION_SHAPES)[number];

export interface GameYear extends GamePlay {
  /** Sessions that started in the year. */
  sessionsCount: number;
}

/** The core's play totals for a year, per day and game and per hour of the week and game. */
export interface YearPlay {
  days: PlayTotal[];
  hours: PlayTotal[];
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
  /** What was played on each local day, by `toDayKey`. Time counts on the day it happened. */
  days: Map<string, BucketPlay>;
  daysPlayed: number;
  longestStreak: Streak | null;
  /** Days in a row with play up to today, or up to yesterday while today has none yet. */
  currentStreak: number;
  /** Active time by weekday, Monday first, then by hour of the day. */
  weekClock: number[][];
  /** What was played in each month, January first. */
  months: BucketPlay[];
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

/**
 * Everything the stats page shows for `year`. Time comes from the core's
 * totals and counts on the day it happened. Sessions count in the year they
 * started. The current streak looks at all sessions.
 */
export function yearStats(allSessions: Session[], year: number, play: YearPlay, now = new Date()): YearStats {
  const sessions = allSessions.filter(countsAsPlay);
  const inYear = sessions.filter((session) => parseVaultimeDate(session.started_at_wall).getFullYear() === year);
  const days = groupByBucket(play.days);
  const monthsByKey = groupByBucket(play.days.map((total) => ({ ...total, bucket: total.bucket.slice(0, 7) })));
  const months = Array.from({ length: MONTHS_PER_YEAR }, (_, index) => {
    const key = `${year}-${String(index + 1).padStart(2, "0")}`;
    return monthsByKey.get(key) ?? { runtimeMs: 0, activeMs: 0, idleMs: 0, games: [] };
  });
  const weekClock = Array.from({ length: DAYS_PER_WEEK }, () => Array<number>(HOURS_PER_DAY).fill(0));
  for (const total of play.hours) {
    const [weekday, hour] = total.bucket.split("-").map(Number);
    if (weekClock[weekday]?.[hour] !== undefined) weekClock[weekday][hour] += total.active_ms;
  }

  const sessionsByGame = new Map<string, number>();
  const shapes = Object.fromEntries(SESSION_SHAPES.map((shape) => [shape, 0])) as Record<SessionShape, number>;
  let longest: Session | null = null;
  for (const session of inYear) {
    sessionsByGame.set(session.game_id, (sessionsByGame.get(session.game_id) ?? 0) + 1);
    shapes[shapeOf(session)] += 1;
    if (!longest || session.runtime_ms > longest.runtime_ms) longest = session;
  }
  const games = sumByGame(play.days).map((game) => ({ ...game, sessionsCount: sessionsByGame.get(game.gameId) ?? 0 }));

  const playedDays = [...days.entries()]
    .filter(([, day]) => day.runtimeMs > 0)
    .map(([key]) => {
      const [y, m, d] = key.split("-").map(Number);
      return new Date(y, m - 1, d);
    });

  const allDays = new Set(sessions.flatMap((session) => daysTouched(session, now)));
  let day = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  if (!allDays.has(toDayKey(day))) day = previousDay(day);
  let currentStreak = 0;
  while (allDays.has(toDayKey(day))) {
    currentStreak += 1;
    day = previousDay(day);
  }

  const [yearStart, yearEnd] = [new Date(year, 0, 1), new Date(year + 1, 0, 1)];
  const clipped = sessions.flatMap((session) => clipToWindow(session, yearStart, yearEnd, now) ?? []);
  const sum = (pick: (day: BucketPlay) => number) => [...days.values()].reduce((total, day) => total + pick(day), 0);

  return {
    year,
    playedMs: playedMs(clipped, now),
    runtimeMs: sum((day) => day.runtimeMs),
    activeMs: sum((day) => day.activeMs),
    idleMs: sum((day) => day.idleMs),
    sessionsCount: inYear.length,
    days,
    daysPlayed: playedDays.length,
    longestStreak: longestRun(playedDays),
    currentStreak,
    weekClock,
    months,
    games,
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
