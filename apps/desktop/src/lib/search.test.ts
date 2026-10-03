// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { matchesSearch } from "./search";

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
