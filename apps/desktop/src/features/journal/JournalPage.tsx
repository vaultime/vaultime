// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useState, type ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { PageHeader } from "@/components/layout/Page";
import { PhraseText } from "@/components/media/PhraseText";
import { useLibrary, type GameSummary } from "@/features/library/library-context";
import { SessionLine } from "@/features/sessions/components/SessionLine";
import {
  DAYS_PER_WEEK,
  HOURS_PER_DAY,
  JOURNAL_MIN_SPAN_PERCENT,
  JOURNAL_TICK_HOURS,
} from "@/lib/constants";
import { tintForTitle } from "@/lib/game-tint";
import { weekSentence } from "@/lib/sentences";
import * as api from "@/lib/tauri";
import {
  clockPercent,
  formatClockTime,
  formatHoursMinutes,
  isoWeekNumber,
  parseVaultimeDate,
  startOfWeek,
  UI_LOCALE,
} from "@/lib/time";
import type { Session, SessionEvent } from "@/lib/types";

interface JournalDay {
  start: Date;
  sessions: Session[];
  runtimeMs: number;
}

const TICKS = Array.from(
  { length: HOURS_PER_DAY / JOURNAL_TICK_HOURS + 1 },
  (_, index) => index * JOURNAL_TICK_HOURS,
);

function addDays(date: Date, days: number): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

/** The sessions that started in the week from `start`, by day, newest day first. */
function groupWeek(sessions: Session[], start: Date): JournalDay[] {
  const end = addDays(start, DAYS_PER_WEEK);
  const grouped = new Map<number, JournalDay>();
  for (const session of sessions) {
    const started = parseVaultimeDate(session.started_at_wall);
    if (started < start || started >= end) continue;
    const dayStart = new Date(started.getFullYear(), started.getMonth(), started.getDate());
    const day = grouped.get(dayStart.getTime()) ?? { start: dayStart, sessions: [], runtimeMs: 0 };
    day.sessions.push(session);
    day.runtimeMs += session.runtime_ms;
    grouped.set(dayStart.getTime(), day);
  }
  for (const day of grouped.values()) {
    day.sessions.sort((a, b) => a.started_at_wall.localeCompare(b.started_at_wall));
  }
  return [...grouped.values()].sort((a, b) => b.start.getTime() - a.start.getTime());
}

/** Play history week by week, one sentence per session. */
export function JournalPage() {
  const { sessions, summaries, loaded } = useLibrary();
  // 0 is this week, -1 the week before and so on.
  const [offset, setOffset] = useState(0);
  const [events, setEvents] = useState<SessionEvent[]>([]);

  const now = new Date();
  const weekStart = addDays(startOfWeek(now), offset * DAYS_PER_WEEK);
  const weekEnd = addDays(weekStart, DAYS_PER_WEEK);

  const byGame = new Map(summaries.map((summary) => [summary.game.id, summary]));

  const days = groupWeek(sessions, weekStart);

  // Events explain flagged sessions and skipped sleep. Only the games of this week are loaded.
  const gameIds = [...new Set(days.flatMap((day) => day.sessions.map((session) => session.game_id)))].sort().join(",");
  useEffect(() => {
    let cancelled = false;
    Promise.all(gameIds ? gameIds.split(",").map((id) => api.getSessionEventsForGame(id)) : [])
      .then((lists) => {
        if (!cancelled) setEvents(lists.flat());
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [gameIds, sessions]);

  const earliest = sessions.reduce<Date | null>((min, session) => {
    const started = parseVaultimeDate(session.started_at_wall);
    return !min || started < min ? started : min;
  }, null);
  const canGoBack = earliest !== null && startOfWeek(earliest) < weekStart;

  if (!loaded) return null;

  const longestDay = [...days].sort((a, b) => b.runtimeMs - a.runtimeMs)[0];
  const title = offset === 0 ? "This week" : offset === -1 ? "Last week" : `Week of ${weekStart.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "short" })}`;
  const sentence = weekSentence({
    sessionsCount: days.reduce((sum, day) => sum + day.sessions.length, 0),
    runtimeMs: days.reduce((sum, day) => sum + day.runtimeMs, 0),
    longestDay: longestDay ? longestDay.start.toLocaleDateString(UI_LOCALE, { weekday: "long" }) : null,
    daysPlayed: days.length,
    current: offset === 0,
  });
  const weekYear = weekEnd.getFullYear() === now.getFullYear() ? "" : `, ${weekStart.getFullYear()}`;

  return (
    <div className="pb-16">
      <PageHeader
        overline={`Journal, week ${isoWeekNumber(weekStart)}${weekYear}`}
        title={title}
        aside={
          <>
            <WeekButton label="Previous week" disabled={!canGoBack} onClick={() => setOffset((value) => value - 1)}>
              <ChevronLeft className="size-[18px]" strokeWidth={1.8} />
            </WeekButton>
            <WeekButton label="Next week" disabled={offset >= 0} onClick={() => setOffset((value) => value + 1)}>
              <ChevronRight className="size-[18px]" strokeWidth={1.8} />
            </WeekButton>
          </>
        }
      >
        <PhraseText phrase={sentence} />
      </PageHeader>

      <div className="px-8 xl:px-14">
        {days.map((day) => (
          <DaySection key={day.start.getTime()} day={day} byGame={byGame} events={events} now={now} />
        ))}
      </div>
    </div>
  );
}

function WeekButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className="flex size-11 items-center justify-center rounded-full border border-hairline text-soft transition-colors hover:bg-raised hover:text-text focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none disabled:border-rule disabled:text-faint/50 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}

function DaySection({
  day,
  byGame,
  events,
  now,
}: {
  day: JournalDay;
  byGame: Map<string, GameSummary>;
  events: SessionEvent[];
  now: Date;
}) {
  return (
    <section className="flex gap-10 border-b border-rule py-7">
      <div className="w-[190px] shrink-0">
        <h2 className="font-display text-4xl leading-none">
          {day.start.toLocaleDateString(UI_LOCALE, { weekday: "long" })}
        </h2>
        <div className="mt-1.5 text-xs tracking-[0.12em] text-faint uppercase">
          {day.start.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "long" })}
        </div>
        <div className="mt-3.5 font-mono text-[22px] tabular-nums">{formatHoursMinutes(day.runtimeMs)}</div>
      </div>

      <div className="flex min-w-0 flex-1 flex-col gap-3.5">
        <div>
          <div aria-hidden="true" className="relative h-2.5 rounded-full bg-raised">
            {day.sessions.map((session) => {
              const start = parseVaultimeDate(session.started_at_wall);
              const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : now;
              const left = clockPercent(start, day.start);
              const width = Math.max(JOURNAL_MIN_SPAN_PERCENT, clockPercent(end, day.start) - left);
              const summary = byGame.get(session.game_id);
              const tint = summary?.tint ?? tintForTitle(summary?.game.title ?? "");
              return (
                <span
                  key={session.id}
                  className="absolute inset-y-0 rounded-full"
                  style={{ left: `${left}%`, width: `${width}%`, background: tint.muted }}
                />
              );
            })}
          </div>
          <div aria-hidden="true" className="mt-1.5 flex justify-between font-mono text-[10px] text-faint">
            {TICKS.map((hour) => (
              <span key={hour}>{String(hour).padStart(2, "0")}</span>
            ))}
          </div>
        </div>

        {day.sessions.map((session) => {
          const start = formatClockTime(parseVaultimeDate(session.started_at_wall));
          const end = session.ended_at_wall ? formatClockTime(parseVaultimeDate(session.ended_at_wall)) : "now";
          return (
            <SessionLine
              key={session.id}
              session={session}
              events={events}
              gameTitle={byGame.get(session.game_id)?.game.title ?? "a removed game"}
              when={`${start} to ${end}`}
              bordered={false}
            />
          );
        })}
      </div>
    </section>
  );
}
