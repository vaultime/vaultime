// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { isGameSort, sortGames } from "./game-sort";

const game = (title: string, totalMs: number, created_at = "2026-01-01T00:00:00.000Z") => ({
  game: { title, created_at },
  totalMs,
});
const titles = (games: { game: { title: string } }[]) => games.map((entry) => entry.game.title);

describe("sortGames", () => {
  const recentFirst = [
    game("hades II", 5),
    game("Ōkami", 30),
    game("Hades", 20),
    game("Celeste", 10),
    game("Hades 10", 1),
    game("Hades 2", 3),
  ];

  it("keeps the most recent first", () => {
    expect(sortGames(recentFirst, "recent")).toBe(recentFirst);
  });

  it("sorts titles the way people read them", () => {
    expect(titles(sortGames(recentFirst, "title"))).toEqual([
      "Celeste",
      "Hades",
      "Hades 2",
      "Hades 10",
      "hades II",
      "Ōkami",
    ]);
  });

  it("puts the games added last first", () => {
    const added = [
      game("Celeste", 0, "2026-03-02T10:00:00.000Z"),
      game("Hades", 0, "2026-11-01T10:00:00.000Z"),
      game("Balatro", 0, "2026-06-15T10:00:00.000Z"),
    ];
    expect(titles(sortGames(added, "added"))).toEqual(["Hades", "Balatro", "Celeste"]);
  });

  it("puts the most played first without touching the input", () => {
    expect(titles(sortGames(recentFirst, "played"))).toEqual(["Ōkami", "Hades", "Celeste", "hades II", "Hades 2", "Hades 10"]);
    expect(titles(recentFirst)[0]).toBe("hades II");
  });
});

describe("isGameSort", () => {
  it("accepts only known sorts", () => {
    expect(isGameSort("played")).toBe(true);
    expect(isGameSort("toString")).toBe(false);
    expect(isGameSort(undefined)).toBe(false);
  });
});
