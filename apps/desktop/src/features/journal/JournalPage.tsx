// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react";
import { Notice, PageHeader, StepButton } from "@/components/layout/Page";
import { PhraseText } from "@/components/media/PhraseText";
import { useAppearance } from "@/features/appearance/appearance-context";
import { useActiveSessions, useLibrary } from "@/features/library/library-context";
import { DAYS_PER_WEEK, JOURNAL_LINK_FLASH_MS, LIVE_TOTALS_REFRESH_MS } from "@/lib/constants";
import { markColors, tintForTitle } from "@/lib/game-tint";
import { weekSentence } from "@/lib/sentences";
import * as api from "@/lib/tauri";
import { formatDate, isoWeekNumber, parseVaultimeDate, startOfDay, startOfWeek, toDayKey } from "@/lib/time";
import type { DayShare, JournalWeekPlay, Session, SessionEvent } from "@/lib/types";
import { useRefreshWhile } from "@/lib/use-refresh-while";
import { scrollMotion } from "@/lib/anchors";
import { describeError, logFailure } from "@/lib/utils";
import { DaySection } from "./DaySection";
import { dayId, entryId, journalDays, weekStart, weeksBack, type JournalDay } from "./week";

/**
 * Chromium, which WebView2 runs on, opens a date picker for a date field.
 * WebKitGTK may not, so there the field itself shows up to type a day.
 */
const HAS_DATE_PICKER = typeof navigator !== "undefined" && /Chrome\//.test(navigator.userAgent);

/** Whether a session has an entry or a part on the day. */
function hasSession(day: JournalDay, sessionId: string): boolean {
  return day.parts.some((part) => part.session_id === sessionId) || day.sessionIds.includes(sessionId);
}

const NO_SHARES: DayShare[] = [];

/** Play history week by week, one sentence per session. */
export function JournalPage() {
  const { sessions: stored, summaries, statusChanges, notes, saveNote, refresh, today, loaded } = useLibrary();
  const active = useActiveSessions();
  // Running sessions as of the last poll, a few seconds old at most. The
  // full list only reloads when a session starts or ends.
  const sessions = useMemo(() => {
    const running = new Map(active.map((session) => [session.id, session]));
    return stored.map((session) => running.get(session.id) ?? session);
  }, [stored, active]);
  const { accentHues, mode } = useAppearance();
  // 0 is this week, -1 the week before and so on. `?day=2026-09-28` opens
  // the week of that day and goes to the day, as the stats page asks for.
  const [params] = useSearchParams();
  const askedDay = params.get("day");
  const [offset, setOffset] = useState(() => weeksBack(askedDay));
  // The week on the page as the core worked it out. It stays until the next
  // one comes, so moving between weeks does not flash an empty page.
  const [shown, setShown] = useState<{ offset: number; start: Date; play: JournalWeekPlay } | null>(null);
  const [weekError, setWeekError] = useState<string | null>(null);
  const [events, setEvents] = useState<SessionEvent[]>([]);
  const dateInput = useRef<HTMLInputElement>(null);
  // Shown as a field to type a day in where the webview has no date picker.
  const [typingDay, setTypingDay] = useState(false);

  // Read again when a session starts or ends, when a status changes, on a
  // new day, and while a game runs and this week is on the page, as its time
  // grows.
  const refreshed = useRefreshWhile(active.length > 0 && offset === 0, LIVE_TOTALS_REFRESH_MS);
  useEffect(() => {
    let cancelled = false;
    const start = weekStart(offset, new Date());
    api
      .getJournalWeek(toDayKey(start))
      .then((play) => {
        if (cancelled) return;
        setShown({ offset, start, play });
        setWeekError(null);
      })
      .catch((error) => {
        if (!cancelled) setWeekError(describeError(error));
      });
    return () => {
      cancelled = true;
    };
  }, [offset, stored, statusChanges, today, refreshed]);

  const byGame = useMemo(() => new Map(summaries.map((summary) => [summary.game.id, summary])), [summaries]);
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
  const days = useMemo(() => (shown ? journalDays(shown.play) : []), [shown]);
  const sharesOf = useCallback((sessionId: string) => shown?.play.shares[sessionId] ?? NO_SHARES, [shown]);
  const weekGames = shown?.play.games;

  // The session whose parts are marked on every day, while one of them is
  // pointed at or focused, or for a moment after a jump between them.
  const [linked, setLinked] = useState<string | null>(null);
  // The line or day a jump goes to, scrolled to once its week is on the page.
  const jumpTarget = useRef<string | null>(askedDay ? dayId(askedDay) : null);
  const flashTimer = useRef<number | undefined>(undefined);
  // The day whose name is marked for a moment after a jump to it.
  const [flashedDay, setFlashedDay] = useState<string | null>(askedDay);
  const dayFlashTimer = useRef<number | undefined>(undefined);
  const goToDay = useCallback((dayKey: string) => {
    jumpTarget.current = dayId(dayKey);
    setOffset(weeksBack(dayKey));
    setFlashedDay(dayKey);
  }, []);
  const jumpTo = useCallback((sessionId: string, day: Date) => {
    const dayKey = toDayKey(day);
    jumpTarget.current = entryId(sessionId, dayKey);
    setOffset(weeksBack(dayKey));
    setLinked(sessionId);
    window.clearTimeout(flashTimer.current);
    flashTimer.current = window.setTimeout(
      () => setLinked((current) => (current === sessionId ? null : current)),
      JOURNAL_LINK_FLASH_MS,
    );
  }, []);
  useEffect(() => {
    if (!jumpTarget.current || shown?.offset !== offset) return;
    const day = jumpTarget.current.startsWith(dayId(""));
    const target = document.getElementById(jumpTarget.current);
    jumpTarget.current = null;
    if (day) {
      window.clearTimeout(dayFlashTimer.current);
      dayFlashTimer.current = window.setTimeout(() => setFlashedDay(null), JOURNAL_LINK_FLASH_MS);
    }
    // A day without play has no section, so the week stays at its top.
    if (!target) return;
    target.focus({ preventScroll: true });
    target.scrollIntoView({ block: day ? "start" : "center", behavior: scrollMotion() });
  });
  useEffect(
    () => () => {
      window.clearTimeout(flashTimer.current);
      window.clearTimeout(dayFlashTimer.current);
    },
    [],
  );

  // A game keeps one color through the week, the main color of its artwork,
  // nudged only when it would look like a game that showed up earlier. The
  // accent stays free for active time.
  const { markOf, fillOf } = useMemo(() => {
    const paletteOf = (gameId: string) => {
      const summary = byGame.get(gameId);
      return (summary?.tint ?? tintForTitle(summary?.game.title ?? "")).colors;
    };
    const games = weekGames ?? [];
    const weekMarks = markColors(games.map(paletteOf), accentHues, mode);
    const marks = new Map(games.map((gameId, index) => [gameId, weekMarks[index]]));
    const markFor = (gameId: string) => marks.get(gameId) ?? markColors([paletteOf(gameId)], [], mode)[0];
    return {
      markOf: (gameId: string) => markFor(gameId).color,
      fillOf: (gameId: string) => markFor(gameId).fill,
    };
  }, [weekGames, byGame, accentHues, mode]);

  const onCorrected = useCallback(() => void refresh(), [refresh]);

  // Events explain flagged sessions and skipped sleep. Only the games of this week are loaded.
  const gameIds = [...new Set(days.flatMap((day) => day.parts.map((part) => part.game_id)))].sort().join(",");
  useEffect(() => {
    let cancelled = false;
    Promise.all(gameIds ? gameIds.split(",").map((id) => api.getNotableEventsForGame(id)) : [])
      .then((lists) => {
        if (!cancelled) setEvents(lists.flat());
      })
      .catch(logFailure("loading what explains the week's flagged sessions"));
    return () => {
      cancelled = true;
    };
  }, [gameIds, stored]);

  const earliest = useMemo(
    () =>
      stored.reduce<Date | null>((min, session) => {
        const started = parseVaultimeDate(session.started_at_wall);
        return !min || started < min ? started : min;
      }, null),
    [stored],
  );

  if (!loaded) return null;
  if (weekError) {
    return (
      <div className="px-8 pt-12 xl:px-14">
        <Notice tone="warning">The week could not be read: {weekError}</Notice>
      </div>
    );
  }
  if (!shown) return null;

  const { play, start, offset: shownOffset } = shown;
  const canGoBack = earliest !== null && startOfWeek(earliest) < start;
  const now = new Date();
  const weekEnd = startOfDay(start, DAYS_PER_WEEK);
  const [top] = play.game_times;
  const gamesMs = play.game_times.reduce((sum, game) => sum + game.runtime_ms, 0);
  const title =
    shownOffset === 0
      ? "This week"
      : shownOffset === -1
        ? "Last week"
        : `Week of ${formatDate(start, { day: "numeric", month: "short" })}`;
  const sentence = weekSentence({
    sessionsCount: play.sessions_count,
    runtimeMs: play.played_ms,
    longestDay: play.longest_day ? formatDate(parseVaultimeDate(play.longest_day), { weekday: "long" }) : null,
    daysPlayed: play.days_played,
    topTitle: top ? (byGame.get(top.game_id)?.game.title ?? null) : null,
    topShare: top && gamesMs > 0 ? top.runtime_ms / gamesMs : 0,
    gamesCount: play.games.length,
    weekNumber: isoWeekNumber(start),
    carriedOver: play.carried_over,
    current: shownOffset === 0,
  });
  const weekYear = weekEnd.getFullYear() === now.getFullYear() ? "" : `, ${start.getFullYear()}`;

  return (
    <div className="pb-16">
      <PageHeader
        overline={`Journal, week ${isoWeekNumber(start)}${weekYear}`}
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
              aria-label="Go to a day"
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
                goToDay(event.target.value);
                setTypingDay(false);
              }}
              onBlur={() => setTypingDay(false)}
            />
            <StepButton label="Previous week" disabled={!canGoBack} onClick={() => setOffset(shownOffset - 1)}>
              <ChevronLeft className="size-[18px]" strokeWidth={1.8} />
            </StepButton>
            <StepButton label="Next week" disabled={shownOffset >= 0} onClick={() => setOffset(shownOffset + 1)}>
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
            notes={notes}
            onSaveNote={saveNote}
            onCorrected={onCorrected}
            flashed={flashedDay === toDayKey(day.start)}
            linked={linked !== null && hasSession(day, linked) ? linked : null}
            onLink={setLinked}
            onJump={jumpTo}
          />
        ))}
      </div>
    </div>
  );
}
