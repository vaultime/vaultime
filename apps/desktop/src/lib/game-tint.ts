// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import {
  BRAND_HUE_DEG,
  GAMUT_SEARCH_MAX_CHROMA,
  GAMUT_SEARCH_STEPS,
  MARK_LEVELS,
  MARK_MAX_CHROMA,
  MARK_MIN_CHROMA,
  MARK_MIN_HUE_GAP_DEG,
  MARK_NEUTRAL_LIGHTNESS,
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
  TINT_TYPICAL_CHROMA,
} from "@/lib/constants";
import { stableHash } from "@/lib/utils";

/** A color by OKLCH hue in degrees and chroma. */
export interface ArtColor {
  hue: number;
  chroma: number;
}

export interface GameTint {
  /** The main color of the artwork, or of the title. Null for black, white and grey art. */
  color: ArtColor | null;
  /** Dark field for covers and tiles. */
  fill: string;
  /** Border of that field. */
  edge: string;
  /** Light text on the field. */
  ink: string;
  /** Body text on a wash, a little dimmer than ink. */
  soft: string;
  /** Small labels on a wash. */
  muted: string;
  /** Very dark wash for page headers. */
  wash: string;
}

/**
 * The same lightness steps for every game, only hue and colorfulness change.
 * `strength` scales colorfulness, 1 is the default and 0 is grey.
 */
function tintFromHue(hue: number, strength: number, color: ArtColor | null): GameTint {
  const tone = ({ lightness, chroma }: { lightness: number; chroma: number }) =>
    `oklch(${lightness} ${(chroma * strength).toFixed(3)} ${Math.round(hue)})`;
  return {
    color,
    fill: tone(TINT_LEVELS.fill),
    edge: tone(TINT_LEVELS.edge),
    ink: tone(TINT_LEVELS.ink),
    soft: tone(TINT_LEVELS.soft),
    muted: tone(TINT_LEVELS.muted),
    wash: tone(TINT_LEVELS.wash),
  };
}

/** A tint in the given hue at the default colorfulness. */
function tintForHue(hue: number): GameTint {
  return tintFromHue(hue, 1, { hue, chroma: MARK_LEVELS.chroma });
}

/** Vaultime violet, for pages that are not about one game. */
export const BRAND_TINT = tintForHue(BRAND_HUE_DEG);

/** A stable tint per title, for games without artwork. */
export function tintForTitle(title: string): GameTint {
  return tintForHue(stableHash(title) % 360);
}

/** Distance between two hues around the color wheel, in degrees. */
function hueDistance(a: number, b: number): number {
  const distance = Math.abs(a - b) % 360;
  return Math.min(distance, 360 - distance);
}

/**
 * Moves hues apart so games shown together are easy to tell apart. The first
 * hue stays where it is and each later one moves as little as it can, so
 * games keep the color of their cover where there is room. `taken` hues are
 * kept free, and with many games the gap shrinks to what fits around the wheel.
 */
export function distinctHues(hues: number[], taken: number[] = []): number[] {
  const gap = Math.min(MARK_MIN_HUE_GAP_DEG, 360 / Math.max(hues.length + taken.length, 1));
  const placed = [...taken];
  for (const hue of hues) {
    const room = (candidate: number) => Math.min(...placed.map((other) => hueDistance(candidate, other)));
    let best = hue;
    let bestRoom = room(hue);
    for (let shift = 1; shift <= 180 && bestRoom < gap; shift += 1) {
      for (const candidate of [hue + shift, hue - shift]) {
        const candidateRoom = room(candidate);
        if (candidateRoom > bestRoom) {
          best = candidate;
          bestRoom = candidateRoom;
        }
        if (bestRoom >= gap) break;
      }
    }
    placed.push((best + 360) % 360);
  }
  return placed.slice(taken.length);
}

/** Whether an OKLCH color fits inside sRGB, see https://bottosson.github.io/posts/oklab/ */
function insideSrgb(lightness: number, chroma: number, hue: number): boolean {
  const radians = (hue * Math.PI) / 180;
  const a = chroma * Math.cos(radians);
  const b = chroma * Math.sin(radians);
  const l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const rgb = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  return rgb.every((channel) => channel >= 0 && channel <= 1);
}

/** The most chroma a lightness and hue can have inside sRGB. */
export function maxSrgbChroma(lightness: number, hue: number): number {
  let inside = 0;
  let outside = GAMUT_SEARCH_MAX_CHROMA;
  for (let step = 0; step < GAMUT_SEARCH_STEPS; step += 1) {
    const middle = (inside + outside) / 2;
    if (insideSrgb(lightness, middle, hue)) inside = middle;
    else outside = middle;
  }
  return inside;
}

/**
 * Marks that tell the games shown together apart, in the main color of their
 * artwork at a lightness that reads on the dark ground. Black, white and grey
 * art gets a grey, the first one silver. A color only moves when it would look
 * like an earlier game or like a `taken` hue, and greys run out into tinted
 * greys. One color per entry of `colors`, in the same order.
 */
export function markColors(colors: (ArtColor | null)[], taken: number[] = []): string[] {
  const greys = new Map<number, number>();
  const hued: { index: number; hue: number; chroma: number }[] = [];
  for (const [index, color] of colors.entries()) {
    if (color) hued.push({ index, hue: color.hue, chroma: color.chroma });
    else if (greys.size < MARK_NEUTRAL_LIGHTNESS.length) greys.set(index, MARK_NEUTRAL_LIGHTNESS[greys.size]);
    else hued.push({ index, hue: 0, chroma: MARK_MIN_CHROMA });
  }

  const marks: string[] = [];
  for (const [index, lightness] of greys) marks[index] = `oklch(${lightness} 0 0)`;
  const hues = distinctHues(
    hued.map((entry) => entry.hue),
    taken,
  );
  for (const [order, entry] of hued.entries()) {
    const hue = hues[order];
    const chroma = Math.min(
      Math.max(entry.chroma, MARK_MIN_CHROMA),
      MARK_MAX_CHROMA,
      maxSrgbChroma(MARK_LEVELS.lightness, hue),
    );
    marks[entry.index] = `oklch(${MARK_LEVELS.lightness} ${chroma.toFixed(3)} ${Math.round(hue)})`;
  }
  return marks;
}

/** sRGB bytes to OKLab, see https://bottosson.github.io/posts/oklab/ */
function toOklab(red: number, green: number, blue: number) {
  const linear = (value: number) => {
    const c = value / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const r = linear(red);
  const g = linear(green);
  const b = linear(blue);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return {
    lightness: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

/**
 * The tint of RGBA pixels. Its hue is the strongest band of hues among the
 * colorful pixels, so a cover in red and blue stays red or blue instead of
 * a mix of both. Art with too few colorful pixels, like black, white or grey
 * art, gets a grey tint and no color.
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
    const { lightness, a, b } = toOklab(data[i], data[i + 1], data[i + 2]);
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
  if (opaque === 0 || colorful / opaque < TINT_COLORFUL_MIN_SHARE) return tintFromHue(BRAND_HUE_DEG, 0, null);

  // A band is a bin with its two neighbors, weighted by colorfulness.
  const band = (center: number) =>
    [center - 1, center, center + 1].map((index) => bins[(index + TINT_HUE_BINS) % TINT_HUE_BINS]);
  const weight = (center: number) => band(center).reduce((sum, bin) => sum + bin.chroma, 0);
  let strongest = 0;
  for (let center = 1; center < TINT_HUE_BINS; center += 1) {
    if (weight(center) > weight(strongest)) strongest = center;
  }
  const picked = band(strongest);
  const sum = (key: "a" | "b" | "chroma" | "count") => picked.reduce((total, bin) => total + bin[key], 0);
  const hue = ((Math.atan2(sum("b"), sum("a")) * 180) / Math.PI + 360) % 360;
  const strength = Math.min(
    TINT_MAX_STRENGTH,
    Math.max(TINT_MIN_STRENGTH, sumChroma / counted / TINT_TYPICAL_CHROMA),
  );
  return tintFromHue(hue, strength, { hue, chroma: sum("chroma") / sum("count") });
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
