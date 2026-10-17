// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS, MINUTE_MS, SHARE_FORMATS, SHARE_TOP_GAMES } from "@/lib/constants";
import { phraseString } from "@/lib/sentences";
import { yearStats } from "@/lib/stats";
import { estimatePlayTotals } from "@/test/play-totals";
import { at, session } from "@/test/sessions";
import type { Session } from "@/lib/types";
import {
  buildShareCard,
  calendarGrid,
  gamesFor,
  previewScale,
  SHARE_FORMAT_IDS,
  shareFileName,
  stackBlocks,
  type ShareGameInfo,
  type ShareOptions,
} from "./share-card";

const now = new Date(2026, 8, 30, 21, 0);

const GAMES: Record<string, ShareGameInfo> = {
  a: { title: "Elden Ring", hidden: false, cover: "data:image/png;base64,a", colors: [{ hue: 80, chroma: 0.08 }] },
  b: { title: "Hades II", hidden: false, cover: null, colors: [{ hue: 150, chroma: 0.1 }] },
  c: { title: "Balatro", hidden: false, cover: "data:image/png;base64,c", colors: [] },
  d: { title: "Secret Game", hidden: true, cover: "data:image/png;base64,d", colors: [{ hue: 10, chroma: 0.1 }] },
  e: { title: "Celeste", hidden: false, cover: null, colors: [{ hue: 330, chroma: 0.1 }] },
  f: { title: "Outer Wilds", hidden: false, cover: null, colors: [{ hue: 40, chroma: 0.1 }] },
  g: { title: "Stardew Valley", hidden: false, cover: null, colors: [{ hue: 110, chroma: 0.1 }] },
};

/** Evenings of play: each game gets `hours` of sessions on days from 1 March on. */
function evenings(plan: [gameId: string, hours: number][]): Session[] {
  const sessions: Session[] = [];
  let day = 1;
  for (const [gameId, hours] of plan) {
    for (let hour = 0; hour < hours; hour += 1) {
      sessions.push(session(at(2026, 3, day, 20, 0), 60, { id: `${gameId}-${day}`, game_id: gameId }));
      day += 1;
    }
  }
  return sessions;
}

function cardFor(sessions: Session[], options: Partial<ShareOptions> = {}, pcName: string | null = "Desktop") {
  const [from, to] = ["2026-01-01", "2027-01-01"];
  const play = {
    days: estimatePlayTotals(sessions, from, to, "day", null, now),
    hours: estimatePlayTotals(sessions, from, to, "hour_of_week", null, now),
  };
  return buildShareCard({
    stats: yearStats(sessions, 2026, play, now),
    gameInfo: (gameId) => GAMES[gameId],
    pcName,
    accentHues: [293],
    options: { showGames: true, showPcName: true, mode: "dark", ...options },
    now,
  });
}

const year = evenings([
  ["d", 9],
  ["a", 8],
  ["b", 6],
  ["c", 5],
  ["e", 4],
  ["f", 3],
  ["g", 2],
]);

describe("buildShareCard", () => {
  it("shows nothing for a year without play", () => {
    expect(cardFor([])).toBeNull();
    // A session whose time was all taken out is no play.
    expect(cardFor([session(at(2026, 3, 1, 20), 60, { runtime_ms: 0, active_ms: 0 })])).toBeNull();
  });

  it("names the most active games and never a hidden one", () => {
    const card = cardFor(year)!;
    expect(card.games.map((game) => game.title)).toEqual(["Elden Ring", "Hades II", "Balatro", "Celeste", "Outer Wilds"]);
    expect(card.games[0].share).toBeCloseTo(8 / 37);
    // The hidden game still counts in the totals, as everywhere.
    expect(card.activeMs).toBe(37 * HOUR_MS);
    expect(card.playedMs).toBe(37 * HOUR_MS);
    expect(card.gamesCount).toBe(7);
    expect(card.topShare).toBeCloseTo(9 / 37);
    expect(JSON.stringify(card)).not.toContain("Secret Game");
    for (const format of SHARE_FORMAT_IDS) {
      expect(gamesFor(card, format)).toHaveLength(SHARE_TOP_GAMES[format]);
    }
  });

  it("gives each game its mark and a hue for a typeset cover", () => {
    const card = cardFor(year)!;
    const [elden, hades, balatro] = card.games;
    expect(elden.marks.length).toBeGreaterThan(0);
    expect(hades.hue).toBe(150);
    // Grey art keeps its cover and gets no hue.
    expect(balatro.hue).toBeNull();
    expect(balatro.cover).toBe(GAMES.c.cover);
  });

  it("names the top game in its sentence only when it is shown", () => {
    // The hidden game played most, so no game may be said to have led.
    expect(cardFor(year)!.sentence.em).not.toBe("Elden Ring");
    const visibleTop = cardFor(year.filter((entry) => entry.game_id !== "d"))!;
    expect(phraseString(visibleTop.sentence)).toContain("Elden Ring");
  });

  it("leaves every title and cover off when games are private", () => {
    const sessions = year.filter((entry) => entry.game_id !== "d");
    const card = cardFor(sessions, { showGames: false })!;
    expect(card.games.every((game) => game.title === null && game.cover === null)).toBe(true);
    expect(card.longestSession?.title).toBeNull();
    for (const info of Object.values(GAMES)) expect(phraseString(card.sentence)).not.toContain(info.title);
    // The split of the time still shows.
    expect(card.games[0].share).toBeGreaterThan(0);
  });

  it("names the PC only when asked to", () => {
    expect(cardFor(year)!.pcName).toBe("Desktop");
    expect(cardFor(year, { showPcName: false })!.pcName).toBeNull();
    expect(cardFor(year, {}, "  ")!.pcName).toBeNull();
  });

  it("reads a year with little play as a quiet one", () => {
    const card = cardFor([
      session(at(2026, 2, 7, 20), 90, { id: "one", game_id: "e" }),
      session(at(2026, 2, 8, 20), 30, { id: "two", game_id: "e" }),
    ])!;
    expect(card.games.map((game) => game.title)).toEqual(["Celeste"]);
    expect(card.daysPlayed).toBe(2);
    expect(card.longestStreakDays).toBe(2);
    expect(card.busiestMonth).toEqual({ month: 1, activeMs: 120 * MINUTE_MS });
    expect(card.longestSession).toEqual({ runtimeMs: 90 * MINUTE_MS, title: "Celeste" });
    expect(phraseString(card.sentence)).toBe("A quiet year of two days, all of it in Celeste.");
  });

  it("lights each day of the year by its active time and leaves days to come dark", () => {
    const card = cardFor([
      session(at(2026, 1, 1, 10), 20, { id: "short" }),
      session(at(2026, 1, 2, 10), 300, { id: "long" }),
    ])!;
    expect(card.days).toHaveLength(365);
    expect(card.days.slice(0, 3)).toEqual([1, 5, 0]);
    // Today, 30 September, is day 273 and counts. 1 October is still to come.
    expect(card.days[272]).toBe(0);
    expect(card.days[273]).toBe(-1);
    expect(card.daysOfYear).toBe(273);
  });
});

describe("card layout", () => {
  it("puts the days of a year into weeks, Monday on top", () => {
    // 1 January 2026 is a Thursday, 31 December too.
    const grid = calendarGrid(2026, 1060);
    expect(grid.columns).toBe(53);
    expect(grid.pitch).toBeCloseTo(20);
    expect(grid.cell).toBeLessThan(grid.pitch);
    expect(grid.places[0]).toEqual({ column: 0, row: 3 });
    expect(grid.places[364]).toEqual({ column: 52, row: 3 });
    expect(grid.monthColumns).toHaveLength(12);
    expect(grid.monthColumns[0]).toBe(0);
    // A leap year has a day more.
    expect(calendarGrid(2024, 1060).places).toHaveLength(366);
  });

  it("spreads space left over between blocks after growing the ones that may grow", () => {
    const placed = stackBlocks([{ height: 100 }, { height: 100, maxHeight: 150 }, { height: 50 }], 0, 500, 20);
    expect(placed.map((block) => block.height)).toEqual([100, 150, 50]);
    // 500 - 250 - 40 of gaps leaves 210, 50 of it grown, 160 spread over two gaps.
    expect(placed.map((block) => block.top)).toEqual([0, 200, 450]);
    expect(placed[2].top + placed[2].height).toBe(500);
  });

  it("keeps blocks at the top or in the middle when asked", () => {
    const blocks = [{ height: 100 }, { height: 100 }];
    expect(stackBlocks(blocks, 10, 410, 20, "start").map((block) => block.top)).toEqual([10, 130]);
    expect(stackBlocks(blocks, 10, 410, 20, "center").map((block) => block.top)).toEqual([100, 220]);
    expect(stackBlocks([{ height: 100 }], 0, 300, 20).map((block) => block.top)).toEqual([100]);
  });

  it("never lets blocks overlap, even without room", () => {
    const placed = stackBlocks([{ height: 300 }, { height: 300 }], 0, 400, 20);
    expect(placed[1].top).toBe(placed[0].top + 300 + 20);
  });

  it("scales the preview down to fit and never up", () => {
    expect(previewScale("wide", 600, 600)).toBeCloseTo(0.5);
    expect(previewScale("story", 1000, 960)).toBeCloseTo(0.5);
    expect(previewScale("square", 5000, 5000)).toBe(1);
    for (const format of SHARE_FORMAT_IDS) {
      const scale = previewScale(format, 700, 520);
      expect(SHARE_FORMATS[format].width * scale).toBeLessThanOrEqual(700);
      expect(SHARE_FORMATS[format].height * scale).toBeLessThanOrEqual(520);
    }
  });

  it("names the file after the year", () => {
    expect(shareFileName(2026)).toBe("vaultime-2026.png");
  });
});
