// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Play history written as plain sentences.

import {
  DAY_MS,
  DAY_PART_HOURS,
  DAYS_PER_WEEK,
  HABIT_MIN_SESSIONS,
  HABIT_MIN_SHARE,
  HOUR_MS,
  MINUTE_MS,
  MONTHS_PER_YEAR,
  SESSION_LEFT_RUNNING_MIN_MS,
  SESSION_LEFT_RUNNING_MIN_SHARE,
  SESSION_PAST_MIDNIGHT_MIN_MS,
  SESSION_RECORD_MIN_EARLIER,
  SESSION_RECORD_MIN_MS,
  SESSION_RETURN_MIN_MS,
  SESSION_WORDS_MINUTE_STEP_MS,
  WEEK_TOP_GAME_MIN_SHARE,
  WEEKEND_DAYS_PER_WEEK,
  WORKING_DAYS_PER_WEEK,
} from "@/lib/constants";
import { normalizeIntegrityStatus, parseIntegrityPayload } from "@/lib/integrity";
import { countsAsPlay } from "@/lib/session-stats";
import { shapeOf, type SessionShape, type Streak } from "@/lib/stats";
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
import { stableHash } from "@/lib/utils";
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

/** What a session line knows beyond the session itself. */
export interface SessionContext {
  /** Names the game, for lists that mix games. */
  gameTitle?: string;
  /**
   * Sessions of the same game in any order, this one may be among them. They
   * tell a first session, a return or a record from a plain one.
   */
  gameSessions?: Session[];
  /** Playtime from before Vaultime, so the first tracked session is no first look. */
  earlierMs?: number;
}

/** The words a session line is built from. */
interface LineWords {
  /** "evening", "late night". */
  adjective: string;
  /** "in the evening", "in the small hours". */
  adverb: string;
  /** "evening", "night". */
  noun: string;
  /** "Sunday afternoon" at the weekend, the plain noun on other days. */
  occasion: string;
  /** Rounded length, "forty-five minutes", "an hour and a half". */
  length: string;
  night: boolean;
}

/** One way to put a session: the words around the title, and the line without a title. */
type Wording = [titled: [before: string, after: string], alone: string];

const LINES: Record<SessionShape, ((words: LineWords) => Wording)[]> = {
  quick: [
    (w) => [["A quick look at ", ` ${w.adverb}`], `A quick look ${w.adverb}`],
    (w) => [["A few minutes of ", ` ${w.adverb}`], `A few minutes ${w.adverb}`],
    (w) => [[`A brief ${w.adjective} visit to `, ""], `A brief ${w.adjective} visit`],
  ],
  short: [
    (w) => {
      const line = w.night ? `A short session ${w.adverb}` : `A short ${w.adjective} session`;
      return [[`${line} in `, ""], line];
    },
    (w) => [[`${capitalize(w.length)} of `, ` ${w.adverb}`], `${capitalize(w.length)} ${w.adverb}`],
    (w) => [[`${withArticle(w.adjective)} round of `, ""], `${withArticle(w.adjective)} round`],
  ],
  plain: [
    (w) => [[`${withArticle(w.adjective)} session in `, ""], `${withArticle(w.adjective)} session`],
    (w) => [[`${capitalize(w.length)} of `, ` ${w.adverb}`], `${capitalize(w.length)} ${w.adverb}`],
    (w) => [[`${withArticle(w.occasion)} in `, ""], `${withArticle(w.occasion)} of play`],
  ],
  long: [
    (w) => [[`A long ${w.noun} in `, ""], `A long ${w.noun}`],
    (w) => [[`Most of the ${w.noun} in `, ""], `Most of the ${w.noun}`],
    (w) => [[`${capitalize(w.length)} deep in `, ""], `${capitalize(w.length)} ${w.adverb}`],
  ],
  marathon: [
    (w) => [[`A marathon ${w.noun} in `, ""], `A marathon ${w.noun}`],
    (w) => [[`The whole ${w.noun} went to `, ""], `A whole ${w.noun} of play`],
    (w) => [[`${capitalize(w.length)} of `, " in one go"], `${capitalize(w.length)} in one go`],
  ],
};

/** "forty-five minutes", "an hour and a half", "three hours", rounded for prose. */
function roundedLength(ms: number): string {
  const step = SESSION_WORDS_MINUTE_STEP_MS;
  if (ms < HOUR_MS - step / 2) {
    const minutes = (Math.max(1, Math.round(ms / step)) * step) / MINUTE_MS;
    return `${numberWords(minutes)} minutes`;
  }
  const halves = Math.round(ms / (HOUR_MS / 2));
  const hours = Math.floor(halves / 2);
  const half = halves % 2 === 1;
  if (hours === 1) return half ? "an hour and a half" : "an hour";
  return `${numberWords(hours)}${half ? " and a half" : ""} hours`;
}

/** "two weeks", "three months", "over a year". */
function awayWords(from: Date, to: Date): string {
  const months =
    (to.getFullYear() - from.getFullYear()) * MONTHS_PER_YEAR +
    to.getMonth() -
    from.getMonth() -
    (to.getDate() < from.getDate() ? 1 : 0);
  if (months >= MONTHS_PER_YEAR) {
    const years = Math.floor(months / MONTHS_PER_YEAR);
    return years === 1 ? "over a year" : `over ${numberWords(years)} years`;
  }
  if (months >= 2) return `${numberWords(months)} months`;
  const weeks = Math.floor((to.getTime() - from.getTime()) / (DAYS_PER_WEEK * DAY_MS));
  return `${numberWords(weeks)} weeks`;
}

function lineWords(session: Session): LineWords {
  const started = parseVaultimeDate(session.started_at_wall);
  const part = dayPartOf(started);
  const smallHours = part === "night" && started.getHours() < DAY_PART_HOURS.morning;
  const words = PART_WORDS[part];
  // Play after midnight still belongs to the night before.
  const day = smallHours ? new Date(started.getFullYear(), started.getMonth(), started.getDate() - 1) : started;
  // Sunday and Saturday.
  const weekend = day.getDay() === 0 || day.getDay() === 6;
  return {
    adjective: words.adjective,
    adverb: smallHours ? "in the small hours" : words.adverb,
    noun: words.noun,
    occasion: weekend ? `${day.toLocaleDateString(UI_LOCALE, { weekday: "long" })} ${words.noun}` : words.noun,
    length: roundedLength(session.runtime_ms),
    night: part === "night",
  };
}

/** Ways to put a session that stands out: a first one, a return, a record, a late night. Null for others. */
function standoutWordings(session: Session, context: SessionContext, words: LineWords): Wording[] | null {
  if (!context.gameSessions) return null;
  const started = parseVaultimeDate(session.started_at_wall);
  const shape = shapeOf(session);
  const brief = shape === "quick" || shape === "short";
  const earlier = context.gameSessions.filter(
    (other) => other.id !== session.id && countsAsPlay(other) && parseVaultimeDate(other.started_at_wall) < started,
  );
  const previous = earlier.reduce<Session | null>(
    (latest, other) => (!latest || other.started_at_wall > latest.started_at_wall ? other : latest),
    null,
  );

  if (!previous && (context.earlierMs ?? 0) < MINUTE_MS) {
    return brief
      ? [
          [["A first look at ", ""], "A first look"],
          [["First steps in ", ""], "First steps"],
        ]
      : [[[`A first ${words.noun} in `, ""], `The first ${words.noun}`]];
  }

  const previousEnd = previous?.ended_at_wall ? parseVaultimeDate(previous.ended_at_wall) : null;
  if (previousEnd && started.getTime() - previousEnd.getTime() >= SESSION_RETURN_MIN_MS) {
    const away = awayWords(previousEnd, started);
    return [
      [["Back to ", ` after ${away}`], `Back after ${away}`],
      [["A return to ", ` after ${away}`], `A return after ${away}`],
    ];
  }

  const idleShare = session.runtime_ms > 0 ? session.idle_ms / session.runtime_ms : 0;
  if (session.runtime_ms >= SESSION_LEFT_RUNNING_MIN_MS && idleShare >= SESSION_LEFT_RUNNING_MIN_SHARE) {
    return [
      [["", ` stayed open through the ${words.noun}`], `Left open through the ${words.noun}`],
      [["", " ran mostly on its own"], "Mostly left running"],
    ];
  }

  const record =
    earlier.length >= SESSION_RECORD_MIN_EARLIER &&
    session.runtime_ms >= SESSION_RECORD_MIN_MS &&
    earlier.every((other) => other.runtime_ms < session.runtime_ms);
  if (record) {
    return [
      [["Your longest session of ", " yet"], "Your longest session yet"],
      [[`${capitalize(words.length)} of `, ", your longest yet"], `${capitalize(words.length)}, your longest yet`],
    ];
  }

  const ended = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : null;
  const midnight = new Date(started.getFullYear(), started.getMonth(), started.getDate() + 1);
  if (ended && ended.getTime() - midnight.getTime() >= SESSION_PAST_MIDNIGHT_MIN_MS) {
    return [
      [["Into the small hours with ", ""], "Into the small hours"],
      [["Past midnight in ", ""], "Well past midnight"],
    ];
  }

  const sameDay = previous && parseVaultimeDate(previous.started_at_wall).toDateString() === started.toDateString();
  if (sameDay && shape !== "long" && shape !== "marathon") {
    return [
      [["Another round of ", ""], "Another round"],
      [["Back to ", " for another go"], "Back for another go"],
    ];
  }
  return null;
}

/**
 * "A long evening", "Forty minutes in the morning", "Playing now". With a
 * title, the title is the italic part: "A long evening in *Elden Ring*". With
 * the other sessions of the game it also notes a first session, a return, a
 * record or a late night. The wording varies between sessions and stays the
 * same for each one.
 */
export function describeSession(session: Session, context: SessionContext = {}): Phrase {
  const title = context.gameTitle;
  if (!session.ended_at_wall) return title ? { before: "Playing ", em: title, after: " now" } : { before: "Playing now" };
  if (!countsAsPlay(session)) return title ? { before: "No play counted for ", em: title } : { before: "No play counted" };
  const words = lineWords(session);
  const options = standoutWordings(session, context, words) ?? LINES[shapeOf(session)].map((line) => line(words));
  const [[before, after], alone] = options[stableHash(session.id) % options.length];
  return title ? { before, em: title, after } : { before: alone };
}

/** A phrase as plain text. */
export function phraseString(phrase: Phrase): string {
  return `${phrase.before}${phrase.em ?? ""}${phrase.after ?? ""}`;
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

/** "1 h 12 in all, 1 h 05 active, 7 min idle", or "1 h 12, all of it active". */
export function sessionAmounts(session: Session): string {
  const all = formatHoursMinutes(session.runtime_ms);
  if (session.runtime_ms >= MINUTE_MS) {
    if (session.idle_ms < MINUTE_MS) return `${all}, all of it active`;
    if (session.active_ms < MINUTE_MS) return `${all}, all of it idle`;
  }
  return [
    `${all} in all`,
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
  if (notes.length === 0 && status === "suspicious") notes.push("The clock jumped or the record was changed outside Vaultime.");
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
  topMs = 0,
  gamesCount = 0,
  weekNumber = 0,
  current,
}: {
  sessionsCount: number;
  runtimeMs: number;
  longestDay: string | null;
  daysPlayed: number;
  /** The game played most that week, with its playtime. */
  topTitle?: string | null;
  topMs?: number;
  gamesCount?: number;
  /** Picks the wording, so weeks read differently. */
  weekNumber?: number;
  /** This week, which may still get more play. */
  current: boolean;
}): Phrase {
  if (sessionsCount === 0 || !longestDay) {
    return { before: current ? "Nothing played yet this week." : "Nothing played that week." };
  }
  const sessions = `${numberWords(sessionsCount)} session${sessionsCount === 1 ? "" : "s"}`;
  const lead =
    weekNumber % 2 === 0
      ? `${capitalize(sessions)}, ${durationWords(runtimeMs)} in all.`
      : `${capitalize(durationWords(runtimeMs))} ${sessionsCount === 1 ? "in one session" : `over ${sessions}`}.`;
  if (daysPlayed === 1) return { before: `${lead} All of it on `, em: longestDay, after: "." };
  if (topTitle && gamesCount === 1) return { before: `${lead} All of it in `, em: topTitle, after: "." };
  const leading = topTitle && runtimeMs > 0 && topMs / runtimeMs >= WEEK_TOP_GAME_MIN_SHARE;
  if (leading && Math.floor(weekNumber / 2) % 2 === 1) {
    return { before: `${lead} Most of it went to `, em: topTitle, after: "." };
  }
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
