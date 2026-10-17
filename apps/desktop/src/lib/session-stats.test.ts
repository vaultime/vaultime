// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { MINUTE_MS } from "@/lib/constants";
import { at, session } from "@/test/sessions";
import {
  clipToWindow,
  countedOnce,
  daysTouched,
  playedByGame,
  playedMs,
  playRuns,
  sideBySide,
  sideBySideGroups,
  splitByDay,
  summarizeRecentPlay,
  toDayKey,
  touchesWindow,
} from "./session-stats";

describe("clipToWindow", () => {
  it("keeps the part of a session inside the window, its time spread evenly", () => {
    // 23:00 to 02:00 with an hour of it idle.
    const late = session(at(2026, 9, 28, 23, 0), 180, { active_ms: 120 * MINUTE_MS, idle_ms: 60 * MINUTE_MS });
    const nextDay = clipToWindow(late, new Date(2026, 8, 29), new Date(2026, 8, 30));
    expect(nextDay?.started_at_wall).toBe(new Date(2026, 8, 29).toISOString());
    expect(nextDay?.runtime_ms).toBe(120 * MINUTE_MS);
    expect(nextDay?.active_ms).toBe(80 * MINUTE_MS);
    expect(nextDay?.idle_ms).toBe(40 * MINUTE_MS);
    expect(clipToWindow(late, new Date(2026, 8, 30), new Date(2026, 9, 1))).toBeNull();
  });

  it("runs a live session up to now", () => {
    const live = session(at(2026, 9, 29, 20, 0), 60, { ended_at_wall: null });
    const clipped = clipToWindow(live, new Date(2026, 8, 29), new Date(2026, 8, 30), new Date(2026, 8, 29, 21, 0));
    expect(clipped?.ended_at_wall).toBe(new Date(2026, 8, 29, 21, 0).toISOString());
  });
});

describe("touchesWindow", () => {
  it("agrees with clipToWindow, also for a session without time on the clock", () => {
    const late = session(at(2026, 9, 28, 23, 0), 180);
    const empty = session(at(2026, 9, 29, 12, 0), 0);
    const windows: [Date, Date][] = [
      [new Date(2026, 8, 29), new Date(2026, 8, 30)],
      [new Date(2026, 8, 30), new Date(2026, 9, 1)],
      [new Date(2026, 8, 29, 2), new Date(2026, 8, 29, 12)],
      [new Date(2026, 8, 29, 12), new Date(2026, 8, 29, 13)],
    ];
    for (const [from, to] of windows) {
      for (const one of [late, empty]) expect(touchesWindow(one, from, to)).toBe(clipToWindow(one, from, to) !== null);
    }
    expect(windows.map(([from, to]) => touchesWindow(empty, from, to))).toEqual([true, false, false, true]);
  });
});

describe("daysTouched", () => {
  it("lists every local day a session ran on", () => {
    expect(daysTouched(session(at(2026, 9, 28, 23, 0), 180))).toEqual(["2026-09-28", "2026-09-29"]);
    expect(daysTouched(session(at(2026, 9, 28, 20, 0), 60))).toEqual(["2026-09-28"]);
  });

  it("stops at a session that ends at midnight", () => {
    expect(daysTouched(session(at(2026, 9, 28, 23, 0), 60))).toEqual(["2026-09-28"]);
  });

  it("counts the night the clocks go back as one day", () => {
    // 25 October 2026 has 25 hours in Europe.
    expect(daysTouched(session(at(2026, 10, 25, 0, 30), 120))).toEqual(["2026-10-25"]);
  });
});

describe("splitByDay", () => {
  it("cuts a session at midnight, each day with its own part", () => {
    // Saturday 22:40 to Sunday 02:15.
    const late = session(at(2026, 10, 17, 22, 40), 215);
    const [saturday, sunday, ...rest] = splitByDay(late);
    expect(rest).toEqual([]);
    expect(saturday.day).toEqual(new Date(2026, 9, 17));
    expect(saturday.start).toEqual(new Date(2026, 9, 17, 22, 40));
    expect(saturday.end).toEqual(new Date(2026, 9, 18));
    expect(saturday.runtimeMs).toBe(80 * MINUTE_MS);
    expect([saturday.fromBefore, saturday.intoNext]).toEqual([false, true]);
    expect(sunday.day).toEqual(new Date(2026, 9, 18));
    expect(sunday.end).toEqual(new Date(2026, 9, 18, 2, 15));
    expect(sunday.runtimeMs).toBe(135 * MINUTE_MS);
    expect([sunday.fromBefore, sunday.intoNext]).toEqual([true, false]);
  });

  it("carries a session from Sunday into the Monday of the next week", () => {
    const shares = splitByDay(session(at(2026, 10, 11, 22, 40), 190));
    expect(shares.map((share) => toDayKey(share.day))).toEqual(["2026-10-11", "2026-10-12"]);
    expect(shares[1].runtimeMs).toBe(110 * MINUTE_MS);
  });

  it("gives a session over more than a day a whole middle day", () => {
    // Wednesday 19:00 to Friday 01:00.
    const shares = splitByDay(session(at(2026, 10, 14, 19, 0), 30 * 60));
    expect(shares.map((share) => share.runtimeMs / MINUTE_MS)).toEqual([5 * 60, 24 * 60, 60]);
    expect(shares[1].start).toEqual(new Date(2026, 9, 15));
    expect(shares[1].end).toEqual(new Date(2026, 9, 16));
    expect([shares[1].fromBefore, shares[1].intoNext]).toEqual([true, true]);
  });

  it("keeps a session that ends at midnight on one day", () => {
    const shares = splitByDay(session(at(2026, 10, 17, 23, 0), 60));
    expect(shares).toHaveLength(1);
    expect(shares[0].intoNext).toBe(false);
  });

  it("runs a live session up to now", () => {
    const live = session(at(2026, 10, 17, 23, 0), 0, { ended_at_wall: null, runtime_ms: 90 * MINUTE_MS });
    const shares = splitByDay(live, [live], new Date(2026, 9, 18, 0, 30));
    expect(shares.map((share) => share.runtimeMs)).toEqual([60 * MINUTE_MS, 30 * MINUTE_MS]);
  });

  it("counts only the parts a game that steps aside is drawn as", () => {
    // A launcher from 22:00 to 02:00, set aside after midnight while a game ran.
    const launcher = session(at(2026, 10, 17, 22, 0), 240);
    const alone = { ...launcher, ended_at_wall: at(2026, 10, 18, 0, 0), runtime_ms: 120 * MINUTE_MS };
    const shares = splitByDay(launcher, [alone]);
    expect(shares).toHaveLength(1);
    expect(shares[0].runtimeMs).toBe(120 * MINUTE_MS);
    expect(shares[0].intoNext).toBe(false);
  });

  it("splits at local midnight on the night the clocks go back", () => {
    // 24 to 25 October 2026, a 25 hour day in Europe.
    const start = new Date(2026, 9, 24, 22, 0);
    const midnight = new Date(2026, 9, 25);
    const late = session(start.toISOString(), 5 * 60);
    const [first, second] = splitByDay(late);
    const firstMs = midnight.getTime() - start.getTime();
    expect(first.runtimeMs).toBe(firstMs);
    expect(second.start).toEqual(midnight);
    expect(first.runtimeMs + second.runtimeMs).toBe(late.runtime_ms);
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

  it("counts two installs of one game running at once once, as the core shares them", () => {
    // Steam and GOG copies from 18:00 and 18:30, an hour each, the core
    // splitting the half hour both ran between them.
    const sessions = [
      session(at(2026, 9, 29, 18, 0), 60, { id: "steam", copy_overlap_ms: 15 * MINUTE_MS, copy_overlap_active_ms: 15 * MINUTE_MS }),
      session(at(2026, 9, 29, 18, 30), 60, {
        id: "gog",
        recorded_game_id: "gog-copy",
        copy_overlap_ms: 15 * MINUTE_MS,
        copy_overlap_active_ms: 15 * MINUTE_MS,
      }),
    ];
    const recent = summarizeRecentPlay(sessions, 7, now);
    expect(recent.runtimeMs).toBe(90 * MINUTE_MS);
    expect(recent.activeMs).toBe(90 * MINUTE_MS);
    expect(recent.runtimeByGame.get("game-1")).toBe(90 * MINUTE_MS);
    expect(recent.playedMs).toBe(90 * MINUTE_MS);
  });
});

describe("countedOnce", () => {
  it("takes out what another copy of the game ran through as well", () => {
    const shared = session(at(2026, 9, 29, 18, 0), 60, {
      idle_ms: 20 * MINUTE_MS,
      active_ms: 40 * MINUTE_MS,
      copy_overlap_ms: 30 * MINUTE_MS,
      copy_overlap_active_ms: 20 * MINUTE_MS,
      copy_overlap_idle_ms: 10 * MINUTE_MS,
    });
    expect(countedOnce(shared)).toEqual({ runtimeMs: 30 * MINUTE_MS, activeMs: 20 * MINUTE_MS, idleMs: 10 * MINUTE_MS });
    expect(countedOnce(session(at(2026, 9, 29, 18, 0), 60)).runtimeMs).toBe(60 * MINUTE_MS);
  });
});

describe("playedByGame", () => {
  const now = new Date(2026, 8, 29, 23, 0);

  it("counts a game's copies at the same time once and games beside it in full", () => {
    const sessions = [
      session(at(2026, 9, 29, 18, 0), 60, { id: "steam" }),
      session(at(2026, 9, 29, 18, 30), 60, { id: "gog", recorded_game_id: "gog-copy" }),
      session(at(2026, 9, 29, 18, 0), 90, { game_id: "b" }),
      session(at(2026, 9, 29, 20, 0), 30, { game_id: "c", runtime_ms: 0 }),
    ];
    expect(playedByGame(sessions, now)).toEqual(
      new Map([
        ["game-1", 90 * MINUTE_MS],
        ["b", 90 * MINUTE_MS],
      ]),
    );
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

  it("draws two installs of one game running at once as one game, not side by side", () => {
    const sessions = [
      session(at(2026, 9, 29, 18, 0), 60, { id: "steam" }),
      session(at(2026, 9, 29, 18, 30), 60, { id: "gog", recorded_game_id: "gog-copy" }),
    ];
    const runs = playRuns(sessions, now);
    expect(runs).toHaveLength(1);
    expect(runs[0].pieces.map((piece) => piece.gameIds)).toEqual([["game-1"]]);
    expect(sideBySide(sessions, now)).toEqual([]);
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
