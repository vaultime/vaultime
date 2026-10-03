// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { RECENT_DAYS } from "@/lib/constants";
import { parseVaultimeDate, UI_LOCALE } from "@/lib/time";
import type { Session } from "@/lib/types";

export interface ActivityPoint {
  key: string;
  label: string;
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
}

export function buildDailyActivity(
  sessions: Session[],
  days: number,
): ActivityPoint[] {
  const today = new Date();
  today.setHours(0, 0, 0, 0);

  const points = new Map<string, ActivityPoint>();

  for (let offset = days - 1; offset >= 0; offset -= 1) {
    const day = new Date(today);
    day.setDate(today.getDate() - offset);
    const key = toDayKey(day);
    points.set(key, {
      key,
      label: day.toLocaleDateString(UI_LOCALE, {
        month: "short",
        day: "numeric",
      }),
      runtimeMs: 0,
      activeMs: 0,
      idleMs: 0,
    });
  }

  for (const session of sessions) {
    const key = toDayKey(parseVaultimeDate(session.started_at_wall));
    const point = points.get(key);
    if (!point) {
      continue;
    }

    point.runtimeMs += session.runtime_ms;
    point.activeMs += session.active_ms;
    point.idleMs += session.idle_ms;
  }

  return [...points.values()];
}

/** False for a finished session whose time was all taken out. It stays in the history as no play. */
export function countsAsPlay(session: Session): boolean {
  return !session.ended_at_wall || session.runtime_ms > 0;
}

/** The local calendar day, "2026-09-29". */
export function toDayKey(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** A session placed on the clock. */
interface Span {
  session: Session;
  start: number;
  end: number;
  /** Runtime per millisecond on the clock. Below 1 when the PC slept in between. */
  density: number;
}

/** A stretch of clock time and the sessions that ran through all of it. */
interface Stretch {
  start: number;
  end: number;
  running: Span[];
}

function toSpans(sessions: Session[], now: Date): Span[] {
  return sessions.filter(countsAsPlay).map((session) => {
    const start = parseVaultimeDate(session.started_at_wall).getTime();
    const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall).getTime() : now.getTime();
    return { session, start, end, density: end > start ? session.runtime_ms / (end - start) : 0 };
  });
}

function toStretches(spans: Span[]): Stretch[] {
  const timed = spans.filter((span) => span.end > span.start);
  const bounds = [...new Set(timed.flatMap((span) => [span.start, span.end]))].sort((a, b) => a - b);
  const stretches: Stretch[] = [];
  for (let index = 0; index + 1 < bounds.length; index += 1) {
    const [start, end] = [bounds[index], bounds[index + 1]];
    const running = timed
      .filter((span) => span.start <= start && span.end >= end)
      .sort((a, b) => a.start - b.start);
    if (running.length > 0) stretches.push({ start, end, running });
  }
  return stretches;
}

/**
 * Time with at least one game running, so games that run side by side count
 * once. Each session spreads its runtime evenly over its time on the clock,
 * which keeps time the PC slept out, as runtime does. A session on its own
 * adds exactly its runtime.
 */
export function playedMs(sessions: Session[], now = new Date()): number {
  const spans = toSpans(sessions, now);
  const untimed = spans
    .filter((span) => span.end <= span.start)
    .reduce((sum, span) => sum + span.session.runtime_ms, 0);
  const timed = toStretches(spans).reduce(
    (sum, stretch) => sum + (stretch.end - stretch.start) * Math.max(...stretch.running.map((span) => span.density)),
    0,
  );
  return untimed + Math.round(timed);
}

/** A stretch of clock time in which more than one game ran. */
export interface SideBySide {
  start: Date;
  end: Date;
  /** In the order the games joined the run, so a game keeps its place while others come and go. */
  gameIds: string[];
}

/** A part of a run in which the same games ran. */
export interface RunPiece {
  start: Date;
  end: Date;
  /** In the order the games joined the run, so a game keeps its place while others come and go. */
  gameIds: string[];
  /** The first start and last end of the sessions running in it. */
  sessionsStart: Date;
  sessionsEnd: Date;
}

/** Sessions that overlap on the clock, with no gap in between. */
export interface PlayRun {
  start: Date;
  end: Date;
  /** From start to end, split where a game joins or leaves. */
  pieces: RunPiece[];
}

/** The sessions as runs of overlapping play, in clock order. Sessions that only touch stay apart. */
export function playRuns(sessions: Session[], now = new Date()): PlayRun[] {
  const groups: Span[][] = [];
  let groupEnd = 0;
  for (const span of toSpans(sessions, now).sort((a, b) => a.start - b.start)) {
    const group = groups.at(-1);
    if (group && span.start < groupEnd) {
      group.push(span);
      groupEnd = Math.max(groupEnd, span.end);
    } else {
      groups.push([span]);
      groupEnd = span.end;
    }
  }
  return groups.map((group) => {
    const start = group[0].start;
    const end = Math.max(...group.map((span) => span.end));
    const order = [...new Set(group.map((span) => span.session.game_id))];
    const pieces: RunPiece[] = [];
    for (const stretch of toStretches(group)) {
      const running = new Set(stretch.running.map((span) => span.session.game_id));
      const gameIds = order.filter((gameId) => running.has(gameId));
      const sessionsStart = Math.min(...stretch.running.map((span) => span.start));
      const sessionsEnd = Math.max(...stretch.running.map((span) => span.end));
      const last = pieces.at(-1);
      if (last && last.gameIds.join() === gameIds.join()) {
        last.end = new Date(stretch.end);
        last.sessionsStart = new Date(Math.min(last.sessionsStart.getTime(), sessionsStart));
        last.sessionsEnd = new Date(Math.max(last.sessionsEnd.getTime(), sessionsEnd));
      } else {
        pieces.push({
          start: new Date(stretch.start),
          end: new Date(stretch.end),
          gameIds,
          sessionsStart: new Date(sessionsStart),
          sessionsEnd: new Date(sessionsEnd),
        });
      }
    }
    // A session without time on the clock still gets a piece, so it shows.
    if (pieces.length === 0) {
      pieces.push({
        start: new Date(start),
        end: new Date(end),
        gameIds: order.slice(0, 1),
        sessionsStart: new Date(start),
        sessionsEnd: new Date(end),
      });
    }
    return { start: new Date(start), end: new Date(end), pieces };
  });
}

/** The stretches in which games ran side by side, joined while the same games run. */
export function sideBySide(sessions: Session[], now = new Date()): SideBySide[] {
  return playRuns(sessions, now)
    .flatMap((run) => run.pieces)
    .filter((piece) => piece.gameIds.length > 1)
    .map(({ start, end, gameIds }) => ({ start, end, gameIds }));
}

/** Games that ran side by side with each other, or with a game they both ran beside. */
export interface SideBySideGroup {
  /** In the order the games joined. */
  gameIds: string[];
  /** The game in every stretch, when more than two games are in the group. */
  alongside: string | null;
  /** Clock time with at least two of the games running. */
  ms: number;
  /** The most games running at the same time. */
  mostAtOnce: number;
}

/** Side by side stretches grouped by the games they share, in clock order. */
export function sideBySideGroups(shared: SideBySide[]): SideBySideGroup[] {
  let groups: SideBySide[][] = [];
  for (const stretch of shared) {
    const sharesGame = (group: SideBySide[]) =>
      group.some((other) => other.gameIds.some((gameId) => stretch.gameIds.includes(gameId)));
    const first = groups.findIndex(sharesGame);
    if (first < 0) {
      groups.push([stretch]);
      continue;
    }
    const joined = [...groups.filter(sharesGame).flat(), stretch].sort((a, b) => a.start.getTime() - b.start.getTime());
    groups = [...groups.slice(0, first), joined, ...groups.slice(first + 1).filter((group) => !sharesGame(group))];
  }
  return groups.map((stretches) => {
    const gameIds = [...new Set(stretches.flatMap((stretch) => stretch.gameIds))];
    const inEvery = gameIds.find((gameId) => stretches.every((stretch) => stretch.gameIds.includes(gameId)));
    const alongside = gameIds.length > 2 ? (inEvery ?? null) : null;
    return {
      gameIds,
      alongside,
      ms: stretches.reduce((sum, stretch) => sum + stretch.end.getTime() - stretch.start.getTime(), 0),
      mostAtOnce: Math.max(...stretches.map((stretch) => stretch.gameIds.length)),
    };
  });
}

export interface RecentPlay {
  /** Time with a game running, games side by side counted once. */
  playedMs: number;
  runtimeMs: number;
  activeMs: number;
  sessionsCount: number;
  /** Days with at least one session. */
  daysCount: number;
  runtimeByGame: Map<string, number>;
  longest: Session | null;
}

/** Totals for the last `days` calendar days, today included. */
export function summarizeRecentPlay(sessions: Session[], days = RECENT_DAYS, now = new Date()): RecentPlay {
  const cutoff = new Date(now.getFullYear(), now.getMonth(), now.getDate() - (days - 1));
  const summary: RecentPlay = {
    playedMs: 0,
    runtimeMs: 0,
    activeMs: 0,
    sessionsCount: 0,
    daysCount: 0,
    runtimeByGame: new Map(),
    longest: null,
  };
  const playedDays = new Set<string>();
  const recent = sessions.filter(
    (session) => countsAsPlay(session) && parseVaultimeDate(session.started_at_wall) >= cutoff,
  );

  for (const session of recent) {
    const started = parseVaultimeDate(session.started_at_wall);
    summary.runtimeMs += session.runtime_ms;
    summary.activeMs += session.active_ms;
    summary.sessionsCount += 1;
    summary.runtimeByGame.set(
      session.game_id,
      (summary.runtimeByGame.get(session.game_id) ?? 0) + session.runtime_ms,
    );
    playedDays.add(toDayKey(started));
    if (!summary.longest || session.runtime_ms > summary.longest.runtime_ms) {
      summary.longest = session;
    }
  }

  summary.daysCount = playedDays.size;
  summary.playedMs = playedMs(recent, now);
  return summary;
}
