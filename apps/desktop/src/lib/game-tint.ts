// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import {
  BRAND_HUE_DEG,
  TINT_LEVELS,
  TINT_MAX_LIGHTNESS,
  TINT_MAX_STRENGTH,
  TINT_MIN_ALPHA,
  TINT_MIN_LIGHTNESS,
  TINT_MIN_STRENGTH,
  TINT_SAMPLE_PX,
  TINT_TYPICAL_CHROMA,
} from "@/lib/constants";

export interface GameTint {
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
 * `chroma` scales colorfulness, 1 is the default and near 0 is grey.
 */
function tintFromHue(hue: number, chroma = 1): GameTint {
  const color = ({ lightness, chroma: base }: { lightness: number; chroma: number }) =>
    `oklch(${lightness} ${(base * chroma).toFixed(3)} ${Math.round(hue)})`;
  return {
    fill: color(TINT_LEVELS.fill),
    edge: color(TINT_LEVELS.edge),
    ink: color(TINT_LEVELS.ink),
    soft: color(TINT_LEVELS.soft),
    muted: color(TINT_LEVELS.muted),
    wash: color(TINT_LEVELS.wash),
  };
}

/** Vaultime violet, for pages that are not about one game. */
export const BRAND_TINT = tintFromHue(BRAND_HUE_DEG);

/** A stable tint per title, for games without artwork. */
export function tintForTitle(title: string): GameTint {
  let hash = 0;
  for (const char of title) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return tintFromHue(hash % 360);
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

const imageTints = new Map<string, Promise<GameTint | null>>();

/**
 * The dominant hue of a cover image. Colorful pixels count more than grey
 * ones, and near black or white pixels are skipped. Null when the image
 * cannot be read.
 */
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
    const { data } = context.getImageData(0, 0, size, size);

    let sumA = 0;
    let sumB = 0;
    let sumChroma = 0;
    let counted = 0;
    for (let i = 0; i < data.length; i += 4) {
      if (data[i + 3] < TINT_MIN_ALPHA) continue;
      const { lightness, a, b } = toOklab(data[i], data[i + 1], data[i + 2]);
      if (lightness < TINT_MIN_LIGHTNESS || lightness > TINT_MAX_LIGHTNESS) continue;
      const chroma = Math.hypot(a, b);
      sumA += a * chroma;
      sumB += b * chroma;
      sumChroma += chroma;
      counted += 1;
    }
    if (counted === 0) return tintFromHue(BRAND_HUE_DEG, TINT_MIN_STRENGTH);

    const hue = (Math.atan2(sumB, sumA) * 180) / Math.PI;
    const meanChroma = sumChroma / counted;
    const strength = Math.min(
      TINT_MAX_STRENGTH,
      Math.max(TINT_MIN_STRENGTH, meanChroma / TINT_TYPICAL_CHROMA),
    );
    return tintFromHue((hue + 360) % 360, strength);
  } catch {
    return null;
  }
}
