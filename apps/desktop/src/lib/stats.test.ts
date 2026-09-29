// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { MINUTE_MS } from "@/lib/constants";
import { at, session } from "@/test/sessions";
import { daysSoFar, shapeOf, weekdayOf, yearStats } from "./stats";

const now = new Date(2026, 8, 30, 21, 0);

describe("yearStats", () => {
  it("counts only sessions that started in the year", () => {
    const stats = yearStats(
      [
        session(at(2026, 3, 2, 20, 0), 60, { game_id: "a" }),
        session(at(2025, 12, 31, 23, 30), 90, { game_id: "b" }),
        session(at(2026, 3, 2, 20, 30), 60, { game_id: "b" }),
      ],
      2026,
      now,
    );
    expect(stats.sessionsCount).toBe(2);
    expect(stats.runtimeMs).toBe(120 * MINUTE_MS);
    expect(stats.playedMs).toBe(90 * MINUTE_MS);
    expect(stats.daysPlayed).toBe(1);
    expect(stats.months[2].activeMs).toBe(120 * MINUTE_MS);
    expect(stats.games.map((game) => game.gameId)).toEqual(["a", "b"]);
  });

  it("spreads active time over the hours on the clock", () => {
    // Monday 2 March, 20:30 to 22:30.
    const stats = yearStats([session(at(2026, 3, 2, 20, 30), 120)], 2026, now);
    const monday = stats.weekClock[0];
    expect(monday[20]).toBe(30 * MINUTE_MS);
    expect(monday[21]).toBe(60 * MINUTE_MS);
    expect(monday[22]).toBe(30 * MINUTE_MS);
    expect(stats.weekClock.flat().reduce((sum, ms) => sum + ms, 0)).toBe(120 * MINUTE_MS);
  });

  it("carries play past midnight into the next weekday", () => {
    // Sunday 1 March, 23:30 to Monday 00:30.
    const stats = yearStats([session(at(2026, 3, 1, 23, 30), 60)], 2026, now);
    expect(stats.weekClock[6][23]).toBe(30 * MINUTE_MS);
    expect(stats.weekClock[0][0]).toBe(30 * MINUTE_MS);
  });

  it("finds the longest streak and the current one", () => {
    const days = [
      [3, 1],
      [3, 2],
      [3, 3],
      [3, 5],
      [9, 28],
      [9, 29],
    ];
    const sessions = days.map(([month, day]) => session(at(2026, month, day, 20, 0), 30));
    const stats = yearStats(sessions, 2026, now);
    expect(stats.longestStreak?.days).toBe(3);
    expect(stats.longestStreak?.start).toEqual(new Date(2026, 2, 1));
    expect(stats.longestStreak?.end).toEqual(new Date(2026, 2, 3));
    // Nothing played today yet, so the streak up to yesterday counts.
    expect(stats.currentStreak).toBe(2);
  });

  it("keeps a streak across the end of March when the clocks change", () => {
    const sessions = [28, 29, 30].map((day) => session(at(2026, 3, day, 20, 0), 30));
    expect(yearStats(sessions, 2026, now).longestStreak?.days).toBe(3);
  });

  it("sorts sessions into lengths and finds the longest", () => {
    const sessions = [10, 45, 90, 200, 300].map((minutes, index) => session(at(2026, 4, 1 + index, 20, 0), minutes));
    const stats = yearStats(sessions, 2026, now);
    expect(stats.shapes).toEqual({ quick: 1, short: 1, plain: 1, long: 1, marathon: 1 });
    expect(stats.longest?.runtime_ms).toBe(300 * MINUTE_MS);
  });
});

describe("helpers", () => {
  it("counts weekdays from Monday", () => {
    expect(weekdayOf(new Date(2026, 8, 28))).toBe(0);
    expect(weekdayOf(new Date(2026, 8, 27))).toBe(6);
  });

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
