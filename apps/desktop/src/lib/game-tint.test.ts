// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { maxSrgbChroma } from "./color";
import { MARK_LEVELS, MARK_MIN_HUE_GAP_DEG } from "./constants";
import { distinctHues, markColors, tintForTitle, tintFromPixels } from "./game-tint";
import { themeTokens, type ThemeMode } from "./theme";

const distance = (a: number, b: number) => Math.min(Math.abs(a - b), 360 - Math.abs(a - b));

/** RGBA pixels from runs of `count` pixels in one color. */
function pixels(...runs: [count: number, red: number, green: number, blue: number, alpha?: number][]) {
  const data: number[] = [];
  for (const [count, red, green, blue, alpha = 255] of runs) {
    for (let i = 0; i < count; i += 1) data.push(red, green, blue, alpha);
  }
  return new Uint8ClampedArray(data);
}

/** Parses `oklch(l c h)`, with the tokens of a mode put in. */
function oklch(color: string, mode: ThemeMode = "dark") {
  const tokens = themeTokens(mode, "vault", "violet");
  const resolved = color.replace(/var\((--[a-z0-9-]+)\)/g, (_, name: string) => tokens[name]);
  const [lightness, chroma, hue] = resolved.slice("oklch(".length, -1).split(" ").map(Number);
  return { lightness, chroma, hue };
}

describe("tintForTitle", () => {
  it("gives the same title the same colors", () => {
    expect(tintForTitle("Hades II")).toEqual(tintForTitle("Hades II"));
    expect(tintForTitle("Hades II").fill).not.toBe(tintForTitle("Celeste").fill);
    expect(tintForTitle("Celeste").wash).toMatch(/^oklch\(/);
    expect(tintForTitle("Celeste").color).not.toBeNull();
  });
});

describe("tintFromPixels", () => {
  it("keeps black art grey, even with a faint cast", () => {
    const tint = tintFromPixels(pixels([900, 0, 0, 0], [124, 38, 44, 38]));
    expect(tint.color).toBeNull();
    expect(oklch(tint.fill).chroma).toBe(0);
  });

  it("keeps white and grey art grey", () => {
    expect(tintFromPixels(pixels([1024, 250, 250, 250])).color).toBeNull();
    expect(tintFromPixels(pixels([1024, 128, 128, 128])).color).toBeNull();
  });

  it("ignores a speck of color on black", () => {
    expect(tintFromPixels(pixels([1000, 0, 0, 0], [24, 220, 30, 30])).color).toBeNull();
  });

  it("takes the main color instead of a mix of two", () => {
    const tint = tintFromPixels(pixels([600, 220, 30, 30], [424, 30, 60, 230]));
    expect(tint.color).not.toBeNull();
    // Red is near 29 degrees and blue near 264, a mix would be magenta.
    expect(distance(tint.color!.hue, 27)).toBeLessThan(10);
  });

  it("finds the color of a logo on a transparent field", () => {
    const tint = tintFromPixels(pixels([900, 0, 0, 0, 0], [124, 40, 160, 60]));
    expect(tint.color).not.toBeNull();
    expect(distance(tint.color!.hue, 145)).toBeLessThan(10);
  });

  it("returns grey for an empty image", () => {
    expect(tintFromPixels(pixels([16, 0, 0, 0, 0])).color).toBeNull();
  });
});

describe("distinctHues", () => {
  it("keeps hues that are already far apart", () => {
    expect(distinctHues([10, 120, 240])).toEqual([10, 120, 240]);
    expect(distinctHues([200])).toEqual([200]);
    expect(distinctHues([20, 350])).toEqual([20, 350]);
  });

  it("moves a close hue only as far as it needs to", () => {
    expect(distinctHues([100, 110])).toEqual([100, 130]);
    expect(distinctHues([20, 0])).toEqual([20, 350]);
  });

  it("keeps taken hues free", () => {
    expect(distinctHues([290, 100], [293])).toEqual([263, 100]);
  });

  it("spreads many games around the wheel", () => {
    const hues = distinctHues(Array.from({ length: 10 }, () => 0));
    for (const [index, hue] of hues.entries()) {
      for (const other of hues.slice(index + 1)) {
        expect(distance(hue, other)).toBeGreaterThanOrEqual(MARK_MIN_HUE_GAP_DEG);
      }
    }
  });
});

describe("markColors", () => {
  it("gives grey art grey marks, silver first", () => {
    const [first, second] = markColors([null, null]).map((color) => oklch(color));
    expect(first.chroma).toBe(0);
    expect(second.chroma).toBe(0);
    expect(first.lightness).not.toBe(second.lightness);
  });

  it("keeps the hue and colorfulness of the art when there is room", () => {
    const [red, green] = markColors([
      { hue: 27, chroma: 0.12 },
      { hue: 145, chroma: 0.09 },
    ]).map((color) => oklch(color));
    expect(red).toEqual({ lightness: MARK_LEVELS.lightness.dark, chroma: 0.12, hue: 27 });
    expect(green).toEqual({ lightness: MARK_LEVELS.lightness.dark, chroma: 0.09, hue: 145 });
  });

  it("nudges a game that would look like an earlier one", () => {
    const [first, second] = markColors([
      { hue: 27, chroma: 0.12 },
      { hue: 35, chroma: 0.12 },
    ]).map((color) => oklch(color));
    expect(first.hue).toBe(27);
    expect(distance(first.hue, second.hue)).toBeGreaterThanOrEqual(MARK_MIN_HUE_GAP_DEG);
  });

  it("keeps marks inside sRGB in both modes", () => {
    const hues = [{ hue: 264, chroma: 0.3 }, { hue: 100, chroma: 0.3 }, { hue: 200, chroma: 0.3 }];
    for (const mode of ["dark", "light"] as const) {
      for (const mark of markColors(hues).map((color) => oklch(color, mode))) {
        expect(mark.lightness).toBe(MARK_LEVELS.lightness[mode]);
        expect(mark.chroma).toBeLessThanOrEqual(maxSrgbChroma(mark.lightness, mark.hue));
      }
    }
  });

  it("gives greys a lightness that reads in light mode too", () => {
    const [dark, light] = (["dark", "light"] as const).map((mode) => oklch(markColors([null])[0], mode));
    expect(dark.lightness).toBeGreaterThan(light.lightness);
  });

  it("tints greys once the grey steps run out", () => {
    const marks = markColors([null, null, null, null]).map((color) => oklch(color));
    expect(marks.slice(0, 3).every((mark) => mark.chroma === 0)).toBe(true);
    expect(marks[3].chroma).toBeGreaterThan(0);
  });

  it("keeps taken hues free", () => {
    const [mark] = markColors([{ hue: 290, chroma: 0.12 }], [293]).map((color) => oklch(color));
    expect(distance(mark.hue, 293)).toBeGreaterThanOrEqual(MARK_MIN_HUE_GAP_DEG);
  });
});
