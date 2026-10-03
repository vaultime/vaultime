// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import { DAYS_PER_WEEK, HEATMAP_STEPS_MS } from "@/lib/constants";
import type { BucketPlay } from "@/lib/play-totals";
import { toDayKey } from "@/lib/session-stats";
import { weekdayOf } from "@/lib/stats";
import { UI_LOCALE } from "@/lib/time";
import { cn } from "@/lib/utils";
import { cardPosition, describePlay, type GameLabel } from "./play-card";
import { PlayCard } from "./PlayCard";

const LEVELS = ["bg-raised", "bg-violet/20", "bg-violet/35", "bg-violet/55", "bg-violet/75", "bg-violet"];
const WEEKDAY_LABELS = ["Mon", "", "Wed", "", "Fri", "", ""];

function levelOf(ms: number): number {
  if (ms <= 0) return 0;
  return 1 + HEATMAP_STEPS_MS.filter((step) => ms >= step).length;
}

function addDays(date: Date, days: number): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

/** Weeks from the Monday on or before 1 January to the week of 31 December. */
function weeksOf(year: number): Date[][] {
  const first = new Date(year, 0, 1);
  const weeks: Date[][] = [];
  let monday = new Date(year, 0, 1 - weekdayOf(first));
  while (monday <= new Date(year, 11, 31)) {
    const start = monday;
    weeks.push(Array.from({ length: DAYS_PER_WEEK }, (_, index) => addDays(start, index)));
    monday = addDays(start, DAYS_PER_WEEK);
  }
  return weeks;
}

const cellId = (day: Date) => `heatmap-${toDayKey(day)}`;

/**
 * The year as a calendar, one square per day, brighter the more active time
 * it had. Pointing at a day, or moving to it with the arrow keys, shows what
 * was played. A click or Enter opens its week in the journal.
 */
export function YearHeatmap({
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
  const weeks = weeksOf(year);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const first = new Date(year, 0, 1);
  const last = new Date(year, 11, 31) < today ? new Date(year, 11, 31) : today;
  const frame = useRef<HTMLDivElement>(null);
  const [focused, setFocused] = useState<Date | null>(null);
  const [shown, setShown] = useState<{ day: Date; style: CSSProperties; byKeyboard: boolean } | null>(null);

  const name = (day: Date) => day.toLocaleDateString(UI_LOCALE, { weekday: "long", day: "numeric", month: "long" });
  const show = (day: Date, byKeyboard: boolean, anchor?: Element | null) => {
    const cell = anchor ?? document.getElementById(cellId(day));
    if (!cell || !frame.current) return;
    setShown({ day, byKeyboard, style: cardPosition(cell.getBoundingClientRect(), frame.current.getBoundingClientRect()) });
  };
  const moveTo = (day: Date) => {
    const clamped = day < first ? first : day > last ? last : day;
    setFocused(clamped);
    show(clamped, true);
  };

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const from = focused ?? last;
    const steps: Record<string, () => Date> = {
      ArrowUp: () => addDays(from, -1),
      ArrowDown: () => addDays(from, 1),
      ArrowLeft: () => addDays(from, -DAYS_PER_WEEK),
      ArrowRight: () => addDays(from, DAYS_PER_WEEK),
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
    <figure className="flex flex-col gap-3">
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
                  {firstOfMonth?.toLocaleDateString(UI_LOCALE, { month: "short" })}
                </span>
              );
            })}
          </div>
          {WEEKDAY_LABELS.map((label, weekday) => (
            <div key={weekday} role="row" className="contents">
              <span role="rowheader" className="pr-2 text-[10px] leading-none text-faint">
                {label}
              </span>
              {weeks.map((week) => {
                const day = week[weekday];
                const inYear = day.getFullYear() === year;
                const played = day <= today;
                if (!inYear || !played) {
                  return (
                    <span
                      key={day.getTime()}
                      role="presentation"
                      className={cn("aspect-square rounded-[2px]", inYear ? "bg-raised/40" : "invisible")}
                    />
                  );
                }
                const play = days.get(toDayKey(day));
                const isFocused = focused !== null && toDayKey(focused) === toDayKey(day);
                return (
                  <span
                    key={day.getTime()}
                    id={cellId(day)}
                    role="gridcell"
                    aria-label={describePlay(name(day), play, labelOf)}
                    onMouseEnter={(event) => show(day, false, event.currentTarget)}
                    onClick={() => onOpenDay(day)}
                    className={cn(
                      "aspect-square cursor-pointer rounded-[2px] transition-shadow",
                      LEVELS[levelOf(play?.activeMs ?? 0)],
                      isFocused && shown?.byKeyboard && "ring-2 ring-text ring-offset-1 ring-offset-ink",
                    )}
                  />
                );
              })}
            </div>
          ))}
        </div>
        {shown && (
          <PlayCard
            label={name(shown.day)}
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
}
