// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { memo, useCallback, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import { DAYS_PER_WEEK } from "@/lib/constants";
import type { BucketPlay } from "@/lib/play-totals";
import { heatLevel } from "@/lib/stats";
import { startOfDay, toDayKey, UI_LOCALE, weekdayOf } from "@/lib/time";
import { cn } from "@/lib/utils";
import { cardPosition, describePlay, type GameLabel } from "./play-card";
import { PlayCard } from "./PlayCard";

/** One class per heat level. The share card repeats the accent shares. */
const LEVELS = ["bg-raised", "bg-violet/20", "bg-violet/35", "bg-violet/55", "bg-violet/75", "bg-violet"];
const WEEKDAY_LABELS = ["Mon", "", "Wed", "", "Fri", "", ""];


/** Weeks from the Monday on or before 1 January to the week of 31 December. */
function weeksOf(year: number): Date[][] {
  const first = new Date(year, 0, 1);
  const weeks: Date[][] = [];
  let monday = new Date(year, 0, 1 - weekdayOf(first));
  while (monday <= new Date(year, 11, 31)) {
    const start = monday;
    weeks.push(Array.from({ length: DAYS_PER_WEEK }, (_, index) => startOfDay(start, index)));
    monday = startOfDay(start, DAYS_PER_WEEK);
  }
  return weeks;
}

const cellId = (day: Date) => `heatmap-${toDayKey(day)}`;

// Made once, since a year of cells formats hundreds of dates on every render.
const DAY_NAME = new Intl.DateTimeFormat(UI_LOCALE, { weekday: "long", day: "numeric", month: "long" });
const MONTH_SHORT = new Intl.DateTimeFormat(UI_LOCALE, { month: "short" });
const MONTH_NARROW = new Intl.DateTimeFormat(UI_LOCALE, { month: "narrow" });
const dayName = (day: Date) => DAY_NAME.format(day);

/**
 * The year as a calendar, one square per day, brighter the more active time
 * it had. Pointing at a day, or moving to it with the arrow keys, shows what
 * was played. A click or Enter opens its week in the journal.
 */
export const YearHeatmap = memo(function YearHeatmap({
  year,
  days,
  now,
  labelOf,
  onOpenDay,
}: {
  year: number;
  days: Map<string, BucketPlay>;
  now: Date;
  labelOf: (gameId: string) => GameLabel;
  onOpenDay: (day: Date) => void;
}) {
  const weeks = useMemo(() => weeksOf(year), [year]);
  const today = startOfDay(now);
  const first = new Date(year, 0, 1);
  const last = new Date(year, 11, 31) < today ? new Date(year, 11, 31) : today;
  const frame = useRef<HTMLDivElement>(null);
  const [focused, setFocused] = useState<Date | null>(null);
  const [shown, setShown] = useState<{ day: Date; style: CSSProperties; byKeyboard: boolean } | null>(null);

  const show = useCallback((day: Date, byKeyboard: boolean, anchor?: Element | null) => {
    const cell = anchor ?? document.getElementById(cellId(day));
    if (!cell || !frame.current) return;
    setShown({ day, byKeyboard, style: cardPosition(cell.getBoundingClientRect(), frame.current.getBoundingClientRect()) });
  }, []);
  const moveTo = (day: Date) => {
    const clamped = day < first ? first : day > last ? last : day;
    setFocused(clamped);
    show(clamped, true);
  };

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const from = focused ?? last;
    const steps: Record<string, () => Date> = {
      ArrowUp: () => startOfDay(from, -1),
      ArrowDown: () => startOfDay(from, 1),
      ArrowLeft: () => startOfDay(from, -DAYS_PER_WEEK),
      ArrowRight: () => startOfDay(from, DAYS_PER_WEEK),
      Home: () => first,
      End: () => last,
    };
    if (steps[event.key]) {
      event.preventDefault();
      moveTo(steps[event.key]());
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onOpenDay(from);
    }
  }

  return (
    <figure className="@container flex flex-col gap-3">
      <div ref={frame} className="relative">
        <div
          role="grid"
          aria-label={`Active time on every day of ${year}. Arrow keys move between days, Enter opens the week in the journal.`}
          aria-activedescendant={focused ? cellId(focused) : undefined}
          tabIndex={0}
          onKeyDown={onKeyDown}
          onFocus={(event) => {
            // A click focuses the grid too, and shows its day by itself.
            if (event.currentTarget.matches(":focus-visible")) moveTo(focused ?? last);
          }}
          onBlur={() => setShown((current) => (current?.byKeyboard ? null : current))}
          onMouseLeave={() => setShown((current) => (current?.byKeyboard ? current : null))}
          className="grid gap-[3px] rounded-[3px] outline-none focus-visible:ring-2 focus-visible:ring-violet/50 focus-visible:ring-offset-4 focus-visible:ring-offset-ink"
          style={{ gridTemplateColumns: `auto repeat(${weeks.length}, minmax(0, 1fr))` }}
        >
          <HeatmapRows
            weeks={weeks}
            year={year}
            today={today.getTime()}
            days={days}
            labelOf={labelOf}
            ringed={focused && shown?.byKeyboard ? toDayKey(focused) : null}
            onPoint={show}
            onOpenDay={onOpenDay}
          />
        </div>
        {shown && (
          <PlayCard
            label={dayName(shown.day)}
            play={days.get(toDayKey(shown.day))}
            labelOf={labelOf}
            hint={shown.byKeyboard ? "Enter opens this week in the journal" : "Click to open this week in the journal"}
            style={shown.style}
          />
        )}
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
});

/** The month names and the days, apart from the card so pointing at a day does not draw them again. */
const HeatmapRows = memo(function HeatmapRows({
  weeks,
  year,
  today,
  days,
  labelOf,
  ringed,
  onPoint,
  onOpenDay,
}: {
  weeks: Date[][];
  year: number;
  /** Midnight of today, in milliseconds. Later days are blank. */
  today: number;
  days: Map<string, BucketPlay>;
  labelOf: (gameId: string) => GameLabel;
  /** The day the keyboard is on, by `toDayKey`. */
  ringed: string | null;
  onPoint: (day: Date, byKeyboard: boolean, anchor: Element) => void;
  onOpenDay: (day: Date) => void;
}) {
  return (
    <>
      <div role="row" className="contents">
        <span role="columnheader" />
        {weeks.map((week) => {
          const firstOfMonth = week.find((day) => day.getDate() === 1 && day.getFullYear() === year);
          return (
            <span
              key={week[0].getTime()}
              role="columnheader"
              className="h-4 font-mono text-[10px] whitespace-nowrap text-faint"
            >
              {/* Single letters where the weeks are too narrow for names. */}
              <span className="@max-[420px]:hidden">
                {firstOfMonth && MONTH_SHORT.format(firstOfMonth)}
              </span>
              <span className="hidden @max-[420px]:inline">
                {firstOfMonth && MONTH_NARROW.format(firstOfMonth)}
              </span>
            </span>
          );
        })}
      </div>
      {WEEKDAY_LABELS.map((label, weekday) => (
        <div key={weekday} role="row" className="contents">
          {/* Out of the flow, so a label never makes its row taller than the days. */}
          <span role="rowheader" className="relative w-8">
            <span className="absolute inset-y-0 left-0 flex items-center text-[10px] leading-none text-faint">
              {label}
            </span>
          </span>
          {weeks.map((week) => {
            const day = week[weekday];
            const inYear = day.getFullYear() === year;
            const played = day.getTime() <= today;
            if (!inYear || !played) {
              return (
                <span
                  key={day.getTime()}
                  role="presentation"
                  className={cn("aspect-square rounded-[2px]", inYear ? "bg-raised/40" : "invisible")}
                />
              );
            }
            const key = toDayKey(day);
            const play = days.get(key);
            return (
              <span
                key={day.getTime()}
                id={cellId(day)}
                role="gridcell"
                aria-label={describePlay(dayName(day), play, labelOf)}
                onMouseEnter={(event) => onPoint(day, false, event.currentTarget)}
                onClick={() => onOpenDay(day)}
                className={cn(
                  "aspect-square cursor-pointer rounded-[2px] transition-shadow",
                  LEVELS[heatLevel(play?.activeMs ?? 0)],
                  key === ringed && "ring-2 ring-text ring-offset-1 ring-offset-ink",
                )}
              />
            );
          })}
        </div>
      ))}
    </>
  );
});
