// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useState, type CSSProperties } from "react";
import { useSearchParams } from "react-router";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { PageHeader, StepButton } from "@/components/layout/Page";
import { PhraseText } from "@/components/media/PhraseText";
import { GameStatusIcon } from "@/components/status/GameStatusIcon";
import { useAppearance } from "@/features/appearance/appearance-context";
import { useLibrary, type GameSummary } from "@/features/library/library-context";
import { SessionLine } from "@/features/sessions/components/SessionLine";
import { DAY_MS, DAYS_PER_WEEK, HOURS_PER_DAY, JOURNAL_MIN_SPAN_PERCENT, JOURNAL_TICK_HOURS } from "@/lib/constants";
import { markColors, tintForTitle } from "@/lib/game-tint";
import { carriedOverPhrase, sideBySideSentence, statusSentence, weekSentence } from "@/lib/sentences";
import { clipToWindow, countsAsPlay, playedMs, playRuns, sideBySideGroups, type PlayRun } from "@/lib/session-stats";
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
  /** Sessions that started on this day. */
  sessions: Session[];
  /**
   * The part of every session that ran on this day, for the strip and the
   * totals, so play past midnight counts on the day it happened.
   */
  spans: Session[];
  /** Status changes of games on this day. */
  changes: GameStatusChange[];
  /** Time with a game running, games side by side counted once. */
  playedMs: number;
}

/** Width of one colored stripe where games ran side by side. */
const HATCH_STRIPE_PX = 3;
/** Dark gap between the stripes, so games with similar colors still read as stripes. */
const HATCH_GAP_PX = 1.5;
/** Half the strip's height. Pieces of a block lean by it at both ends, along the stripes. */
const HATCH_LEAN_PX = 5;
/** How far a piece reaches under the next one, so no hairline shows between them. */
const PIECE_OVERLAP_PX = 1;

const TICKS = Array.from(
  { length: HOURS_PER_DAY / JOURNAL_TICK_HOURS + 1 },
  (_, index) => index * JOURNAL_TICK_HOURS,
);

function addDays(date: Date, days: number): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

/** How many weeks back the week of `day` ("2026-09-28") lies, 0 for this week or no day. */
function weeksBack(day: string | null): number {
  const [year, month, date] = (day ?? "").split("-").map(Number);
  if (!year || !month || !date) return 0;
  const thisWeek = startOfWeek(new Date());
  const asked = startOfWeek(new Date(year, month - 1, date));
  // Rounded, since a week with a clock change is an hour longer or shorter.
  return Math.min(0, Math.round((asked.getTime() - thisWeek.getTime()) / (DAYS_PER_WEEK * DAY_MS)));
}

/** The sessions and status changes of the week from `start`, by day, newest day first. */
function groupWeek(sessions: Session[], changes: GameStatusChange[], start: Date, now: Date): JournalDay[] {
  const end = addDays(start, DAYS_PER_WEEK);
  const grouped = new Map<number, JournalDay>();
  const dayOf = (moment: Date) => {
    const dayStart = new Date(moment.getFullYear(), moment.getMonth(), moment.getDate());
    const day = grouped.get(dayStart.getTime()) ?? {
      start: dayStart,
      sessions: [],
      spans: [],
      changes: [],
      playedMs: 0,
    };
    grouped.set(dayStart.getTime(), day);
    return day;
  };
  // Sessions with time in the week, also those that started before it.
  const inWeek = sessions.filter((session) => clipToWindow(session, start, end, now) !== null);
  for (const session of inWeek) {
    const started = parseVaultimeDate(session.started_at_wall);
    if (started >= start) dayOf(started).sessions.push(session);
  }
  for (let day = start; day < end; day = addDays(day, 1)) {
    const next = addDays(day, 1);
    const spans = inWeek.flatMap((session) => clipToWindow(session, day, next, now) ?? []);
    if (spans.some(countsAsPlay) || grouped.has(day.getTime())) dayOf(day).spans.push(...spans);
  }
  for (const change of changes) {
    const changed = parseVaultimeDate(change.changed_at);
    if (change.status === "none" || changed < start || changed >= end) continue;
    dayOf(changed).changes.push(change);
  }
  for (const day of grouped.values()) {
    day.sessions.sort((a, b) => a.started_at_wall.localeCompare(b.started_at_wall));
    day.spans.sort((a, b) => a.started_at_wall.localeCompare(b.started_at_wall));
    day.playedMs = playedMs(day.spans, now);
  }
  return [...grouped.values()].sort((a, b) => b.start.getTime() - a.start.getTime());
}

/** Diagonal stripes in the colors of games that ran side by side, one color per stripe in turn. */
function hatch(colors: string[]): string {
  const step = HATCH_STRIPE_PX + HATCH_GAP_PX;
  const stripes = colors.map((color, index) => {
    const start = index * step;
    const gap = start + HATCH_STRIPE_PX;
    return `${color} ${start}px ${gap}px, var(--ink) ${gap}px ${start + step}px`;
  });
  return `repeating-linear-gradient(135deg, ${stripes.join(", ")})`;
}

/** Where something sits on the day's strip, in percent of the day. */
interface Place {
  left: number;
  width: number;
}

function placeOnDay(start: Date, end: Date, day: Date): Place {
  const left = clockPercent(start, day);
  const width = Math.max(JOURNAL_MIN_SPAN_PERCENT, clockPercent(end, day) - left);
  return { left: Math.min(left, 100 - width), width };
}

/**
 * Cuts `place` out of a layer that covers `box`, with ends that lean along
 * the stripes. An end at the end of its block stays upright.
 */
function leaningClip(box: Place, place: Place, leanStart: boolean, leanEnd: boolean): string {
  const inBox = (at: number) => ((at - box.left) / box.width) * 100;
  const edge = (at: number, shiftPx: number) => (shiftPx ? `calc(${inBox(at)}% + ${shiftPx}px)` : `${inBox(at)}%`);
  const start = place.left;
  const end = place.left + place.width;
  const [startTop, startBottom] = leanStart ? [HATCH_LEAN_PX, -HATCH_LEAN_PX] : [0, 0];
  const [endTop, endBottom] = leanEnd
    ? [HATCH_LEAN_PX + PIECE_OVERLAP_PX, -HATCH_LEAN_PX + PIECE_OVERLAP_PX]
    : [0, 0];
  return `polygon(${edge(start, startTop)} 0, ${edge(end, endTop)} 0, ${edge(end, endBottom)} 100%, ${edge(start, startBottom)} 100%)`;
}

/** Play history week by week, one sentence per session. */
export function JournalPage() {
  const { sessions: stored, active, summaries, statusChanges, notes, saveNote, refresh, loaded } = useLibrary();
  // Running sessions as of the last poll, a few seconds old at most. The
  // full list only reloads when a session starts or ends.
  const sessions = useMemo(() => {
    const running = new Map(active.map((session) => [session.id, session]));
    return stored.map((session) => running.get(session.id) ?? session);
  }, [stored, active]);
  const { accentHues, mode } = useAppearance();
  // 0 is this week, -1 the week before and so on. `?week=2026-09-28` opens
  // the week of that day, as the stats page asks for.
  const [params] = useSearchParams();
  const [offset, setOffset] = useState(() => weeksBack(params.get("week")));
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
    ...new Set([...days].reverse().flatMap((day) => day.spans.filter(countsAsPlay).map((session) => session.game_id))),
  ];
  const paletteOf = (gameId: string) => {
    const summary = byGame.get(gameId);
    return (summary?.tint ?? tintForTitle(summary?.game.title ?? "")).colors;
  };
  const weekMarks = markColors(weekGames.map(paletteOf), accentHues, mode);
  const marks = new Map(weekGames.map((gameId, index) => [gameId, weekMarks[index]]));
  const markFor = (gameId: string) => marks.get(gameId) ?? markColors([paletteOf(gameId)], [], mode)[0];
  const markOf = (gameId: string) => markFor(gameId).color;
  const fillOf = (gameId: string) => markFor(gameId).fill;

  /** A game's playtime up to a moment, with the playtime from before Vaultime. */
  const playedBefore = (gameId: string, moment: string) =>
    sessions
      .filter((session) => session.game_id === gameId && session.started_at_wall < moment)
      .reduce((sum, session) => sum + session.runtime_ms, byGame.get(gameId)?.earlier?.earlier_ms ?? 0);

  // Events explain flagged sessions and skipped sleep. Only the games of this week are loaded.
  const gameIds = [...new Set(days.flatMap((day) => day.spans.map((session) => session.game_id)))].sort().join(",");
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
  }, [gameIds, stored]);

  const earliest = sessions.reduce<Date | null>((min, session) => {
    const started = parseVaultimeDate(session.started_at_wall);
    return !min || started < min ? started : min;
  }, null);
  const canGoBack = earliest !== null && startOfWeek(earliest) < weekStart;

  if (!loaded) return null;

  const longestDay = [...days].sort((a, b) => b.playedMs - a.playedMs)[0];
  // Each game's own time in the week, so games side by side both count.
  const weekRuntime = new Map<string, number>();
  for (const span of days.flatMap((day) => day.spans).filter(countsAsPlay)) {
    weekRuntime.set(span.game_id, (weekRuntime.get(span.game_id) ?? 0) + span.runtime_ms);
  }
  const gamesMs = [...weekRuntime.values()].reduce((sum, ms) => sum + ms, 0);
  const [topGameId, topMs] = [...weekRuntime.entries()].sort((a, b) => b[1] - a[1])[0] ?? [null, 0];
  const title = offset === 0 ? "This week" : offset === -1 ? "Last week" : `Week of ${weekStart.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "short" })}`;
  const sentence = weekSentence({
    sessionsCount: days.reduce((sum, day) => sum + day.sessions.filter(countsAsPlay).length, 0),
    runtimeMs: days.reduce((sum, day) => sum + day.playedMs, 0),
    longestDay: longestDay ? longestDay.start.toLocaleDateString(UI_LOCALE, { weekday: "long" }) : null,
    daysPlayed: days.filter((day) => day.spans.some(countsAsPlay)).length,
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
            fillOf={fillOf}
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
 * Sessions that overlap as one rounded block, in the fill of the game that
 * ran alone and in stripes where games ran side by side. Every change leans
 * with the stripes, so the block reads as one piece of play however many
 * games come and go.
 */
function PlayBlock({
  run,
  day,
  fillOf,
  markOf,
}: {
  run: PlayRun;
  day: Date;
  fillOf: (gameId: string) => string;
  markOf: (gameId: string) => string;
}) {
  const block = placeOnDay(run.start, run.end, day);
  const layer = (box: Place): CSSProperties => ({
    left: `${((box.left - block.left) / block.width) * 100}%`,
    width: `${(box.width / block.width) * 100}%`,
  });
  // Stripes come from layers as wide as the strip, so they run on from one
  // stretch to the next and keep their spot when a game joins or leaves. A
  // game's fill spans its sessions, so two colored art stays one gradient.
  const strip: Place = { left: 0, width: 100 };
  // Stripes go on top, so a short stretch side by side keeps its narrowest width.
  const pieces = [...run.pieces].sort((a, b) => Number(a.gameIds.length > 1) - Number(b.gameIds.length > 1));
  return (
    <span
      className="absolute inset-y-0 overflow-hidden rounded-full"
      style={{ left: `${block.left}%`, width: `${block.width}%` }}
    >
      {pieces.map((piece) => {
        const alone = piece.gameIds.length === 1;
        const box = alone ? placeOnDay(piece.sessionsStart, piece.sessionsEnd, day) : strip;
        const place = placeOnDay(piece.start, piece.end, day);
        return (
          <span
            key={piece.start.getTime()}
            className="absolute inset-y-0"
            style={{
              ...layer(box),
              background: alone ? fillOf(piece.gameIds[0]) : hatch(piece.gameIds.map(markOf)),
              clipPath: leaningClip(box, place, piece.start > run.start, piece.end < run.end),
            }}
          />
        );
      })}
    </span>
  );
}

function DaySection({
  day,
  byGame,
  sessionsByGame,
  markOf,
  fillOf,
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
  /** What a game's time bars are filled with. */
  fillOf: (gameId: string) => string;
  events: SessionEvent[];
  now: Date;
  notes: Record<string, string>;
  onSaveNote: (sessionId: string, note: string) => Promise<void>;
  onCorrected: () => void;
  playedBefore: (gameId: string, moment: string) => number;
}) {
  const runs = playRuns(day.spans, now);
  // Parts of sessions that started the day before.
  const startedToday = new Set(day.sessions.map((session) => session.id));
  const carried = day.spans.filter((span) => countsAsPlay(span) && !startedToday.has(span.id));
  const carriedEnd = (span: Session) => {
    const end = parseVaultimeDate(span.ended_at_wall ?? span.started_at_wall);
    if (end >= addDays(day.start, 1)) return "midnight";
    const original = sessionsByGame.get(span.game_id)?.find((session) => session.id === span.id);
    return original && !original.ended_at_wall ? "now" : formatClockTime(end);
  };
  const groups = sideBySideGroups(runs.flatMap((run) => run.pieces).filter((piece) => piece.gameIds.length > 1));
  const titleOf = (gameId: string) => byGame.get(gameId)?.game.title ?? "a removed game";

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
            {runs.map((run) => (
              <PlayBlock key={run.start.getTime()} run={run} day={day.start} fillOf={fillOf} markOf={markOf} />
            ))}
          </div>
          <div aria-hidden="true" className="mt-1.5 flex justify-between font-mono text-[10px] text-faint">
            {TICKS.map((hour) => (
              <span key={hour}>{String(hour).padStart(2, "0")}</span>
            ))}
          </div>
          {groups.length > 0 && (
            <div className="mt-2.5 flex flex-col gap-1.5">
              {groups.map((group) => (
                <p key={group.gameIds.join()} className="flex items-center gap-2.5 text-[13px] text-faint">
                  <span
                    aria-hidden="true"
                    className="h-2.5 w-6 shrink-0 rounded-full"
                    style={{ backgroundImage: hatch(group.gameIds.map(markOf)) }}
                  />
                  {sideBySideSentence(group, titleOf)}
                </p>
              ))}
            </div>
          )}
        </div>

        {carried.map((span) => (
          <article key={`carried-${span.id}`} className="flex items-baseline gap-5">
            <span className="w-[20ch] shrink-0 font-mono text-[13px] text-faint">
              {formatClockTime(parseVaultimeDate(span.started_at_wall))} to {carriedEnd(span)}
            </span>
            <p className="font-prose min-w-0 flex-1 text-[20px] leading-snug text-soft">
              <PhraseText phrase={carriedOverPhrase(titleOf(span.game_id))} emColor={markOf(span.game_id)} />
            </p>
          </article>
        ))}
        {[
          ...day.sessions.map((session) => ({ at: session.started_at_wall, session, change: null })),
          ...day.changes.map((change) => ({ at: change.changed_at, session: null, change })),
        ]
          .sort((a, b) => a.at.localeCompare(b.at))
          .map(({ session, change }) => {
            if (change) {
              if (change.status === "none") return null;
              const title = titleOf(change.game_id);
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
                gameTitle={titleOf(session.game_id)}
                titleColor={markOf(session.game_id)}
                titleFill={fillOf(session.game_id) === markOf(session.game_id) ? undefined : fillOf(session.game_id)}
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
