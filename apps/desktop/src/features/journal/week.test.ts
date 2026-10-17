// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { DAYS_PER_WEEK, HOUR_MS } from "@/lib/constants";
import { toDayKey } from "@/lib/session-stats";
import { at, session } from "@/test/sessions";
import type { GameStatusChange } from "@/lib/types";
import { buildWeek, weeksBack } from "./week";

// A Wednesday. The week runs from Monday 5 October.
const NOW = new Date(2026, 9, 7, 12, 0);

function change(changedAt: string, status: GameStatusChange["status"]): GameStatusChange {
  return { id: `change-${changedAt}`, game_id: "game-1", status, changed_at: changedAt };
}

describe("buildWeek", () => {
  it("lists days newest first and counts play past midnight on the day it happened", () => {
    const late = session(at(2026, 10, 5, 23, 0), 120, { id: "late" });
    const week = buildWeek([late], [], new Set(), 0, NOW);
    expect(week.start).toEqual(new Date(2026, 9, 5));
    expect(week.days.map((day) => toDayKey(day.start))).toEqual(["2026-10-06", "2026-10-05"]);
    const [tuesday, monday] = week.days;
    expect(monday.sessions.map((each) => each.id)).toEqual(["late"]);
    expect(tuesday.sessions).toEqual([]);
    expect(monday.playedMs).toBe(HOUR_MS);
    expect(tuesday.playedMs).toBe(HOUR_MS);
    expect(week.sharesOf(late).map((share) => share.runtimeMs)).toEqual([HOUR_MS, HOUR_MS]);
  });

  it("counts a session from the week before as carried over", () => {
    const sunday = session(at(2026, 10, 4, 23, 0), 120, { id: "sunday" });
    const week = buildWeek([sunday], [], new Set(), 0, NOW);
    expect(week.carriedOver).toBe(1);
    expect(week.days).toHaveLength(1);
    expect(week.days[0].sessions).toEqual([]);
    expect(week.days[0].spans.map((span) => span.id)).toEqual(["sunday"]);
  });

  it("puts status changes on their day and leaves cleared ones out", () => {
    const changes = [change(at(2026, 10, 6, 20, 0), "finished"), change(at(2026, 10, 7, 9, 0), "none")];
    const week = buildWeek([], changes, new Set(), 0, NOW);
    expect(week.days).toHaveLength(1);
    expect(week.days[0].changes.map((each) => each.status)).toEqual(["finished"]);
  });

  it("names the games in the order they first played", () => {
    const sessions = [
      session(at(2026, 10, 6, 18, 0), 60, { id: "b", game_id: "game-b" }),
      session(at(2026, 10, 5, 18, 0), 60, { id: "a", game_id: "game-a" }),
      session(at(2026, 10, 6, 20, 0), 60, { id: "a2", game_id: "game-a" }),
    ];
    expect(buildWeek(sessions, [], new Set(), 0, NOW).games).toEqual(["game-a", "game-b"]);
  });

  it("goes back a week at a time", () => {
    const earlier = session(at(2026, 9, 29, 18, 0), 60);
    const week = buildWeek([earlier], [], new Set(), -1, NOW);
    expect(week.start).toEqual(new Date(2026, 8, 28));
    expect(week.days.map((day) => toDayKey(day.start))).toEqual(["2026-09-29"]);
    expect(buildWeek([earlier], [], new Set(), 0, NOW).days).toEqual([]);
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
