// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { DAY_MS, DAYS_PER_WEEK } from "@/lib/constants";
import { clipToWindow, countsAsPlay, playedMs, splitByDay, type DayShare } from "@/lib/session-stats";
import { stepAsideWithin } from "@/lib/steps-aside";
import { parseVaultimeDate, startOfWeek } from "@/lib/time";
import type { GameStatusChange, Session } from "@/lib/types";

export interface JournalDay {
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

export interface JournalWeek {
  /** The moment the week was worked out at, the end of running sessions. */
  now: Date;
  start: Date;
  end: Date;
  /** Days with play or status changes, newest first. */
  days: JournalDay[];
  /** A session cut at midnight, each day with the time it counts there. */
  sharesOf: (session: Session) => DayShare[];
  /** Games that played in the week, in the order they first showed up. */
  games: string[];
  /** Sessions that began before the week and ran on into it. */
  carriedOver: number;
}

export function addDays(date: Date, days: number): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

/** The element of a session's entry or its line on a later day, for jumps between them. */
export function entryId(sessionId: string, dayKey: string): string {
  return `journal-${sessionId}-${dayKey}`;
}

/** How many weeks back the week of `day` ("2026-09-28") lies, 0 for this week or no day. */
export function weeksBack(day: string | null): number {
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

/**
 * The week `offset` weeks from this one, 0 for this week. `stepping` are the
 * games that step aside, shown only where no other game ran.
 */
export function buildWeek(
  sessions: Session[],
  changes: GameStatusChange[],
  stepping: Set<string>,
  offset: number,
  now: Date,
): JournalWeek {
  const start = addDays(startOfWeek(now), offset * DAYS_PER_WEEK);
  const end = addDays(start, DAYS_PER_WEEK);
  // Sessions that began in the week may run on into the next one, and the
  // entry says how much of them counts there, so the window reaches that far.
  const reach = sessions.reduce((latest, session) => {
    const started = parseVaultimeDate(session.started_at_wall);
    if (started < start || started >= end) return latest;
    const ended = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : now;
    return ended > latest ? ended : latest;
  }, end);
  // A game that steps aside shows only where no other game ran.
  const drawn = stepAsideWithin(sessions, stepping, start, reach, now);
  const days = groupWeek(sessions, drawn, changes, start, now);
  const partsById = new Map<string, Session[]>();
  for (const part of drawn) partsById.set(part.id, [...(partsById.get(part.id) ?? []), part]);
  const sharesCache = new Map<string, DayShare[]>();
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
      parseVaultimeDate(session.started_at_wall) < start &&
      clipToWindow(session, start, end, now) !== null,
  ).length;
  const games = [
    ...new Set([...days].reverse().flatMap((day) => day.spans.filter(countsAsPlay).map((session) => session.game_id))),
  ];
  return { now, start, end, days, sharesOf, games, carriedOver };
}
