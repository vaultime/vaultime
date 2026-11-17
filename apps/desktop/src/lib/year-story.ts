// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The chapters of Year in games, worked out from the stats page's year, so
// the story counts play exactly as the page does.

import {
  MINUTE_MS,
  MONTHS_PER_YEAR,
  HOURS_PER_DAY,
  YEAR_STORY_LAUNCHERS,
  YEAR_STORY_TOP_GAMES,
  YEAR_STORY_WEEK_GAMES,
} from "@/lib/constants";
import { launcherKey } from "@/lib/launchers";
import { mostActiveFirst } from "@/lib/play-totals";
import type { GameYear, YearStats } from "@/lib/stats";
import { compareTimes, dayPartOf, parseVaultimeDate, startOfWeek, toDayKey } from "@/lib/time";
import type { GameStatusChange, LauncherPlay, Session } from "@/lib/types";
import { numberWords } from "@/lib/words";

export const YEAR_CHAPTERS = ["numbers", "game", "top", "longest", "week", "rhythm", "launchers", "finished"] as const;
export type YearChapter = (typeof YEAR_CHAPTERS)[number];

/** A game's part of a stretch of play, by active time as on the share card. */
export interface StoryGame {
  gameId: string;
  activeMs: number;
  /** Share of the stretch's active time, from 0 to 1. */
  share: number;
}

export interface YearStory {
  year: number;
  /** Time with a game running, games side by side counted once. */
  playedMs: number;
  daysPlayed: number;
  sessionsCount: number;
  gamesCount: number;
  longestStreakDays: number;
  firstDay: Date | null;
  lastDay: Date | null;
  /** The most active game that may be named. */
  game: GameOfYear | null;
  /** The most active games that may be named, the most first. */
  top: StoryGame[];
  longest: LongestSession | null;
  /** The week with the most time played. */
  week: { start: Date; playedMs: number; daysPlayed: number; games: StoryGame[] } | null;
  /** Active time by hour of the day, midnight first. */
  hours: number[];
  /** Each launcher's time by its key, the most first. */
  launchers: StoryLauncher[];
  /** Games marked finished in the year, the earliest first. */
  finished: { gameId: string; at: Date }[];
  /** The chapters with something to tell, in order. */
  chapters: YearChapter[];
}

export interface GameOfYear extends StoryGame {
  runtimeMs: number;
  /** Days it was played on. */
  days: number;
  /** January is 0. */
  bestMonth: number;
  longestSessionMs: number;
}

export interface LongestSession {
  gameId: string;
  start: Date;
  /** Null while it runs. */
  end: Date | null;
  runtimeMs: number;
  activeMs: number;
  /** Started in the evening or later, or ran past midnight. */
  night: boolean;
}

export interface StoryLauncher {
  launcher: string;
  runtimeMs: number;
  share: number;
}

/**
 * The story of `stats`' year. Hidden games count in the hours, as on the
 * stats page, but `named` keeps them out of every list and chapter.
 */
export function yearStory({
  stats,
  sessions,
  launchers,
  statusChanges,
  named,
}: {
  stats: YearStats;
  /** The library's sessions, for the longest one of the game of the year. */
  sessions: Session[];
  launchers: LauncherPlay[];
  statusChanges: GameStatusChange[];
  /** Whether a game may be named: known and not hidden. */
  named: (gameId: string) => boolean;
}): YearStory {
  const { year } = stats;
  const share = (ms: number) => (stats.activeMs > 0 ? ms / stats.activeMs : 0);
  // A game left running while the player was away does not win.
  const shown = stats.games.filter((game) => game.activeMs >= MINUTE_MS && named(game.gameId)).sort(mostActiveFirst);
  const top = shown.slice(0, YEAR_STORY_TOP_GAMES).map((game) => storyGame(game, share(game.activeMs)));
  const days = [...stats.played]
    .filter(([, ms]) => ms > 0)
    .map(([day]) => day)
    .sort();
  const longest = stats.longest && named(stats.longest.game_id) ? longestSession(stats.longest) : null;
  const week = busiestWeek(stats, named);
  const hours = Array.from({ length: HOURS_PER_DAY }, (_, hour) =>
    stats.weekClock.reduce((sum, weekday) => sum + (weekday[hour] ?? 0), 0),
  );
  const byLauncher = storyLaunchers(launchers);
  const finished = finishedIn(year, statusChanges, named);
  const game = shown[0] ? gameOfYear(stats, sessions, shown[0], share(shown[0].activeMs)) : null;
  const tells: Record<YearChapter, boolean> = {
    numbers: stats.playedMs >= MINUTE_MS,
    game: game !== null,
    top: top.length > 1,
    longest: longest !== null,
    week: week !== null,
    rhythm: hours.some((ms) => ms > 0),
    launchers: byLauncher.length > 0,
    finished: finished.length > 0,
  };
  return {
    year,
    playedMs: stats.playedMs,
    daysPlayed: stats.daysPlayed,
    sessionsCount: stats.sessionsCount,
    gamesCount: stats.games.length,
    longestStreakDays: stats.longestStreak?.days ?? 0,
    firstDay: days.length > 0 ? parseVaultimeDate(days[0]) : null,
    lastDay: days.length > 0 ? parseVaultimeDate(days[days.length - 1]) : null,
    game,
    top,
    longest,
    week,
    hours,
    launchers: byLauncher,
    finished,
    // Without play there is nothing to tell at all.
    chapters: tells.numbers ? YEAR_CHAPTERS.filter((chapter) => tells[chapter]) : [],
  };
}

/** How the chapter list and the card name a chapter. */
export function chapterTitle(chapter: YearChapter, story: YearStory): string {
  switch (chapter) {
    case "numbers":
      return "Your year in numbers";
    case "game":
      return "Game of the year";
    case "top":
      return `Your top ${numberWords(story.top.length)}`;
    case "longest":
      return story.longest?.night ? "The longest night" : "The longest day";
    case "week":
      return "The week you could not stop";
    case "rhythm":
      return "When you play";
    case "launchers":
      return "Where your hours came from";
    case "finished":
      return "Finished this year";
  }
}

function storyGame(game: { gameId: string; activeMs: number }, share: number): StoryGame {
  return { gameId: game.gameId, activeMs: game.activeMs, share };
}

function gameOfYear(stats: YearStats, sessions: Session[], first: GameYear, share: number): GameOfYear {
  const months = Array<number>(MONTHS_PER_YEAR).fill(0);
  let days = 0;
  for (const [day, play] of stats.days) {
    const time = play.games.find((game) => game.gameId === first.gameId);
    if (!time || time.runtimeMs <= 0) continue;
    days += 1;
    months[Number(day.slice(5, 7)) - 1] += time.activeMs;
  }
  const longestSessionMs = sessions
    .filter(
      (session) =>
        session.game_id === first.gameId && parseVaultimeDate(session.started_at_wall).getFullYear() === stats.year,
    )
    .reduce((most, session) => Math.max(most, session.runtime_ms), 0);
  return {
    ...storyGame(first, share),
    runtimeMs: first.runtimeMs,
    days,
    bestMonth: months.indexOf(Math.max(...months)),
    longestSessionMs,
  };
}

function longestSession(session: Session): LongestSession {
  const start = parseVaultimeDate(session.started_at_wall);
  const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : null;
  const part = dayPartOf(start);
  return {
    gameId: session.game_id,
    start,
    end,
    runtimeMs: session.runtime_ms,
    activeMs: session.active_ms,
    night: part === "evening" || part === "night" || (end !== null && toDayKey(end) !== toDayKey(start)),
  };
}

/** The week, Monday first, with the most time played, the earliest on a tie. */
function busiestWeek(stats: YearStats, named: (gameId: string) => boolean): YearStory["week"] {
  const weeks = new Map<string, { start: Date; playedMs: number; days: string[] }>();
  for (const [day, ms] of stats.played) {
    if (ms <= 0) continue;
    const start = startOfWeek(parseVaultimeDate(day));
    const week = weeks.get(toDayKey(start)) ?? { start, playedMs: 0, days: [] };
    week.playedMs += ms;
    week.days.push(day);
    weeks.set(toDayKey(start), week);
  }
  const best = [...weeks.values()].sort((a, b) => b.playedMs - a.playedMs || a.start.getTime() - b.start.getTime())[0];
  if (!best) return null;
  const byGame = new Map<string, number>();
  let activeMs = 0;
  for (const day of best.days) {
    for (const game of stats.days.get(day)?.games ?? []) {
      byGame.set(game.gameId, (byGame.get(game.gameId) ?? 0) + game.activeMs);
      activeMs += game.activeMs;
    }
  }
  const games = [...byGame]
    .filter(([gameId, ms]) => ms >= MINUTE_MS && named(gameId))
    .sort((a, b) => b[1] - a[1])
    .slice(0, YEAR_STORY_WEEK_GAMES)
    .map(([gameId, ms]) => storyGame({ gameId, activeMs: ms }, activeMs > 0 ? ms / activeMs : 0));
  return { start: best.start, playedMs: best.playedMs, daysPlayed: best.days.length, games };
}

function storyLaunchers(launchers: LauncherPlay[]): StoryLauncher[] {
  const byKey = new Map<string, number>();
  for (const play of launchers) {
    const key = launcherKey(play.launcher);
    byKey.set(key, (byKey.get(key) ?? 0) + play.runtime_ms);
  }
  const kept = [...byKey].filter(([, ms]) => ms >= MINUTE_MS);
  const total = kept.reduce((sum, [, ms]) => sum + ms, 0);
  return kept
    .sort((a, b) => b[1] - a[1])
    .slice(0, YEAR_STORY_LAUNCHERS)
    .map(([launcher, runtimeMs]) => ({ launcher, runtimeMs, share: runtimeMs / total }));
}

/** Games marked finished in `year`, each at its first finish there. Clearing the status takes it back. */
function finishedIn(year: number, changes: GameStatusChange[], named: (gameId: string) => boolean) {
  const finished = new Map<string, Date>();
  for (const change of [...changes].sort((a, b) => compareTimes(a.changed_at, b.changed_at))) {
    const at = parseVaultimeDate(change.changed_at);
    if (at.getFullYear() !== year || !named(change.game_id)) continue;
    if (change.status === "none") finished.delete(change.game_id);
    else if (change.status === "finished" && !finished.has(change.game_id)) finished.set(change.game_id, at);
  }
  return [...finished].map(([gameId, at]) => ({ gameId, at })).sort((a, b) => a.at.getTime() - b.at.getTime());
}
