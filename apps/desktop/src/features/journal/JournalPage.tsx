// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { PageHeader, StepButton } from "@/components/layout/Page";
import { PhraseText } from "@/components/media/PhraseText";
import { useLibrary, type GameSummary } from "@/features/library/library-context";
import { SessionLine } from "@/features/sessions/components/SessionLine";
import {
  DAYS_PER_WEEK,
  HOURS_PER_DAY,
  JOURNAL_MIN_SPAN_PERCENT,
  JOURNAL_TICK_HOURS,
} from "@/lib/constants";
import { tintForTitle, type GameTint } from "@/lib/game-tint";
import { sideBySideSentence, weekSentence } from "@/lib/sentences";
import { playedMs, sideBySide, type SideBySide } from "@/lib/session-stats";
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
  /** Time with a game running, games side by side counted once. */
  playedMs: number;
}

/** Width of one colored stripe where games ran side by side. */
const HATCH_STRIPE_PX = 3;
/** Dark gap between the stripes, so games with similar colors still read as stripes. */
const HATCH_GAP_PX = 1.5;

const TICKS = Array.from(
  { length: HOURS_PER_DAY / JOURNAL_TICK_HOURS + 1 },
  (_, index) => index * JOURNAL_TICK_HOURS,
);

function addDays(date: Date, days: number): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

/** The sessions that started in the week from `start`, by day, newest day first. */
function groupWeek(sessions: Session[], start: Date, now: Date): JournalDay[] {
  const end = addDays(start, DAYS_PER_WEEK);
  const grouped = new Map<number, JournalDay>();
  for (const session of sessions) {
    const started = parseVaultimeDate(session.started_at_wall);
    if (started < start || started >= end) continue;
    const dayStart = new Date(started.getFullYear(), started.getMonth(), started.getDate());
    const day = grouped.get(dayStart.getTime()) ?? { start: dayStart, sessions: [], playedMs: 0 };
    day.sessions.push(session);
    grouped.set(dayStart.getTime(), day);
  }
  for (const day of grouped.values()) {
    day.sessions.sort((a, b) => a.started_at_wall.localeCompare(b.started_at_wall));
    day.playedMs = playedMs(day.sessions, now);
  }
  return [...grouped.values()].sort((a, b) => b.start.getTime() - a.start.getTime());
}

/** Diagonal stripes in the colors of games that ran side by side. */
function hatch(colors: string[]): string {
  const step = HATCH_STRIPE_PX + HATCH_GAP_PX;
  const stripes = colors.map((color, index) => {
    const start = index * step;
    const gap = start + HATCH_STRIPE_PX;
    return `${color} ${start}px ${gap}px, var(--ink) ${gap}px ${start + step}px`;
  });
  return `repeating-linear-gradient(135deg, ${stripes.join(", ")})`;
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

  const days = groupWeek(sessions, weekStart, now);

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

  const longestDay = [...days].sort((a, b) => b.playedMs - a.playedMs)[0];
  const title = offset === 0 ? "This week" : offset === -1 ? "Last week" : `Week of ${weekStart.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "short" })}`;
  const sentence = weekSentence({
    sessionsCount: days.reduce((sum, day) => sum + day.sessions.length, 0),
    runtimeMs: days.reduce((sum, day) => sum + day.playedMs, 0),
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
            <StepButton label="Previous week" disabled={!canGoBack} onClick={() => setOffset((value) => value - 1)}>
              <ChevronLeft className="size-[18px]" strokeWidth={1.8} />
            </StepButton>
            <StepButton label="Next week" disabled={offset >= 0} onClick={() => setOffset((value) => value + 1)}>
              <ChevronRight className="size-[18px]" strokeWidth={1.8} />
            </StepButton>
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

/**
 * Stripes over the stretch where games ran side by side. Its ends are round
 * only where no other session carries on, so it sits flush inside the bars.
 */
function SharedStretch({
  stretch,
  day,
  now,
  colors,
}: {
  stretch: SideBySide;
  day: JournalDay;
  now: Date;
  colors: string[];
}) {
  const spans = day.sessions.map((session) => ({
    start: parseVaultimeDate(session.started_at_wall),
    end: session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : now,
  }));
  const carriesOnBefore = spans.some((span) => span.start < stretch.start && span.end >= stretch.start);
  const carriesOnAfter = spans.some((span) => span.start <= stretch.end && span.end > stretch.end);
  const left = clockPercent(stretch.start, day.start);
  const width = Math.max(JOURNAL_MIN_SPAN_PERCENT, clockPercent(stretch.end, day.start) - left);
  return (
    <span
      className={`absolute inset-y-0 ${carriesOnBefore ? "" : "rounded-l-full"} ${carriesOnAfter ? "" : "rounded-r-full"}`}
      style={{ left: `${left}%`, width: `${width}%`, backgroundImage: hatch(colors) }}
    />
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
  const tintOf = (gameId: string): GameTint => {
    const summary = byGame.get(gameId);
    return summary?.tint ?? tintForTitle(summary?.game.title ?? "");
  };
  const shared = sideBySide(day.sessions, now);
  const sharedGames = [...new Set(shared.flatMap((stretch) => stretch.gameIds))];
  const sharedMs = shared.reduce((sum, stretch) => sum + stretch.end.getTime() - stretch.start.getTime(), 0);

  return (
    <section className="flex flex-col gap-5 border-b border-rule py-7 xl:flex-row xl:gap-10">
      <div className="flex shrink-0 items-baseline gap-4 xl:block xl:w-[190px]">
        <h2 className="font-display text-4xl leading-none">
          {day.start.toLocaleDateString(UI_LOCALE, { weekday: "long" })}
        </h2>
        <div className="text-xs tracking-[0.12em] text-faint uppercase xl:mt-1.5">
          {day.start.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "long" })}
        </div>
        <div className="ml-auto font-mono text-[22px] tabular-nums xl:mt-3.5 xl:ml-0">{formatHoursMinutes(day.playedMs)}</div>
      </div>

      <div className="flex min-w-0 flex-1 flex-col gap-3.5">
        <div>
          <div aria-hidden="true" className="relative h-2.5 rounded-full bg-raised">
            {day.sessions.map((session) => {
              const start = parseVaultimeDate(session.started_at_wall);
              const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : now;
              const left = clockPercent(start, day.start);
              const width = Math.max(JOURNAL_MIN_SPAN_PERCENT, clockPercent(end, day.start) - left);
              return (
                <span
                  key={session.id}
                  className="absolute inset-y-0 rounded-full"
                  style={{ left: `${left}%`, width: `${width}%`, background: tintOf(session.game_id).muted }}
                />
              );
            })}
            {shared.map((stretch) => (
              <SharedStretch
                key={stretch.start.getTime()}
                stretch={stretch}
                day={day}
                now={now}
                colors={stretch.gameIds.map((gameId) => tintOf(gameId).muted)}
              />
            ))}
          </div>
          <div aria-hidden="true" className="mt-1.5 flex justify-between font-mono text-[10px] text-faint">
            {TICKS.map((hour) => (
              <span key={hour}>{String(hour).padStart(2, "0")}</span>
            ))}
          </div>
          {shared.length > 0 && (
            <p className="mt-2.5 flex items-center gap-2.5 text-[13px] text-faint">
              <span
                aria-hidden="true"
                className="h-2.5 w-6 shrink-0 rounded-full"
                style={{ backgroundImage: hatch(shared[0].gameIds.map((gameId) => tintOf(gameId).muted)) }}
              />
              {sideBySideSentence(
                sharedGames.map((gameId) => byGame.get(gameId)?.game.title ?? "a removed game"),
                sharedMs,
              )}
            </p>
          )}
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
