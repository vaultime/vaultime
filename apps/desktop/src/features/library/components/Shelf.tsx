// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Link } from "react-router";
import { Cover } from "@/components/media/Cover";
import type { GameSummary } from "@/features/library/library-context";
import { formatHoursShort, formatRelativeDay } from "@/lib/time";
import { cn } from "@/lib/utils";

// Only as many covers as fit in one row. The columns grow with the page.
const VISIBLE = ["", "", "", "", "hidden @4xl:flex", "hidden @5xl:flex", "hidden @6xl:flex"];

/** The games played recently, one row of covers. */
export function Shelf({
  games,
  playing,
  totalCount,
  onShowAll,
}: {
  games: GameSummary[];
  playing: Set<string>;
  totalCount: number;
  onShowAll: () => void;
}) {
  return (
    <section aria-labelledby="shelf-title" className="@container px-8 pt-8 xl:px-14">
      <div className="flex items-baseline justify-between">
        <h2 id="shelf-title" className="font-display text-[34px] font-normal">
          Continue playing
        </h2>
        <button
          type="button"
          onClick={onShowAll}
          className="text-sm text-violet transition-colors hover:text-text focus-visible:underline focus-visible:outline-none"
        >
          All {totalCount} games
        </button>
      </div>
      <ul className="mt-5 grid grid-cols-4 gap-5 @4xl:grid-cols-5 @5xl:grid-cols-6 @6xl:grid-cols-7">
        {games.slice(0, VISIBLE.length).map((summary, index) => (
          <li key={summary.game.id} className={cn("flex", VISIBLE[index])}>
            <Link
              to={`/library/${summary.game.id}`}
              className="group flex w-full min-w-0 flex-col gap-2.5 rounded-md focus-visible:ring-2 focus-visible:ring-violet/70 focus-visible:ring-offset-4 focus-visible:ring-offset-ink focus-visible:outline-none"
            >
              <Cover
                title={summary.game.title}
                src={summary.cover}
                variant="card"
                className="aspect-[3/4] w-full p-3.5 text-2xl transition-transform duration-300 group-hover:-translate-y-1"
              />
              <span className="flex items-baseline justify-between gap-2 text-[13px] text-faint">
                <span className={cn("truncate", playing.has(summary.game.id) && "text-violet")}>
                  {playing.has(summary.game.id)
                    ? "Playing now"
                    : summary.lastPlayedAt
                      ? formatRelativeDay(summary.lastPlayedAt)
                      : ""}
                </span>
                <span className="shrink-0 tabular-nums">{formatHoursShort(summary.runtimeMs)}</span>
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}
