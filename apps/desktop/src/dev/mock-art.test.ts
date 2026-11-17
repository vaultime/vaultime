// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { artFromPixels } from "./mock-art";

const distance = (a: number, b: number) => Math.min(Math.abs(a - b), 360 - Math.abs(a - b));

/** RGBA pixels from runs of `count` pixels in one color. */
function pixels(...runs: [count: number, red: number, green: number, blue: number, alpha?: number][]) {
  const data: number[] = [];
  for (const [count, red, green, blue, alpha = 255] of runs) {
    for (let i = 0; i < count; i += 1) data.push(red, green, blue, alpha);
  }
  return new Uint8ClampedArray(data);
}

// The same cases as the tests of `assets/tint.rs`, so the preview shows the colors the app does.
describe("artFromPixels", () => {
  it("keeps black art grey, even with a faint cast", () => {
    const art = artFromPixels(pixels([900, 0, 0, 0], [124, 38, 44, 38]));
    expect(art.colors).toEqual([]);
    expect(art.strength).toBe(0);
  });

  it("measures how light the art is", () => {
    expect(artFromPixels(pixels([1024, 255, 255, 255])).lightness).toBeCloseTo(1, 2);
    expect(artFromPixels(pixels([512, 0, 0, 0], [512, 255, 255, 255])).lightness).toBeCloseTo(0.5, 2);
    expect(artFromPixels(pixels([1024, 0, 0, 0, 0])).lightness).toBeNull();
  });

  it("keeps white and grey art grey", () => {
    expect(artFromPixels(pixels([1024, 250, 250, 250])).colors).toEqual([]);
    expect(artFromPixels(pixels([1024, 128, 128, 128])).colors).toEqual([]);
  });

  it("ignores a speck of color on black", () => {
    expect(artFromPixels(pixels([1000, 0, 0, 0], [24, 220, 30, 30])).colors).toEqual([]);
  });

  it("takes the main colors instead of a mix of them", () => {
    const tint = artFromPixels(pixels([600, 220, 30, 30], [424, 30, 60, 230]));
    // Red is near 29 degrees and blue near 264, a mix would be magenta.
    expect(tint.colors).toHaveLength(2);
    expect(distance(tint.colors[0].hue, 27)).toBeLessThan(10);
    expect(distance(tint.colors[1].hue, 264)).toBeLessThan(10);
  });

  it("keeps both colors of a logo in gold and blue", () => {
    const tint = artFromPixels(pixels([300, 0, 0, 0], [360, 200, 160, 70], [364, 20, 110, 150]));
    expect(tint.colors).toHaveLength(2);
    const hues = tint.colors.map((color) => color.hue);
    expect(hues.some((hue) => distance(hue, 75) < 15)).toBe(true);
    expect(hues.some((hue) => distance(hue, 225) < 20)).toBe(true);
  });

  it("leaves out a small second color", () => {
    const tint = artFromPixels(pixels([900, 220, 30, 30], [124, 30, 60, 230]));
    expect(tint.colors).toHaveLength(1);
  });

  it("does not count a neighboring shade as a second color", () => {
    const tint = artFromPixels(pixels([512, 220, 30, 30], [512, 230, 110, 30]));
    expect(tint.colors).toHaveLength(1);
  });

  it("finds the color of a logo on a transparent field", () => {
    const tint = artFromPixels(pixels([900, 0, 0, 0, 0], [124, 40, 160, 60]));
    expect(tint.colors).toHaveLength(1);
    expect(distance(tint.colors[0].hue, 145)).toBeLessThan(10);
  });

  it("returns grey for an empty image", () => {
    expect(artFromPixels(pixels([16, 0, 0, 0, 0])).colors).toEqual([]);
  });
});
