// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { CHART_MIN_BAR_PERCENT } from "@/lib/constants";
import { formatHoursMinutes, UI_LOCALE } from "@/lib/time";
import { cn } from "@/lib/utils";

/** One column per month, active time in violet with idle time stacked on top. */
export function MonthBars({ months, year }: { months: { activeMs: number; idleMs: number }[]; year: number }) {
  const totals = months.map((month) => month.activeMs + month.idleMs);
  const max = Math.max(1, ...totals);
  const busiest = totals.indexOf(Math.max(...totals));
  const height = (ms: number) => `${Math.max(CHART_MIN_BAR_PERCENT, (ms / max) * 100)}%`;
  const name = (index: number, style: "short" | "long") =>
    new Date(year, index, 1).toLocaleDateString(UI_LOCALE, { month: style });

  return (
    <figure className="flex flex-col gap-2.5">
      <div
        role="img"
        aria-label={`Active and idle time for each month of ${year}`}
        className="flex h-[150px] items-end gap-2.5 border-b border-hairline"
      >
        {months.map((month, index) => (
          <div
            key={index}
            title={`${name(index, "long")}: ${formatHoursMinutes(month.activeMs)} active, ${formatHoursMinutes(month.idleMs)} idle`}
            className="flex h-full min-w-0 flex-1 flex-col justify-end gap-0.5"
          >
            {month.idleMs > 0 && <span className="rounded-[2px] bg-idle" style={{ height: height(month.idleMs) }} />}
            {month.activeMs > 0 && (
              <span className="rounded-[2px] bg-violet" style={{ height: height(month.activeMs) }} />
            )}
          </div>
        ))}
      </div>
      <figcaption className="flex gap-2.5 font-mono text-[11px] text-faint">
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
