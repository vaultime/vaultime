// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Play history written as plain sentences.

import {
  HABIT_MIN_SESSIONS,
  HABIT_MIN_SHARE,
  HOUR_MS,
  MINUTE_MS,
  SESSION_LONG_MAX_MS,
  SESSION_PLAIN_MAX_MS,
  SESSION_QUICK_MAX_MS,
  SESSION_SHORT_MAX_MS,
  WEEKEND_DAYS_PER_WEEK,
  WORKING_DAYS_PER_WEEK,
} from "@/lib/constants";
import { normalizeIntegrityStatus, parseIntegrityPayload } from "@/lib/integrity";
import { countsAsPlay } from "@/lib/session-stats";
import type { SessionShape, Streak } from "@/lib/stats";
import {
  dayPartOf,
  formatDayRange,
  formatHoursMinutes,
  formatRelativeDay,
  parseVaultimeDate,
  UI_LOCALE,
  type DayPart,
} from "@/lib/time";
import type { GameStatus, Session, SessionEvent } from "@/lib/types";
import { capitalize, numberWords } from "@/lib/words";

/** A sentence with an optional part set in italics. */
export interface Phrase {
  before: string;
  em?: string;
  after?: string;
}

const PART_WORDS: Record<DayPart, { adjective: string; adverb: string; noun: string }> = {
  morning: { adjective: "morning", adverb: "in the morning", noun: "morning" },
  afternoon: { adjective: "afternoon", adverb: "in the afternoon", noun: "afternoon" },
  evening: { adjective: "evening", adverb: "in the evening", noun: "evening" },
  night: { adjective: "late night", adverb: "late at night", noun: "night" },
};

const REASON_NOTES: Record<string, string> = {
  wall_clock_moved_backwards: "The system clock moved back during it.",
  wall_clock_step_mismatch: "The system clock jumped during it.",
  wall_clock_drift_exceeded: "The system clock drifted away from real time.",
  startup_orphan_cleanup: "Rebuilt after Vaultime closed without ending it.",
};

function withArticle(word: string): string {
  return `${/^[aeiou]/i.test(word) ? "An" : "A"} ${word}`;
}

/** "twenty-eight hours", "forty minutes". */
function amount(ms: number): string {
  if (ms < HOUR_MS) {
    const minutes = Math.floor(ms / MINUTE_MS);
    return `${numberWords(minutes)} minute${minutes === 1 ? "" : "s"}`;
  }
  const hours = Math.floor(ms / HOUR_MS);
  return `${numberWords(hours)} hour${hours === 1 ? "" : "s"}`;
}

/** "Twenty-eight hours so far, *ten* of them this past week." */
export function libraryPlaytime(runtimeMs: number, weekRuntimeMs: number): Phrase {
  if (runtimeMs < MINUTE_MS) return { before: "The clock has just started." };
  const total = `${capitalize(amount(runtimeMs))} so far`;
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
  const runtimeMs = sessions.reduce((sum, session) => sum + session.runtime_ms, 0);
  const lead =
    sessions.length === 1
      ? `${capitalize(amount(runtimeMs))} in one session.`
      : `${capitalize(amount(runtimeMs))} across ${numberWords(sessions.length)} sessions.`;
  const habit = habitOf(sessions);
  if (!habit) return { before: lead };
  return { before: `${lead} You mostly play it `, em: PART_WORDS[habit].adverb, after: "." };
}

/** The part of the day that holds most of the playtime, when there is a clear one. */
function habitOf(sessions: Session[]): DayPart | null {
  if (sessions.length < HABIT_MIN_SESSIONS) return null;
  const byPart = new Map<DayPart, number>();
  let total = 0;
  for (const session of sessions) {
    const part = dayPartOf(parseVaultimeDate(session.started_at_wall));
    byPart.set(part, (byPart.get(part) ?? 0) + session.runtime_ms);
    total += session.runtime_ms;
  }
  const [part, runtime] = [...byPart.entries()].sort((a, b) => b[1] - a[1])[0];
  return total > 0 && runtime / total >= HABIT_MIN_SHARE ? part : null;
}

/**
 * "A long evening", "A quick look in the morning", "Playing now". With a
 * title: "A long evening in Elden Ring", "Playing Elden Ring now".
 */
export function describeSession(session: Session, gameTitle?: string): string {
  if (!session.ended_at_wall) return gameTitle ? `Playing ${gameTitle} now` : "Playing now";
  if (!countsAsPlay(session)) return gameTitle ? `No play counted for ${gameTitle}` : "No play counted";
  const line = sessionShape(session);
  return gameTitle ? `${line} in ${gameTitle}` : line;
}

function sessionShape(session: Session): string {
  const part = dayPartOf(parseVaultimeDate(session.started_at_wall));
  const words = PART_WORDS[part];
  const ms = session.runtime_ms;
  if (ms < SESSION_QUICK_MAX_MS) return `A quick look ${words.adverb}`;
  if (ms < SESSION_SHORT_MAX_MS) {
    return part === "night" ? "A short session late at night" : `A short ${words.adjective} session`;
  }
  if (ms < SESSION_PLAIN_MAX_MS) return `${withArticle(words.adjective)} session`;
  if (ms < SESSION_LONG_MAX_MS) return `A long ${words.noun}`;
  return `A marathon ${words.noun}`;
}

/** "Elden Ring and Hades II ran side by side for 1 h 40." */
export function sideBySideSentence(titles: string[], ms: number): string {
  const names = titles.length > 1 ? `${titles.slice(0, -1).join(", ")} and ${titles.at(-1)}` : (titles[0] ?? "");
  return `${names} ran side by side for ${formatHoursMinutes(ms)}.`;
}

/** Ends the player's own words with a period unless they already end a sentence. */
function endSentence(text: string): string {
  return /[.!?…]$/.test(text) ? text : `${text}.`;
}

/** "1 h 12 in all, 1 h 05 active, 7 min idle". */
export function sessionAmounts(session: Session): string {
  return [
    `${formatHoursMinutes(session.runtime_ms)} in all`,
    `${formatHoursMinutes(session.active_ms)} active`,
    `${formatHoursMinutes(session.idle_ms)} idle`,
  ].join(", ");
}

/** Why a session is flagged or rebuilt, and time left out, in plain words. Null when all is normal. */
export function sessionTrustNote(session: Session, events: SessionEvent[]): string | null {
  const notes: string[] = [];
  let gapMs = 0;

  for (const event of events) {
    if (event.session_id !== session.id) continue;
    const payload = parseIntegrityPayload(event.payload_json);
    const reason = typeof payload?.reason === "string" ? REASON_NOTES[payload.reason] : undefined;
    if ((event.event_type === "integrity_flagged" || event.event_type === "recovered") && reason) {
      if (!notes.includes(reason)) notes.push(reason);
    }
    if (event.event_type === "tracking_gap" && typeof payload?.wall_gap_ms === "number") {
      gapMs += payload.wall_gap_ms;
    }
    const why = typeof payload?.reason === "string" ? payload.reason.trim() : "";
    const withReason = (lead: string) => (why ? `${lead}: ${endSentence(why)}` : `${lead}.`);
    if (event.event_type === "corrected") {
      const previous = payload?.previous as Record<string, unknown> | undefined;
      const before = typeof previous?.runtime_ms === "number" ? formatHoursMinutes(previous.runtime_ms) : null;
      const cut = payload?.runtime_ms === 0 ? "All time taken out" : "Cut short";
      notes.push(withReason(before ? `${cut} by you, it had ${before}` : `${cut} by you`));
    }
    if (event.event_type === "added_manually") {
      notes.push(withReason("Added by you"));
    }
  }

  const status = normalizeIntegrityStatus(session.integrity_status);
  if (notes.length === 0 && status === "suspicious") notes.push("Timing looked off during it.");
  if (notes.length === 0 && status === "recovered") notes.push("Rebuilt after an unclean exit.");
  if (gapMs >= MINUTE_MS) notes.push(`${formatHoursMinutes(gapMs)} of sleep or pause left out.`);
  return notes.length > 0 ? notes.join(" ") : null;
}

/** "Last played 3 hours ago", "Last played on Monday", "Last played in August". */
export function lastPlayedLine(lastPlayedAt: string | null): string {
  if (!lastPlayedAt) return "Not played yet";
  const when = formatRelativeDay(lastPlayedAt);
  if (when !== "Yesterday" && when.endsWith("day")) return `Last played on ${when}`;
  if (when.startsWith("In ")) return `Last played in ${when.slice("In ".length)}`;
  return `Last played ${when.charAt(0).toLowerCase()}${when.slice(1)}`;
}

/** "eleven hours and twenty minutes", "forty minutes". */
function durationWords(ms: number): string {
  const hours = Math.floor(ms / HOUR_MS);
  const minutes = Math.floor((ms % HOUR_MS) / MINUTE_MS);
  const minutePart = `${numberWords(minutes)} minute${minutes === 1 ? "" : "s"}`;
  if (hours === 0) return minutePart;
  const hourPart = `${numberWords(hours)} hour${hours === 1 ? "" : "s"}`;
  return minutes > 0 ? `${hourPart} and ${minutePart}` : hourPart;
}

/** "Nine sessions, eleven hours and twenty minutes in all. *Saturday* was the longest day." */
export function weekSentence({
  sessionsCount,
  runtimeMs,
  longestDay,
  daysPlayed,
  current,
}: {
  sessionsCount: number;
  runtimeMs: number;
  longestDay: string | null;
  daysPlayed: number;
  /** This week, which may still get more play. */
  current: boolean;
}): Phrase {
  if (sessionsCount === 0 || !longestDay) {
    return { before: current ? "Nothing played yet this week." : "Nothing played that week." };
  }
  const lead = `${capitalize(numberWords(sessionsCount))} session${sessionsCount === 1 ? "" : "s"}, ${durationWords(runtimeMs)} in all.`;
  if (daysPlayed === 1) return { before: `${lead} All of it on `, em: longestDay, after: "." };
  return { before: `${lead} `, em: longestDay, after: " was the longest day." };
}

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
  const lead = `${capitalize(amount(playedMs))} on ${numberWords(daysPlayed)} day${daysPlayed === 1 ? "" : "s"}.`;
  if (gamesCount === 1) return { before: `${lead} All of it in `, em: topTitle, after: "." };
  return { before: `${lead} `, em: topTitle, after: ` led with ${amount(topMs)}.` };
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
  const totals = new Map<string, number>();
  weekClock.forEach((hours, weekday) => {
    const weekend = weekday >= WORKING_DAYS_PER_WEEK;
    hours.forEach((ms, hour) => {
      const key = `${weekend ? "weekend" : "weekday"} ${PART_PLURALS[dayPartOf(new Date(2000, 0, 1, hour))]}`;
      totals.set(key, (totals.get(key) ?? 0) + ms / (weekend ? WEEKEND_DAYS_PER_WEEK : WORKING_DAYS_PER_WEEK));
    });
  });
  const [best, ms] = [...totals.entries()].sort((a, b) => b[1] - a[1])[0] ?? ["", 0];
  if (ms <= 0) return null;
  return { before: "Your time to play is ", em: best, after: "." };
}

/** "October was your busiest month, thirty-one hours." */
export function busiestMonthSentence(months: { activeMs: number; idleMs: number }[], year: number): string | null {
  const totals = months.map((month) => month.activeMs + month.idleMs);
  const busiest = totals.indexOf(Math.max(...totals));
  if (busiest < 0 || totals[busiest] < MINUTE_MS) return null;
  const name = new Date(year, busiest, 1).toLocaleDateString(UI_LOCALE, { month: "long" });
  return `${name} was your busiest month, ${amount(totals[busiest])}.`;
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

/**
 * A status change in the journal: "You finished *Hades II* after forty-two
 * hours." `playedMs` is the game's playtime up to the change.
 */
export function statusSentence(status: GameStatus, title: string, playedMs: number): Phrase {
  const after = playedMs >= MINUTE_MS ? ` after ${amount(playedMs)}` : "";
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
