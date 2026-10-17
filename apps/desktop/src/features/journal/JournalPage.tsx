// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react";
import { PageHeader, StepButton } from "@/components/layout/Page";
import { PhraseText } from "@/components/media/PhraseText";
import { useAppearance } from "@/features/appearance/appearance-context";
import { useActiveSessions, useLibrary } from "@/features/library/library-context";
import { JOURNAL_LINK_FLASH_MS } from "@/lib/constants";
import { markColors, tintForTitle } from "@/lib/game-tint";
import { weekSentence } from "@/lib/sentences";
import { countedOnce, countsAsPlay, playedByGame, toDayKey } from "@/lib/session-stats";
import { stepsAside } from "@/lib/steps-aside";
import * as api from "@/lib/tauri";
import { isoWeekNumber, parseVaultimeDate, startOfWeek, UI_LOCALE } from "@/lib/time";
import type { Session, SessionEvent } from "@/lib/types";
import { DaySection } from "./DaySection";
import { buildWeek, entryId, weeksBack, type JournalDay } from "./week";

/**
 * Chromium, which WebView2 runs on, opens a date picker for a date field.
 * WebKitGTK may not, so there the field itself shows up to type a day.
 */
const HAS_DATE_PICKER = typeof navigator !== "undefined" && /Chrome\//.test(navigator.userAgent);

/** Whether a session has an entry or a part on the day. */
function hasSession(day: JournalDay, sessionId: string): boolean {
  return day.spans.some((span) => span.id === sessionId) || day.sessions.some((session) => session.id === sessionId);
}

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
  // 0 is this week, -1 the week before and so on. `?week=2026-09-28` opens
  // the week of that day, as the stats page asks for.
  const [params] = useSearchParams();
  const [offset, setOffset] = useState(() => weeksBack(params.get("week")));
  const [events, setEvents] = useState<SessionEvent[]>([]);
  const dateInput = useRef<HTMLInputElement>(null);
  // Shown as a field to type a day in where the webview has no date picker.
  const [typingDay, setTypingDay] = useState(false);

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
  const stepping = useMemo(
    () => new Set(summaries.filter((summary) => stepsAside(summary.game)).map((summary) => summary.game.id)),
    [summaries],
  );
  // Worked out again when the sessions change or a new day begins, not each
  // time a line is pointed at.
  const week = useMemo(
    () => buildWeek(sessions, statusChanges, stepping, offset, new Date()),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a new day moves the week along
    [sessions, statusChanges, stepping, offset, today],
  );
  const { now, start: weekStart, end: weekEnd, days, sharesOf, carriedOver, games: weekGames } = week;

  // The session whose parts are marked on every day, while one of them is
  // pointed at or focused, or for a moment after a jump between them.
  const [linked, setLinked] = useState<string | null>(null);
  // The line a jump goes to, scrolled to once its week is on the page, which
  // is the render right after the jump.
  const jumpTarget = useRef<string | null>(null);
  const flashTimer = useRef<number | undefined>(undefined);
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
  const { markOf, fillOf } = useMemo(() => {
    const paletteOf = (gameId: string) => {
      const summary = byGame.get(gameId);
      return (summary?.tint ?? tintForTitle(summary?.game.title ?? "")).colors;
    };
    const weekMarks = markColors(weekGames.map(paletteOf), accentHues, mode);
    const marks = new Map(weekGames.map((gameId, index) => [gameId, weekMarks[index]]));
    const markFor = (gameId: string) => marks.get(gameId) ?? markColors([paletteOf(gameId)], [], mode)[0];
    return {
      markOf: (gameId: string) => markFor(gameId).color,
      fillOf: (gameId: string) => markFor(gameId).fill,
    };
  }, [weekGames, byGame, accentHues, mode]);

  /** A game's playtime up to a moment, with the playtime from before Vaultime. */
  const playedBefore = useCallback(
    (gameId: string, moment: string) =>
      sessions
        .filter((session) => session.game_id === gameId && session.started_at_wall < moment)
        .reduce((sum, session) => sum + countedOnce(session).runtimeMs, byGame.get(gameId)?.earlier?.earlier_ms ?? 0),
    [sessions, byGame],
  );
  const onCorrected = useCallback(() => void refresh(), [refresh]);

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

  const earliest = useMemo(
    () =>
      stored.reduce<Date | null>((min, session) => {
        const started = parseVaultimeDate(session.started_at_wall);
        return !min || started < min ? started : min;
      }, null),
    [stored],
  );
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
            onCorrected={onCorrected}
            playedBefore={playedBefore}
            linked={linked !== null && hasSession(day, linked) ? linked : null}
            onLink={setLinked}
            onJump={jumpTo}
          />
        ))}
      </div>
    </div>
  );
}
