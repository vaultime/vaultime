// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Play history written as plain sentences. Lines about one session live in
// session-lines.ts, the sentences of the stats page in year-sentences.ts.

import { HABIT_MIN_SESSIONS, HABIT_MIN_SHARE, HOUR_MS, MINUTE_MS, WEEK_TOP_GAME_MIN_SHARE } from "@/lib/constants";
import { countsAsPlay } from "@/lib/sessions";
import { dayPartOf, formatHoursMinutes, formatRelativeDay, parseVaultimeDate, type DayPart } from "@/lib/time";
import type { GameStatus, LedgerReport, Session, SideBySideGroup } from "@/lib/types";
import { amountWords, capitalize, countWords, DAY_PART_WORDS, durationWords, numberWords } from "@/lib/words";

/** A sentence with an optional part set in italics. */
export interface Phrase {
  before: string;
  em?: string;
  after?: string;
}

/** "Twenty-eight hours so far, *ten* of them this past week." */
export function libraryPlaytime(runtimeMs: number, weekRuntimeMs: number): Phrase {
  if (runtimeMs < MINUTE_MS) return { before: "The clock has just started." };
  const total = `${capitalize(amountWords(runtimeMs))} so far`;
  if (runtimeMs < HOUR_MS) return { before: `${total}.` };

  const hours = Math.floor(runtimeMs / HOUR_MS);
  const weekHours = Math.floor(weekRuntimeMs / HOUR_MS);
  if (weekHours >= hours) return { before: `${total}, all of them this past week.` };
  if (weekHours === 0) return { before: `${total}.` };
  return { before: `${total}, `, em: numberWords(weekHours), after: " of them this past week." };
}

/** "Thirty-eight hours across twenty-one sessions. You mostly play it *late at night*." */
export function gamePlaytime(allSessions: Session[]): Phrase {
  const sessions = allSessions.filter(countsAsPlay);
  if (sessions.length === 0) {
    return { before: "Not played yet. Start it however you usually do and the clock starts on its own." };
  }
  const runtimeMs = sessions.reduce((sum, session) => sum + session.counted_runtime_ms, 0);
  const lead =
    sessions.length === 1
      ? `${capitalize(amountWords(runtimeMs))} in one session.`
      : `${capitalize(amountWords(runtimeMs))} across ${countWords(sessions.length, "session")}.`;
  const habit = habitOf(sessions);
  if (!habit) return { before: lead };
  return { before: `${lead} You mostly play it `, em: DAY_PART_WORDS[habit].adverb, after: "." };
}

/** The part of the day that holds most of the playtime, when there is a clear one. */
function habitOf(sessions: Session[]): DayPart | null {
  if (sessions.length < HABIT_MIN_SESSIONS) return null;
  const byPart = new Map<DayPart, number>();
  let total = 0;
  for (const session of sessions) {
    const part = dayPartOf(parseVaultimeDate(session.started_at_wall));
    const runtimeMs = session.counted_runtime_ms;
    byPart.set(part, (byPart.get(part) ?? 0) + runtimeMs);
    total += runtimeMs;
  }
  const [part, runtime] = [...byPart.entries()].sort((a, b) => b[1] - a[1])[0];
  return total > 0 && runtime / total >= HABIT_MIN_SHARE ? part : null;
}

/** "Elden Ring, Hades II and Balatro". */
function listOf(titles: string[]): string {
  return titles.length > 1 ? `${titles.slice(0, -1).join(", ")} and ${titles.at(-1)}` : (titles[0] ?? "");
}

/**
 * "Elden Ring and Hades II ran side by side for 1 h 40." A game that ran
 * through all of it, like a launcher left open, is named first: "Stardew
 * Valley ran alongside Hades II and Balatro for 2 h 05."
 */
export function sideBySideSentence(group: SideBySideGroup, titleOf: (gameId: string) => string): string {
  const time = formatHoursMinutes(group.ms);
  const titles = group.game_ids.map(titleOf);
  if (titles.length <= 2) return `${listOf(titles)} ran side by side for ${time}.`;
  if (group.alongside) {
    const others = group.game_ids.filter((gameId) => gameId !== group.alongside).map(titleOf);
    return `${titleOf(group.alongside)} ran alongside ${listOf(others)} for ${time}.`;
  }
  const atOnce = group.most_at_once > 2 ? `up to ${numberWords(group.most_at_once)}` : "two";
  return `${listOf(titles)} ran side by side, ${atOnce} at a time, for ${time}.`;
}

/** What the ledger of this PC says, in a sentence. */
export function ledgerSentence(ledger: LedgerReport): string {
  if (ledger.broken) return "Its ledger was changed outside Vaultime, so the sessions it covers are marked Suspicious.";
  if (ledger.missing_sessions > 0) {
    const one = ledger.missing_sessions === 1;
    return `${one ? "One session" : `${ledger.missing_sessions} sessions`} in the ledger ${one ? "is" : "are"} gone from the history without a note that you removed ${one ? "it" : "them"}.`;
  }
  if (ledger.covered_sessions === 0) return "No sessions yet.";
  return ledger.covered_sessions === 1
    ? "It covers your one session, and none is missing."
    : `It covers all ${ledger.covered_sessions} sessions, and none is missing.`;
}

/** "Last played 3 hours ago", "Last played on Monday", "Last played in August". */
export function lastPlayedLine(lastPlayedAt: string | null): string {
  if (!lastPlayedAt) return "Not played yet";
  const when = formatRelativeDay(lastPlayedAt);
  if (when !== "Yesterday" && when.endsWith("day")) return `Last played on ${when}`;
  if (when.startsWith("In ")) return `Last played in ${when.slice("In ".length)}`;
  return `Last played ${when.charAt(0).toLowerCase()}${when.slice(1)}`;
}

/**
 * "Nine sessions, eleven hours and twenty minutes in all. *Saturday* was the
 * longest day." Other weeks read "Eleven hours and twenty minutes over nine
 * sessions" or name the game that took most of the time.
 */
export function weekSentence({
  sessionsCount,
  runtimeMs,
  longestDay,
  daysPlayed,
  topTitle = null,
  topShare = 0,
  gamesCount = 0,
  weekNumber = 0,
  carriedOver = 0,
  current,
}: {
  /** Sessions that began in the week. */
  sessionsCount: number;
  runtimeMs: number;
  longestDay: string | null;
  daysPlayed: number;
  /** The game played most that week, and its share of the time all games ran. */
  topTitle?: string | null;
  topShare?: number;
  gamesCount?: number;
  /** Picks the wording, so weeks read differently. */
  weekNumber?: number;
  /** Sessions from the week before that went on past its last midnight. */
  carriedOver?: number;
  /** This week, which may still get more play. */
  current: boolean;
}): Phrase {
  if ((sessionsCount === 0 && carriedOver === 0) || !longestDay) {
    return { before: current ? "Nothing played yet this week." : "Nothing played that week." };
  }
  const sessions = countWords(sessionsCount, "session");
  // A session that began the week before counts only its part in this week.
  const ends = carriedOver === 1 ? "the end of one" : `the ends of ${numberWords(carriedOver)}`;
  const fromBefore = carriedOver > 0 ? ` and ${ends} from the week before` : "";
  const onlyCarried = carriedOver === 1 ? "the end of a session" : `${ends} sessions`;
  const lead =
    sessionsCount === 0
      ? `${capitalize(durationWords(runtimeMs))} from ${onlyCarried} that began the week before.`
      : weekNumber % 2 === 0
        ? `${capitalize(sessions)}${fromBefore}, ${durationWords(runtimeMs)} in all.`
        : `${capitalize(durationWords(runtimeMs))} ${sessionsCount === 1 ? "in one session" : `over ${sessions}`}${fromBefore}.`;
  if (daysPlayed === 1) return { before: `${lead} All of it on `, em: longestDay, after: "." };
  if (topTitle && gamesCount === 1) return { before: `${lead} All of it in `, em: topTitle, after: "." };
  const leading = topTitle && topShare >= WEEK_TOP_GAME_MIN_SHARE;
  if (leading && Math.floor(weekNumber / 2) % 2 === 1) {
    return { before: `${lead} Most of it went to `, em: topTitle, after: "." };
  }
  return { before: `${lead} `, em: longestDay, after: " was the longest day." };
}

/**
 * A status change in the journal: "You finished *Hades II* after forty-two
 * hours." `playedMs` is the game's playtime up to the change.
 */
export function statusSentence(status: GameStatus, title: string, playedMs: number): Phrase {
  const after = playedMs >= MINUTE_MS ? ` after ${amountWords(playedMs)}` : "";
  switch (status) {
    case "finished":
      return { before: "You finished ", em: title, after: `${after}.` };
    case "dropped":
      return { before: "You put ", em: title, after: ` down${after}.` };
    case "playing":
      return { before: "You picked up ", em: title, after: "." };
    case "backlog":
      return { before: "", em: title, after: " went on your list." };
  }
}
