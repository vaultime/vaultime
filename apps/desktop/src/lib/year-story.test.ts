// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS, MINUTE_MS } from "@/lib/constants";
import { yearStats } from "@/lib/stats";
import { estimateYearPlay } from "@/test/play-estimates";
import { at, session } from "@/test/sessions";
import type { GameStatusChange, LauncherPlay, Session } from "@/lib/types";
import { chapterTitle, yearStory } from "./year-story";

const now = new Date(2026, 11, 20, 12, 0);

function storyOf(
  sessions: Session[],
  {
    launchers = [],
    changes = [],
    hidden = [],
  }: { launchers?: LauncherPlay[]; changes?: GameStatusChange[]; hidden?: string[] } = {},
) {
  return yearStory({
    stats: yearStats(sessions, 2026, estimateYearPlay(sessions, 2026, now.getTime())),
    sessions,
    launchers,
    statusChanges: changes,
    named: (gameId) => !hidden.includes(gameId),
  });
}

function played(gameId: string, month: number, day: number, hour: number, minutes: number) {
  return session(at(2026, month, day, hour), minutes, { id: `${gameId}-${month}-${day}-${hour}`, game_id: gameId });
}

const year = [
  played("elden", 3, 2, 20, 120),
  played("elden", 3, 3, 20, 180),
  played("elden", 3, 4, 21, 60),
  played("elden", 7, 1, 20, 60),
  played("hades", 3, 5, 19, 90),
  played("hades", 5, 1, 14, 60),
  played("secret", 6, 1, 13, 400),
  played("balatro", 9, 9, 22, 45),
];

describe("yearStory", () => {
  it("tells every chapter with something in it", () => {
    const story = storyOf(year, {
      launchers: [
        { launcher: "steam", runtime_ms: 10 * HOUR_MS },
        { launcher: null, runtime_ms: 2 * HOUR_MS },
      ],
      changes: [{ id: "c1", game_id: "hades", status: "finished", changed_at: at(2026, 5, 2, 12) }],
    });
    expect(story.chapters).toEqual(["numbers", "game", "top", "longest", "week", "rhythm", "launchers", "finished"]);
    expect(story.daysPlayed).toBe(8);
    expect(story.firstDay).toEqual(new Date(2026, 2, 2));
    expect(story.lastDay).toEqual(new Date(2026, 8, 9));
  });

  it("names the game of the year with its days, its best month and its longest session", () => {
    const { game } = storyOf(year, { hidden: ["secret"] });
    expect(game).toMatchObject({
      gameId: "elden",
      runtimeMs: 420 * MINUTE_MS,
      days: 4,
      bestMonth: 2,
      longestSessionMs: 180 * MINUTE_MS,
    });
    expect(game?.share).toBeCloseTo(420 / 1015);
  });

  it("keeps hidden games out of every list but in the hours", () => {
    const story = storyOf(year, { hidden: ["secret"] });
    expect(story.top.map((game) => game.gameId)).toEqual(["elden", "hades", "balatro"]);
    expect(story.longest?.gameId).toBeUndefined();
    expect(story.chapters).not.toContain("longest");
    expect(story.playedMs).toBe(1015 * MINUTE_MS);
  });

  it("finds the longest session and whether it was a night", () => {
    const { longest } = storyOf(year);
    expect(longest).toMatchObject({ gameId: "secret", runtimeMs: 400 * MINUTE_MS, night: false });
    expect(storyOf([played("elden", 3, 2, 21, 300)]).longest?.night).toBe(true);
    // An afternoon that runs past midnight counts as a night too.
    expect(storyOf([played("elden", 3, 2, 15, 600)]).longest?.night).toBe(true);
  });

  it("finds the week with the most play and its games", () => {
    const { week } = storyOf(year);
    expect(week?.start).toEqual(new Date(2026, 2, 2));
    expect(week?.playedMs).toBe(450 * MINUTE_MS);
    expect(week?.daysPlayed).toBe(4);
    expect(week?.games.map((game) => [game.gameId, game.runtimeMs / MINUTE_MS])).toEqual([
      ["elden", 360],
      ["hades", 90],
    ]);
  });

  it("adds up the hours of the day across the week", () => {
    const { hours } = storyOf([
      played("elden", 3, 2, 20, 60),
      played("elden", 3, 7, 20, 30),
      played("elden", 3, 8, 9, 30),
    ]);
    expect(hours[20]).toBe(90 * MINUTE_MS);
    expect(hours[9]).toBe(30 * MINUTE_MS);
    expect(hours[12]).toBe(0);
  });

  it("merges launchers by key and gives each its share", () => {
    const { launchers } = storyOf(year, {
      launchers: [
        { launcher: "steam", runtime_ms: 6 * HOUR_MS },
        { launcher: null, runtime_ms: 3 * HOUR_MS },
        { launcher: "manual", runtime_ms: HOUR_MS },
        { launcher: "gog", runtime_ms: 30 * 1000 },
      ],
    });
    expect(launchers.map((launcher) => [launcher.launcher, launcher.share])).toEqual([
      ["steam", 0.6],
      ["manual", 0.4],
    ]);
  });

  it("lists games finished in the year once, unless the status was cleared", () => {
    const { finished } = storyOf(year, {
      changes: [
        { id: "1", game_id: "elden", status: "finished", changed_at: at(2026, 4, 1, 12) },
        { id: "2", game_id: "elden", status: "playing", changed_at: at(2026, 6, 1, 12) },
        { id: "3", game_id: "elden", status: "finished", changed_at: at(2026, 8, 1, 12) },
        { id: "4", game_id: "hades", status: "finished", changed_at: at(2026, 2, 1, 12) },
        { id: "5", game_id: "hades", status: "none", changed_at: at(2026, 2, 2, 12) },
        { id: "6", game_id: "balatro", status: "finished", changed_at: at(2025, 12, 30, 12) },
        { id: "7", game_id: "secret", status: "finished", changed_at: at(2026, 3, 1, 12) },
      ],
      hidden: ["secret"],
    });
    expect(finished).toEqual([{ gameId: "elden", at: new Date(2026, 3, 1, 12) }]);
  });

  it("names the top chapter by its count and the longest by when it was", () => {
    const story = storyOf(year);
    expect(chapterTitle("top", story)).toBe("Your top four");
    expect(chapterTitle("longest", story)).toBe("The longest day");
    expect(chapterTitle("longest", storyOf([played("elden", 3, 2, 22, 60)]))).toBe("The longest night");
  });

  it("has nothing to tell for a year without play", () => {
    const story = storyOf([]);
    expect(story.chapters).toEqual([]);
    expect(story.game).toBeNull();
    expect(story.week).toBeNull();
  });
});
