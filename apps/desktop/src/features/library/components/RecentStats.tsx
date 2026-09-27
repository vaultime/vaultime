// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import type { GameSummary } from "@/features/library/library-context";
import { RECENT_DAYS } from "@/lib/constants";
import type { RecentPlay } from "@/lib/session-stats";
import { formatDayPart, formatHoursMinutes } from "@/lib/time";
import { numberWords } from "@/lib/words";
import { cn } from "@/lib/utils";

// Hairlines and padding per cell, for two columns and for four from lg on.
const CELL_BORDERS = [
  "border-r border-b lg:border-b-0",
  "border-b pl-6 lg:border-b-0 lg:border-r",
  "border-r lg:pl-6",
  "pl-6",
];

/** Four numbers for the last seven days, separated by hairlines. */
export function RecentStats({ recent, summaries }: { recent: RecentPlay; summaries: GameSummary[] }) {
  const gamesCount = recent.runtimeByGame.size;
  const longestTitle = recent.longest
    ? summaries.find((summary) => summary.game.id === recent.longest?.game_id)?.game.title
    : undefined;
  const activeShare = recent.runtimeMs > 0 ? Math.round((recent.activeMs / recent.runtimeMs) * 100) : 0;

  const stats = [
    {
      label: `Past ${RECENT_DAYS} days`,
      value: formatHoursMinutes(recent.runtimeMs),
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

  return (
    <section aria-label={`Past ${RECENT_DAYS} days`} className="grid grid-cols-2 border-b border-rule px-8 lg:grid-cols-4 xl:px-14">
      {stats.map((stat, index) => (
        <div
          key={stat.label}
          className={cn("min-w-0 border-rule py-6 pr-6", CELL_BORDERS[index])}
        >
          <div className="label-caps">{stat.label}</div>
          <div
            className={cn(
              "mt-2.5 font-mono text-[clamp(22px,2.3vw,32px)] tracking-[-0.02em] tabular-nums",
              stat.accent && "text-violet",
            )}
          >
            {stat.value}
          </div>
          <div className="mt-1.5 truncate text-[13px] text-faint">{stat.note}</div>
        </div>
      ))}
    </section>
  );
}
