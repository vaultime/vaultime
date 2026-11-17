// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The colors of the sample art of the preview, which has no core to work
// them out. Follows `tint_of_pixels` in `src-tauri/src/assets/tint.rs`.

import { srgbToOklab } from "@/lib/color";
import {
  TINT_COLORFUL_MIN_CHROMA,
  TINT_COLORFUL_MIN_SHARE,
  TINT_HUE_BINS,
  TINT_MAX_LIGHTNESS,
  TINT_MAX_STRENGTH,
  TINT_MIN_ALPHA,
  TINT_MIN_LIGHTNESS,
  TINT_MIN_STRENGTH,
  TINT_SECOND_MIN_GAP_DEG,
  TINT_SECOND_MIN_SHARE,
  TINT_TYPICAL_CHROMA,
} from "@/lib/constants";
import { readPixels, type ArtColor, type ArtTint } from "@/lib/game-tint";

/** The colors of RGBA pixels. */
export function artFromPixels(data: Uint8ClampedArray): ArtTint {
  const bins = Array.from({ length: TINT_HUE_BINS }, () => ({ a: 0, b: 0, chroma: 0, count: 0 }));
  let opaque = 0;
  let colorful = 0;
  let sumChroma = 0;
  let sumLightness = 0;
  let counted = 0;
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] < TINT_MIN_ALPHA) continue;
    opaque += 1;
    const { lightness, a, b } = srgbToOklab(data[i], data[i + 1], data[i + 2]);
    sumLightness += lightness;
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
  if (opaque === 0) return { colors: [], strength: 0, lightness: null };
  const lightness = sumLightness / opaque;
  if (colorful / opaque < TINT_COLORFUL_MIN_SHARE) return { colors: [], strength: 0, lightness };

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
  const strength = Math.min(TINT_MAX_STRENGTH, Math.max(TINT_MIN_STRENGTH, sumChroma / counted / TINT_TYPICAL_CHROMA));
  return { colors, strength, lightness };
}

const arts = new Map<string, Promise<ArtTint | null>>();

/** The colors of a sample image, read once. */
export function artOfImage(src: string): Promise<ArtTint | null> {
  let art = arts.get(src);
  if (!art) {
    art = readPixels(src).then((data) => (data ? artFromPixels(data) : null));
    arts.set(src, art);
  }
  return art;
}
