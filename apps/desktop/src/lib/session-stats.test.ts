// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MINUTE_MS } from "@/lib/constants";
import { at, session } from "@/test/sessions";
import { buildDailyActivity, playedMs, sideBySide, summarizeRecentPlay } from "./session-stats";

describe("buildDailyActivity", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 8, 29, 21, 30));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("has one point per day, oldest first", () => {
    const points = buildDailyActivity([], 14);
    expect(points).toHaveLength(14);
    expect(points[0].key).toBe("2026-09-16");
    expect(points[13].key).toBe("2026-09-29");
  });

  it("counts late evening play on the local day", () => {
    // 22:30 in Los Angeles is already the next day in UTC.
    const evening = session(at(2026, 9, 28, 22, 30), 60);
    const points = buildDailyActivity([evening], 14);
    expect(points.find((point) => point.key === "2026-09-28")?.runtimeMs).toBe(60 * MINUTE_MS);
    expect(points.find((point) => point.key === "2026-09-29")?.runtimeMs).toBe(0);
  });

  it("counts play after midnight on the local day", () => {
    // 00:30 in Berlin is still the previous day in UTC.
    const lateNight = session(at(2026, 9, 29, 0, 30), 45, { active_ms: 40 * MINUTE_MS, idle_ms: 5 * MINUTE_MS });
    const points = buildDailyActivity([lateNight], 14);
    const today = points.find((point) => point.key === "2026-09-29");
    expect(today?.activeMs).toBe(40 * MINUTE_MS);
    expect(today?.idleMs).toBe(5 * MINUTE_MS);
    expect(points.find((point) => point.key === "2026-09-28")?.runtimeMs).toBe(0);
  });
});

describe("buildDailyActivity around daylight saving", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("has every calendar day once", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 10, 5, 12, 0));
    const keys = buildDailyActivity([], 60).map((point) => point.key);
    expect(new Set(keys).size).toBe(60);
    expect(keys[0]).toBe("2026-09-07");
    expect(keys.at(-1)).toBe("2026-11-05");
    expect(keys).toContain("2026-09-27");
    expect(keys).toContain("2026-10-25");
    expect(keys).toContain("2026-11-01");
  });
});

describe("summarizeRecentPlay", () => {
  const now = new Date(2026, 8, 29, 21, 30);

  it("counts the last seven calendar days", () => {
    const sessions = [
      session(at(2026, 9, 29, 20, 0), 60, { game_id: "a" }),
      session(at(2026, 9, 26, 18, 0), 240, { game_id: "b" }),
      session(at(2026, 9, 23, 0, 30), 30, { game_id: "a" }),
      session(at(2026, 9, 22, 23, 0), 90, { game_id: "c" }),
    ];
    const recent = summarizeRecentPlay(sessions, 7, now);
    expect(recent.sessionsCount).toBe(3);
    expect(recent.daysCount).toBe(3);
    expect(recent.runtimeMs).toBe(330 * MINUTE_MS);
    expect(recent.runtimeByGame.get("a")).toBe(90 * MINUTE_MS);
    expect(recent.runtimeByGame.has("c")).toBe(false);
    expect(recent.longest?.game_id).toBe("b");
  });

  it("counts games that ran side by side once", () => {
    const sessions = [
      session(at(2026, 9, 29, 16, 0), 250, { game_id: "a" }),
      session(at(2026, 9, 29, 18, 30), 190, { game_id: "b" }),
    ];
    const recent = summarizeRecentPlay(sessions, 7, now);
    expect(recent.runtimeMs).toBe(440 * MINUTE_MS);
    expect(recent.playedMs).toBe(340 * MINUTE_MS);
  });
});

describe("playedMs", () => {
  const now = new Date(2026, 8, 29, 23, 0);

  it("adds sessions that do not overlap", () => {
    const sessions = [session(at(2026, 9, 29, 10, 0), 60), session(at(2026, 9, 29, 12, 0), 30, { game_id: "b" })];
    expect(playedMs(sessions, now)).toBe(90 * MINUTE_MS);
  });

  it("counts a game inside another once", () => {
    const sessions = [session(at(2026, 9, 29, 10, 0), 120), session(at(2026, 9, 29, 10, 30), 30, { game_id: "b" })];
    expect(playedMs(sessions, now)).toBe(120 * MINUTE_MS);
  });

  it("leaves out a sleep that both games slept through", () => {
    // Two hours on the clock, one of them asleep, so each ran an hour.
    const sleepy = (game: string) =>
      session(at(2026, 9, 29, 10, 0), 60, {
        game_id: game,
        ended_at_wall: new Date(2026, 8, 29, 12, 0).toISOString(),
      });
    expect(playedMs([sleepy("a"), sleepy("b")], now)).toBe(60 * MINUTE_MS);
  });

  it("gives a single session exactly its runtime", () => {
    const odd = session(at(2026, 9, 29, 10, 0), 61, {
      runtime_ms: 61 * MINUTE_MS - 7,
      ended_at_wall: new Date(2026, 8, 29, 11, 13).toISOString(),
    });
    expect(playedMs([odd], now)).toBe(61 * MINUTE_MS - 7);
  });

  it("runs a live session up to now", () => {
    const live = session(at(2026, 9, 29, 22, 0), 60, { ended_at_wall: null });
    const other = session(at(2026, 9, 29, 22, 30), 30, { game_id: "b" });
    expect(playedMs([live, other], now)).toBe(60 * MINUTE_MS);
  });
});

describe("sideBySide", () => {
  const now = new Date(2026, 8, 29, 23, 0);

  it("finds the stretch two games shared, in start order", () => {
    const sessions = [
      session(at(2026, 9, 29, 18, 30), 190, { game_id: "b" }),
      session(at(2026, 9, 29, 16, 0), 250, { game_id: "a" }),
    ];
    const [shared, ...rest] = sideBySide(sessions, now);
    expect(rest).toHaveLength(0);
    expect(shared.gameIds).toEqual(["a", "b"]);
    expect(shared.start).toEqual(new Date(2026, 8, 29, 18, 30));
    expect(shared.end).toEqual(new Date(2026, 8, 29, 20, 10));
  });

  it("joins a stretch while the same games run and splits when a third joins", () => {
    const sessions = [
      session(at(2026, 9, 29, 10, 0), 120, { game_id: "a" }),
      session(at(2026, 9, 29, 10, 30), 60, { game_id: "b" }),
      session(at(2026, 9, 29, 11, 0), 15, { game_id: "c" }),
    ];
    expect(sideBySide(sessions, now).map((stretch) => stretch.gameIds.join(""))).toEqual(["ab", "abc", "ab"]);
  });

  it("finds nothing when games take turns", () => {
    const sessions = [session(at(2026, 9, 29, 10, 0), 60), session(at(2026, 9, 29, 11, 0), 60, { game_id: "b" })];
    expect(sideBySide(sessions, now)).toEqual([]);
  });
});
