// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { PageHeader, StepButton } from "@/components/layout/Page";
import { PhraseText } from "@/components/media/PhraseText";
import { GameStatusIcon } from "@/components/status/GameStatusIcon";
import { useAppearance } from "@/features/appearance/appearance-context";
import { useLibrary, type GameSummary } from "@/features/library/library-context";
import { SessionLine } from "@/features/sessions/components/SessionLine";
import { DAYS_PER_WEEK, HOURS_PER_DAY, JOURNAL_MIN_SPAN_PERCENT, JOURNAL_TICK_HOURS } from "@/lib/constants";
import { markColors, tintForTitle } from "@/lib/game-tint";
import { sideBySideSentence, statusSentence, weekSentence } from "@/lib/sentences";
import { countsAsPlay, playedMs, sideBySide, type SideBySide } from "@/lib/session-stats";
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
import type { GameStatusChange, Session, SessionEvent } from "@/lib/types";

interface JournalDay {
  start: Date;
  sessions: Session[];
  /** Status changes of games on this day. */
  changes: GameStatusChange[];
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

/** The sessions and status changes of the week from `start`, by day, newest day first. */
function groupWeek(sessions: Session[], changes: GameStatusChange[], start: Date, now: Date): JournalDay[] {
  const end = addDays(start, DAYS_PER_WEEK);
  const grouped = new Map<number, JournalDay>();
  const dayOf = (moment: Date) => {
    const dayStart = new Date(moment.getFullYear(), moment.getMonth(), moment.getDate());
    const day = grouped.get(dayStart.getTime()) ?? { start: dayStart, sessions: [], changes: [], playedMs: 0 };
    grouped.set(dayStart.getTime(), day);
    return day;
  };
  for (const session of sessions) {
    const started = parseVaultimeDate(session.started_at_wall);
    if (started < start || started >= end) continue;
    dayOf(started).sessions.push(session);
  }
  for (const change of changes) {
    const changed = parseVaultimeDate(change.changed_at);
    if (change.status === "none" || changed < start || changed >= end) continue;
    dayOf(changed).changes.push(change);
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
  const { sessions, summaries, statusChanges, notes, saveNote, refresh, loaded } = useLibrary();
  const { accentHues } = useAppearance();
  // 0 is this week, -1 the week before and so on.
  const [offset, setOffset] = useState(0);
  const [events, setEvents] = useState<SessionEvent[]>([]);

  const now = new Date();
  const weekStart = addDays(startOfWeek(now), offset * DAYS_PER_WEEK);
  const weekEnd = addDays(weekStart, DAYS_PER_WEEK);

  const byGame = new Map(summaries.map((summary) => [summary.game.id, summary]));
  // Kept between polls, so session lines can reuse their sentences.
  const sessionsByGame = useMemo(() => {
    const byId = new Map<string, Session[]>();
    for (const session of sessions) {
      const list = byId.get(session.game_id);
      if (list) list.push(session);
      else byId.set(session.game_id, [session]);
    }
    return byId;
  }, [sessions]);

  const days = groupWeek(sessions, statusChanges, weekStart, now);

  // A game keeps one color through the week, the main color of its artwork,
  // nudged only when it would look like a game that showed up earlier. The
  // accent stays free for active time.
  const weekGames = [
    ...new Set([...days].reverse().flatMap((day) => day.sessions.filter(countsAsPlay).map((session) => session.game_id))),
  ];
  const colorOf = (gameId: string) => {
    const summary = byGame.get(gameId);
    return (summary?.tint ?? tintForTitle(summary?.game.title ?? "")).color;
  };
  const weekMarks = markColors(weekGames.map(colorOf), accentHues);
  const marks = new Map(weekGames.map((gameId, index) => [gameId, weekMarks[index]]));
  const markOf = (gameId: string) => marks.get(gameId) ?? markColors([colorOf(gameId)])[0];

  /** A game's playtime up to a moment, with the playtime from before Vaultime. */
  const playedBefore = (gameId: string, moment: string) =>
    sessions
      .filter((session) => session.game_id === gameId && session.started_at_wall < moment)
      .reduce((sum, session) => sum + session.runtime_ms, byGame.get(gameId)?.earlier?.earlier_ms ?? 0);

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
  // Each game's own time, so games side by side both count. A live session counts up to now.
  const weekRuntime = new Map<string, number>();
  for (const session of days.flatMap((day) => day.sessions).filter(countsAsPlay)) {
    const ms = session.ended_at_wall
      ? session.runtime_ms
      : Math.max(session.runtime_ms, now.getTime() - parseVaultimeDate(session.started_at_wall).getTime());
    weekRuntime.set(session.game_id, (weekRuntime.get(session.game_id) ?? 0) + ms);
  }
  const gamesMs = [...weekRuntime.values()].reduce((sum, ms) => sum + ms, 0);
  const [topGameId, topMs] = [...weekRuntime.entries()].sort((a, b) => b[1] - a[1])[0] ?? [null, 0];
  const title = offset === 0 ? "This week" : offset === -1 ? "Last week" : `Week of ${weekStart.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "short" })}`;
  const sentence = weekSentence({
    sessionsCount: days.reduce((sum, day) => sum + day.sessions.filter(countsAsPlay).length, 0),
    runtimeMs: days.reduce((sum, day) => sum + day.playedMs, 0),
    longestDay: longestDay ? longestDay.start.toLocaleDateString(UI_LOCALE, { weekday: "long" }) : null,
    daysPlayed: days.filter((day) => day.sessions.some(countsAsPlay)).length,
    topTitle: topGameId ? (byGame.get(topGameId)?.game.title ?? null) : null,
    topShare: gamesMs > 0 ? topMs / gamesMs : 0,
    gamesCount: weekGames.length,
    weekNumber: isoWeekNumber(weekStart),
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
          <DaySection
            key={day.start.getTime()}
            day={day}
            byGame={byGame}
            sessionsByGame={sessionsByGame}
            markOf={markOf}
            events={events}
            now={now}
            notes={notes}
            onSaveNote={saveNote}
            onCorrected={() => void refresh()}
            playedBefore={playedBefore}
          />
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
  sessionsByGame,
  markOf,
  events,
  now,
  notes,
  onSaveNote,
  onCorrected,
  playedBefore,
}: {
  day: JournalDay;
  byGame: Map<string, GameSummary>;
  sessionsByGame: Map<string, Session[]>;
  /** The game's color in this week. */
  markOf: (gameId: string) => string;
  events: SessionEvent[];
  now: Date;
  notes: Record<string, string>;
  onSaveNote: (sessionId: string, note: string) => Promise<void>;
  onCorrected: () => void;
  playedBefore: (gameId: string, moment: string) => number;
}) {
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
            {day.sessions.filter(countsAsPlay).map((session) => {
              const start = parseVaultimeDate(session.started_at_wall);
              const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : now;
              const left = clockPercent(start, day.start);
              const width = Math.max(JOURNAL_MIN_SPAN_PERCENT, clockPercent(end, day.start) - left);
              return (
                <span
                  key={session.id}
                  className="absolute inset-y-0 rounded-full"
                  style={{ left: `${left}%`, width: `${width}%`, background: markOf(session.game_id) }}
                />
              );
            })}
            {shared.map((stretch) => (
              <SharedStretch
                key={stretch.start.getTime()}
                stretch={stretch}
                day={day}
                now={now}
                colors={stretch.gameIds.map(markOf)}
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
                style={{ backgroundImage: hatch(shared[0].gameIds.map(markOf)) }}
              />
              {sideBySideSentence(
                sharedGames.map((gameId) => byGame.get(gameId)?.game.title ?? "a removed game"),
                sharedMs,
              )}
            </p>
          )}
        </div>

        {[
          ...day.sessions.map((session) => ({ at: session.started_at_wall, session, change: null })),
          ...day.changes.map((change) => ({ at: change.changed_at, session: null, change })),
        ]
          .sort((a, b) => a.at.localeCompare(b.at))
          .map(({ session, change }) => {
            if (change) {
              if (change.status === "none") return null;
              const title = byGame.get(change.game_id)?.game.title ?? "a removed game";
              return (
                <article key={change.id} className="flex items-baseline gap-5">
                  <span className="w-[20ch] shrink-0 font-mono text-[13px] text-faint">
                    {formatClockTime(parseVaultimeDate(change.changed_at))}
                  </span>
                  <p className="font-prose flex min-w-0 flex-1 items-baseline gap-2.5 text-[20px] leading-snug">
                    <GameStatusIcon status={change.status} className="size-4 shrink-0 translate-y-0.5 text-violet" />
                    <span>
                      <PhraseText
                        phrase={statusSentence(change.status, title, playedBefore(change.game_id, change.changed_at))}
                      />
                    </span>
                  </p>
                </article>
              );
            }
            if (!session) return null;
            const start = formatClockTime(parseVaultimeDate(session.started_at_wall));
            const end = session.ended_at_wall ? formatClockTime(parseVaultimeDate(session.ended_at_wall)) : "now";
            return (
              <SessionLine
                key={session.id}
                session={session}
                events={events}
                gameTitle={byGame.get(session.game_id)?.game.title ?? "a removed game"}
                titleColor={markOf(session.game_id)}
                gameSessions={sessionsByGame.get(session.game_id)}
                earlierMs={byGame.get(session.game_id)?.earlier?.earlier_ms}
                when={`${start} to ${end}`}
                bordered={false}
                note={notes[session.id]}
                onSaveNote={(note) => onSaveNote(session.id, note)}
                onCorrected={onCorrected}
              />
            );
          })}
      </div>
    </section>
  );
}
