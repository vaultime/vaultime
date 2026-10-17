// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The lines about one session: how it reads in the journal and on game
// pages, how it goes on past midnight, and why it is flagged.

import {
  DAY_MS,
  DAYS_PER_WEEK,
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
  WEEKDAY_NAME_DAYS,
  WORKING_DAYS_PER_WEEK,
} from "@/lib/constants";
import {
  normalizeIntegrityStatus,
  parseIntegrityPayload,
  playerReason,
  previousRuntimeMs,
  quotedReason,
} from "@/lib/integrity";
import type { Phrase } from "@/lib/sentences";
import { countsAsPlay } from "@/lib/sessions";
import { shapeOf, type SessionShape } from "@/lib/stats";
import {
  calendarDaysAgo,
  dayPartOf,
  formatDate,
  formatDayAndMonth,
  formatHoursMinutes,
  inSmallHours,
  parseVaultimeDate,
  startOfDay,
  weekdayOf,
} from "@/lib/time";
import type { Session, SessionEvent } from "@/lib/types";
import { stableHash } from "@/lib/utils";
import { capitalize, DAY_PART_WORDS, numberWords } from "@/lib/words";

const NO_BREAK_SPACE = "\u00a0";

const REASON_NOTES: Record<string, string> = {
  wall_clock_moved_backwards: "The system clock moved back during it.",
  wall_clock_step_mismatch: "The system clock jumped during it.",
  wall_clock_drift_exceeded: "The system clock drifted away from real time.",
  startup_orphan_cleanup: "Rebuilt after Vaultime closed without ending it.",
};

function withArticle(word: string): string {
  return `${/^[aeiou]/i.test(word) ? "An" : "A"} ${word}`;
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
  const smallHours = inSmallHours(started);
  const words = DAY_PART_WORDS[part];
  const day = smallHours ? startOfDay(started, -1) : started;
  const weekend = weekdayOf(day) >= WORKING_DAYS_PER_WEEK;
  return {
    adjective: words.adjective,
    adverb: smallHours ? "in the small hours" : words.adverb,
    noun: words.noun,
    occasion: weekend ? `${formatDate(day, { weekday: "long" })} ${words.noun}` : words.noun,
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
    (other) => other.id !== session.id && countsAsPlay(other) && other.started_at_wall < session.started_at_wall,
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
    const mostly: Wording = [["", " ran mostly on its own"], "Mostly left running"];
    // Only a long session can have stayed open through a whole part of the day.
    return brief || shape === "plain"
      ? [mostly]
      : [[["", ` stayed open through the ${words.noun}`], `Left open through the ${words.noun}`], mostly];
  }

  // Play from before Vaultime may hold a longer session, so only tracked history can set a record.
  const record =
    (context.earlierMs ?? 0) < MINUTE_MS &&
    earlier.length >= SESSION_RECORD_MIN_EARLIER &&
    session.runtime_ms >= SESSION_RECORD_MIN_MS &&
    earlier.every((other) => other.runtime_ms < session.runtime_ms);
  if (record) {
    return [
      [["Your longest session of ", " yet"], "Your longest session yet"],
      [[`${capitalize(words.length)} of `, ", your longest yet"], `${capitalize(words.length)}, your longest yet`],
    ];
  }

  // The wall clock end counts sleep, the runtime does not, so a session slept through is no late night.
  const ended = session.ended_at_wall
    ? Math.min(parseVaultimeDate(session.ended_at_wall).getTime(), started.getTime() + session.runtime_ms)
    : null;
  const midnight = startOfDay(started, 1);
  if (ended !== null && ended - midnight.getTime() >= SESSION_PAST_MIDNIGHT_MIN_MS) {
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

/**
 * A line for the day a session went on to, built around two parts: the
 * title in italics and the time it began, which links back to it. "Still in
 * *Elden Ring* from [Saturday night]."
 */
export interface ContinuedPhrase {
  before: string;
  em: string;
  middle: string;
  link: string;
  after: string;
}

/** "Saturday night", "early Saturday", "3 October": when a session that went on began, seen from `day`. */
export function beganWords(started: Date, day: Date): string {
  if (Math.abs(calendarDaysAgo(started, day)) >= WEEKDAY_NAME_DAYS) {
    return formatDayAndMonth(started);
  }
  const weekday = formatDate(started, { weekday: "long" });
  const part = dayPartOf(started);
  if (inSmallHours(started)) return `early ${weekday}`;
  return `${weekday} ${DAY_PART_WORDS[part].noun}`;
}

/**
 * The line for a session on a day after the one it began on, so it reads as
 * the same session going on rather than a new one. `throughDay` is a day it
 * ran through to the end. The wording varies between sessions and stays the
 * same for each one.
 */
export function continuedPhrase(
  session: Session,
  title: string,
  day: Date,
  { throughDay = false }: { throughDay?: boolean } = {},
): ContinuedPhrase {
  const link = beganWords(parseVaultimeDate(session.started_at_wall), day);
  const options: [before: string, middle: string, after: string][] = !session.ended_at_wall
    ? [
        ["Still playing ", " from ", ""],
        ["", " has been running since ", ""],
      ]
    : throughDay
      ? [
          ["Still in ", " from ", ", all through the day"],
          ["", " ran on all day, open since ", ""],
        ]
      : [
          ["Still in ", " from ", ""],
          ["", ", carried on from ", ""],
        ];
  const [before, middle, after] = options[stableHash(session.id) % options.length];
  return { before, em: title, middle, link, after };
}

/** "2 h 15" kept on one line, or "Under a minute" at the start of a sentence. */
function partAmount(ms: number, start: boolean): string {
  if (ms >= MINUTE_MS) return formatHoursMinutes(ms).replaceAll(" ", NO_BREAK_SPACE);
  return start ? "Under a minute" : "under a minute";
}

/** "2 h 15 of it after midnight, counted on ", followed by the days. */
export function afterMidnightLead(laterMs: number): string {
  return `${partAmount(laterMs, true)} of it after midnight, counted on `;
}

/** "2 h 15 of its 3 h 35 count on this day." for a day a session went on to. */
export function continuedAmounts(partMs: number, wholeMs: number, live: boolean): string {
  return `${partAmount(partMs, true)} of its ${partAmount(wholeMs, false)}${live ? " so far" : ""} count on this day.`;
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
    const withReason = (lead: string) => `${lead}${quotedReason(playerReason(payload))}.`;
    if (event.event_type === "corrected") {
      const previousMs = previousRuntimeMs(payload);
      const before = previousMs === null ? null : formatHoursMinutes(previousMs);
      const lead =
        payload?.runtime_ms === 0
          ? before
            ? `All ${before} taken out by you`
            : "All time taken out by you"
          : before
            ? `Cut short by you from ${before}`
            : "Cut short by you";
      notes.push(withReason(lead));
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
