// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// What the core's journal, year and recent play commands answer, estimated
// from sessions alone for tests and the browser preview. The core places
// time by the session events, this spreads each session evenly over its
// span instead, and nothing steps aside here.

import {
  DAY_MS,
  DAYS_PER_WEEK,
  SESSION_LONG_MAX_MS,
  SESSION_PLAIN_MAX_MS,
  SESSION_QUICK_MAX_MS,
  SESSION_SHORT_MAX_MS,
} from "@/lib/constants";
import { countsAsPlay } from "@/lib/sessions";
import { parseVaultimeDate, startOfDay, toDayKey } from "@/lib/time";
import type {
  DayShare,
  GameStatusChange,
  GameTime,
  GameTotals,
  JournalDayPlay,
  JournalWeekPlay,
  PlayRun,
  RecentPlay,
  RunPiece,
  Session,
  SideBySideGroup,
  Streak,
  StripPart,
  WeekStatusChange,
  YearPlay,
} from "@/lib/types";

import { estimatePlayTotals } from "@/test/play-totals";

interface Span {
  session: Session;
  start: number;
  end: number;
}

function spanOf(session: Session, now: number): Span {
  const start = parseVaultimeDate(session.started_at_wall).getTime();
  const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall).getTime() : now;
  return { session, start, end };
}

function touches({ start, end }: Span, from: number, to: number): boolean {
  if (end <= start) return start >= from && start < to;
  return Math.max(start, from) < Math.min(end, to);
}

/** The runtime of a span that falls between `from` and `to`, spread evenly. */
function runtimeWithin({ session, start, end }: Span, from: number, to: number): number {
  if (end <= start) return start >= from && start < to ? session.runtime_ms : 0;
  const overlap = Math.min(end, to) - Math.max(start, from);
  return overlap > 0 ? Math.round((session.runtime_ms * overlap) / (end - start)) : 0;
}

/** Time with a game running between `from` and `to`, the densest session counting at each moment. */
function playedBetween(spans: Span[], from: number, to: number): number {
  const inside = spans.filter((span) => touches(span, from, to));
  const untimed = inside.filter((span) => span.end <= span.start).reduce((sum, span) => sum + span.session.runtime_ms, 0);
  const timed = inside.filter((span) => span.end > span.start);
  const bounds = [...new Set(timed.flatMap((span) => [Math.max(span.start, from), Math.min(span.end, to)]))].sort(
    (a, b) => a - b,
  );
  let played = untimed;
  for (let index = 0; index + 1 < bounds.length; index += 1) {
    const [a, b] = [bounds[index], bounds[index + 1]];
    const densities = timed
      .filter((span) => span.start <= a && span.end >= b)
      .map((span) => span.session.runtime_ms / (span.end - span.start));
    if (densities.length > 0) played += (b - a) * Math.max(...densities);
  }
  return Math.round(played);
}

/** Runs of overlapping parts with their pieces, as the core draws a strip. */
function playRuns(parts: StripPart[]): PlayRun[] {
  const sorted = [...parts].sort((a, b) => a.start_ms - b.start_ms);
  const groups: StripPart[][] = [];
  let groupEnd = 0;
  for (const part of sorted) {
    const group = groups.at(-1);
    if (group && part.start_ms < groupEnd) {
      group.push(part);
      groupEnd = Math.max(groupEnd, part.end_ms);
    } else {
      groups.push([part]);
      groupEnd = part.end_ms;
    }
  }
  return groups.map((group) => {
    const start = group[0].start_ms;
    const end = Math.max(...group.map((part) => part.end_ms));
    const order = [...new Set(group.map((part) => part.game_id))];
    const timed = group.filter((part) => part.end_ms > part.start_ms);
    const bounds = [...new Set(timed.flatMap((part) => [part.start_ms, part.end_ms]))].sort((a, b) => a - b);
    const pieces: RunPiece[] = [];
    for (let index = 0; index + 1 < bounds.length; index += 1) {
      const [a, b] = [bounds[index], bounds[index + 1]];
      const running = timed.filter((part) => part.start_ms <= a && part.end_ms >= b);
      if (running.length === 0) continue;
      const games = new Set(running.map((part) => part.game_id));
      const gameIds = order.filter((gameId) => games.has(gameId));
      const first = Math.min(...running.map((part) => part.start_ms));
      const last = Math.max(...running.map((part) => part.end_ms));
      const previous = pieces.at(-1);
      if (previous && previous.game_ids.join() === gameIds.join()) {
        previous.end_ms = b;
        previous.sessions_start_ms = Math.min(previous.sessions_start_ms, first);
        previous.sessions_end_ms = Math.max(previous.sessions_end_ms, last);
      } else {
        pieces.push({ start_ms: a, end_ms: b, game_ids: gameIds, sessions_start_ms: first, sessions_end_ms: last });
      }
    }
    if (pieces.length === 0) {
      pieces.push({ start_ms: start, end_ms: end, game_ids: order.slice(0, 1), sessions_start_ms: start, sessions_end_ms: end });
    }
    return { start_ms: start, end_ms: end, pieces };
  });
}

/** Side by side stretches grouped by the games they share. */
function sideBySide(runs: PlayRun[]): SideBySideGroup[] {
  let groups: RunPiece[][] = [];
  for (const stretch of runs.flatMap((run) => run.pieces).filter((piece) => piece.game_ids.length > 1)) {
    const sharesGame = (group: RunPiece[]) =>
      group.some((other) => other.game_ids.some((gameId) => stretch.game_ids.includes(gameId)));
    const first = groups.findIndex(sharesGame);
    if (first < 0) {
      groups.push([stretch]);
      continue;
    }
    const joined = [...groups.filter(sharesGame).flat(), stretch].sort((a, b) => a.start_ms - b.start_ms);
    groups = [...groups.slice(0, first), joined, ...groups.slice(first + 1).filter((group) => !sharesGame(group))];
  }
  return groups.map((stretches) => {
    const gameIds = [...new Set(stretches.flatMap((stretch) => stretch.game_ids))];
    const inEvery = gameIds.find((gameId) => stretches.every((stretch) => stretch.game_ids.includes(gameId)));
    return {
      game_ids: gameIds,
      alongside: gameIds.length > 2 ? (inEvery ?? null) : null,
      ms: stretches.reduce((sum, stretch) => sum + stretch.end_ms - stretch.start_ms, 0),
      most_at_once: Math.max(...stretches.map((stretch) => stretch.game_ids.length)),
    };
  });
}

/** A session cut at local midnights, the day it started first. */
function sharesOf(span: Span): DayShare[] {
  const shares: DayShare[] = [];
  let day = startOfDay(new Date(span.start));
  do {
    const next = startOfDay(day, 1);
    if (touches(span, day.getTime(), next.getTime()) || shares.length === 0) {
      const from = Math.max(span.start, day.getTime());
      shares.push({
        day: toDayKey(day),
        start_ms: from,
        end_ms: Math.max(from, Math.min(span.end, next.getTime())),
        runtime_ms: runtimeWithin(span, day.getTime(), next.getTime()),
        from_before: shares.length > 0,
        into_next: false,
      });
    }
    day = next;
  } while (day.getTime() < span.end);
  for (const share of shares.slice(0, -1)) share.into_next = true;
  return shares;
}

function gameTimes(sessions: Session[], from: Date, to: Date, now: number): GameTime[] {
  const games = new Map<string, GameTime>();
  for (const total of estimatePlayTotals(sessions, toDayKey(from), toDayKey(to), "day", null, new Date(now))) {
    const game = games.get(total.game_id) ?? { game_id: total.game_id, runtime_ms: 0, active_ms: 0, idle_ms: 0 };
    game.runtime_ms += total.runtime_ms;
    game.active_ms += total.active_ms;
    game.idle_ms += total.idle_ms;
    games.set(total.game_id, game);
  }
  return [...games.values()].filter((game) => game.runtime_ms > 0).sort((a, b) => b.runtime_ms - a.runtime_ms);
}

/**
 * The status changes from `from` up to `to`, each with its game's time
 * before it and the game's earlier playtime from `earlierMs`.
 */
function statusChangesBetween(
  changes: GameStatusChange[],
  spans: Span[],
  earlierMs: Record<string, number>,
  from: number,
  to: number,
): WeekStatusChange[] {
  return changes.flatMap(({ status, ...change }) => {
    const moment = parseVaultimeDate(change.changed_at).getTime();
    if (status === "none" || moment < from || moment >= to) return [];
    const played = spans
      .filter((span) => span.session.game_id === change.game_id)
      .reduce((sum, span) => sum + runtimeWithin(span, -Infinity, moment), 0);
    return [{ ...change, status, played_before_ms: played + (earlierMs[change.game_id] ?? 0) }];
  });
}

/**
 * What `get_journal_week` answers for the week from the local Monday `from`,
 * with the status changes among `changes` and the earlier playtime of each
 * game in `earlierMs`.
 */
export function estimateJournalWeek(
  sessions: Session[],
  from: string,
  now = Date.now(),
  changes: GameStatusChange[] = [],
  earlierMs: Record<string, number> = {},
): JournalWeekPlay {
  const start = parseVaultimeDate(from);
  const end = startOfDay(start, DAYS_PER_WEEK);
  const spans = sessions
    .map((session) => spanOf(session, now))
    .filter((span) => touches(span, start.getTime(), end.getTime()));
  const counting = spans.filter((span) => countsAsPlay(span.session));
  const days: JournalDayPlay[] = [];
  for (let day = start; day < end; day = startOfDay(day, 1)) {
    const [from, to] = [day.getTime(), startOfDay(day, 1).getTime()];
    const sessionIds = spans
      .filter((span) => span.start >= from && span.start < to)
      .sort((a, b) => a.session.started_at_wall.localeCompare(b.session.started_at_wall))
      .map((span) => span.session.id);
    const parts: StripPart[] = counting
      .filter((span) => touches(span, from, to))
      .sort((a, b) => Math.max(a.start, from) - Math.max(b.start, from) || a.start - b.start)
      .map((span) => {
        const [partStart, partEnd] = span.end <= span.start ? [span.start, span.end] : [Math.max(span.start, from), Math.min(span.end, to)];
        return {
          session_id: span.session.id,
          game_id: span.session.game_id,
          start_ms: partStart,
          end_ms: partEnd,
          open_start: partStart === from && span.start < from,
          open_end: partEnd === to && (!span.session.ended_at_wall || span.end > to),
        };
      });
    if (parts.length === 0 && sessionIds.length === 0) continue;
    const runs = playRuns(parts);
    days.push({
      day: toDayKey(day),
      played_ms: playedBetween(counting, from, to),
      session_ids: sessionIds,
      parts,
      runs,
      side_by_side: sideBySide(runs),
    });
  }
  days.reverse();
  const longest = days.reduce<JournalDayPlay | null>(
    (best, day) => (day.played_ms > (best?.played_ms ?? 0) ? day : best),
    null,
  );
  return {
    days,
    shares: Object.fromEntries(counting.map((span) => [span.session.id, sharesOf(span)])),
    games: [...new Set([...days].reverse().flatMap((day) => day.parts.map((part) => part.game_id)))],
    played_ms: days.reduce((sum, day) => sum + day.played_ms, 0),
    game_times: gameTimes(counting.map((span) => span.session), start, end, now),
    sessions_count: counting.filter((span) => span.start >= start.getTime()).length,
    days_played: days.filter((day) => day.played_ms > 0).length,
    longest_day: longest?.day ?? null,
    carried_over: counting.filter((span) => span.start < start.getTime()).length,
    status_changes: statusChangesBetween(
      changes,
      sessions.filter(countsAsPlay).map((session) => spanOf(session, now)),
      earlierMs,
      start.getTime(),
      end.getTime(),
    ),
  };
}

/** Days with play among `sessions`, by local day. */
function playedDays(sessions: Session[], now: number): Set<string> {
  const days = new Set<string>();
  for (const span of sessions.filter(countsAsPlay).map((session) => spanOf(session, now))) {
    for (let day = startOfDay(new Date(span.start)); day.getTime() <= Math.max(span.start, span.end - 1); day = startOfDay(day, 1)) {
      if (runtimeWithin(span, day.getTime(), startOfDay(day, 1).getTime()) > 0) days.add(toDayKey(day));
    }
  }
  return days;
}

/** What `get_year_play` answers for `year`. */
export function estimateYearPlay(sessions: Session[], year: number, now = Date.now()): YearPlay {
  return estimatePlayBetween(sessions, `${year}-01-01`, `${year + 1}-01-01`, now);
}

/** What `get_all_time_play` says: every day from the first session up to today. */
export function estimateAllTimePlay(sessions: Session[], now = Date.now()): YearPlay {
  const first = sessions.reduce(
    (min, session) => Math.min(min, parseVaultimeDate(session.started_at_wall).getTime()),
    now,
  );
  return estimatePlayBetween(sessions, toDayKey(new Date(first)), toDayKey(startOfDay(new Date(now), 1)), now);
}

/** The stats page's numbers for the local days `from` up to but not including `to`, both "2026-01-01". */
function estimatePlayBetween(sessions: Session[], from: string, to: string, now: number): YearPlay {
  const days = playedDays(sessions, now);
  const inYear = [...days].filter((day) => day >= from && day < to).sort();
  let longest: Streak | null = null;

  let runStart = 0;
  for (let index = 0; index < inYear.length; index += 1) {
    const follows = index > 0 && toDayKey(startOfDay(parseVaultimeDate(inYear[index - 1]), 1)) === inYear[index];
    if (!follows) runStart = index;
    const length = index - runStart + 1;
    if (!longest || length > longest.days) longest = { days: length, start: inYear[runStart], end: inYear[index] };
  }
  let day = startOfDay(new Date(now));
  if (!days.has(toDayKey(day))) day = startOfDay(day, -1);
  let current = 0;
  while (days.has(toDayKey(day))) {
    current += 1;
    day = startOfDay(day, -1);
  }
  const spans = sessions.filter(countsAsPlay).map((session) => spanOf(session, now));
  const [start, end] = [parseVaultimeDate(from), parseVaultimeDate(to)];
  const played: Record<string, number> = {};
  for (let day = start; day < end; day = startOfDay(day, 1)) {
    const ms = playedBetween(spans, day.getTime(), startOfDay(day, 1).getTime());
    if (ms > 0) played[toDayKey(day)] = ms;
  }
  return {
    days: estimatePlayTotals(sessions, from, to, "day", null, new Date(now)),
    hours: estimatePlayTotals(sessions, from, to, "hour_of_week", null, new Date(now)),
    played_ms: playedBetween(spans, start.getTime(), end.getTime()),
    played,
    days_played: inYear.length,
    longest_streak: longest,
    current_streak: current,
    ...estimateSessionsBetween(sessions, start, end),
  };
}

/** What `get_year_play` says about the sessions with play that started in `year`. */
export function estimateYearSessions(
  sessions: Session[],
  year: number,
): Pick<YearPlay, "sessions_count" | "sessions_by_game" | "session_lengths" | "longest_session_id"> {
  return estimateSessionsBetween(sessions, new Date(year, 0, 1), new Date(year + 1, 0, 1));
}

function estimateSessionsBetween(
  sessions: Session[],
  from: Date,
  to: Date,
): Pick<YearPlay, "sessions_count" | "sessions_by_game" | "session_lengths" | "longest_session_id"> {
  const started = sessions.filter(countsAsPlay).filter((session) => {
    const at = parseVaultimeDate(session.started_at_wall);
    return at >= from && at < to;
  });
  const byGame: Record<string, number> = {};
  const lengths: YearPlay["session_lengths"] = [0, 0, 0, 0, 0];
  let longest: Session | null = null;
  for (const session of started) {
    byGame[session.game_id] = (byGame[session.game_id] ?? 0) + 1;
    const limits = [SESSION_QUICK_MAX_MS, SESSION_SHORT_MAX_MS, SESSION_PLAIN_MAX_MS, SESSION_LONG_MAX_MS];
    lengths[limits.filter((max) => session.runtime_ms >= max).length] += 1;
    if (!longest || session.runtime_ms > longest.runtime_ms) longest = session;
  }
  return {
    sessions_count: started.length,
    sessions_by_game: byGame,
    session_lengths: lengths,
    longest_session_id: longest?.id ?? null,
  };
}

/** What `list_sessions` answers for each game, trust labels of the last `recentDays` days counting as recent. */
export function estimateGameTotals(sessions: Session[], recentDays: number, now = Date.now()): GameTotals[] {
  const recentSince = now - recentDays * DAY_MS;
  const nowWall = new Date(now).toISOString();
  const byGame = new Map<string, GameTotals>();
  for (const session of sessions.filter(countsAsPlay)) {
    const total = byGame.get(session.game_id) ?? {
      game_id: session.game_id,
      runtime_ms: 0,
      active_ms: 0,
      idle_ms: 0,
      sessions_count: 0,
      longest_session_ms: 0,
      first_played_at: null,
      last_played_at: null,
      suspicious_count: 0,
      recovered_count: 0,
      recent_suspicious_count: 0,
      recent_recovered_count: 0,
    };
    total.runtime_ms += session.counted_runtime_ms;
    total.active_ms += session.counted_active_ms;
    total.idle_ms += session.counted_idle_ms;
    total.sessions_count += 1;
    total.longest_session_ms = Math.max(total.longest_session_ms, session.runtime_ms);
    const start = session.started_at_wall;
    const end = session.ended_at_wall ?? nowWall;
    if (!total.first_played_at || start < total.first_played_at) total.first_played_at = start;
    if (!total.last_played_at || end > total.last_played_at) total.last_played_at = end;
    const recent = parseVaultimeDate(start).getTime() >= recentSince;
    if (session.integrity_status === "suspicious") {
      total.suspicious_count += 1;
      if (recent) total.recent_suspicious_count += 1;
    }
    if (session.integrity_status === "recovered") {
      total.recovered_count += 1;
      if (recent) total.recent_recovered_count += 1;
    }
    byGame.set(session.game_id, total);
  }
  return [...byGame.values()];
}

/** What `get_recent_play` answers for the last `days` days. */
export function estimateRecentPlay(sessions: Session[], days: number, now = Date.now()): RecentPlay {
  const from = startOfDay(new Date(now), -(days - 1));
  const to = startOfDay(new Date(now), 1);
  const counting = sessions.filter(countsAsPlay);
  const spans = counting.map((session) => spanOf(session, now));
  const started = spans.filter((span) => span.start >= from.getTime()).map((span) => span.session);
  const games = gameTimes(counting, from, to, now);
  const longest = started.reduce<Session | null>((best, session) => (!best || session.runtime_ms > best.runtime_ms ? session : best), null);
  return {
    played_ms: playedBetween(spans, from.getTime(), to.getTime()),
    runtime_ms: games.reduce((sum, game) => sum + game.runtime_ms, 0),
    active_ms: games.reduce((sum, game) => sum + game.active_ms, 0),
    sessions_count: started.length,
    days_count: [...playedDays(counting, now)].filter((day) => day >= toDayKey(from)).length,
    games,
    longest_session_id: longest?.id ?? null,
  };
}

/** Time with a game running today. */
export function estimatePlayedToday(sessions: Session[], now = Date.now()): number {
  const spans = sessions.filter(countsAsPlay).map((session) => spanOf(session, now));
  return playedBetween(spans, startOfDay(new Date(now)).getTime(), startOfDay(new Date(now), 1).getTime());
}
