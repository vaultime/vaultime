// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { CSSProperties } from "react";
import { MINUTE_MS, PLAY_CARD_GAMES } from "@/lib/constants";
import type { BucketPlay } from "@/lib/play-totals";
import { formatHoursMinutes } from "@/lib/time";

/** Width of a hover card. */
export const CARD_WIDTH_PX = 240;
/** Space between a card and what it describes. */
const CARD_GAP_PX = 8;

/** A game's name and its color in the chart. */
export interface GameLabel {
  title: string;
  color: string;
}

/** A game's active time in a card, "idle" when it only sat idle, or less than a minute. */
export function gameTime(game: { activeMs: number; idleMs: number }): string {
  if (game.activeMs >= MINUTE_MS) return formatHoursMinutes(game.activeMs);
  if (game.idleMs >= MINUTE_MS) return "idle";
  return "under a minute";
}

/** "3 h 12 active, Elden Ring 2 h 05, Hades II 58 min", for screen readers. */
export function describePlay(label: string, play: BucketPlay | undefined, labelOf: (gameId: string) => GameLabel): string {
  if (!play || play.runtimeMs <= 0) return `${label}, not played`;
  const games = play.games
    .slice(0, PLAY_CARD_GAMES)
    .map((game) => `${labelOf(game.gameId).title} ${gameTime(game)}`);
  return `${label}, ${formatHoursMinutes(play.activeMs)} active, ${games.join(", ")}`;
}

/**
 * Where a card sits next to `anchor`, relative to `frame`, the element it is
 * placed in: on the side of the window with more room, never past the
 * frame's sides.
 */
export function cardPosition(anchor: DOMRect, frame: DOMRect): CSSProperties {
  const center = anchor.left + anchor.width / 2 - frame.left;
  const left = Math.min(Math.max(center - CARD_WIDTH_PX / 2, 0), Math.max(frame.width - CARD_WIDTH_PX, 0));
  return anchor.top + anchor.height / 2 > window.innerHeight / 2
    ? { left, bottom: frame.bottom - anchor.top + CARD_GAP_PX }
    : { left, top: anchor.bottom - frame.top + CARD_GAP_PX };
}
