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
  return parseVaultimeDate(value).toLocaleDateString(UI_LOCALE, {
    month: "short",
    day: "numeric",
  });
}

export function formatLongDate(value: string): string {
  return parseVaultimeDate(value).toLocaleDateString(UI_LOCALE, {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: "numeric",
  });
}

/** Whole calendar days between two dates, 0 for the same day. */
function calendarDaysAgo(date: Date, now: Date): number {
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const startOfDay = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  return Math.round((startOfToday.getTime() - startOfDay.getTime()) / DAY_MS);
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
  if (days < WEEKDAY_NAME_DAYS) return date.toLocaleDateString(UI_LOCALE, { weekday: "long" });

  const month = date.toLocaleDateString(UI_LOCALE, { month: "long" });
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
  // Play after midnight still belongs to the night before.
  const afterMidnight = date.getHours() < DAY_PART_HOURS.morning;
  const day = new Date(date.getTime() - (afterMidnight ? DAY_PART_HOURS.morning * HOUR_MS : 0));
  const days = calendarDaysAgo(day, now);
  if (days === 0) return part === "night" ? "Tonight" : `This ${part}`;
  if (days === 1) return part === "night" ? "Last night" : `Yesterday ${part}`;
  if (days < WEEKDAY_NAME_DAYS) return `${day.toLocaleDateString(UI_LOCALE, { weekday: "long" })} ${part}`;
  return formatCalendarDay(value);
}

/** "Today 22:10", "Yesterday 22:10", "Sun 21:40", "16 Sept 21:40". */
export function formatSessionStart(value: string, now = new Date()): string {
  const date = parseVaultimeDate(value);
  const clock = formatClockTime(date);
  const days = calendarDaysAgo(date, now);
  if (days === 0) return `Today ${clock}`;
  if (days === 1) return `Yesterday ${clock}`;
  if (days < WEEKDAY_NAME_DAYS) return `${date.toLocaleDateString(UI_LOCALE, { weekday: "short" })} ${clock}`;
  return `${date.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "short" })} ${clock}`;
}

/** Midnight at the start of the Monday of the week that holds `date`. */
export function startOfWeek(date: Date): Date {
  const daysSinceMonday = (date.getDay() + DAYS_PER_WEEK - 1) % DAYS_PER_WEEK;
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() - daysSinceMonday);
}

/** ISO 8601 week number. A week belongs to the year that holds its Thursday. */
export function isoWeekNumber(date: Date): number {
  const THURSDAY_AFTER_MONDAY = 3;
  const monday = startOfWeek(date);
  const thursday = new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + THURSDAY_AFTER_MONDAY);
  const firstOfYear = new Date(thursday.getFullYear(), 0, 1);
  return Math.floor(calendarDaysAgo(firstOfYear, thursday) / DAYS_PER_WEEK) + 1;
}

/** "14:10", in the regional clock. */
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
  return date.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "long" });
}

/** "3 to 14 March", or "27 February to 2 March" across months. */
export function formatDayRange(start: Date, end: Date): string {
  const sameMonth = start.getFullYear() === end.getFullYear() && start.getMonth() === end.getMonth();
  return `${sameMonth ? String(start.getDate()) : formatDayAndMonth(start)} to ${formatDayAndMonth(end)}`;
}

export function formatClockTime(value: Date): string {
  return value.toLocaleTimeString(UI_LOCALE, { hour: "2-digit", minute: "2-digit" });
}

/**
 * Where a moment sits on the 24 hour strip of `day`, in percent. It uses the
 * clock time, so the hour labels stay right on days with a daylight saving
 * switch. Moments before the day give 0, moments after it 100.
 */
export function clockPercent(moment: Date, day: Date): number {
  const dayStart = new Date(day.getFullYear(), day.getMonth(), day.getDate());
  const nextDay = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1);
  if (moment < dayStart) return 0;
  if (moment >= nextDay) return 100;
  const minutes = moment.getHours() * MINUTES_PER_HOUR + moment.getMinutes() + moment.getSeconds() / SECONDS_PER_MINUTE;
  return (minutes / (HOURS_PER_DAY * MINUTES_PER_HOUR)) * 100;
}
