// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The sentences of the stats page and the year card.

import { MINUTE_MS, SHARE_QUIET_MAX_DAYS, WEEKEND_DAYS_PER_WEEK, WORKING_DAYS_PER_WEEK } from "@/lib/constants";
import type { Phrase } from "@/lib/sentences";
import type { SessionShape, Streak } from "@/lib/stats";
import { dayPartOf, formatDate, formatDayRange, type DayPart } from "@/lib/time";
import { stableHash } from "@/lib/utils";
import { amountWords, capitalize, countWords, numberWords } from "@/lib/words";

/** "Two hundred twelve hours on ninety days. *Elden Ring* led with sixty-one hours." */
export function yearSentence({
  playedMs,
  daysPlayed,
  topTitle,
  topMs,
  gamesCount,
  year,
  current,
}: {
  playedMs: number;
  daysPlayed: number;
  topTitle: string | null;
  topMs: number;
  gamesCount: number;
  year: number;
  current: boolean;
}): Phrase {
  if (playedMs < MINUTE_MS || !topTitle) {
    return { before: current ? "Nothing played this year yet." : `Nothing played in ${year}.` };
  }
  const lead = `${capitalize(amountWords(playedMs))} on ${countWords(daysPlayed, "day")}.`;
  if (gamesCount === 1) return { before: `${lead} All of it in `, em: topTitle, after: "." };
  return { before: `${lead} `, em: topTitle, after: ` led with ${amountWords(topMs)}.` };
}

/** "Four hundred hours on 210 days since March 2024. *Elden Ring* led with ninety hours." */
export function allTimeSentence({
  playedMs,
  daysPlayed,
  topTitle,
  topMs,
  gamesCount,
  since,
}: {
  playedMs: number;
  daysPlayed: number;
  topTitle: string | null;
  topMs: number;
  gamesCount: number;
  /** The first day with play. */
  since: Date | null;
}): Phrase {
  if (playedMs < MINUTE_MS || !topTitle || !since) return { before: "Nothing played yet." };
  const lead = `${capitalize(amountWords(playedMs))} on ${countWords(daysPlayed, "day")} since ${formatDate(since, { month: "long", year: "numeric" })}.`;
  if (gamesCount === 1) return { before: `${lead} All of it in `, em: topTitle, after: "." };
  return { before: `${lead} `, em: topTitle, after: ` led with ${amountWords(topMs)}.` };
}

/** "Your longest streak was twelve days, 3 to 14 March. Right now you are on four days in a row." */
export function streakSentence(longest: Streak | null, current: number, showCurrent: boolean): string {
  if (!longest) return "No days played yet.";
  const now = showCurrent && current > 1 ? ` Right now you are on ${numberWords(current)} days in a row.` : "";
  if (longest.days < 2) return `No two days in a row yet.${now}`;
  return `Your longest streak was ${numberWords(longest.days)} days, ${formatDayRange(longest.start, longest.end)}.${now}`;
}

const PART_PLURALS: Record<DayPart, string> = {
  morning: "mornings",
  afternoon: "afternoons",
  evening: "evenings",
  night: "late nights",
};

/**
 * "Your time to play is *weekend afternoons*." Weekdays and weekends are
 * compared per day, so five working days do not outweigh two free ones.
 * `weekClock` holds time by weekday, Monday first, then by hour.
 */
export function rhythmSentence(weekClock: number[][]): Phrase | null {
  const best = rhythmOf(weekClock);
  return best ? { before: "Your time to play is ", em: best, after: "." } : null;
}

/** "weekend afternoons", the time of the week with the most play per day, null without play. */
function rhythmOf(weekClock: number[][]): string | null {
  const totals = new Map<string, number>();
  weekClock.forEach((hours, weekday) => {
    const weekend = weekday >= WORKING_DAYS_PER_WEEK;
    hours.forEach((ms, hour) => {
      const key = `${weekend ? "weekend" : "weekday"} ${PART_PLURALS[dayPartOf(new Date(2000, 0, 1, hour))]}`;
      totals.set(key, (totals.get(key) ?? 0) + ms / (weekend ? WEEKEND_DAYS_PER_WEEK : WORKING_DAYS_PER_WEEK));
    });
  });
  const [best, ms] = [...totals.entries()].sort((a, b) => b[1] - a[1])[0] ?? ["", 0];
  return ms > 0 ? best : null;
}

/**
 * One line for the year card: "Weekend evenings, mostly in *Elden Ring*." or,
 * without game titles, "A year of *weekend evenings* across seven games." A
 * year with few days reads as a quiet one. The wording varies between years
 * and stays the same for each one.
 */
export function shareSentence({
  year,
  topTitle,
  gamesCount,
  daysPlayed,
  weekClock,
}: {
  year: number;
  /** The game played most, null when titles stay off the card. */
  topTitle: string | null;
  gamesCount: number;
  daysPlayed: number;
  weekClock: number[][];
}): Phrase {
  const days = countWords(daysPlayed, "day");
  const games = countWords(gamesCount, "game");
  const rhythm = rhythmOf(weekClock);
  if (daysPlayed < SHARE_QUIET_MAX_DAYS || !rhythm) {
    if (!topTitle) return { before: `A quiet year of ${days} across ${games}.` };
    return { before: `A quiet year of ${days}, ${gamesCount === 1 ? "all" : "most"} of it in `, em: topTitle, after: "." };
  }
  const pick = stableHash(String(year));
  if (!topTitle) {
    return pick % 2 === 0
      ? { before: "A year of ", em: rhythm, after: ` across ${games}.` }
      : { before: `${capitalize(games)} on ${days}, most often on `, em: rhythm, after: "." };
  }
  if (gamesCount === 1) return { before: `A year of ${rhythm}, all of it in `, em: topTitle, after: "." };
  const wordings: Phrase[] = [
    { before: `${capitalize(rhythm)}, mostly in `, em: topTitle, after: "." },
    { before: "", em: topTitle, after: ` led a year of ${rhythm}.` },
    { before: `A year of ${rhythm} across ${games}, led by `, em: topTitle, after: "." },
  ];
  return wordings[pick % wordings.length];
}

/** "October was your busiest month, thirty-one hours." */
export function busiestMonthSentence(months: { activeMs: number; idleMs: number }[], year: number): string | null {
  const totals = months.map((month) => month.activeMs + month.idleMs);
  const busiest = totals.indexOf(Math.max(...totals));
  if (busiest < 0 || totals[busiest] < MINUTE_MS) return null;
  const name = formatDate(new Date(year, busiest, 1), { month: "long" });
  return `${name} was your busiest month, ${amountWords(totals[busiest])}.`;
}

/** "2025 was your busiest year, three hundred hours." Null with fewer than two years to compare. */
export function busiestYearSentence(years: { year: number; play: { activeMs: number; idleMs: number } }[]): string | null {
  const played = years.filter(({ play }) => play.activeMs + play.idleMs >= MINUTE_MS);
  if (played.length < 2) return null;
  const busiest = played.reduce((best, each) =>
    each.play.activeMs + each.play.idleMs > best.play.activeMs + best.play.idleMs ? each : best,
  );
  return `${busiest.year} was your busiest year, ${amountWords(busiest.play.activeMs + busiest.play.idleMs)}.`;
}

const SHAPE_WORDS: Record<SessionShape, string> = {
  quick: "a quick look under twenty minutes",
  short: "a short one under an hour",
  plain: "one to two hours",
  long: "a long one of two to four hours",
  marathon: "a marathon of four hours or more",
};

/** "Your most common session was one to two hours." */
export function shapesSentence(shapes: Record<SessionShape, number>): string | null {
  const [shape, count] = (Object.entries(shapes) as [SessionShape, number][]).sort((a, b) => b[1] - a[1])[0];
  return count > 0 ? `Your most common session was ${SHAPE_WORDS[shape]}.` : null;
}
