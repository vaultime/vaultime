// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { UI_LOCALE } from "@/lib/time";

export const GAME_SORTS = {
  recent: "Recent",
  title: "A to Z",
  played: "Most played",
} as const;

export type GameSort = keyof typeof GAME_SORTS;

export function isGameSort(value: unknown): value is GameSort {
  return typeof value === "string" && Object.hasOwn(GAME_SORTS, value);
}

interface Sortable {
  game: { title: string };
  totalMs: number;
}

/** Games come most recently played first, so `recent` keeps their order. */
export function sortGames<T extends Sortable>(games: T[], sort: GameSort): T[] {
  if (sort === "recent") return games;
  const copy = [...games];
  if (sort === "title") {
    copy.sort((a, b) => a.game.title.localeCompare(b.game.title, UI_LOCALE, { sensitivity: "base", numeric: true }));
  }
  if (sort === "played") copy.sort((a, b) => b.totalMs - a.totalMs);
  return copy;
}
