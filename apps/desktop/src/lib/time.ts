// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import {
  DAY_MS,
  DAY_PART_HOURS,
  HOUR_MS,
  JUST_NOW_MINUTES,
  MINUTE_MS,
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

  if (/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    return new Date(`${value}T00:00:00Z`);
  }

  return new Date(`${value}Z`);
}

export function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / SECOND_MS);
  const hours = Math.floor(totalSeconds / SECONDS_PER_HOUR);
  const minutes = Math.floor((totalSeconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  const seconds = totalSeconds % SECONDS_PER_MINUTE;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }

  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }

  return `${seconds}s`;
}

export function formatCompactDuration(ms: number): string {
  const hours = Math.floor(ms / HOUR_MS);
  const minutes = Math.floor((ms % HOUR_MS) / MINUTE_MS);

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }

  return `${minutes}m`;
}

export function formatSessionDate(value: string): string {
  return parseVaultimeDate(value).toLocaleDateString(UI_LOCALE, {
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
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

export function formatWeekday(value: string): string {
  return parseVaultimeDate(value).toLocaleDateString(UI_LOCALE, {
    weekday: "short",
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
  const clock = date.toLocaleTimeString(UI_LOCALE, { hour: "2-digit", minute: "2-digit" });
  const days = calendarDaysAgo(date, now);
  if (days === 0) return `Today ${clock}`;
  if (days === 1) return `Yesterday ${clock}`;
  if (days < WEEKDAY_NAME_DAYS) return `${date.toLocaleDateString(UI_LOCALE, { weekday: "short" })} ${clock}`;
  return `${date.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "short" })} ${clock}`;
}
