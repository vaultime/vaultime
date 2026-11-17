// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import type { GameSummary } from "@/features/library/library-context";
import { tintForTitle } from "@/lib/game-tint";
import { gameArt, gameInFront } from "./game-in-front";

function summary(id: string, lastPlayedAt: string | null): GameSummary {
  return { game: { id, title: id }, lastPlayedAt, cover: `/covers/${id}.jpg` } as unknown as GameSummary;
}

const recent = summary("recent", "2026-11-16T20:00:00Z");
const older = summary("older", "2026-11-01T20:00:00Z");
const hidden = summary("hidden", "2026-11-17T09:00:00Z");
const fresh = summary("fresh", null);

describe("gameInFront", () => {
  it("is the game a page is about, a hidden one too", () => {
    expect(gameInFront([recent, older, hidden], [recent, older], new Set(), "hidden")).toBe(hidden);
    expect(gameInFront([recent], [recent], new Set(), "gone")).toBeNull();
  });

  it("lights the look only with art read from a cover", () => {
    const withArt = { ...recent, tint: { ...tintForTitle("recent"), lightness: 0.4 } };
    expect(gameArt(withArt)).toEqual({ cover: "/covers/recent.jpg", colors: withArt.tint.colors, lightness: 0.4 });
    const titleOnly = { ...older, cover: null, tint: tintForTitle("older") };
    expect(gameArt(titleOnly)).toEqual({ cover: null, colors: [], lightness: null });
    expect(gameArt(null)).toBeNull();
  });

  it("is the game playing, else the one played last, as Home shows it", () => {
    expect(gameInFront([fresh, recent, older], [fresh, recent, older], new Set(["older"]), undefined)).toBe(older);
    expect(gameInFront([fresh, recent, older], [fresh, recent, older], new Set(), undefined)).toBe(recent);
    expect(gameInFront([fresh], [fresh], new Set(), undefined)).toBeNull();
  });
});
