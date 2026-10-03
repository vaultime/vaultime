// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useRef, useState, type CSSProperties } from "react";
import { CHART_MIN_BAR_PERCENT } from "@/lib/constants";
import type { BucketPlay } from "@/lib/play-totals";
import { UI_LOCALE } from "@/lib/time";
import { cn } from "@/lib/utils";
import { cardPosition, describePlay, type GameLabel } from "./play-card";
import { PlayCard } from "./PlayCard";

/**
 * One column per month, active time in violet with idle time stacked on
 * top. Pointing at a month, or tabbing to it, shows what was played.
 */
export function MonthBars({
  months,
  year,
  labelOf,
}: {
  months: BucketPlay[];
  year: number;
  labelOf: (gameId: string) => GameLabel;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState<{ index: number; style: CSSProperties } | null>(null);
  const totals = months.map((month) => month.activeMs + month.idleMs);
  const max = Math.max(1, ...totals);
  const busiest = totals.indexOf(Math.max(...totals));
  const height = (ms: number) => `${Math.max(CHART_MIN_BAR_PERCENT, (ms / max) * 100)}%`;
  const name = (index: number, style: "short" | "long") =>
    new Date(year, index, 1).toLocaleDateString(UI_LOCALE, { month: style });
  const show = (index: number, column: Element) => {
    if (!frame.current) return;
    setShown({ index, style: cardPosition(column.getBoundingClientRect(), frame.current.getBoundingClientRect()) });
  };

  return (
    <figure className="flex flex-col gap-2.5">
      <div ref={frame} className="relative">
        <ol
          aria-label={`Active and idle time for each month of ${year}`}
          onMouseLeave={() => setShown(null)}
          className="flex h-[150px] items-end gap-2.5 border-b border-hairline"
        >
          {months.map((month, index) => (
            <li
              key={index}
              tabIndex={0}
              aria-label={describePlay(`${name(index, "long")} ${year}`, month, labelOf)}
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
            label={`${name(shown.index, "long")} ${year}`}
            play={months[shown.index]}
            labelOf={labelOf}
            style={shown.style}
          />
        )}
      </div>
      <figcaption aria-hidden="true" className="flex gap-2.5 font-mono text-[11px] text-faint">
        {months.map((_, index) => (
          <span
            key={index}
            className={cn("min-w-0 flex-1 text-center", index === busiest && totals[index] > 0 && "text-text")}
          >
            {name(index, "short")}
          </span>
        ))}
      </figcaption>
    </figure>
  );
}
