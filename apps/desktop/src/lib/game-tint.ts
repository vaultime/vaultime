// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

export interface GameTint {
  /** Dark field for covers and tiles. */
  fill: string;
  /** Border of that field. */
  edge: string;
  /** Light text on the field. */
  ink: string;
  /** Very dark wash for page headers. */
  wash: string;
}

/** A stable tint per title, used until the cover art provides colors. */
export function tintForTitle(title: string): GameTint {
  let hash = 0;
  for (const char of title) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  const hue = hash % 360;
  return {
    fill: `oklch(0.3 0.06 ${hue})`,
    edge: `oklch(0.38 0.07 ${hue})`,
    ink: `oklch(0.92 0.05 ${hue})`,
    wash: `oklch(0.18 0.035 ${hue})`,
  };
}
