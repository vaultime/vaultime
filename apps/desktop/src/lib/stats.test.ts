// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { MINUTE_MS } from "@/lib/constants";
import { estimatePlayTotals } from "@/test/play-totals";
import { at, session } from "@/test/sessions";
import type { Session, YearPlay } from "@/lib/types";
import { daysSoFar, shapeOf, yearStats } from "./stats";

const now = new Date(2026, 8, 30, 21, 0);

/** The stats of `year` with the totals the core would give for `sessions`. */
function statsFor(sessions: Session[], year = 2026, play: Partial<YearPlay> = {}) {
  const [from, to] = [`${year}-01-01`, `${year + 1}-01-01`];
  return yearStats(sessions, year, {
    days: estimatePlayTotals(sessions, from, to, "day", null, now),
    hours: estimatePlayTotals(sessions, from, to, "hour_of_week", null, now),
    played_ms: 0,
    days_played: 0,
    longest_streak: null,
    current_streak: 0,
    ...play,
  });
}

describe("yearStats", () => {
  it("counts time in the year it happened and sessions in the year they started", () => {
    const stats = statsFor([
      session(at(2026, 3, 2, 20, 0), 60, { game_id: "a" }),
      // 23:30 to 01:00: an hour of it falls into the new year.
      session(at(2025, 12, 31, 23, 30), 90, { game_id: "b" }),
      session(at(2026, 3, 2, 20, 30), 60, { game_id: "b" }),
    ]);
    expect(stats.sessionsCount).toBe(2);
    expect(stats.runtimeMs).toBe(180 * MINUTE_MS);
    expect(stats.months[0].activeMs).toBe(60 * MINUTE_MS);
    expect(stats.months[2].activeMs).toBe(120 * MINUTE_MS);
    expect(stats.months[2].games.map((game) => game.gameId)).toEqual(["a", "b"]);
    expect(stats.games.map((game) => game.gameId)).toEqual(["b", "a"]);
    expect(stats.games.map((game) => game.sessionsCount)).toEqual([1, 1]);
  });

  it("spreads active time over the hours on the clock", () => {
    // Monday 2 March, 20:30 to 22:30.
    const stats = statsFor([session(at(2026, 3, 2, 20, 30), 120)]);
    const monday = stats.weekClock[0];
    expect(monday[20]).toBe(30 * MINUTE_MS);
    expect(monday[21]).toBe(60 * MINUTE_MS);
    expect(monday[22]).toBe(30 * MINUTE_MS);
    expect(stats.weekClock.flat().reduce((sum, ms) => sum + ms, 0)).toBe(120 * MINUTE_MS);
  });

  it("carries play past midnight into the next weekday", () => {
    // Sunday 1 March, 23:30 to Monday 00:30.
    const stats = statsFor([session(at(2026, 3, 1, 23, 30), 60)]);
    expect(stats.weekClock[6][23]).toBe(30 * MINUTE_MS);
    expect(stats.weekClock[0][0]).toBe(30 * MINUTE_MS);
  });

  it("takes time played and streaks from the core", () => {
    const stats = statsFor([], 2026, {
      played_ms: 150 * MINUTE_MS,
      days_played: 6,
      longest_streak: { days: 3, start: "2026-03-01", end: "2026-03-03" },
      current_streak: 2,
    });
    expect(stats.playedMs).toBe(150 * MINUTE_MS);
    expect(stats.daysPlayed).toBe(6);
    expect(stats.longestStreak?.days).toBe(3);
    expect(stats.longestStreak?.start).toEqual(new Date(2026, 2, 1));
    expect(stats.longestStreak?.end).toEqual(new Date(2026, 2, 3));
    expect(stats.currentStreak).toBe(2);
  });

  it("sorts sessions into lengths and finds the longest", () => {
    const sessions = [10, 45, 90, 200, 300].map((minutes, index) => session(at(2026, 4, 1 + index, 20, 0), minutes));
    const stats = statsFor(sessions);
    expect(stats.shapes).toEqual({ quick: 1, short: 1, plain: 1, long: 1, marathon: 1 });
    expect(stats.longest?.runtime_ms).toBe(300 * MINUTE_MS);
  });
});

describe("helpers", () => {
  it("names session lengths at their limits", () => {
    expect(shapeOf(session(at(2026, 1, 1), 19))).toBe("quick");
    expect(shapeOf(session(at(2026, 1, 1), 20))).toBe("short");
    expect(shapeOf(session(at(2026, 1, 1), 240))).toBe("marathon");
  });

  it("counts the days of a year so far", () => {
    expect(daysSoFar(2025, now)).toBe(365);
    expect(daysSoFar(2024, now)).toBe(366);
    expect(daysSoFar(2026, now)).toBe(273);
  });
});
