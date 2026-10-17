// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import {
  DAY_MS,
  DAY_PART_HOURS,
  DAYS_PER_WEEK,
  HOUR_MS,
  HOURS_PER_DAY,
  JUST_NOW_MINUTES,
  MINUTE_MS,
  MINUTES_PER_HOUR,
  SECOND_MS,
  SECONDS_PER_HOUR,
  SECONDS_PER_MINUTE,
  WEEKDAY_NAME_DAYS,
} from "@/lib/constants";

/**
 * English words to match the UI, with the date order and clock of the user's
 * region. A German system gets "Tuesday, 29 September" and a 24 hour clock.
 */
export const UI_LOCALE = (() => {
  try {
    const region = new Intl.Locale(navigator.language).maximize().region;
    return region ? `en-${region}` : "en";
  } catch {
    return "en";
  }
})();

const dateFormats = new Map<string, Intl.DateTimeFormat>();

/**
 * A date in the UI locale, like `toLocaleDateString` with these options.
 * Making a formatter costs far more than using one, so each is made once.
 */
export function formatDate(date: Date, options: Intl.DateTimeFormatOptions): string {
  const key = JSON.stringify(options);
  let format = dateFormats.get(key);
  if (!format) {
    format = new Intl.DateTimeFormat(UI_LOCALE, options);
    dateFormats.set(key, format);
  }
  return format.format(date);
}

export function parseVaultimeDate(value: string): Date {
  if (/[zZ]$|[+-]\d{2}:\d{2}$/.test(value)) {
    return new Date(value);
  }

  // A bare date is a calendar day, so it starts at local midnight.
  const day = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (day) {
    return new Date(Number(day[1]), Number(day[2]) - 1, Number(day[3]));
  }

  return new Date(`${value}Z`);
}

export function formatCalendarDay(value: string): string {
  return formatDate(parseVaultimeDate(value), {
    month: "short",
    day: "numeric",
  });
}

export function formatLongDate(value: string): string {
  return formatDate(parseVaultimeDate(value), {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: "numeric",
  });
}

/** Local midnight at the start of the day that holds `date`, or of the day `plusDays` after it. */
export function startOfDay(date: Date, plusDays = 0): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + plusDays);
}

/** Monday is 0. */
export function weekdayOf(date: Date): number {
  return (date.getDay() + DAYS_PER_WEEK - 1) % DAYS_PER_WEEK;
}

/** Whole calendar days between two dates, 0 for the same day. Rounded, so a daylight saving switch does not count. */
export function calendarDaysAgo(date: Date, now: Date): number {
  return Math.round((startOfDay(now).getTime() - startOfDay(date).getTime()) / DAY_MS);
}

/** Between midnight and morning, which prose counts to the night before. */
export function inSmallHours(date: Date): boolean {
  return date.getHours() < DAY_PART_HOURS.morning;
}

/** "Just now", "12 min ago", "3 hours ago", "Yesterday", "Monday", "In August". */
export function formatRelativeDay(value: string, now = new Date()): string {
  const date = parseVaultimeDate(value);
  const elapsedMs = now.getTime() - date.getTime();
  if (elapsedMs < JUST_NOW_MINUTES * MINUTE_MS) return "Just now";
  if (elapsedMs < HOUR_MS) return `${Math.floor(elapsedMs / MINUTE_MS)} min ago`;

  const days = calendarDaysAgo(date, now);
  if (days === 0) {
    const hours = Math.floor(elapsedMs / HOUR_MS);
    return hours === 1 ? "An hour ago" : `${hours} hours ago`;
  }
  if (days === 1) return "Yesterday";
  if (days < WEEKDAY_NAME_DAYS) return formatDate(date, { weekday: "long" });

  const month = formatDate(date, { month: "long" });
  return date.getFullYear() === now.getFullYear()
    ? `In ${month}`
    : `In ${month} ${date.getFullYear()}`;
}

/** Whole hours for lists: "40 min", "3 h", "142 h". */
export function formatHoursShort(ms: number): string {
  if (ms < HOUR_MS) return `${Math.floor(ms / MINUTE_MS)} min`;
  return `${Math.floor(ms / HOUR_MS)} h`;
}

/** Stopwatch style: "01:24:10". */
export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / SECOND_MS));
  const hours = Math.floor(total / SECONDS_PER_HOUR);
  const minutes = Math.floor((total % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(hours)}:${pad(minutes)}:${pad(total % SECONDS_PER_MINUTE)}`;
}

/** Hours and minutes with a space: "4 h 05", "38 min". */
export function formatHoursMinutes(ms: number): string {
  if (ms < HOUR_MS) return `${Math.floor(ms / MINUTE_MS)} min`;
  const minutes = Math.floor((ms % HOUR_MS) / MINUTE_MS);
  return `${Math.floor(ms / HOUR_MS)} h ${String(minutes).padStart(2, "0")}`;
}

export type DayPart = "morning" | "afternoon" | "evening" | "night";

export function dayPartOf(date: Date): DayPart {
  const hour = date.getHours();
  const { morning, afternoon, evening, night } = DAY_PART_HOURS;
  if (hour < morning || hour >= night) return "night";
  if (hour < afternoon) return "morning";
  return hour < evening ? "afternoon" : "evening";
}

/** Part of the day within the last week: "This morning", "Last night", "Saturday evening". */
export function formatDayPart(value: string, now = new Date()): string {
  const date = parseVaultimeDate(value);
  const part = dayPartOf(date);
  const day = inSmallHours(date) ? startOfDay(date, -1) : date;
  const days = calendarDaysAgo(day, now);
  if (days === 0) return part === "night" ? "Tonight" : `This ${part}`;
  if (days === 1) return part === "night" ? "Last night" : `Yesterday ${part}`;
  if (days < WEEKDAY_NAME_DAYS) return `${formatDate(day, { weekday: "long" })} ${part}`;
  return formatCalendarDay(value);
}

/** "Today 22:10", "Yesterday 22:10", "Sun 21:40", "16 Sept 21:40". */
export function formatSessionStart(value: string, now = new Date()): string {
  const date = parseVaultimeDate(value);
  const clock = formatClockTime(date);
  const days = calendarDaysAgo(date, now);
  if (days === 0) return `Today ${clock}`;
  if (days === 1) return `Yesterday ${clock}`;
  if (days < WEEKDAY_NAME_DAYS) return `${formatDate(date, { weekday: "short" })} ${clock}`;
  return `${formatDate(date, { day: "numeric", month: "short" })} ${clock}`;
}

/** Midnight at the start of the Monday of the week that holds `date`. */
export function startOfWeek(date: Date): Date {
  return startOfDay(date, -weekdayOf(date));
}

/** ISO 8601 week number. A week belongs to the year that holds its Thursday. */
export function isoWeekNumber(date: Date): number {
  const THURSDAY_AFTER_MONDAY = 3;
  const thursday = startOfDay(startOfWeek(date), THURSDAY_AFTER_MONDAY);
  const firstOfYear = new Date(thursday.getFullYear(), 0, 1);
  return Math.floor(calendarDaysAgo(firstOfYear, thursday) / DAYS_PER_WEEK) + 1;
}

/** A local time as the value of a datetime-local input, "2026-09-30T20:15". */
export function toLocalInput(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** The value of a datetime-local input as a local time, null when empty or broken. */
export function fromLocalInput(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const [year, month, day, hour, minute] = match.slice(1).map(Number);
  return new Date(year, month - 1, day, hour, minute);
}

/** "14 March", in the order of the user's region. */
export function formatDayAndMonth(date: Date): string {
  return formatDate(date, { day: "numeric", month: "long" });
}

/** "3 to 14 March", or "27 February to 2 March" across months. */
export function formatDayRange(start: Date, end: Date): string {
  const sameMonth = start.getFullYear() === end.getFullYear() && start.getMonth() === end.getMonth();
  return `${sameMonth ? String(start.getDate()) : formatDayAndMonth(start)} to ${formatDayAndMonth(end)}`;
}

/** "14:10", in the regional clock. */
export function formatClockTime(value: Date): string {
  return formatDate(value, { hour: "2-digit", minute: "2-digit" });
}

/** "Sun" for a day within a week of `day`, "3 Oct" further away. */
export function formatNearDay(date: Date, day: Date): string {
  return Math.abs(calendarDaysAgo(date, day)) < WEEKDAY_NAME_DAYS
    ? formatDate(date, { weekday: "short" })
    : formatDate(date, { day: "numeric", month: "short" });
}

/**
 * When a session ran, as read from the journal page of `day`: "22:40 to
 * 23:10", "22:40 to Sun 02:15" on the day it began, "Sat 22:40 to 02:15" on
 * the day it ended, "since Wed 19:00" on a day it ran through. A running
 * session runs "to now".
 */
export function formatSpanOnDay(start: Date, end: Date | null, day: Date): string {
  const onDay = (moment: Date) => calendarDaysAgo(moment, day) === 0;
  const at = (moment: Date) =>
    onDay(moment) ? formatClockTime(moment) : `${formatNearDay(moment, day)} ${formatClockTime(moment)}`;
  if (!end) return `${at(start)} to now`;
  const nextDay = startOfDay(day, 1);
  if (!onDay(start) && end > nextDay) return `since ${at(start)}`;
  if (end.getTime() === nextDay.getTime()) return `${at(start)} to midnight`;
  return `${at(start)} to ${at(end)}`;
}

/**
 * Where a moment sits on the 24 hour strip of `day`, in percent. It uses the
 * clock time, so the hour labels stay right on days with a daylight saving
 * switch. Moments before the day give 0, moments after it 100.
 */
export function clockPercent(moment: Date, day: Date): number {
  if (moment < startOfDay(day)) return 0;
  if (moment >= startOfDay(day, 1)) return 100;
  const minutes = moment.getHours() * MINUTES_PER_HOUR + moment.getMinutes() + moment.getSeconds() / SECONDS_PER_MINUTE;
  return (minutes / (HOURS_PER_DAY * MINUTES_PER_HOUR)) * 100;
}
