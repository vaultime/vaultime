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

/** Titles in the order of the user's region, case and accents aside, "Game 9" before "Game 10". */
const TITLE_ORDER = new Intl.Collator(UI_LOCALE, { sensitivity: "base", numeric: true });

interface Sortable {
  game: { title: string };
  totalMs: number;
}

/** Games come most recently played first, so `recent` keeps their order. */
export function sortGames<T extends Sortable>(games: T[], sort: GameSort): T[] {
  if (sort === "recent") return games;
  const copy = [...games];
  if (sort === "title") copy.sort((a, b) => TITLE_ORDER.compare(a.game.title, b.game.title));
  if (sort === "played") copy.sort((a, b) => b.totalMs - a.totalMs);
  return copy;
}
