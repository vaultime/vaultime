// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { DAYS_PER_WEEK, HEATMAP_STEPS_MS } from "@/lib/constants";
import { toDayKey } from "@/lib/session-stats";
import { weekdayOf } from "@/lib/stats";
import { formatHoursMinutes, UI_LOCALE } from "@/lib/time";
import { cn } from "@/lib/utils";

const LEVELS = ["bg-raised", "bg-violet/20", "bg-violet/35", "bg-violet/55", "bg-violet/75", "bg-violet"];
const WEEKDAY_LABELS = ["Mon", "", "Wed", "", "Fri", "", ""];

function levelOf(ms: number): number {
  if (ms <= 0) return 0;
  return 1 + HEATMAP_STEPS_MS.filter((step) => ms >= step).length;
}

/** Weeks from the Monday on or before 1 January to the week of 31 December. */
function weeksOf(year: number): Date[][] {
  const first = new Date(year, 0, 1);
  const weeks: Date[][] = [];
  let monday = new Date(year, 0, 1 - weekdayOf(first));
  while (monday <= new Date(year, 11, 31)) {
    const start = monday;
    weeks.push(Array.from({ length: DAYS_PER_WEEK }, (_, index) => new Date(start.getFullYear(), start.getMonth(), start.getDate() + index)));
    monday = new Date(start.getFullYear(), start.getMonth(), start.getDate() + DAYS_PER_WEEK);
  }
  return weeks;
}

/** The year as a calendar, one square per day, brighter the more active time it had. */
export function YearHeatmap({ year, activeByDay, now }: { year: number; activeByDay: Map<string, number>; now: Date }) {
  const weeks = weeksOf(year);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());

  return (
    <figure className="flex flex-col gap-3">
      <div
        role="img"
        aria-label={`Active time on every day of ${year}`}
        className="grid gap-[3px]"
        style={{ gridTemplateColumns: `auto repeat(${weeks.length}, minmax(0, 1fr))` }}
      >
        <span />
        {weeks.map((week) => {
          const first = week.find((day) => day.getDate() === 1 && day.getFullYear() === year);
          return (
            <span key={week[0].getTime()} className="h-4 font-mono text-[10px] whitespace-nowrap text-faint">
              {first?.toLocaleDateString(UI_LOCALE, { month: "short" })}
            </span>
          );
        })}
        {WEEKDAY_LABELS.map((label, weekday) => [
          <span key={`label-${weekday}`} className="pr-2 text-[10px] leading-none text-faint">
            {label}
          </span>,
          ...weeks.map((week) => {
            const day = week[weekday];
            const inYear = day.getFullYear() === year;
            const ms = activeByDay.get(toDayKey(day)) ?? 0;
            const name = day.toLocaleDateString(UI_LOCALE, { weekday: "short", day: "numeric", month: "short" });
            return (
              <span
                key={day.getTime()}
                title={inYear ? `${name}: ${ms > 0 ? `${formatHoursMinutes(ms)} active` : "not played"}` : undefined}
                className={cn(
                  "aspect-square rounded-[2px]",
                  !inYear ? "invisible" : day > today ? "bg-raised/40" : LEVELS[levelOf(ms)],
                )}
              />
            );
          }),
        ])}
      </div>
      <figcaption className="flex items-center justify-end gap-1.5 text-xs text-faint">
        Less
        {LEVELS.map((level) => (
          <span key={level} className={cn("size-2.5 rounded-[2px]", level)} />
        ))}
        More
      </figcaption>
    </figure>
  );
}
