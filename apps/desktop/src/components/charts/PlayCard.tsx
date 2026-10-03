// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { CSSProperties } from "react";
import { MINUTE_MS, PLAY_CARD_GAMES } from "@/lib/constants";
import type { BucketPlay } from "@/lib/play-totals";
import { formatHoursMinutes } from "@/lib/time";
import { CARD_WIDTH_PX, gameTime, type GameLabel } from "./play-card";

/** What was played in a day or a month, each game in its color. */
export function PlayCard({
  label,
  play,
  labelOf,
  hint,
  style,
}: {
  label: string;
  play: BucketPlay | undefined;
  labelOf: (gameId: string) => GameLabel;
  /** A last line, like what a click does. */
  hint?: string;
  style?: CSSProperties;
}) {
  const shown = play?.games.slice(0, PLAY_CARD_GAMES) ?? [];
  const more = (play?.games.length ?? 0) - shown.length;
  return (
    <div
      aria-hidden="true"
      style={{ width: CARD_WIDTH_PX, ...style }}
      className="pointer-events-none absolute z-20 rounded-xl border border-hairline bg-surface px-4 py-3 shadow-lg shadow-scrim/40"
    >
      <div className="text-[11px] tracking-[0.12em] text-faint uppercase">{label}</div>
      {!play || play.runtimeMs <= 0 ? (
        <div className="mt-1.5 text-sm text-soft">Not played</div>
      ) : (
        <>
          <div className="mt-1 font-mono text-[15px] tabular-nums">{formatHoursMinutes(play.activeMs)} active</div>
          <ul className="mt-2.5 flex flex-col gap-1.5">
            {shown.map((game) => {
              const { title, color } = labelOf(game.gameId);
              return (
                <li key={game.gameId} className="flex items-center gap-2.5 text-[13px]">
                  <span className="size-2 shrink-0 rounded-full" style={{ background: color }} />
                  <span className="min-w-0 flex-1 truncate">{title}</span>
                  <span className="shrink-0 font-mono text-xs text-soft tabular-nums">
                    {gameTime(game)}
                  </span>
                </li>
              );
            })}
          </ul>
          {more > 0 && <div className="mt-1.5 text-xs text-faint">and {more} more</div>}
          {play.idleMs >= MINUTE_MS && (
            <div className="mt-2 text-xs text-faint">Plus {formatHoursMinutes(play.idleMs)} idle</div>
          )}
        </>
      )}
      {hint && <div className="mt-2.5 border-t border-rule pt-2 text-xs text-faint">{hint}</div>}
    </div>
  );
}
