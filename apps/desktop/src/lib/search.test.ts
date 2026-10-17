// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { matchesGame, matchesSearch } from "./search";

describe("matchesSearch", () => {
  it("matches everything for an empty query", () => {
    expect(matchesSearch("Elden Ring", "")).toBe(true);
    expect(matchesSearch("Elden Ring", "   ")).toBe(true);
  });

  it("ignores case, accents and the order of words", () => {
    expect(matchesSearch("Pokémon Legends: Z-A", "pokemon")).toBe(true);
    expect(matchesSearch("Elden Ring", "ring ELDEN")).toBe(true);
  });

  it("needs every word", () => {
    expect(matchesSearch("Elden Ring", "elden souls")).toBe(false);
  });
});

describe("matchesGame", () => {
  it("finds a game by the title of an install that counts as it", () => {
    expect(matchesGame("The Witcher 3", ["The Witcher 3 GOTY"], "goty")).toBe(true);
    expect(matchesGame("The Witcher 3", [], "goty")).toBe(false);
    expect(matchesGame("The Witcher 3", [], "")).toBe(true);
  });
});
