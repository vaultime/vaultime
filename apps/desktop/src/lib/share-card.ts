// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// What the year card shows and where it goes. The numbers come from the stats
// page, so the card counts play exactly as the page does. Drawing lives in
// features/stats/share-card-draw.ts.

import { parseOklchList, type Oklch } from "@/lib/color";
import { DAYS_PER_WEEK, MINUTE_MS, SHARE_CALENDAR_CELL_SHARE, SHARE_FORMATS, SHARE_TOP_GAMES } from "@/lib/constants";
import { markColors, type ArtColor } from "@/lib/game-tint";
import type { Phrase } from "@/lib/sentences";
import { toDayKey } from "@/lib/session-stats";
import { daysSoFar, heatLevel, type YearStats } from "@/lib/stats";
import type { ThemeMode } from "@/lib/theme";
import { startOfDay, weekdayOf } from "@/lib/time";
import { stableHash } from "@/lib/utils";
import { shareSentence } from "@/lib/year-sentences";

export type ShareFormat = keyof typeof SHARE_FORMATS;
export const SHARE_FORMAT_IDS = Object.keys(SHARE_FORMATS) as ShareFormat[];

/** What the player picked in the share dialog. */
export interface ShareOptions {
  format: ShareFormat;
  /** Game titles and covers on the card. Off, the card names no game. */
  showGames: boolean;
  showCalendar: boolean;
  showPcName: boolean;
  mode: ThemeMode;
}

/** What the card may know about a game. */
export interface ShareGameInfo {
  title: string;
  hidden: boolean;
  cover: string | null;
  /** The main colors of its art, as its tint has them. */
  colors: ArtColor[];
}

export interface ShareGame {
  gameId: string;
  /** Null when titles and covers stay off the card. */
  title: string | null;
  cover: string | null;
  activeMs: number;
  /** Share of the year's active time, from 0 to 1. */
  share: number;
  /** Its journal mark, one color or two for art with two. */
  marks: Oklch[];
  /** Hue of a typeset cover for a game without art, null for grey art. */
  hue: number | null;
}

export interface ShareCard {
  year: number;
  /** This year, which may still get more play. */
  current: boolean;
  pcName: string | null;
  activeMs: number;
  /** Time with a game running, games side by side counted once, as the stats page's Played. */
  playedMs: number;
  idleMs: number;
  daysPlayed: number;
  /** Days of the year so far, all of them for a past year. */
  daysOfYear: number;
  /** Games played in the year, hidden ones included. */
  gamesCount: number;
  longestStreakDays: number;
  longestSession: { runtimeMs: number; title: string | null } | null;
  /** January is 0. */
  busiestMonth: { month: number; activeMs: number } | null;
  /** The most active games first, as many as the largest format names, never a hidden one. */
  games: ShareGame[];
  /** Share of the year's active time of the game played most, hidden or not. */
  topShare: number;
  /** Heat level of every day from 1 January, -1 for days still to come. */
  days: number[];
  sentence: Phrase;
}

/** The most games any format names. */
const MOST_GAMES = Math.max(...Object.values(SHARE_TOP_GAMES));

/**
 * The card for a year of the stats page, null for a year without play.
 * Hidden games count in the totals, as everywhere, but are never named or
 * shown, and the sentence names the game played most only when it is shown.
 */
export function buildShareCard({
  stats,
  gameInfo,
  pcName,
  accentHues,
  options,
  now = new Date(),
}: {
  stats: YearStats;
  gameInfo: (gameId: string) => ShareGameInfo | undefined;
  pcName: string | null;
  accentHues: number[];
  options: Pick<ShareOptions, "showGames" | "showPcName" | "mode">;
  now?: Date;
}): ShareCard | null {
  if (stats.runtimeMs < MINUTE_MS) return null;
  const { year } = stats;

  // Marks as the stats page gives them, so a game keeps its color.
  const marks = markColors(
    stats.games.map((game) => gameInfo(game.gameId)?.colors ?? []),
    accentHues,
    options.mode,
  );
  const markOf = new Map(stats.games.map((game, index) => [game.gameId, parseOklchList(marks[index].fill)]));

  const byActive = [...stats.games].sort((a, b) => b.activeMs - a.activeMs || b.runtimeMs - a.runtimeMs);
  const shown = (gameId: string) => {
    const info = gameInfo(gameId);
    return info !== undefined && !info.hidden;
  };
  const games: ShareGame[] = byActive
    .filter((game) => game.activeMs > 0 && shown(game.gameId))
    .slice(0, MOST_GAMES)
    .map((game) => {
      const info = gameInfo(game.gameId)!;
      return {
        gameId: game.gameId,
        title: options.showGames ? info.title : null,
        cover: options.showGames ? info.cover : null,
        activeMs: game.activeMs,
        share: stats.activeMs > 0 ? game.activeMs / stats.activeMs : 0,
        marks: markOf.get(game.gameId) ?? [],
        hue: info.colors[0]?.hue ?? (info.cover ? null : stableHash(info.title) % 360),
      };
    });

  const top = byActive[0];
  const topShown = top && options.showGames && shown(top.gameId) ? gameInfo(top.gameId)!.title : null;
  const longest = stats.longest;
  const longestTitle =
    longest && options.showGames && shown(longest.game_id) ? gameInfo(longest.game_id)!.title : null;
  const busiest = stats.months.reduce(
    (best, month, index) => (month.activeMs > (best?.activeMs ?? 0) ? { month: index, activeMs: month.activeMs } : best),
    null as { month: number; activeMs: number } | null,
  );

  return {
    year,
    current: year === now.getFullYear(),
    pcName: options.showPcName && pcName?.trim() ? pcName.trim() : null,
    activeMs: stats.activeMs,
    playedMs: stats.playedMs,
    idleMs: stats.idleMs,
    daysPlayed: stats.daysPlayed,
    daysOfYear: daysSoFar(year, now),
    gamesCount: stats.games.length,
    longestStreakDays: stats.longestStreak?.days ?? 0,
    longestSession: longest ? { runtimeMs: longest.runtime_ms, title: longestTitle } : null,
    busiestMonth: busiest && busiest.activeMs >= MINUTE_MS ? busiest : null,
    games,
    topShare: top && stats.activeMs > 0 ? top.activeMs / stats.activeMs : 0,
    days: yearDays(year).map((day) =>
      day > now ? -1 : heatLevel(stats.days.get(toDayKey(day))?.activeMs ?? 0),
    ),
    sentence: shareSentence({
      year,
      topTitle: topShown,
      gamesCount: stats.games.length,
      daysPlayed: stats.daysPlayed,
      weekClock: stats.weekClock,
    }),
  };
}

/** The games a format names. */
export function gamesFor(card: ShareCard, format: ShareFormat): ShareGame[] {
  return card.games.slice(0, SHARE_TOP_GAMES[format]);
}

/** Every day of `year`, 1 January first, at local midnight. */
function yearDays(year: number): Date[] {
  const days: Date[] = [];
  for (let day = new Date(year, 0, 1); day.getFullYear() === year; day = startOfDay(day, 1)) {
    days.push(day);
  }
  return days;
}

/** Where the days of a year sit in a calendar of weeks, Monday on top, and how large a day is. */
export interface CalendarGrid {
  columns: number;
  /** Distance from one day to the next, in pixels. */
  pitch: number;
  /** Side of a day's square. */
  cell: number;
  height: number;
  /** Column and row of each day, 1 January first. */
  places: { column: number; row: number }[];
  /** Column of the week that holds the first of each month, January first. */
  monthColumns: number[];
}

/** A year of days in weeks across `width` pixels. */
export function calendarGrid(year: number, width: number): CalendarGrid {
  const lead = weekdayOf(new Date(year, 0, 1));
  const days = yearDays(year);
  const places = days.map((_, index) => {
    const slot = index + lead;
    return { column: Math.floor(slot / DAYS_PER_WEEK), row: slot % DAYS_PER_WEEK };
  });
  const columns = places[places.length - 1].column + 1;
  const pitch = width / columns;
  const cell = pitch * SHARE_CALENDAR_CELL_SHARE;
  const monthColumns = days.flatMap((day, index) => (day.getDate() === 1 ? [places[index].column] : []));
  return { columns, pitch, cell, height: (DAYS_PER_WEEK - 1) * pitch + cell, places, monthColumns };
}

/** A block of a column of the card: its height, and whether it grows into space left over. */
export interface StackBlock {
  height: number;
  /** Most it may grow to. Blocks without one keep their height. */
  maxHeight?: number;
}

/**
 * Tops and heights of blocks stacked from `top` to `bottom` with at least
 * `minGap` between them. Space left over first lets growing blocks grow, up
 * to their largest size. The rest spreads evenly over the gaps, or stays
 * below the blocks, or splits above and below them.
 */
export function stackBlocks(
  blocks: StackBlock[],
  top: number,
  bottom: number,
  minGap: number,
  align: "spread" | "start" | "center" = "spread",
) {
  const gaps = Math.max(blocks.length - 1, 0);
  const fixed = blocks.reduce((sum, block) => sum + block.height, 0) + gaps * minGap;
  let spare = Math.max(0, bottom - top - fixed);
  const heights = blocks.map((block) => {
    const room = Math.max(0, (block.maxHeight ?? block.height) - block.height);
    const grow = Math.min(room, spare);
    spare -= grow;
    return block.height + grow;
  });
  const spread = align === "spread" && gaps > 0;
  const gap = minGap + (spread ? spare / gaps : 0);
  let y = spread || align === "start" ? top : top + spare / 2;
  return heights.map((height) => {
    const placed = { top: y, height };
    y += height + gap;
    return placed;
  });
}

/** How far the preview of a format scales down to fit a box, never up. */
export function previewScale(format: ShareFormat, boxWidth: number, boxHeight: number): number {
  const { width, height } = SHARE_FORMATS[format];
  return Math.max(0, Math.min(1, boxWidth / width, boxHeight / height));
}

/** The file name a card is saved under. */
export function shareFileName(year: number): string {
  return `vaultime-${year}.png`;
}
