// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { DiscoveredGame, NewGames } from "@/lib/types";

/** Paths of the games a discovery list offers, which the look at start then never offers again. */
export function offeredPaths(games: DiscoveredGame[]): string[] {
  return games.filter((game) => !game.already_added).map((game) => game.executable_path);
}

/** Whether a dialog that comes from the address, or the palette, is open, so found games wait. */
export function otherDialogOpen(search: URLSearchParams, paletteOpen: boolean): boolean {
  return paletteOpen || search.get("discover") === "1" || search.get("add") === "1";
}

/** Heading and sentence of the discover dialog when it offers the games found at start. */
export function foundGamesText(found: NewGames): { title: string; description: string } {
  const many = found.games.length !== 1;
  return {
    title: many ? "New games found" : "A new game found",
    description: found.since_last
      ? `Vaultime looked through your launchers as it started and found ${many ? "these" : "this one"} since last time.`
      : `Vaultime looked through your launchers as it started. ${many ? "These games are" : "This game is"} installed but not in your library yet.`,
  };
}
