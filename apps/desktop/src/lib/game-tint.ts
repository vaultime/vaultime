// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import {
  BRAND_HUE_DEG,
  MARK_CHROMA_BOOST,
  MARK_LEVELS,
  MARK_MAX_CHROMA,
  MARK_MIN_CHROMA,
  MARK_MIN_HUE_GAP_DEG,
  MARK_NEUTRAL_LIGHTNESS,
  MARK_TINTED_GREY_CHROMA,
  TINT_COLORFUL_MIN_CHROMA,
  TINT_COLORFUL_MIN_SHARE,
  TINT_HUE_BINS,
  TINT_LEVELS,
  TINT_MAX_LIGHTNESS,
  TINT_MAX_STRENGTH,
  TINT_MIN_ALPHA,
  TINT_MIN_LIGHTNESS,
  TINT_MIN_STRENGTH,
  TINT_SAMPLE_PX,
  TINT_SECOND_MIN_GAP_DEG,
  TINT_SECOND_MIN_SHARE,
  TINT_TYPICAL_CHROMA,
} from "@/lib/constants";
import { maxSrgbChroma, srgbToOklab } from "@/lib/color";
import type { ThemeMode } from "@/lib/theme";
import { stableHash } from "@/lib/utils";

/** A color by OKLCH hue in degrees and chroma. */
export interface ArtColor {
  hue: number;
  chroma: number;
}

// Tints are CSS colors whose lightness is a token, so they follow dark and
// light mode without being worked out again. The theme in lib/theme.ts sets
// the tokens. Marks are worked out for the mode.

export interface GameTint {
  /**
   * The main colors of the artwork, the strongest first and at most two, or
   * the color of the title. Empty for black, white and grey art.
   */
  colors: ArtColor[];
  /** Field for covers and tiles, dark in dark mode and pale in light mode. */
  fill: string;
  /** Border of that field. */
  edge: string;
  /** Text on the field. */
  ink: string;
  /** Body text on a wash, a little softer than ink. */
  soft: string;
  /** Small labels on a wash. */
  muted: string;
  /** The deepest or palest wash, for page headers. */
  wash: string;
}

/**
 * The same lightness steps for every game, only hue and colorfulness change.
 * `strength` scales colorfulness, 1 is the default and 0 is grey. The hue may
 * be a CSS value such as a token.
 */
function tintFromHue(hue: number | string, strength: number, colors: ArtColor[]): GameTint {
  const angle = typeof hue === "number" ? String(Math.round(hue)) : hue;
  const tone = (role: keyof typeof TINT_LEVELS) =>
    `oklch(var(--tint-${role}) ${(TINT_LEVELS[role].chroma * strength).toFixed(3)} ${angle})`;
  return {
    colors,
    fill: tone("fill"),
    edge: tone("edge"),
    ink: tone("ink"),
    soft: tone("soft"),
    muted: tone("muted"),
    wash: tone("wash"),
  };
}

/** A tint in the given hue at the default colorfulness. */
function tintForHue(hue: number): GameTint {
  return tintFromHue(hue, 1, [{ hue, chroma: MARK_LEVELS.chroma }]);
}

/** The accent color, for pages that are not about one game. Violet unless the player picks another. */
export const BRAND_TINT = tintFromHue("var(--accent-hue)", 1, [{ hue: BRAND_HUE_DEG, chroma: MARK_LEVELS.chroma }]);

/** A stable tint per title, for games without artwork. */
export function tintForTitle(title: string): GameTint {
  return tintForHue(stableHash(title) % 360);
}

/** Distance between two hues around the color wheel, in degrees. */
function hueDistance(a: number, b: number): number {
  const distance = Math.abs(a - b) % 360;
  return Math.min(distance, 360 - distance);
}

/** The least distance from a hue to any of the placed ones, endless with none placed. */
function roomFrom(hue: number, placed: number[]): number {
  return Math.min(Infinity, ...placed.map((other) => hueDistance(hue, other)));
}

/** A hue moved as little as it can to keep `gap` from the placed ones. */
function placeHue(hue: number, placed: number[], gap: number): number {
  let best = hue;
  let bestRoom = roomFrom(hue, placed);
  for (let shift = 1; shift <= 180 && bestRoom < gap; shift += 1) {
    for (const candidate of [hue + shift, hue - shift]) {
      const candidateRoom = roomFrom(candidate, placed);
      if (candidateRoom > bestRoom) {
        best = candidate;
        bestRoom = candidateRoom;
      }
      if (bestRoom >= gap) break;
    }
  }
  return (best + 360) % 360;
}

/** The gap games keep between their hues, smaller when many share the wheel. */
function hueGap(count: number): number {
  return Math.min(MARK_MIN_HUE_GAP_DEG, 360 / Math.max(count, 1));
}

/** How a game shows in the journal. */
export interface Mark {
  /** Its main color, for stretches it shares with other games. */
  color: string;
  /** What its time bars and its name are painted with, the main color blending into the second for art with two. */
  fill: string;
}

/**
 * Marks that tell the games shown together apart, in the main colors of their
 * artwork, a little more colorful than the art and at a lightness that reads
 * on the ground of the mode. A game whose first color looks like an earlier
 * game or a `taken` hue takes its second one, and only when neither has room
 * does its color move. Black, white and grey art gets a grey, the first one
 * silver, and greys run out into tinted greys. One mark per palette, in the
 * same order.
 */
export function markColors(palettes: ArtColor[][], taken: number[] = [], mode: ThemeMode = "dark"): Mark[] {
  const lightness = MARK_LEVELS.lightness[mode];
  const greySteps = MARK_NEUTRAL_LIGHTNESS[mode];
  let greys = 0;
  const entries = palettes.map((palette) => {
    if (palette.length > 0) return { palette };
    if (greys < greySteps.length) return { grey: greySteps[greys++] };
    return { palette: [{ hue: 0, chroma: MARK_TINTED_GREY_CHROMA }], tinted: true };
  });

  // A tinted grey keeps its low chroma, art is made a little more colorful.
  const tone = ({ hue, chroma }: ArtColor, tinted = false) => {
    const wanted = tinted ? chroma : Math.min(Math.max(chroma * MARK_CHROMA_BOOST, MARK_MIN_CHROMA), MARK_MAX_CHROMA);
    const shown = Math.min(wanted, maxSrgbChroma(lightness, hue));
    // Rounded down, so the color stays inside sRGB.
    return `oklch(${lightness} ${(Math.floor(shown * 1000) / 1000).toFixed(3)} ${Math.round(hue)})`;
  };
  const gap = hueGap(entries.filter((entry) => entry.palette).length + taken.length);
  const placed = [...taken];
  return entries.map((entry) => {
    if (!entry.palette) {
      const grey = `oklch(${entry.grey} 0 0)`;
      return { color: grey, fill: grey };
    }
    const [first, ...others] = entry.palette;
    const fitting = entry.palette.find((color) => roomFrom(color.hue, placed) >= gap);
    const main = fitting ?? { ...first, hue: placeHue(first.hue, placed, gap) };
    placed.push(main.hue);
    const second = fitting && fitting !== first ? first : others[0];
    const color = tone(main, entry.tinted);
    return { color, fill: second ? `linear-gradient(90deg, ${color}, ${tone(second)})` : color };
  });
}

/**
 * The tint of RGBA pixels. Its main colors are the strongest bands of hues
 * among the colorful pixels, so a cover in red and blue gives red and blue
 * instead of a mix of both. A second band counts when it is far enough from
 * the first and nearly as strong. Art with too few colorful pixels, like
 * black, white or grey art, gets a grey tint and no colors.
 */
export function tintFromPixels(data: Uint8ClampedArray): GameTint {
  const bins = Array.from({ length: TINT_HUE_BINS }, () => ({ a: 0, b: 0, chroma: 0, count: 0 }));
  let opaque = 0;
  let colorful = 0;
  let sumChroma = 0;
  let counted = 0;
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] < TINT_MIN_ALPHA) continue;
    opaque += 1;
    const { lightness, a, b } = srgbToOklab(data[i], data[i + 1], data[i + 2]);
    if (lightness < TINT_MIN_LIGHTNESS || lightness > TINT_MAX_LIGHTNESS) continue;
    const chroma = Math.hypot(a, b);
    sumChroma += chroma;
    counted += 1;
    if (chroma < TINT_COLORFUL_MIN_CHROMA) continue;
    colorful += 1;
    const hue = ((Math.atan2(b, a) * 180) / Math.PI + 360) % 360;
    const bin = bins[Math.floor((hue / 360) * TINT_HUE_BINS) % TINT_HUE_BINS];
    bin.a += a;
    bin.b += b;
    bin.chroma += chroma;
    bin.count += 1;
  }
  if (opaque === 0 || colorful / opaque < TINT_COLORFUL_MIN_SHARE) return tintFromHue(BRAND_HUE_DEG, 0, []);

  // A band is a bin with its two neighbors, weighted by colorfulness.
  const band = (center: number) =>
    [center - 1, center, center + 1].map((index) => bins[(index + TINT_HUE_BINS) % TINT_HUE_BINS]);
  const weight = (center: number) => band(center).reduce((sum, bin) => sum + bin.chroma, 0);
  const colorOf = (center: number): ArtColor => {
    const picked = band(center);
    const sum = (key: "a" | "b" | "chroma" | "count") => picked.reduce((total, bin) => total + bin[key], 0);
    return { hue: ((Math.atan2(sum("b"), sum("a")) * 180) / Math.PI + 360) % 360, chroma: sum("chroma") / sum("count") };
  };
  const centers = Array.from({ length: TINT_HUE_BINS }, (_, center) => center);
  const strongest = centers.reduce((best, center) => (weight(center) > weight(best) ? center : best));
  const binGap = Math.ceil((TINT_SECOND_MIN_GAP_DEG / 360) * TINT_HUE_BINS);
  const apart = (center: number) => {
    const distance = Math.abs(center - strongest);
    return Math.min(distance, TINT_HUE_BINS - distance) >= binGap;
  };
  const runnerUp = centers.filter(apart).reduce<number | null>(
    (best, center) => (best === null || weight(center) > weight(best) ? center : best),
    null,
  );

  const colors = [colorOf(strongest)];
  if (runnerUp !== null && weight(runnerUp) >= weight(strongest) * TINT_SECOND_MIN_SHARE) colors.push(colorOf(runnerUp));
  const strength = Math.min(
    TINT_MAX_STRENGTH,
    Math.max(TINT_MIN_STRENGTH, sumChroma / counted / TINT_TYPICAL_CHROMA),
  );
  return tintFromHue(colors[0].hue, strength, colors);
}

const imageTints = new Map<string, Promise<GameTint | null>>();

/** The tint of a cover image, null when the image cannot be read. */
export function tintFromImage(src: string): Promise<GameTint | null> {
  let tint = imageTints.get(src);
  if (!tint) {
    tint = readImageTint(src);
    imageTints.set(src, tint);
  }
  return tint;
}

async function readImageTint(src: string): Promise<GameTint | null> {
  try {
    const image = new Image();
    image.src = src;
    await image.decode();

    const size = TINT_SAMPLE_PX;
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) return null;
    context.drawImage(image, 0, 0, size, size);
    return tintFromPixels(context.getImageData(0, 0, size, size).data);
  } catch {
    return null;
  }
}
