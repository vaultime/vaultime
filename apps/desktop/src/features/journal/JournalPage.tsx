// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { useSearchParams } from "react-router";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react";
import { CARD_WIDTH_PX, cardPosition } from "@/components/charts/play-card";
import { PageHeader, StepButton } from "@/components/layout/Page";
import { PhraseText } from "@/components/media/PhraseText";
import { GameStatusIcon } from "@/components/status/GameStatusIcon";
import { useAppearance } from "@/features/appearance/appearance-context";
import { useLibrary, type GameSummary } from "@/features/library/library-context";
import { SessionLine } from "@/features/sessions/components/SessionLine";
import {
  DAY_MS,
  DAYS_PER_WEEK,
  HOURS_PER_DAY,
  JOURNAL_LINK_FLASH_MS,
  JOURNAL_MIN_SPAN_PERCENT,
  JOURNAL_TICK_HOURS,
} from "@/lib/constants";
import { markColors, tintForTitle } from "@/lib/game-tint";
import { sideBySideSentence, statusSentence, weekSentence } from "@/lib/sentences";
import { afterMidnightLead, continuedAmounts, continuedPhrase, sessionAmounts } from "@/lib/session-lines";
import {
  clipToWindow,
  countedOnce,
  countsAsPlay,
  playedByGame,
  playedMs,
  playRuns,
  sideBySideGroups,
  splitByDay,
  toDayKey,
  type DayShare,
  type PlayRun,
} from "@/lib/session-stats";
import { stepAsideWithin, stepsAside } from "@/lib/steps-aside";
import * as api from "@/lib/tauri";
import {
  clockPercent,
  formatClockTime,
  formatHoursMinutes,
  formatSpanOnDay,
  isoWeekNumber,
  parseVaultimeDate,
  startOfWeek,
  UI_LOCALE,
} from "@/lib/time";
import type { GameStatusChange, Session, SessionEvent } from "@/lib/types";
import { cn } from "@/lib/utils";

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
/** How far play that goes on past midnight runs out of the strip, fading. */
const BLEED_PX = 14;
/** A two colored blend stretched far past a fading end, so only its color on that side shows. */
const BLEND_STRETCH = "2000% 100%";
/** Gap between a marked session and the outline around it. */
const OUTLINE_GAP_PX = 3;
/** How far a strip part reaches above and below the strip for the pointer. */
const HIT_SLOP_PX = 6;

const TICKS = Array.from(
  { length: HOURS_PER_DAY / JOURNAL_TICK_HOURS + 1 },
  (_, index) => index * JOURNAL_TICK_HOURS,
);

function addDays(date: Date, days: number): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

/** The element of a session's entry or its line on a later day, for jumps between them. */
function entryId(sessionId: string, dayKey: string): string {
  return `journal-${sessionId}-${dayKey}`;
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

/**
 * The sessions and status changes of the week from `start`, by day, newest
 * day first. `drawn` are the sessions as the strip draws them, with games
 * that step aside cut to where they ran alone.
 */
function groupWeek(
  sessions: Session[],
  drawn: Session[],
  changes: GameStatusChange[],
  start: Date,
  now: Date,
): JournalDay[] {
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
    const spans = drawn.flatMap((session) => clipToWindow(session, day, next, now) ?? []);
    if (spans.some(countsAsPlay) || grouped.has(day.getTime())) dayOf(day).spans.push(...spans);
  }
  for (const change of changes) {
    const changed = parseVaultimeDate(change.changed_at);
    if (change.status === "none" || changed < start || changed >= end) continue;
    dayOf(changed).changes.push(change);
  }
  // Parts cut at midnight keep the order their sessions began in, so games
  // side by side past midnight keep their order and stripes on the next day.
  const began = new Map(sessions.map((session) => [session.id, session.started_at_wall]));
  const byStart = (a: Session, b: Session) =>
    a.started_at_wall.localeCompare(b.started_at_wall) ||
    (began.get(a.id) ?? "").localeCompare(began.get(b.id) ?? "");
  for (const day of grouped.values()) {
    day.sessions.sort((a, b) => a.started_at_wall.localeCompare(b.started_at_wall));
    day.spans.sort(byStart);
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
/**
 * Chromium, which WebView2 runs on, opens a date picker for a date field.
 * WebKitGTK may not, so there the field itself shows up to type a day.
 */
const HAS_DATE_PICKER = typeof navigator !== "undefined" && /Chrome\//.test(navigator.userAgent);

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
  const dateInput = useRef<HTMLInputElement>(null);
  // Shown as a field to type a day in where the webview has no date picker.
  const [typingDay, setTypingDay] = useState(false);

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

  const sessionsById = useMemo(() => new Map(sessions.map((session) => [session.id, session])), [sessions]);
  const stepping = new Set(summaries.filter((summary) => stepsAside(summary.game)).map((summary) => summary.game.id));
  // Sessions that began in the week may run on into the next one, and the
  // entry says how much of them counts there, so the window reaches that far.
  const reach = sessions.reduce((latest, session) => {
    const started = parseVaultimeDate(session.started_at_wall);
    if (started < weekStart || started >= weekEnd) return latest;
    const ended = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : now;
    return ended > latest ? ended : latest;
  }, weekEnd);
  // A game that steps aside shows only where no other game ran.
  const drawn = stepAsideWithin(sessions, stepping, weekStart, reach, now);
  const days = groupWeek(sessions, drawn, statusChanges, weekStart, now);
  const partsById = new Map<string, Session[]>();
  for (const part of drawn) partsById.set(part.id, [...(partsById.get(part.id) ?? []), part]);
  const sharesCache = new Map<string, DayShare[]>();
  /** A session cut at midnight, each day with the time it counts there. */
  const sharesOf = (session: Session) => {
    const cached = sharesCache.get(session.id);
    if (cached) return cached;
    const shares = splitByDay(session, partsById.get(session.id) ?? [], now);
    sharesCache.set(session.id, shares);
    return shares;
  };
  const carriedOver = sessions.filter(
    (session) =>
      countsAsPlay(session) &&
      parseVaultimeDate(session.started_at_wall) < weekStart &&
      clipToWindow(session, weekStart, weekEnd, now) !== null,
  ).length;

  // The session whose parts are marked on every day, while one of them is
  // pointed at or focused, or for a moment after a jump between them.
  const [linked, setLinked] = useState<string | null>(null);
  // The line a jump goes to, scrolled to once its week is on the page, which
  // is the render right after the jump.
  const jumpTarget = useRef<string | null>(null);
  const flashTimer = useRef<number | undefined>(undefined);
  const jumpTo = (sessionId: string, day: Date) => {
    const dayKey = toDayKey(day);
    jumpTarget.current = entryId(sessionId, dayKey);
    setOffset(weeksBack(dayKey));
    setLinked(sessionId);
    window.clearTimeout(flashTimer.current);
    flashTimer.current = window.setTimeout(
      () => setLinked((current) => (current === sessionId ? null : current)),
      JOURNAL_LINK_FLASH_MS,
    );
  };
  useEffect(() => {
    if (!jumpTarget.current) return;
    const target = document.getElementById(jumpTarget.current);
    jumpTarget.current = null;
    if (!target) return;
    target.focus({ preventScroll: true });
    target.scrollIntoView({ block: "center", behavior: "smooth" });
  });
  useEffect(() => () => window.clearTimeout(flashTimer.current), []);

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
      .reduce((sum, session) => sum + countedOnce(session).runtimeMs, byGame.get(gameId)?.earlier?.earlier_ms ?? 0);

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
  // Each game's own time in the week, so games side by side both count, and
  // two installs of one game running at once count once.
  const weekRuntime = playedByGame(days.flatMap((day) => day.spans), now);
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
    carriedOver,
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
            <StepButton
              label="Go to a day"
              disabled={!earliest}
              onClick={() => {
                try {
                  if (!HAS_DATE_PICKER) throw new Error("no date picker");
                  dateInput.current?.showPicker();
                } catch {
                  setTypingDay(true);
                  setTimeout(() => dateInput.current?.focus());
                }
              }}
            >
              <CalendarDays className="size-[17px]" strokeWidth={1.8} />
            </StepButton>
            <input
              ref={dateInput}
              type="date"
              aria-label="Go to the week of a day"
              tabIndex={typingDay ? 0 : -1}
              className={
                typingDay
                  ? "h-9 rounded-md border border-rule bg-raised px-2 font-mono text-[13px] text-text"
                  : "sr-only"
              }
              min={earliest ? toDayKey(earliest) : undefined}
              max={toDayKey(now)}
              onChange={(event) => {
                if (!event.target.value) return;
                setOffset(weeksBack(event.target.value));
                setTypingDay(false);
              }}
              onBlur={() => setTypingDay(false)}
            />
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
            sessionsById={sessionsById}
            sharesOf={sharesOf}
            markOf={markOf}
            fillOf={fillOf}
            events={events}
            now={now}
            notes={notes}
            onSaveNote={saveNote}
            onCorrected={() => void refresh()}
            playedBefore={playedBefore}
            linked={linked}
            onLink={setLinked}
            onJump={jumpTo}
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
  openStart,
  openEnd,
}: {
  run: PlayRun;
  day: Date;
  fillOf: (gameId: string) => string;
  markOf: (gameId: string) => string;
  /** Play went on from the day before, so the block runs in from the left edge. */
  openStart: boolean;
  /** Play goes on past midnight, so the block runs out at the right edge. */
  openEnd: boolean;
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
  // At midnight the block keeps going out of the strip and fades, in the
  // colors of the games that ran on, so it reads as cut by the day, not ended.
  // A fill in two colors shows the end of its blend on that side.
  const bleed = (side: "left" | "right", gameIds: string[]) => {
    const fill = fillOf(gameIds[0]);
    const blend = fill === markOf(gameIds[0]) ? undefined : fill;
    return (
      <span
        className="absolute inset-y-0"
        style={{
          [side]: `-${BLEED_PX}px`,
          width: BLEED_PX,
          backgroundColor: markOf(gameIds[0]),
          backgroundImage: gameIds.length > 1 ? hatch(gameIds.map(markOf)) : blend,
          backgroundSize: gameIds.length > 1 ? undefined : BLEND_STRETCH,
          backgroundPosition: side,
          maskImage: `linear-gradient(to ${side}, black, transparent)`,
        }}
      />
    );
  };
  return (
    <>
      {openStart && bleed("left", run.pieces[0].gameIds)}
      {openEnd && bleed("right", run.pieces[run.pieces.length - 1].gameIds)}
      <span
        className={cn("absolute inset-y-0 overflow-hidden", !openStart && "rounded-l-full", !openEnd && "rounded-r-full")}
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
    </>
  );
}

/** A session's part on a day's strip, with whether it runs in from or out to midnight. */
interface StripPart {
  session: Session;
  place: Place;
  openStart: boolean;
  openEnd: boolean;
}

/**
 * What a session on the strip is, for the pointer: its game, when it ran,
 * and with play past midnight each day's part and the whole.
 */
function SessionCard({
  session,
  shares,
  day,
  title,
  mark,
  style,
}: {
  session: Session;
  shares: DayShare[];
  day: Date;
  title: string;
  mark: string;
  style: CSSProperties;
}) {
  const started = parseVaultimeDate(session.started_at_wall);
  const ended = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : null;
  const dayName = (share: DayShare) =>
    shares.length > DAYS_PER_WEEK
      ? share.day.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "short" })
      : share.day.toLocaleDateString(UI_LOCALE, { weekday: "long" });
  return (
    <div
      aria-hidden="true"
      style={{ width: CARD_WIDTH_PX, ...style }}
      className="pointer-events-none absolute z-20 rounded-xl border border-hairline bg-surface px-4 py-3 shadow-lg shadow-scrim/40"
    >
      <div className="flex items-center gap-2.5 text-[13px]">
        <span className="size-2 shrink-0 rounded-full" style={{ background: mark }} />
        <span className="min-w-0 flex-1 truncate">{title}</span>
      </div>
      <div className="mt-1 font-mono text-xs text-faint">{formatSpanOnDay(started, ended, shares[0].day)}</div>
      {shares.length > 1 ? (
        <>
          <ul className="mt-2.5 flex flex-col gap-1">
            {shares.map((share) => {
              const here = share.day.getTime() === day.getTime();
              return (
                <li key={share.day.getTime()} className={cn("flex items-baseline gap-2.5 text-[13px]", !here && "text-faint")}>
                  <span className="min-w-0 flex-1 truncate">{dayName(share)}</span>
                  <span className="shrink-0 font-mono text-xs tabular-nums">{formatHoursMinutes(share.runtimeMs)}</span>
                </li>
              );
            })}
          </ul>
          <div className="mt-2 flex items-baseline gap-2.5 border-t border-rule pt-2 text-[13px] text-soft">
            <span className="min-w-0 flex-1">{ended ? "In all" : "So far"}</span>
            <span className="shrink-0 font-mono text-xs tabular-nums">{formatHoursMinutes(session.runtime_ms)}</span>
          </div>
        </>
      ) : (
        <div className="mt-1.5 text-xs text-soft">{sessionAmounts(session)}</div>
      )}
    </div>
  );
}

/** A link inside a sentence, to the other part of a session past midnight. */
function JumpLink({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className="cursor-pointer rounded-[2px] underline decoration-hairline-strong decoration-1 underline-offset-[0.2em] transition-colors hover:decoration-current focus-visible:decoration-current focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet/50"
    >
      {children}
    </button>
  );
}

/** "Thursday", "Thursday and Friday" or "Thursday to Sunday", each day a link to that day. */
function DayLinks({ shares, onJump }: { shares: DayShare[]; onJump: (day: Date) => void }) {
  const link = (share: DayShare) => {
    const name = share.day.toLocaleDateString(UI_LOCALE, { weekday: "long" });
    return (
      <JumpLink label={`${name}, where this session went on`} onClick={() => onJump(share.day)}>
        {name}
      </JumpLink>
    );
  };
  const first = shares[0];
  const last = shares[shares.length - 1];
  if (shares.length === 1) return link(first);
  return (
    <>
      {link(first)}
      {shares.length === 2 ? " and " : " to "}
      {link(last)}
    </>
  );
}

function DaySection({
  day,
  byGame,
  sessionsByGame,
  sessionsById,
  sharesOf,
  markOf,
  fillOf,
  events,
  now,
  notes,
  onSaveNote,
  onCorrected,
  playedBefore,
  linked,
  onLink,
  onJump,
}: {
  day: JournalDay;
  byGame: Map<string, GameSummary>;
  sessionsByGame: Map<string, Session[]>;
  sessionsById: Map<string, Session>;
  /** A session cut at midnight, each day with the time it counts there. */
  sharesOf: (session: Session) => DayShare[];
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
  /** The session marked on every day it ran. */
  linked: string | null;
  onLink: (sessionId: string | null) => void;
  /** Goes to a session's entry or line on a day, in another week too. */
  onJump: (sessionId: string, day: Date) => void;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const [card, setCard] = useState<{ sessionId: string; style: CSSProperties } | null>(null);
  const nextDay = addDays(day.start, 1);
  const dayKey = toDayKey(day.start);
  const runs = playRuns(day.spans, now);
  const titleOf = (gameId: string) => byGame.get(gameId)?.game.title ?? "a removed game";
  const fillFor = (gameId: string) => (fillOf(gameId) === markOf(gameId) ? undefined : fillOf(gameId));

  // Each session's parts on the strip, and whether they run in from the day
  // before or out past midnight.
  const parts: StripPart[] = day.spans.filter(countsAsPlay).map((span) => {
    const original = sessionsById.get(span.id) ?? span;
    const start = parseVaultimeDate(span.started_at_wall);
    const end = parseVaultimeDate(span.ended_at_wall ?? span.started_at_wall);
    const originalEnd = original.ended_at_wall ? parseVaultimeDate(original.ended_at_wall) : null;
    return {
      session: original,
      place: placeOnDay(start, end, day.start),
      openStart: start.getTime() === day.start.getTime() && parseVaultimeDate(original.started_at_wall) < day.start,
      openEnd: end.getTime() === nextDay.getTime() && (!originalEnd || originalEnd > nextDay),
    };
  });
  const runOpenStart = (run: PlayRun) =>
    run.start.getTime() === day.start.getTime() && parts.some((part) => part.openStart);
  const runOpenEnd = (run: PlayRun) => run.end.getTime() === nextDay.getTime() && parts.some((part) => part.openEnd);

  // Sessions that began on an earlier day get a quiet line that leads back
  // to their entry, one per session however many parts it has here.
  const startedToday = new Set(day.sessions.map((session) => session.id));
  const continued = [...new Map(parts.map((part) => [part.session.id, part.session])).values()]
    .filter((session) => !startedToday.has(session.id))
    .sort((a, b) => a.started_at_wall.localeCompare(b.started_at_wall));
  const groups = sideBySideGroups(runs.flatMap((run) => run.pieces).filter((piece) => piece.gameIds.length > 1));

  /** Pointing at or focusing a session's line marks all its parts, on this day and others. */
  const linkProps = (sessionId: string) => ({
    onMouseEnter: () => onLink(sessionId),
    onMouseLeave: () => onLink(null),
    onFocus: () => onLink(sessionId),
    onBlur: () => onLink(null),
  });
  const showCard = (sessionId: string, anchor: Element) => {
    if (!frame.current) return;
    onLink(sessionId);
    setCard({ sessionId, style: cardPosition(anchor.getBoundingClientRect(), frame.current.getBoundingClientRect()) });
  };
  const cardSession = card ? sessionsById.get(card.sessionId) : undefined;
  // Outlines fade out past an open end, like the play they mark.
  const outlineMask = (part: StripPart) => {
    const from = part.openStart ? `transparent, black ${BLEED_PX}px` : "black";
    const to = part.openEnd ? `black calc(100% - ${BLEED_PX}px), transparent` : "black";
    return `linear-gradient(to right, ${from}, ${to})`;
  };

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
          <div ref={frame} className="relative">
            <div aria-hidden="true" className="relative h-2.5 rounded-full bg-raised">
              {runs.map((run) => (
                <PlayBlock
                  key={run.start.getTime()}
                  run={run}
                  day={day.start}
                  fillOf={fillOf}
                  markOf={markOf}
                  openStart={runOpenStart(run)}
                  openEnd={runOpenEnd(run)}
                />
              ))}
              {parts
                .filter((part) => part.session.id === linked)
                .map((part) => (
                  <span
                    key={`outline-${part.place.left}`}
                    className={cn(
                      "pointer-events-none absolute border-[1.5px]",
                      part.openStart ? "border-l-0" : "rounded-l-full",
                      part.openEnd ? "border-r-0" : "rounded-r-full",
                    )}
                    style={{
                      top: -OUTLINE_GAP_PX,
                      bottom: -OUTLINE_GAP_PX,
                      left: part.openStart ? -BLEED_PX : `calc(${part.place.left}% - ${OUTLINE_GAP_PX}px)`,
                      right: part.openEnd
                        ? -BLEED_PX
                        : `calc(${100 - part.place.left - part.place.width}% - ${OUTLINE_GAP_PX}px)`,
                      borderColor: markOf(part.session.game_id),
                      maskImage: outlineMask(part),
                    }}
                  />
                ))}
              {/* Shorter parts on top, so a session beside a long one can still be pointed at. */}
              {[...parts]
                .sort((a, b) => b.place.width - a.place.width)
                .map((part) => (
                  <span
                    key={`hit-${part.session.id}-${part.place.left}`}
                    className="absolute"
                    style={{
                      left: `${part.place.left}%`,
                      width: `${part.place.width}%`,
                      top: -HIT_SLOP_PX,
                      bottom: -HIT_SLOP_PX,
                    }}
                    onMouseEnter={(event) => showCard(part.session.id, event.currentTarget)}
                    onMouseLeave={() => {
                      onLink(null);
                      setCard(null);
                    }}
                  />
                ))}
            </div>
            <div aria-hidden="true" className="mt-1.5 flex justify-between font-mono text-[10px] text-faint">
              {TICKS.map((hour) => (
                <span key={hour}>{String(hour).padStart(2, "0")}</span>
              ))}
            </div>
            {card && cardSession && (
              <SessionCard
                session={cardSession}
                shares={sharesOf(cardSession)}
                day={day.start}
                title={titleOf(cardSession.game_id)}
                mark={markOf(cardSession.game_id)}
                style={card.style}
              />
            )}
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

        {continued.map((session) => {
          const shares = sharesOf(session);
          const share = shares.find((each) => each.day.getTime() === day.start.getTime());
          if (!share) return null;
          const phrase = continuedPhrase(session, titleOf(session.game_id), day.start, { throughDay: share.intoNext });
          const mark = markOf(session.game_id);
          const started = parseVaultimeDate(session.started_at_wall);
          const ended = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : null;
          return (
            <article
              key={`continued-${session.id}`}
              id={entryId(session.id, dayKey)}
              tabIndex={-1}
              {...linkProps(session.id)}
              className="flex items-baseline gap-5 outline-none"
            >
              <span
                className="w-[20ch] shrink-0 font-mono text-[13px] text-faint transition-colors"
                style={linked === session.id ? { color: mark } : undefined}
              >
                {formatSpanOnDay(started, ended, day.start)}
              </span>
              <div className="min-w-0 flex-1">
                <p className="font-prose text-[19px] leading-snug text-pretty text-soft">
                  <PhraseText phrase={{ before: phrase.before, em: phrase.em }} emColor={mark} emFill={fillFor(session.game_id)} />
                  {phrase.middle}
                  {/* A link is a box of its own, so the stop after it is kept on its line. */}
                  <span className="whitespace-nowrap">
                    <JumpLink
                      label={`${phrase.link}, where this session began`}
                      onClick={() => onJump(session.id, shares[0].day)}
                    >
                      {phrase.link}
                    </JumpLink>
                    {!phrase.after && "."}
                  </span>
                  {phrase.after && `${phrase.after}.`}
                </p>
                <div className="mt-1 text-[13px] text-faint">
                  {continuedAmounts(share.runtimeMs, session.runtime_ms, !session.ended_at_wall)}
                </div>
              </div>
            </article>
          );
        })}
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
            const later = sharesOf(session).slice(1);
            const laterMs = later.reduce((sum, share) => sum + share.runtimeMs, 0);
            const ended = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : null;
            return (
              <div
                key={session.id}
                id={entryId(session.id, dayKey)}
                tabIndex={-1}
                {...linkProps(session.id)}
                className="outline-none"
              >
                <SessionLine
                  session={session}
                  events={events}
                  gameTitle={titleOf(session.game_id)}
                  titleColor={markOf(session.game_id)}
                  titleFill={fillFor(session.game_id)}
                  gameSessions={sessionsByGame.get(session.game_id)}
                  earlierMs={byGame.get(session.game_id)?.earlier?.earlier_ms}
                  when={formatSpanOnDay(parseVaultimeDate(session.started_at_wall), ended, day.start)}
                  highlighted={linked === session.id}
                  amountsNote={
                    later.length > 0 && (
                      <>
                        {afterMidnightLead(laterMs)}
                        <span className="whitespace-nowrap">
                          <DayLinks shares={later} onJump={(target) => onJump(session.id, target)} />.
                        </span>
                      </>
                    )
                  }
                  bordered={false}
                  note={notes[session.id]}
                  onSaveNote={(note) => onSaveNote(session.id, note)}
                  onCorrected={onCorrected}
                />
              </div>
            );
          })}
      </div>
    </section>
  );
}

