// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { STILL_BLUR_SMALL_RADIUS_PX, STILL_BLUR_STORAGE_KEY } from "@/lib/constants";

let still: boolean | undefined;

/**
 * Whether blurs are drawn once into a canvas instead of worked out live, which
 * a computer without a graphics chip does with every repaint. A switch until
 * the look is agreed, read once.
 */
export function stillBlur(): boolean {
  if (still === undefined) {
    try {
      still = localStorage.getItem(STILL_BLUR_STORAGE_KEY) === "still";
    } catch {
      still = false;
    }
  }
  return still;
}

type Size = { width: number; height: number };

/** The small canvas a still blur draws into and the blur in its pixels. */
export function stillBlurSize(box: Size, blur: number, pixelRatio: number) {
  // So small that the blur spans a few of its pixels, sharp without one.
  const scale = blur > 0 ? Math.min(1, STILL_BLUR_SMALL_RADIUS_PX / blur) : pixelRatio;
  return {
    width: Math.max(1, Math.round(box.width * scale)),
    height: Math.max(1, Math.round(box.height * scale)),
    radius: blur * scale,
  };
}

/**
 * Where an image goes to cover a box the way `object-cover` does, centered,
 * reaching `margin` past each edge.
 */
export function coverRect(box: Size, image: Size, margin: number) {
  const scale = Math.max((box.width + margin * 2) / image.width, (box.height + margin * 2) / image.height);
  const width = image.width * scale;
  const height = image.height * scale;
  return { x: (box.width - width) / 2, y: (box.height - height) / 2, width, height };
}
