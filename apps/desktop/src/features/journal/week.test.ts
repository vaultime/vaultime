// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { DAYS_PER_WEEK } from "@/lib/constants";
import { toDayKey } from "@/lib/time";
import { at } from "@/test/sessions";
import type { JournalWeekPlay, WeekStatusChange } from "@/lib/types";
import { journalDays, weekStart, weeksBack } from "./week";

// A Wednesday. The week runs from Monday 5 October.
const NOW = new Date(2026, 9, 7, 12, 0);

function change(changedAt: string, status: WeekStatusChange["status"]): WeekStatusChange {
  return { id: `change-${changedAt}`, game_id: "game-1", status, changed_at: changedAt, played_before_ms: 0 };
}

const EMPTY_WEEK: JournalWeekPlay = {
  days: [],
  shares: {},
  games: [],
  played_ms: 0,
  game_times: [],
  sessions_count: 0,
  days_played: 0,
  longest_day: null,
  carried_over: 0,
  status_changes: [],
};

describe("weekStart", () => {
  it("finds the Monday of a week and goes back a week at a time", () => {
    expect(weekStart(0, NOW)).toEqual(new Date(2026, 9, 5));
    expect(weekStart(-1, NOW)).toEqual(new Date(2026, 8, 28));
  });
});

describe("journalDays", () => {
  it("keeps the core's days, newest first, and puts status changes on their day", () => {
    const week: JournalWeekPlay = {
      ...EMPTY_WEEK,
      days: [
        { day: "2026-10-06", played_ms: 1, session_ids: [], parts: [], runs: [], side_by_side: [] },
        { day: "2026-10-05", played_ms: 2, session_ids: ["late"], parts: [], runs: [], side_by_side: [] },
      ],
      status_changes: [change(at(2026, 10, 6, 20, 0), "finished"), change(at(2026, 10, 7, 9, 0), "playing")],
    };
    const days = journalDays(week);
    expect(days.map((day) => toDayKey(day.start))).toEqual(["2026-10-07", "2026-10-06", "2026-10-05"]);
    expect(days[0].changes.map((each) => each.status)).toEqual(["playing"]);
    expect(days[0].parts).toEqual([]);
    expect(days[1].changes.map((each) => each.status)).toEqual(["finished"]);
    expect(days[2].sessionIds).toEqual(["late"]);
    expect(days[2].start).toEqual(new Date(2026, 9, 5));
  });
});

describe("weeksBack", () => {
  it("is 0 for this week, no day or a day it cannot read", () => {
    expect(weeksBack(null)).toBe(0);
    expect(weeksBack("soon")).toBe(0);
    expect(weeksBack(toDayKey(new Date()))).toBe(0);
  });

  it("counts whole weeks back and never forward", () => {
    const today = new Date();
    const weeksAway = (weeks: number) =>
      toDayKey(new Date(today.getFullYear(), today.getMonth(), today.getDate() + weeks * DAYS_PER_WEEK));
    expect(weeksBack(weeksAway(-1))).toBe(-1);
    expect(weeksBack(weeksAway(-3))).toBe(-3);
    expect(weeksBack(weeksAway(1))).toBe(0);
  });
});
