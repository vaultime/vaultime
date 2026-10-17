// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS } from "@/lib/constants";
import { phraseString } from "@/test/phrases";
import {
  busiestMonthSentence,
  rhythmSentence,
  shapesSentence,
  shareSentence,
  streakSentence,
  yearSentence,
} from "./year-sentences";

describe("stats sentences", () => {
  const year = { daysPlayed: 90, topMs: 61 * HOUR_MS, gamesCount: 7, year: 2026, current: true };

  it("sums up a year and names the game that led", () => {
    expect(yearSentence({ ...year, playedMs: 212 * HOUR_MS, topTitle: "Elden Ring" })).toEqual({
      before: "Two hundred twelve hours on ninety days. ",
      em: "Elden Ring",
      after: " led with sixty-one hours.",
    });
    expect(yearSentence({ ...year, playedMs: 0, topTitle: null }).before).toBe("Nothing played this year yet.");
    expect(yearSentence({ ...year, playedMs: 0, topTitle: null, current: false }).before).toBe("Nothing played in 2026.");
  });

  it("writes streaks within and across months", () => {
    const march = { days: 12, start: new Date(2026, 2, 3), end: new Date(2026, 2, 14) };
    expect(streakSentence(march, 4, true)).toBe(
      "Your longest streak was twelve days, 3 to 14 March. Right now you are on four days in a row.",
    );
    const turn = { days: 4, start: new Date(2026, 1, 27), end: new Date(2026, 2, 2) };
    expect(streakSentence(turn, 4, false)).toBe("Your longest streak was four days, 27 February to 2 March.");
    expect(streakSentence({ days: 1, start: new Date(2026, 0, 1), end: new Date(2026, 0, 1) }, 1, true)).toBe(
      "No two days in a row yet.",
    );
  });

  it("compares weekdays and weekends per day", () => {
    const clock = Array.from({ length: 7 }, () => Array<number>(24).fill(0));
    // Five weekday evenings of an hour against two weekend afternoons of two hours.
    for (let day = 0; day < 5; day += 1) clock[day][20] = HOUR_MS;
    clock[5][14] = 2 * HOUR_MS;
    clock[6][14] = 2 * HOUR_MS;
    expect(rhythmSentence(clock)?.em).toBe("weekend afternoons");
    expect(rhythmSentence(clock.map((hours) => hours.map(() => 0)))).toBeNull();
  });

  it("sums up a year for the card, the same way each time", () => {
    const clock = Array.from({ length: 7 }, () => Array<number>(24).fill(0));
    clock[5][14] = 2 * HOUR_MS;
    const base = { topTitle: "Elden Ring", gamesCount: 4, daysPlayed: 90, weekClock: clock };
    const lines = new Set([2024, 2025, 2026, 2027, 2028, 2029].map((year) => phraseString(shareSentence({ ...base, year }))));
    expect(lines.size).toBeGreaterThan(1);
    for (const line of lines) expect(line).toMatch(/Elden Ring/);
    for (const line of lines) expect(line).toMatch(/weekend afternoons/i);
    expect(shareSentence({ ...base, year: 2026 })).toEqual(shareSentence({ ...base, year: 2026 }));
    expect(phraseString(shareSentence({ ...base, gamesCount: 1, year: 2026 }))).toBe(
      "A year of weekend afternoons, all of it in Elden Ring.",
    );
    // Without titles the time of the week leads.
    const untitled = phraseString(shareSentence({ ...base, topTitle: null, year: 2026 }));
    expect(untitled).not.toContain("Elden Ring");
    expect(untitled).toMatch(/four games/i);
    expect(phraseString(shareSentence({ ...base, topTitle: null, daysPlayed: 3, year: 2026 }))).toBe(
      "A quiet year of three days across four games.",
    );
  });

  it("names the busiest month and the most common session", () => {
    const months = Array.from({ length: 12 }, (_, index) => ({ activeMs: index === 9 ? 31 * HOUR_MS : HOUR_MS, idleMs: 0 }));
    expect(busiestMonthSentence(months, 2026)).toBe("October was your busiest month, thirty-one hours.");
    expect(shapesSentence({ quick: 2, short: 3, plain: 9, long: 4, marathon: 1 })).toBe(
      "Your most common session was one to two hours.",
    );
    expect(shapesSentence({ quick: 0, short: 0, plain: 0, long: 0, marathon: 0 })).toBeNull();
    expect(busiestMonthSentence(months.map(() => ({ activeMs: 0, idleMs: 0 })), 2026)).toBeNull();
  });
});
