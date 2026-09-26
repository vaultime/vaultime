// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MINUTE_MS } from "@/lib/constants";
import { at, session } from "@/test/sessions";
import { buildDailyActivity, summarizeRecentPlay } from "./session-stats";

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
});
