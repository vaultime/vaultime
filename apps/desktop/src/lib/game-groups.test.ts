// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { DAY_MS } from "@/lib/constants";
import { groupGames, isGameGroup, type GroupContext } from "@/lib/game-groups";
import { BY_HAND } from "@/lib/launchers";
import type { Device, Game, GameLink, GameStatus } from "@/lib/types";

const NOW = Date.parse("2026-11-17T12:00:00.000Z");
const daysAgo = (days: number) => new Date(NOW - days * DAY_MS).toISOString();

function entry(
  id: string,
  { launchers = ["steam"], status = null, lastPlayedAt = null, origin = null }: {
    launchers?: string[];
    status?: GameStatus | null;
    lastPlayedAt?: string | null;
    origin?: string | null;
  } = {},
) {
  return { game: { id, title: id, origin_device_id: origin } as Game, launchers, status, lastPlayedAt };
}

const context: GroupContext = {
  now: NOW,
  links: [{ game_id: "hades-laptop", linked_game_id: "hades", origin_device_id: "laptop" } as GameLink],
  devices: [
    { id: "desktop", name: "Desktop" },
    { id: "laptop", name: "Laptop" },
    { id: "deck", name: "Couch" },
  ] as Device[],
  thisDeviceId: "desktop",
};

const shape = (groups: { title: string; games: { game: Game }[] }[]) =>
  groups.map((group) => [group.title, group.games.map((member) => member.game.id)]);

describe("groupGames", () => {
  it("shows every game in one group without a title when not grouping", () => {
    expect(shape(groupGames([entry("a"), entry("b")], "none", context))).toEqual([["", ["a", "b"]]]);
  });

  it("groups by launcher with the most games first and a game under each of its launchers", () => {
    const games = [
      entry("hades", { launchers: ["steam", "gog"] }),
      entry("celeste", { launchers: ["folder_scan"] }),
      entry("balatro", { launchers: ["steam"] }),
      entry("tetris", { launchers: [BY_HAND] }),
    ];
    expect(shape(groupGames(games, "launcher", context))).toEqual([
      ["Steam", ["hades", "balatro"]],
      ["GOG", ["hades"]],
      ["Added by hand", ["tetris"]],
      ["Found in folders", ["celeste"]],
    ]);
  });

  it("groups by when a game was last played, keeping the order of the games", () => {
    const games = [
      entry("today", { lastPlayedAt: daysAgo(0.5) }),
      entry("weeks", { lastPlayedAt: daysAgo(20) }),
      entry("months", { lastPlayedAt: daysAgo(200) }),
      entry("old", { lastPlayedAt: daysAgo(800) }),
      entry("six-days", { lastPlayedAt: daysAgo(6.9) }),
      entry("new"),
    ];
    expect(shape(groupGames(games, "played", context))).toEqual([
      ["Past week", ["today", "six-days"]],
      ["Past month", ["weeks"]],
      ["Past year", ["months"]],
      ["Longer ago", ["old"]],
      ["Not played yet", ["new"]],
    ]);
  });

  it("groups by status in the order statuses go, games without one last", () => {
    const games = [entry("a", { status: "finished" }), entry("b"), entry("c", { status: "backlog" })];
    expect(shape(groupGames(games, "status", context))).toEqual([
      ["Backlog", ["c"]],
      ["Finished", ["a"]],
      ["No status", ["b"]],
    ]);
  });

  it("groups by PC with this PC first and a game under every PC with a copy", () => {
    const games = [entry("hades"), entry("sun-haven", { origin: "laptop" }), entry("stardew", { origin: "deck" })];
    expect(shape(groupGames(games, "pc", context))).toEqual([
      ["This PC", ["hades"]],
      ["Couch", ["stardew"]],
      ["Laptop", ["hades", "sun-haven"]],
    ]);
  });

  it("names this PC before its id is known", () => {
    expect(shape(groupGames([entry("a")], "pc", { ...context, thisDeviceId: null }))).toEqual([["This PC", ["a"]]]);
  });
});

describe("isGameGroup", () => {
  it("accepts only known groupings", () => {
    expect(isGameGroup("launcher")).toBe(true);
    expect(isGameGroup("hasOwnProperty")).toBe(false);
    expect(isGameGroup(null)).toBe(false);
  });
});
