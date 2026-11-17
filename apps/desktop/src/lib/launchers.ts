// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The library groups games by where they came from: a launcher, the folder
// scan, or by hand. A game counts for the launcher of every copy of it.

import { compareTitles } from "@/lib/game-sort";
import { FOLDER_SCAN, launcherName } from "@/lib/links";
import type { Game, GameLink } from "@/lib/types";

/** The launcher of games added by hand, which name none. */
export const BY_HAND = "manual";

/** A game's launcher as the library groups it: its launcher source, or `BY_HAND`. */
export function launcherKey(source: string | null): string {
  return source ?? BY_HAND;
}

/** "Steam", "Found in folders" or "Added by hand". */
export function launcherTitle(launcher: string): string {
  if (launcher === FOLDER_SCAN) return "Found in folders";
  if (launcher === BY_HAND) return "Added by hand";
  return launcherName(launcher) ?? launcher;
}

/** The launchers of a game and of the games that count as it, here or on another PC, each once. */
export function launchersOfGame(game: Game, links: GameLink[]): string[] {
  const linked = links.filter((link) => link.linked_game_id === game.id).map((link) => link.launcher_source);
  return [...new Set([game.launcher_source, ...linked].map(launcherKey))];
}

/** Each launcher with how many games count for it, most first, with the folder scan and games added by hand last. */
export function launcherCounts(games: { launchers: string[] }[]): { launcher: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const game of games) {
    for (const launcher of game.launchers) counts.set(launcher, (counts.get(launcher) ?? 0) + 1);
  }
  const notALauncher = (launcher: string) => Number(launcher === FOLDER_SCAN || launcher === BY_HAND);
  return [...counts]
    .map(([launcher, count]) => ({ launcher, count }))
    .sort(
      (a, b) =>
        notALauncher(a.launcher) - notALauncher(b.launcher) ||
        b.count - a.count ||
        compareTitles(launcherTitle(a.launcher), launcherTitle(b.launcher)),
    );
}
