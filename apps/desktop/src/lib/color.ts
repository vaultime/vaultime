// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// OKLab and OKLCH math, see https://bottosson.github.io/posts/oklab/

import { GAMUT_SEARCH_MAX_CHROMA, GAMUT_SEARCH_STEPS } from "@/lib/constants";

/** A color by OKLCH lightness, chroma and hue in degrees. */
export interface Oklch {
  lightness: number;
  chroma: number;
  hue: number;
}

const RADIANS_PER_DEGREE = Math.PI / 180;

function toLinear(byte: number): number {
  const c = byte / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function toGamma(linear: number): number {
  return linear <= 0.0031308 ? linear * 12.92 : 1.055 * linear ** (1 / 2.4) - 0.055;
}

/** sRGB bytes to OKLab. */
export function srgbToOklab(red: number, green: number, blue: number) {
  const r = toLinear(red);
  const g = toLinear(green);
  const b = toLinear(blue);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return {
    lightness: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

/** OKLCH to linear sRGB channels, outside 0 to 1 when the color does not fit. */
function toLinearSrgb({ lightness, chroma, hue }: Oklch): [number, number, number] {
  const a = chroma * Math.cos(hue * RADIANS_PER_DEGREE);
  const b = chroma * Math.sin(hue * RADIANS_PER_DEGREE);
  const l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}

/** Whether an OKLCH color fits inside sRGB. */
export function insideSrgb(color: Oklch): boolean {
  return toLinearSrgb(color).every((channel) => channel >= 0 && channel <= 1);
}

/** The most chroma a lightness and hue can have inside sRGB. */
export function maxSrgbChroma(lightness: number, hue: number): number {
  let inside = 0;
  let outside = GAMUT_SEARCH_MAX_CHROMA;
  for (let step = 0; step < GAMUT_SEARCH_STEPS; step += 1) {
    const middle = (inside + outside) / 2;
    if (insideSrgb({ lightness, chroma: middle, hue })) inside = middle;
    else outside = middle;
  }
  return inside;
}

/** `#rrggbb` to OKLCH, null for anything else. */
export function hexToOklch(hex: string): Oklch | null {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  if (!match) return null;
  const [red, green, blue] = match.slice(1).map((pair) => parseInt(pair, 16));
  const { lightness, a, b } = srgbToOklab(red, green, blue);
  const hue = (Math.atan2(b, a) / RADIANS_PER_DEGREE + 360) % 360;
  return { lightness, chroma: Math.hypot(a, b), hue };
}

/** OKLCH to `#rrggbb`, clipped into sRGB. */
export function oklchToHex(color: Oklch): string {
  return `#${toLinearSrgb(color)
    .map((channel) => Math.round(toGamma(Math.min(1, Math.max(0, channel))) * 255))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")}`;
}

/** Relative luminance as WCAG defines it, of the color clipped into sRGB. */
function luminance(color: Oklch): number {
  const [r, g, b] = toLinearSrgb(color).map((channel) => Math.min(1, Math.max(0, channel)));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio of two colors, from 1 to 21. */
export function contrastRatio(first: Oklch, second: Oklch): number {
  const [light, dark] = [luminance(first), luminance(second)].sort((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}

/** A CSS `oklch()` value. */
export function formatOklch({ lightness, chroma, hue }: Oklch): string {
  return `oklch(${lightness.toFixed(3)} ${chroma.toFixed(3)} ${Math.round(hue)})`;
}
