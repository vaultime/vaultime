// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Link } from "react-router";
import { Cover } from "@/components/media/Cover";
import type { GameSummary } from "@/features/library/library-context";
import { JOURNAL_WEEK_BAR_MIN_SHARE, JOURNAL_WEEK_GAMES } from "@/lib/constants";
import { formatDate, formatHoursMinutes, parseVaultimeDate } from "@/lib/time";
import type { GameTime } from "@/lib/types";
import { numberWords } from "@/lib/words";

/** The games of a week with their covers and hours, the most played first, then its longest day. */
export function WeekGames({
  games,
  byGame,
  fillOf,
  longestDay,
  onDay,
}: {
  games: GameTime[];
  byGame: Map<string, GameSummary>;
  /** The game's mark, as its title and strip show it. */
  fillOf: (gameId: string) => string;
  longestDay: { day: string; playedMs: number } | null;
  onDay: (dayKey: string) => void;
}) {
  const shown = games.slice(0, JOURNAL_WEEK_GAMES);
  if (shown.length === 0) return null;
  const most = shown[0].runtime_ms;
  const more = games.length - shown.length;
  return (
    <div className="flex items-start gap-8 border-b border-rule pt-7 pb-6">
      <ul aria-label="Games this week" className="flex min-w-0 flex-1 gap-4 overflow-x-auto pb-1">
        {shown.map((time) => {
          const summary = byGame.get(time.game_id);
          const title = summary?.game.title ?? "";
          const share = most > 0 ? Math.max(JOURNAL_WEEK_BAR_MIN_SHARE, time.runtime_ms / most) : 1;
          return (
            <li key={time.game_id} className="w-[88px] shrink-0">
              <Link
                to={`/library/${time.game_id}`}
                className="group block rounded-md focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
              >
                <Cover title={title} src={summary?.cover} className="aspect-[2/3] w-full p-2 text-[11px]" />
                <span
                  aria-hidden="true"
                  className="mt-2 block h-[3px] rounded-full"
                  style={{ width: `${share * 100}%`, background: fillOf(time.game_id) }}
                />
                <span className="mt-1.5 block truncate text-[13px] text-soft transition-colors group-hover:text-text">
                  {title}
                </span>
                <span className="block text-xs text-faint tabular-nums">{formatHoursMinutes(time.runtime_ms)}</span>
              </Link>
            </li>
          );
        })}
        {more > 0 && <li className="self-center pl-1 text-[13px] text-faint">and {numberWords(more)} more</li>}
      </ul>
      {longestDay && (
        <button
          type="button"
          onClick={() => onDay(longestDay.day)}
          className="group shrink-0 cursor-pointer rounded-md border-l border-rule pl-8 text-left focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
        >
          <span className="label-small block">Longest day</span>
          <span className="font-display mt-1.5 block text-[18px] transition-colors group-hover:text-violet">
            {formatDate(parseVaultimeDate(longestDay.day), { weekday: "long" })}
          </span>
          <span className="block text-[13px] text-faint tabular-nums">{formatHoursMinutes(longestDay.playedMs)}</span>
        </button>
      )}
    </div>
  );
}
