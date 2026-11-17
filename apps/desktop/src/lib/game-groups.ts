// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// All games can show its games in groups: by launcher, by when they were
// last played, by status or by PC. A game with copies from two launchers or
// on two PCs shows in both groups.

import { DAY_MS, PLAYED_GROUP_MONTH_DAYS, PLAYED_GROUP_WEEK_DAYS, PLAYED_GROUP_YEAR_DAYS } from "@/lib/constants";
import { pcName } from "@/lib/devices";
import { compareTitles } from "@/lib/game-sort";
import { GAME_STATUS_LABELS, GAME_STATUSES } from "@/lib/game-status";
import { launcherCounts, launcherTitle } from "@/lib/launchers";
import { parseVaultimeDate } from "@/lib/time";
import type { Device, Game, GameLink, GameStatus } from "@/lib/types";

export const GAME_GROUPS = {
  none: "None",
  launcher: "Launcher",
  played: "Last played",
  status: "Status",
  pc: "PC",
} as const;

export type GameGroup = keyof typeof GAME_GROUPS;

export function isGameGroup(value: unknown): value is GameGroup {
  return typeof value === "string" && Object.hasOwn(GAME_GROUPS, value);
}

/** Games of one group, in the order they came. */
export interface GroupOfGames<T> {
  key: string;
  /** Empty for the one group of games shown without groups. */
  title: string;
  games: T[];
}

/** What grouping needs to know besides the games. */
export interface GroupContext {
  now: number;
  links: GameLink[];
  devices: Device[];
  thisDeviceId: string | null;
}

interface Groupable {
  game: Game;
  launchers: string[];
  status: GameStatus | null;
  lastPlayedAt: string | null;
}

/** The group of games without a status. */
const NO_STATUS = "none";
/** This PC while its id is not known yet. */
const THIS_PC = "this-pc";

/** The "Last played" groups, newest first, each reaching back its days. */
const PLAYED = [
  { key: "week", title: "Past week", days: PLAYED_GROUP_WEEK_DAYS },
  { key: "month", title: "Past month", days: PLAYED_GROUP_MONTH_DAYS },
  { key: "year", title: "Past year", days: PLAYED_GROUP_YEAR_DAYS },
  { key: "earlier", title: "Longer ago", days: Infinity },
];
const NOT_PLAYED = { key: "never", title: "Not played yet" };

function playedKey(lastPlayedAt: string | null, now: number): string {
  if (!lastPlayedAt) return NOT_PLAYED.key;
  const ago = now - parseVaultimeDate(lastPlayedAt).getTime();
  return (PLAYED.find((group) => ago < group.days * DAY_MS) ?? PLAYED[PLAYED.length - 1]).key;
}

/** The PCs with a copy of a game: its own and those of the games that count as it. */
function pcsOf(game: Game, { links, thisDeviceId }: GroupContext): string[] {
  const here = thisDeviceId ?? THIS_PC;
  const linked = links.filter((link) => link.linked_game_id === game.id).map((link) => link.origin_device_id ?? here);
  return [...new Set([game.origin_device_id ?? here, ...linked])];
}

/** The games in groups in the order the grouping reads, each game in every group it belongs to, keeping their order. */
export function groupGames<T extends Groupable>(games: T[], group: GameGroup, context: GroupContext): GroupOfGames<T>[] {
  if (group === "none") return [{ key: "all", title: "", games }];

  const keysOf = (game: T): string[] => {
    if (group === "launcher") return game.launchers;
    if (group === "status") return [game.status ?? NO_STATUS];
    if (group === "played") return [playedKey(game.lastPlayedAt, context.now)];
    return pcsOf(game.game, context);
  };
  const byKey = new Map<string, T[]>();
  for (const game of games) {
    for (const key of keysOf(game)) byKey.set(key, [...(byKey.get(key) ?? []), game]);
  }

  const here = context.thisDeviceId ?? THIS_PC;
  const titleOf = (key: string): string => {
    if (group === "launcher") return launcherTitle(key);
    if (group === "status") return key === NO_STATUS ? "No status" : GAME_STATUS_LABELS[key as GameStatus];
    if (group === "played") return [...PLAYED, NOT_PLAYED].find((entry) => entry.key === key)?.title ?? key;
    return key === here ? "This PC" : pcName(context.devices, key);
  };
  const order: string[] =
    group === "launcher"
      ? launcherCounts(games).map((entry) => entry.launcher)
      : group === "status"
        ? [...GAME_STATUSES, NO_STATUS]
        : group === "played"
          ? [...PLAYED, NOT_PLAYED].map((entry) => entry.key)
          : [here, ...[...byKey.keys()].filter((key) => key !== here).sort((a, b) => compareTitles(titleOf(a), titleOf(b)))];

  return order.flatMap((key) => {
    const members = byKey.get(key);
    return members ? [{ key, title: titleOf(key), games: members }] : [];
  });
}
