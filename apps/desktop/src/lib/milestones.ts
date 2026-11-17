// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Hour marks in a game: ten hours, twenty-five, fifty, a hundred and on. A
// game's hours are its counted time with its playtime from before Vaultime,
// as its totals have them.

import { HOUR_MS, MILESTONE_HOURS, MILESTONE_STEP_HOURS } from "@/lib/constants";
import { countsAsPlay } from "@/lib/sessions";
import { compareTimes } from "@/lib/time";
import type { Session } from "@/lib/types";
import { numberWords } from "@/lib/words";

/** The first hour mark past `hours`. */
export function markAfter(hours: number): number {
  return (
    MILESTONE_HOURS.find((mark) => mark > hours) ??
    (Math.floor(hours / MILESTONE_STEP_HOURS) + 1) * MILESTONE_STEP_HOURS
  );
}

/** The hour mark before `mark`, 0 before the first. */
export function markBefore(mark: number): number {
  const index = MILESTONE_HOURS.indexOf(mark);
  if (index >= 0) return MILESTONE_HOURS[index - 1] ?? 0;
  return Math.max(mark - MILESTONE_STEP_HOURS, MILESTONE_HOURS[MILESTONE_HOURS.length - 1]);
}

/** The hour marks past `fromMs` up to and including `toMs`, the smallest first. */
export function marksBetween(fromMs: number, toMs: number): number[] {
  const marks: number[] = [];
  for (let mark = markAfter(fromMs / HOUR_MS); mark * HOUR_MS <= toMs; mark = markAfter(mark)) marks.push(mark);
  return marks;
}

/** The next hour mark of a game, how far along from the one before, from 0 to 1, and the time left. */
export interface NextMark {
  mark: number;
  progress: number;
  leftMs: number;
}

/** The next hour mark of a game with `totalMs`. */
export function nextMark(totalMs: number): NextMark {
  const mark = markAfter(totalMs / HOUR_MS);
  const from = markBefore(mark) * HOUR_MS;
  return { mark, progress: (totalMs - from) / (mark * HOUR_MS - from), leftMs: mark * HOUR_MS - totalMs };
}

/** The hour marks a game passed, the smallest first, each with the session that took it past, none for play before Vaultime. */
export function reachedMarks(sessions: Session[], earlierMs = 0): { mark: number; session: Session | null }[] {
  const reached = marksBetween(0, earlierMs).map((mark) => ({ mark, session: null as Session | null }));
  let total = earlierMs;
  for (const session of inOrder(sessions)) {
    const after = total + session.counted_runtime_ms;
    for (const mark of marksBetween(total, after)) reached.push({ mark, session });
    total = after;
  }
  return reached;
}

/** The hour marks `session` took its game past, given the game's `sessions`. */
export function marksPassedBy(session: Session, sessions: Session[] | undefined, earlierMs = 0): number[] {
  if (!sessions || !countsAsPlay(session)) return [];
  const before = inOrder(sessions)
    .filter((other) => other.id !== session.id && other.started_at_wall < session.started_at_wall)
    .reduce((total, other) => total + other.counted_runtime_ms, earlierMs);
  return marksBetween(before, before + session.counted_runtime_ms);
}

/** "ten hours", "a hundred hours", "two thousand hours". */
export function markWords(mark: number): string {
  const THOUSAND = 1000;
  const HUNDRED = 100;
  if (mark % THOUSAND === 0) return `${mark === THOUSAND ? "a" : numberWords(mark / THOUSAND)} thousand hours`;
  return `${mark === HUNDRED ? "a hundred" : numberWords(mark)} hours`;
}

function inOrder(sessions: Session[]): Session[] {
  return sessions.filter(countsAsPlay).sort((a, b) => compareTimes(a.started_at_wall, b.started_at_wall));
}
