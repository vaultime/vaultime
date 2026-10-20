// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useRef, useState, type CSSProperties } from "react";
import { CHART_MIN_BAR_PERCENT } from "@/lib/constants";
import type { BucketPlay } from "@/lib/play-totals";
import { cn } from "@/lib/utils";
import { cardPosition, describePlay, type GameLabel } from "./play-card";
import { PlayCard } from "./PlayCard";

/** A month or a year on the bars, with its short name under it and its long one for the card. */
export interface PeriodColumn {
  short: string;
  long: string;
  play: BucketPlay;
}

/**
 * One column per month or year, active time in violet with idle time
 * stacked on top. Pointing at a column, or tabbing to it, shows what was
 * played.
 */
export function PeriodBars({
  columns,
  label,
  labelOf,
}: {
  columns: PeriodColumn[];
  /** What the columns are, for screen readers: "Active and idle time for each month of 2026". */
  label: string;
  labelOf: (gameId: string) => GameLabel;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState<{ index: number; style: CSSProperties } | null>(null);
  const totals = columns.map((column) => column.play.activeMs + column.play.idleMs);
  const max = Math.max(1, ...totals);
  const busiest = totals.indexOf(Math.max(...totals));
  const height = (ms: number) => `${Math.max(CHART_MIN_BAR_PERCENT, (ms / max) * 100)}%`;
  const show = (index: number, column: Element) => {
    if (!frame.current) return;
    setShown({ index, style: cardPosition(column.getBoundingClientRect(), frame.current.getBoundingClientRect()) });
  };

  return (
    <figure className="flex flex-col gap-2.5">
      <div ref={frame} className="relative">
        <ol
          aria-label={label}
          onMouseLeave={() => setShown(null)}
          className="flex h-[150px] items-end gap-2.5 border-b border-hairline"
        >
          {columns.map(({ long, play: month }, index) => (
            <li
              key={long}
              tabIndex={0}
              aria-label={describePlay(long, month, labelOf)}
              onMouseEnter={(event) => show(index, event.currentTarget)}
              onFocus={(event) => show(index, event.currentTarget)}
              onBlur={() => setShown(null)}
              className="flex h-full min-w-0 flex-1 flex-col justify-end gap-0.5 rounded-[2px] outline-none focus-visible:ring-2 focus-visible:ring-violet/50 focus-visible:ring-offset-2 focus-visible:ring-offset-ink"
            >
              {month.idleMs > 0 && <span className="rounded-[2px] bg-idle" style={{ height: height(month.idleMs) }} />}
              {month.activeMs > 0 && (
                <span className="rounded-[2px] bg-violet" style={{ height: height(month.activeMs) }} />
              )}
            </li>
          ))}
        </ol>
        {shown && (
          <PlayCard
            label={columns[shown.index].long}
            play={columns[shown.index].play}
            labelOf={labelOf}
            style={shown.style}
          />
        )}
      </div>
      <figcaption aria-hidden="true" className="flex gap-2.5 font-mono text-[11px] text-faint">
        {columns.map(({ short, long }, index) => (
          <span
            key={long}
            className={cn("min-w-0 flex-1 text-center", index === busiest && totals[index] > 0 && "text-text")}
          >
            {short}
          </span>
        ))}
      </figcaption>
    </figure>
  );
}
