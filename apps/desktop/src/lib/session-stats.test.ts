// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MINUTE_MS } from "@/lib/constants";
import { at, session } from "@/test/sessions";
import {
  buildDailyActivity,
  playedMs,
  playRuns,
  sideBySide,
  sideBySideGroups,
  summarizeRecentPlay,
} from "./session-stats";

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

  it("does not let a session with its time taken out run side by side", () => {
    const played = session(at(2026, 9, 29, 10, 0), 60);
    const discarded = session(at(2026, 9, 29, 10, 0), 60, { game_id: "b", runtime_ms: 0, active_ms: 0 });
    expect(playedMs([played, discarded], now)).toBe(60 * MINUTE_MS);
    expect(sideBySide([played, discarded], now)).toEqual([]);
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

  it("keeps one stretch when a game starts again while the other runs", () => {
    const sessions = [
      session(at(2026, 9, 29, 10, 0), 120, { game_id: "a" }),
      session(at(2026, 9, 29, 10, 30), 120, { game_id: "b" }),
      session(at(2026, 9, 29, 12, 0), 60, { game_id: "a" }),
    ];
    const shared = sideBySide(sessions, now);
    expect(shared.map((stretch) => stretch.gameIds.join(""))).toEqual(["ab"]);
    expect(shared[0].end).toEqual(new Date(2026, 8, 29, 12, 30));
  });
});

describe("playRuns", () => {
  const now = new Date(2026, 8, 29, 23, 0);

  it("puts a launcher and the matches inside it into one run", () => {
    const sessions = [
      session(at(2026, 9, 29, 18, 57), 129, { game_id: "client" }),
      session(at(2026, 9, 29, 19, 1), 35, { game_id: "match", id: "match-1" }),
      session(at(2026, 9, 29, 19, 37), 41, { game_id: "match", id: "match-2" }),
      session(at(2026, 9, 29, 20, 26), 39, { game_id: "match", id: "match-3" }),
    ];
    const runs = playRuns(sessions, now);
    expect(runs).toHaveLength(1);
    expect(runs[0].start).toEqual(new Date(2026, 8, 29, 18, 57));
    expect(runs[0].end).toEqual(new Date(2026, 8, 29, 21, 6));
    expect(runs[0].pieces.map((piece) => piece.gameIds.join("+"))).toEqual([
      "client",
      "client+match",
      "client",
      "client+match",
      "client",
      "client+match",
      "client",
    ]);
    // The client's pieces carry the whole session, so its fill spans all of it.
    expect(runs[0].pieces[2].sessionsStart).toEqual(new Date(2026, 8, 29, 18, 57));
    expect(runs[0].pieces[2].sessionsEnd).toEqual(new Date(2026, 8, 29, 21, 6));
  });

  it("keeps sessions that only touch apart", () => {
    const sessions = [session(at(2026, 9, 29, 10, 0), 60), session(at(2026, 9, 29, 11, 0), 60, { game_id: "b" })];
    expect(playRuns(sessions, now).map((run) => run.pieces.map((piece) => piece.gameIds.join("")))).toEqual([["game-1"], ["b"]]);
  });

  it("gives a session without time on the clock a piece", () => {
    const instant = session(at(2026, 9, 29, 10, 0), 0, { runtime_ms: 1 });
    expect(playRuns([instant], now)[0].pieces).toHaveLength(1);
  });
});

describe("sideBySideGroups", () => {
  const now = new Date(2026, 8, 29, 23, 0);
  const groupsOf = (sessions: ReturnType<typeof session>[]) => sideBySideGroups(sideBySide(sessions, now));

  it("keeps two pairs that never met apart", () => {
    const groups = groupsOf([
      session(at(2026, 9, 29, 13, 0), 120, { game_id: "a" }),
      session(at(2026, 9, 29, 14, 0), 120, { game_id: "b" }),
      session(at(2026, 9, 29, 20, 0), 180, { game_id: "c" }),
      session(at(2026, 9, 29, 21, 0), 60, { game_id: "d" }),
    ]);
    expect(groups.map((group) => group.gameIds)).toEqual([
      ["a", "b"],
      ["c", "d"],
    ]);
    expect(groups.map((group) => group.ms)).toEqual([60 * MINUTE_MS, 60 * MINUTE_MS]);
  });

  it("names the game that ran through every stretch", () => {
    const [group] = groupsOf([
      session(at(2026, 9, 29, 9, 0), 600, { game_id: "a" }),
      session(at(2026, 9, 29, 11, 0), 150, { game_id: "b" }),
      session(at(2026, 9, 29, 13, 0), 120, { game_id: "c" }),
      session(at(2026, 9, 29, 17, 0), 60, { game_id: "d" }),
    ]);
    expect(group.gameIds).toEqual(["a", "b", "c", "d"]);
    expect(group.alongside).toBe("a");
    expect(group.mostAtOnce).toBe(3);
    expect(group.ms).toBe((150 + 90 + 60) * MINUTE_MS);
  });

  it("joins games that met through another game, with none in every stretch", () => {
    const [group, ...rest] = groupsOf([
      session(at(2026, 9, 29, 10, 0), 60, { game_id: "a" }),
      session(at(2026, 9, 29, 10, 30), 60, { game_id: "b" }),
      session(at(2026, 9, 29, 11, 15), 60, { game_id: "c" }),
      session(at(2026, 9, 29, 12, 0), 60, { game_id: "d" }),
    ]);
    expect(rest).toHaveLength(0);
    expect(group.gameIds).toEqual(["a", "b", "c", "d"]);
    expect(group.alongside).toBeNull();
    expect(group.mostAtOnce).toBe(2);
  });
});
