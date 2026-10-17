// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { StatTiles, type StatTile } from "@/components/layout/StatTiles";
import { useActiveSessions, type GameSummary } from "@/features/library/library-context";
import { RECENT_DAYS } from "@/lib/constants";
import { formatDayPart, formatHoursMinutes } from "@/lib/time";
import type { RecentPlay, Session } from "@/lib/types";
import { numberWords } from "@/lib/words";

/** Four numbers for the last `RECENT_DAYS` days, separated by hairlines. */
export function RecentStats({
  recent,
  summaries,
  sessions,
}: {
  recent: RecentPlay;
  summaries: GameSummary[];
  sessions: Session[];
}) {
  const gamesCount = recent.games.length;
  // A running session as of the last poll, as the stored list only reloads when one starts or ends.
  const active = useActiveSessions();
  const longest = [...active, ...sessions].find((session) => session.id === recent.longest_session_id) ?? null;
  const longestTitle = longest
    ? summaries.find((summary) => summary.game.id === longest.game_id)?.game.title
    : undefined;
  const activeShare = recent.runtime_ms > 0 ? Math.round((recent.active_ms / recent.runtime_ms) * 100) : 0;

  const stats: StatTile[] = [
    {
      label: `Past ${RECENT_DAYS} days`,
      value: formatHoursMinutes(recent.played_ms),
      note: gamesCount > 0 ? `across ${numberWords(gamesCount)} game${gamesCount === 1 ? "" : "s"}` : "Nothing played yet",
    },
    {
      label: "Active share",
      value: `${activeShare} %`,
      note: "of runtime, idle left out",
      accent: recent.runtime_ms > 0,
    },
    {
      label: "Sessions",
      value: String(recent.sessions_count),
      note:
        recent.days_count > 0
          ? `on ${numberWords(recent.days_count)} day${recent.days_count === 1 ? "" : "s"}`
          : "Start a game to add one",
    },
    {
      label: "Longest",
      value: longest ? formatHoursMinutes(longest.runtime_ms) : "0 min",
      note: longest
        ? [formatDayPart(longest.started_at_wall), longestTitle].filter(Boolean).join(", ")
        : "Nothing yet",
    },
  ];

  return <StatTiles label={`Past ${RECENT_DAYS} days`} tiles={stats} />;
}
