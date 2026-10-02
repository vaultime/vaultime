// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { maxSrgbChroma } from "./color";
import { MARK_CHROMA_BOOST, MARK_LEVELS, MARK_MAX_CHROMA, MARK_MIN_CHROMA, MARK_MIN_HUE_GAP_DEG } from "./constants";
import { distinctHues, markColors, tintForTitle, tintFromPixels, type ArtColor } from "./game-tint";

const distance = (a: number, b: number) => Math.min(Math.abs(a - b), 360 - Math.abs(a - b));

/** RGBA pixels from runs of `count` pixels in one color. */
function pixels(...runs: [count: number, red: number, green: number, blue: number, alpha?: number][]) {
  const data: number[] = [];
  for (const [count, red, green, blue, alpha = 255] of runs) {
    for (let i = 0; i < count; i += 1) data.push(red, green, blue, alpha);
  }
  return new Uint8ClampedArray(data);
}

/** Parses `oklch(l c h)`. */
function oklch(color: string) {
  const [lightness, chroma, hue] = color.slice("oklch(".length, -1).split(" ").map(Number);
  return { lightness, chroma, hue };
}

describe("tintForTitle", () => {
  it("gives the same title the same colors", () => {
    expect(tintForTitle("Hades II")).toEqual(tintForTitle("Hades II"));
    expect(tintForTitle("Hades II").fill).not.toBe(tintForTitle("Celeste").fill);
    expect(tintForTitle("Celeste").wash).toMatch(/^oklch\(/);
    expect(tintForTitle("Celeste").colors).toHaveLength(1);
  });
});

describe("tintFromPixels", () => {
  it("keeps black art grey, even with a faint cast", () => {
    const tint = tintFromPixels(pixels([900, 0, 0, 0], [124, 38, 44, 38]));
    expect(tint.colors).toEqual([]);
    expect(oklch(tint.fill.replace("var(--tint-fill)", "0.3")).chroma).toBe(0);
  });

  it("keeps white and grey art grey", () => {
    expect(tintFromPixels(pixels([1024, 250, 250, 250])).colors).toEqual([]);
    expect(tintFromPixels(pixels([1024, 128, 128, 128])).colors).toEqual([]);
  });

  it("ignores a speck of color on black", () => {
    expect(tintFromPixels(pixels([1000, 0, 0, 0], [24, 220, 30, 30])).colors).toEqual([]);
  });

  it("takes the main colors instead of a mix of them", () => {
    const tint = tintFromPixels(pixels([600, 220, 30, 30], [424, 30, 60, 230]));
    // Red is near 29 degrees and blue near 264, a mix would be magenta.
    expect(tint.colors).toHaveLength(2);
    expect(distance(tint.colors[0].hue, 27)).toBeLessThan(10);
    expect(distance(tint.colors[1].hue, 264)).toBeLessThan(10);
  });

  it("keeps both colors of a logo in gold and blue", () => {
    const tint = tintFromPixels(pixels([300, 0, 0, 0], [360, 200, 160, 70], [364, 20, 110, 150]));
    expect(tint.colors).toHaveLength(2);
    const hues = tint.colors.map((color) => color.hue);
    expect(hues.some((hue) => distance(hue, 75) < 15)).toBe(true);
    expect(hues.some((hue) => distance(hue, 225) < 20)).toBe(true);
  });

  it("leaves out a small second color", () => {
    const tint = tintFromPixels(pixels([900, 220, 30, 30], [124, 30, 60, 230]));
    expect(tint.colors).toHaveLength(1);
  });

  it("does not count a neighboring shade as a second color", () => {
    const tint = tintFromPixels(pixels([512, 220, 30, 30], [512, 230, 110, 30]));
    expect(tint.colors).toHaveLength(1);
  });

  it("finds the color of a logo on a transparent field", () => {
    const tint = tintFromPixels(pixels([900, 0, 0, 0, 0], [124, 40, 160, 60]));
    expect(tint.colors).toHaveLength(1);
    expect(distance(tint.colors[0].hue, 145)).toBeLessThan(10);
  });

  it("returns grey for an empty image", () => {
    expect(tintFromPixels(pixels([16, 0, 0, 0, 0])).colors).toEqual([]);
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
  const color = (palettes: ArtColor[][], taken: number[] = [], mode: "dark" | "light" = "dark") =>
    markColors(palettes, taken, mode).map((mark) => oklch(mark.color));

  it("gives grey art grey marks, silver first", () => {
    const [first, second] = color([[], []]);
    expect(first.chroma).toBe(0);
    expect(second.chroma).toBe(0);
    expect(first.lightness).not.toBe(second.lightness);
  });

  it("keeps the hue of the art and makes it a little more colorful", () => {
    const [red, green] = color([[{ hue: 27, chroma: 0.1 }], [{ hue: 145, chroma: 0.09 }]]);
    expect(red.hue).toBe(27);
    expect(red.chroma).toBeCloseTo(0.1 * MARK_CHROMA_BOOST, 3);
    expect(green.hue).toBe(145);
    expect(green.chroma).toBeCloseTo(0.09 * MARK_CHROMA_BOOST, 3);
    expect(red.lightness).toBe(MARK_LEVELS.lightness.dark);
  });

  it("keeps muted art a color and vivid art from glaring", () => {
    const [muted, vivid] = color([[{ hue: 145, chroma: 0.02 }], [{ hue: 330, chroma: 0.3 }]]);
    expect(muted.chroma).toBe(MARK_MIN_CHROMA);
    expect(vivid.chroma).toBe(MARK_MAX_CHROMA);
  });

  it("uses all the color each mode allows", () => {
    // Yellow has little room in light mode and blue little in dark mode.
    const [yellowDark] = color([[{ hue: 90, chroma: 0.12 }]], [], "dark");
    const [blueLight] = color([[{ hue: 264, chroma: 0.12 }]], [], "light");
    expect(yellowDark.chroma).toBeGreaterThan(maxSrgbChroma(MARK_LEVELS.lightness.light, 90));
    expect(blueLight.chroma).toBeGreaterThan(maxSrgbChroma(MARK_LEVELS.lightness.dark, 264));
  });

  it("blends the bars of two-colored art from the first color into the second", () => {
    const [mark] = markColors([[{ hue: 80, chroma: 0.09 }, { hue: 225, chroma: 0.08 }]]);
    expect(oklch(mark.color).hue).toBe(80);
    expect(mark.fill).toMatch(/^linear-gradient\(90deg, oklch\([^)]+ 80\), oklch\([^)]+ 225\)\)$/);
  });

  it("fills the bars of one-colored art with its color", () => {
    const [mark] = markColors([[{ hue: 80, chroma: 0.09 }]]);
    expect(mark.fill).toBe(mark.color);
  });

  it("takes the second real color when the first is taken", () => {
    const [orange, logo] = color([[{ hue: 60, chroma: 0.1 }], [{ hue: 70, chroma: 0.1 }, { hue: 225, chroma: 0.1 }]]);
    expect(orange.hue).toBe(60);
    expect(logo.hue).toBe(225);
    const [mark] = markColors([[{ hue: 70, chroma: 0.1 }, { hue: 225, chroma: 0.1 }]], [60]);
    expect(oklch(mark.color).hue).toBe(225);
    // The bars still show the first color, after the one the game is known by this week.
    expect(mark.fill).toMatch(/ 225\), oklch\([^)]+ 70\)\)$/);
  });

  it("nudges a game only when none of its colors has room", () => {
    const [first, second] = color([[{ hue: 27, chroma: 0.12 }], [{ hue: 35, chroma: 0.12 }]]);
    expect(first.hue).toBe(27);
    expect(distance(first.hue, second.hue)).toBeGreaterThanOrEqual(MARK_MIN_HUE_GAP_DEG);
  });

  it("keeps marks inside sRGB in both modes", () => {
    const palettes = [[{ hue: 264, chroma: 0.3 }], [{ hue: 100, chroma: 0.3 }], [{ hue: 200, chroma: 0.3 }]];
    for (const mode of ["dark", "light"] as const) {
      for (const mark of color(palettes, [], mode)) {
        expect(mark.lightness).toBe(MARK_LEVELS.lightness[mode]);
        expect(mark.chroma).toBeLessThanOrEqual(maxSrgbChroma(mark.lightness, mark.hue));
      }
    }
  });

  it("gives greys a lightness that reads in light mode too", () => {
    const [[dark], [light]] = [color([[]], [], "dark"), color([[]], [], "light")];
    expect(dark.lightness).toBeGreaterThan(light.lightness);
  });

  it("tints greys once the grey steps run out", () => {
    const marks = color([[], [], [], []]);
    expect(marks.slice(0, 3).every((mark) => mark.chroma === 0)).toBe(true);
    expect(marks[3].chroma).toBeGreaterThan(0);
  });

  it("keeps taken hues free", () => {
    const [mark] = color([[{ hue: 290, chroma: 0.12 }]], [293]);
    expect(distance(mark.hue, 293)).toBeGreaterThanOrEqual(MARK_MIN_HUE_GAP_DEG);
  });
});
