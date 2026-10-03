// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useRef, useState, type CSSProperties } from "react";
import { CHART_MIN_BAR_PERCENT } from "@/lib/constants";
import { formatHoursMinutes } from "@/lib/time";
import { cn } from "@/lib/utils";
import { cardPosition } from "./play-card";
import { PlayCard } from "./PlayCard";

/** One column of a bar chart. */
export interface PlayPoint {
  key: string;
  /** Under the column, when the axis shows it. */
  label: string;
  /** At the top of the hover card, like "Tuesday, 29 September". */
  name: string;
  activeMs: number;
  idleMs: number;
}

/**
 * One column per point, active time in violet with idle time stacked on top.
 * Pointing at a column, or tabbing to it, shows its time. The axis names
 * every column, or the first, the middle and the last one.
 */
export function PlayBars({
  points,
  label,
  axis,
  lastLabel,
  empty,
}: {
  points: PlayPoint[];
  /** What the chart shows, for screen readers. */
  label: string;
  axis: "all" | "ends";
  /** Replaces the last label, like "Today". */
  lastLabel?: string;
  /** Said when no column has time. */
  empty: string;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState<{ index: number; style: CSSProperties } | null>(null);
  const max = Math.max(1, ...points.map((point) => point.activeMs + point.idleMs));
  const height = (ms: number) => `${Math.max(CHART_MIN_BAR_PERCENT, (ms / max) * 100)}%`;
  const nothing = points.every((point) => point.activeMs + point.idleMs === 0);
  const show = (index: number, column: Element) => {
    if (!frame.current) return;
    setShown({ index, style: cardPosition(column.getBoundingClientRect(), frame.current.getBoundingClientRect()) });
  };
  const labelAt = (index: number) =>
    index === points.length - 1 && lastLabel ? lastLabel : (points[index]?.label ?? "");
  const point = shown ? points[shown.index] : null;

  return (
    <figure className="flex flex-col gap-2.5">
      <div ref={frame} className="relative">
        <ol
          aria-label={label}
          onMouseLeave={() => setShown(null)}
          className={cn(
            "relative flex h-[132px] items-end gap-2.5 border-b border-hairline",
            axis === "all" && "justify-center",
          )}
        >
          {nothing && (
            <li className="absolute inset-0 flex items-center justify-center text-sm text-faint">{empty}</li>
          )}
          {points.map((point, index) => (
            <li
              key={point.key}
              tabIndex={0}
              aria-label={`${point.name}, ${formatHoursMinutes(point.activeMs)} active, ${formatHoursMinutes(point.idleMs)} idle`}
              onMouseEnter={(event) => show(index, event.currentTarget)}
              onFocus={(event) => show(index, event.currentTarget)}
              onBlur={() => setShown(null)}
              className={cn(
                "flex h-full min-w-0 flex-1 flex-col justify-end gap-0.5 rounded-[2px] outline-none focus-visible:ring-2 focus-visible:ring-violet/50 focus-visible:ring-offset-2 focus-visible:ring-offset-ink",
                // A few years would make wide slabs, so columns stop at a bar's width.
                axis === "all" && "max-w-16",
              )}
            >
              {point.idleMs > 0 && <span className="rounded-[2px] bg-idle" style={{ height: height(point.idleMs) }} />}
              {point.activeMs > 0 && (
                <span className="rounded-[2px] bg-violet" style={{ height: height(point.activeMs) }} />
              )}
            </li>
          ))}
        </ol>
        {shown && point && (
          <PlayCard
            label={point.name}
            play={{
              runtimeMs: point.activeMs + point.idleMs,
              activeMs: point.activeMs,
              idleMs: point.idleMs,
              games: [],
            }}
            labelOf={() => ({ title: "", color: "" })}
            style={shown.style}
          />
        )}
      </div>
      <figcaption
        aria-hidden="true"
        className={cn("flex gap-2.5 font-mono text-[11px] text-faint", axis === "all" && "justify-center")}
      >
        {axis === "all" ? (
          points.map((point, index) => (
            <span key={point.key} className="min-w-0 max-w-16 flex-1 truncate text-center">
              {labelAt(index)}
            </span>
          ))
        ) : (
          <span className="flex flex-1 justify-between">
            <span>{labelAt(0)}</span>
            <span>{labelAt(Math.floor(points.length / 2))}</span>
            <span>{labelAt(points.length - 1)}</span>
          </span>
        )}
      </figcaption>
    </figure>
  );
}

/** The two colors of the bar charts, for a heading. */
export function ActiveIdleLegend() {
  return (
    <div className="flex gap-4 text-xs text-faint">
      <span className="flex items-center gap-1.5">
        <span className="size-2.5 rounded-[2px] bg-violet" />
        Active
      </span>
      <span className="flex items-center gap-1.5">
        <span className="size-2.5 rounded-[2px] bg-idle" />
        Idle
      </span>
    </div>
  );
}
