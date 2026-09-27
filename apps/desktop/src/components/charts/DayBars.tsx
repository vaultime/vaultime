// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { CHART_MIN_BAR_PERCENT } from "@/lib/constants";
import type { ActivityPoint } from "@/lib/session-stats";
import { formatHoursMinutes } from "@/lib/time";

/** One column per day, active time in violet with idle time stacked on top. */
export function DayBars({ points }: { points: ActivityPoint[] }) {
  const max = Math.max(1, ...points.map((point) => point.activeMs + point.idleMs));
  const height = (ms: number) => `${Math.max(CHART_MIN_BAR_PERCENT, (ms / max) * 100)}%`;
  const middle = points[Math.floor(points.length / 2)];
  const empty = points.every((point) => point.activeMs + point.idleMs === 0);

  return (
    <figure className="flex flex-col gap-2.5">
      <div
        role="img"
        aria-label={`Active and idle time for each of the last ${points.length} days`}
        className="relative flex h-[132px] items-end gap-2.5 border-b border-hairline"
      >
        {empty && (
          <p className="absolute inset-0 flex items-center justify-center text-sm text-faint">
            Not played on any of these days.
          </p>
        )}
        {points.map((point) => (
          <div
            key={point.key}
            title={`${point.label}: ${formatHoursMinutes(point.activeMs)} active, ${formatHoursMinutes(point.idleMs)} idle`}
            className="flex h-full min-w-0 flex-1 flex-col justify-end gap-0.5"
          >
            {point.idleMs > 0 && <span className="rounded-[2px] bg-idle" style={{ height: height(point.idleMs) }} />}
            {point.activeMs > 0 && (
              <span className="rounded-[2px] bg-violet" style={{ height: height(point.activeMs) }} />
            )}
          </div>
        ))}
      </div>
      <figcaption className="flex justify-between font-mono text-[11px] text-faint">
        <span>{points[0]?.label}</span>
        <span>{middle?.label}</span>
        <span>Today</span>
      </figcaption>
    </figure>
  );
}

/** The two colors of DayBars, for a heading. */
export function DayBarsLegend() {
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
