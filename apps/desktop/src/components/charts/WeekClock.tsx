// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { JOURNAL_TICK_HOURS, WEEK_CLOCK_MIN_DOT_PERCENT } from "@/lib/constants";
import { formatHoursMinutes } from "@/lib/time";

const WEEKDAYS = ["Mondays", "Tuesdays", "Wednesdays", "Thursdays", "Fridays", "Saturdays", "Sundays"];
const WEEKDAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

const hourLabel = (hour: number) => `${String(hour).padStart(2, "0")}:00`;

/**
 * When in the week the play happens: one dot per weekday and hour, its area
 * growing with the active time in that hour.
 */
export function WeekClock({ weekClock }: { weekClock: number[][] }) {
  const max = Math.max(1, ...weekClock.flat());
  const hours = weekClock[0]?.length ?? 0;

  return (
    <div
      role="img"
      aria-label="Active time by weekday and hour"
      className="grid gap-y-1"
      style={{ gridTemplateColumns: `auto repeat(${hours}, minmax(0, 1fr))` }}
    >
      {weekClock.map((row, weekday) => [
        <span key={`label-${weekday}`} className="self-center pr-3 text-xs text-faint">
          {WEEKDAY_LABELS[weekday]}
        </span>,
        ...row.map((ms, hour) => {
          const size =
            ms > 0 ? WEEK_CLOCK_MIN_DOT_PERCENT + (100 - WEEK_CLOCK_MIN_DOT_PERCENT) * Math.sqrt(ms / max) : 0;
          return (
            <span
              key={`${weekday}-${hour}`}
              title={`${WEEKDAYS[weekday]} ${hourLabel(hour)} to ${hourLabel(hour + 1)}: ${ms > 0 ? `${formatHoursMinutes(ms)} active` : "no play"}`}
              className="flex aspect-square items-center justify-center"
            >
              {size > 0 ? (
                <span className="rounded-full bg-violet" style={{ width: `${size}%`, height: `${size}%` }} />
              ) : (
                <span className="size-[3px] rounded-full bg-hairline" />
              )}
            </span>
          );
        }),
      ])}
      <span />
      {Array.from({ length: hours }, (_, hour) => (
        <span key={`tick-${hour}`} className="mt-1 font-mono text-[10px] text-faint">
          {hour % JOURNAL_TICK_HOURS === 0 ? String(hour).padStart(2, "0") : ""}
        </span>
      ))}
    </div>
  );
}
