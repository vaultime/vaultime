// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Games that count as another game: another install of it on this PC, like a
// copy from a second launcher, or the game as another PC recorded it. The
// core links them and shows their sessions as the game they count as. This
// file names them.

import type { Game, GameLink, Session } from "@/lib/types";

/** What each launcher is called, by the source the core reports. */
const LAUNCHER_NAMES: Record<string, string> = {
  steam: "Steam",
  epic: "Epic Games",
  battlenet: "Battle.net",
  riot: "Riot",
  hoyoplay: "HoYoPlay",
  ubisoft: "Ubisoft Connect",
  ea: "EA app",
  rockstar: "Rockstar",
  xbox: "Xbox",
  amazon: "Amazon Games",
  itch: "itch.io",
  gog: "GOG",
  heroic: "Heroic",
  lutris: "Lutris",
};

/** Found by the folder scan rather than read from a launcher. */
const FOLDER_SCAN = "folder_scan";

/** "Steam", or null for a game found in a folder or added by hand. */
export function launcherName(source: string | null | undefined): string | null {
  return source ? (LAUNCHER_NAMES[source] ?? null) : null;
}

/** "on Steam", "found in a folder" or "added by hand", to follow a title. */
export function whereFrom(source: string | null | undefined): string {
  const name = launcherName(source);
  if (name) return `on ${name}`;
  return source === FOLDER_SCAN ? "found in a folder" : "added by hand";
}

/** Whether a linked game is another install on this PC rather than a game of another PC. */
export function isInstallHere(link: GameLink, thisDeviceId: string | null): boolean {
  return link.origin_device_id === null || link.origin_device_id === thisDeviceId;
}

/** The games that count as `gameId`: other installs on this PC, and games of other PCs. */
export function linkedTo(
  gameId: string,
  links: GameLink[],
  thisDeviceId: string | null,
): { installs: GameLink[]; elsewhere: GameLink[] } {
  const linked = links.filter((link) => link.linked_game_id === gameId);
  return {
    installs: linked.filter((link) => isInstallHere(link, thisDeviceId)),
    elsewhere: linked.filter((link) => !isInstallHere(link, thisDeviceId)),
  };
}

/** "Steam and GOG": the launchers of a game and of its other installs here, or null when none has one. */
export function launchersOf(game: Game, links: GameLink[], thisDeviceId: string | null): string | null {
  const { installs } = linkedTo(game.id, links, thisDeviceId);
  const names = [...new Set([game.launcher_source, ...installs.map((link) => link.launcher_source)].map(launcherName))].filter(
    (name): name is string => name !== null,
  );
  if (names.length === 0) return null;
  return names.length > 1 ? `${names.slice(0, -1).join(", ")} and ${names.at(-1)}` : names[0];
}

/** Games of this PC a game can count as: every other one the library shows. */
export function linkTargets(game: Game, games: Game[]): Game[] {
  return games.filter((candidate) => candidate.origin_device_id === null && candidate.id !== game.id);
}

/** The programs of a game and of its other installs here, each once with its title. */
export function programsOf(
  game: Game,
  links: GameLink[],
  thisDeviceId: string | null,
): { path: string; title: string }[] {
  const { installs } = linkedTo(game.id, links, thisDeviceId);
  const programs = [game, ...installs].flatMap((install) =>
    install.executable_path ? [{ path: install.executable_path, title: install.title }] : [],
  );
  return programs.filter((program, index) => programs.findIndex((other) => other.path === program.path) === index);
}

/** What deleting a game takes along besides it, as a sentence, or nothing. */
export function linkedGoToo(count: number): string {
  if (count <= 0) return "";
  if (count === 1) return "The game linked to it goes too, with its sessions. Unlink it first to keep it.";
  return `The ${count} games linked to it go too, with their sessions. Unlink them first to keep them.`;
}

/** The launcher of the other install here that recorded a session, when one did. */
export function recordedThrough(session: Session, links: GameLink[], thisDeviceId: string | null): string | null {
  if (!session.recorded_game_id) return null;
  const link = links.find((candidate) => candidate.game_id === session.recorded_game_id);
  if (!link || !isInstallHere(link, thisDeviceId)) return null;
  return launcherName(link.launcher_source) ?? (link.launcher_source === FOLDER_SCAN ? "a copy found in a folder" : "a copy added by hand");
}
