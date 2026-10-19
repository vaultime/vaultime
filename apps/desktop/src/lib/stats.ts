// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The numbers of the stats page, from the core's year of play.

import {
  DAYS_PER_WEEK,
  HEATMAP_STEPS_MS,
  HOURS_PER_DAY,
  MONTHS_PER_YEAR,
  SESSION_LONG_MAX_MS,
  SESSION_PLAIN_MAX_MS,
  SESSION_QUICK_MAX_MS,
  SESSION_SHORT_MAX_MS,
} from "@/lib/constants";
import { groupByBucket, sumByGame, type BucketPlay, type GamePlay } from "@/lib/play-totals";
import { calendarDaysAgo, parseVaultimeDate, startOfDay } from "@/lib/time";
import type { Session, YearPlay } from "@/lib/types";

/** Session lengths from a quick look to a marathon, shortest first, as the core sorts them. */
export const SESSION_SHAPES = ["quick", "short", "plain", "long", "marathon"] as const;
export type SessionShape = (typeof SESSION_SHAPES)[number];

export interface GameYear extends GamePlay {
  /** Sessions that started in the year. */
  sessionsCount: number;
}

export interface Streak {
  days: number;
  start: Date;
  end: Date;
}

export interface YearStats {
  year: number;
  /** Time with a game running, games side by side counted once. */
  playedMs: number;
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
  sessionsCount: number;
  /** What was played on each local day, by `toDayKey`. Time counts on the day it happened. */
  days: Map<string, BucketPlay>;
  /** Time with a game running on each local day, games side by side counted once, as the journal counts its days. */
  played: Map<string, number>;
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

/** How long one session reads, for its line in the journal. The stats page gets its counts from the core. */
export function shapeOf(session: Session): SessionShape {
  const ms = session.runtime_ms;
  if (ms < SESSION_QUICK_MAX_MS) return "quick";
  if (ms < SESSION_SHORT_MAX_MS) return "short";
  if (ms < SESSION_PLAIN_MAX_MS) return "plain";
  if (ms < SESSION_LONG_MAX_MS) return "long";
  return "marathon";
}

/** How bright a day of the year calendar is for its time played, 0 for none up to one past the last step. */
export function heatLevel(playedMs: number): number {
  if (playedMs <= 0) return 0;
  return 1 + HEATMAP_STEPS_MS.filter((step) => playedMs >= step).length;
}

/**
 * Everything the stats page shows for `year`, from the core's year of play.
 * Time and streaks count on the day they happened, sessions in the year they
 * started. `sessions` holds the longest one to show.
 */
export function yearStats(sessions: Session[], year: number, play: YearPlay): YearStats {
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

  const shapes = Object.fromEntries(
    SESSION_SHAPES.map((shape, index) => [shape, play.session_lengths[index] ?? 0]),
  ) as Record<SessionShape, number>;
  const games = sumByGame(play.days).map((game) => ({
    ...game,
    sessionsCount: play.sessions_by_game[game.gameId] ?? 0,
  }));

  const sum = (pick: (day: BucketPlay) => number) => [...days.values()].reduce((total, day) => total + pick(day), 0);
  const streak = play.longest_streak;

  return {
    year,
    playedMs: play.played_ms,
    runtimeMs: sum((day) => day.runtimeMs),
    activeMs: sum((day) => day.activeMs),
    idleMs: sum((day) => day.idleMs),
    sessionsCount: play.sessions_count,
    days,
    played: new Map(Object.entries(play.played)),
    daysPlayed: play.days_played,
    longestStreak: streak
      ? { days: streak.days, start: parseVaultimeDate(streak.start), end: parseVaultimeDate(streak.end) }
      : null,
    currentStreak: play.current_streak,
    weekClock,
    months,
    games,
    shapes,
    longest: sessions.find((session) => session.id === play.longest_session_id) ?? null,
  };
}

/** Days of the year up to and including today, or all of them for a past year. */
export function daysSoFar(year: number, now = new Date()): number {
  const end = year < now.getFullYear() ? new Date(year + 1, 0, 1) : startOfDay(now, 1);
  return Math.max(0, calendarDaysAgo(new Date(year, 0, 1), end));
}
