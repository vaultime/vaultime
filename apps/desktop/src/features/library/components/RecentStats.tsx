// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { StatTiles, type StatTile } from "@/components/layout/StatTiles";
import type { GameSummary } from "@/features/library/library-context";
import { RECENT_DAYS } from "@/lib/constants";
import type { RecentPlay } from "@/lib/session-stats";
import { formatDayPart, formatHoursMinutes } from "@/lib/time";
import { numberWords } from "@/lib/words";

/** Four numbers for the last seven days, separated by hairlines. */
export function RecentStats({ recent, summaries }: { recent: RecentPlay; summaries: GameSummary[] }) {
  const gamesCount = recent.runtimeByGame.size;
  const longestTitle = recent.longest
    ? summaries.find((summary) => summary.game.id === recent.longest?.game_id)?.game.title
    : undefined;
  const activeShare = recent.runtimeMs > 0 ? Math.round((recent.activeMs / recent.runtimeMs) * 100) : 0;

  const stats: StatTile[] = [
    {
      label: `Past ${RECENT_DAYS} days`,
      value: formatHoursMinutes(recent.playedMs),
      note: gamesCount > 0 ? `across ${numberWords(gamesCount)} game${gamesCount === 1 ? "" : "s"}` : "Nothing played yet",
    },
    {
      label: "Active share",
      value: `${activeShare} %`,
      note: "of runtime, idle left out",
      accent: recent.runtimeMs > 0,
    },
    {
      label: "Sessions",
      value: String(recent.sessionsCount),
      note:
        recent.daysCount > 0
          ? `on ${numberWords(recent.daysCount)} day${recent.daysCount === 1 ? "" : "s"}`
          : "Start a game to add one",
    },
    {
      label: "Longest",
      value: recent.longest ? formatHoursMinutes(recent.longest.runtime_ms) : "0 min",
      note: recent.longest
        ? [formatDayPart(recent.longest.started_at_wall), longestTitle].filter(Boolean).join(", ")
        : "Nothing yet",
    },
  ];

  return <StatTiles label={`Past ${RECENT_DAYS} days`} tiles={stats} />;
}
