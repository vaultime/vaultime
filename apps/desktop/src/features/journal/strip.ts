// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { HOURS_PER_DAY, JOURNAL_MIN_SPAN_PERCENT, JOURNAL_TICK_HOURS } from "@/lib/constants";
import { clockPercent } from "@/lib/time";

/** Width of one colored stripe where games ran side by side. */
const HATCH_STRIPE_PX = 3;
/** Dark gap between the stripes, so games with similar colors still read as stripes. */
const HATCH_GAP_PX = 1.5;
/** Half the strip's height. Pieces of a block lean by it at both ends, along the stripes. */
const HATCH_LEAN_PX = 5;
/** How far a piece reaches under the next one, so no hairline shows between them. */
const PIECE_OVERLAP_PX = 1;
/** How far play that goes on past midnight runs out of the strip, fading. */
export const BLEED_PX = 14;
/** A two colored blend stretched far past a fading end, so only its color on that side shows. */
export const BLEND_STRETCH = "2000% 100%";
/** Gap between a marked session and the outline around it. */
export const OUTLINE_GAP_PX = 3;
/** How far a strip part reaches above and below the strip for the pointer. */
export const HIT_SLOP_PX = 6;

/** The hours written under a day's strip. */
export const TICKS = Array.from(
  { length: HOURS_PER_DAY / JOURNAL_TICK_HOURS + 1 },
  (_, index) => index * JOURNAL_TICK_HOURS,
);

/** Diagonal stripes in the colors of games that ran side by side, one color per stripe in turn. */
export function hatch(colors: string[]): string {
  const step = HATCH_STRIPE_PX + HATCH_GAP_PX;
  const stripes = colors.map((color, index) => {
    const start = index * step;
    const gap = start + HATCH_STRIPE_PX;
    return `${color} ${start}px ${gap}px, var(--ink) ${gap}px ${start + step}px`;
  });
  return `repeating-linear-gradient(135deg, ${stripes.join(", ")})`;
}

/** Where something sits on the day's strip, in percent of the day. */
export interface Place {
  left: number;
  width: number;
}

export function placeOnDay(start: Date, end: Date, day: Date): Place {
  const left = clockPercent(start, day);
  const width = Math.max(JOURNAL_MIN_SPAN_PERCENT, clockPercent(end, day) - left);
  return { left: Math.min(left, 100 - width), width };
}

/**
 * Cuts `place` out of a layer that covers `box`, with ends that lean along
 * the stripes. An end at the end of its block stays upright.
 */
export function leaningClip(box: Place, place: Place, leanStart: boolean, leanEnd: boolean): string {
  const inBox = (at: number) => ((at - box.left) / box.width) * 100;
  const edge = (at: number, shiftPx: number) => (shiftPx ? `calc(${inBox(at)}% + ${shiftPx}px)` : `${inBox(at)}%`);
  const start = place.left;
  const end = place.left + place.width;
  const [startTop, startBottom] = leanStart ? [HATCH_LEAN_PX, -HATCH_LEAN_PX] : [0, 0];
  const [endTop, endBottom] = leanEnd
    ? [HATCH_LEAN_PX + PIECE_OVERLAP_PX, -HATCH_LEAN_PX + PIECE_OVERLAP_PX]
    : [0, 0];
  return `polygon(${edge(start, startTop)} 0, ${edge(end, endTop)} 0, ${edge(end, endBottom)} 100%, ${edge(start, startBottom)} 100%)`;
}
