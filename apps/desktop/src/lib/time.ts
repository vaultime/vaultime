// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

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
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }

  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }

  return `${seconds}s`;
}

export function formatCompactDuration(ms: number): string {
  const totalMinutes = Math.floor(ms / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

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

/** "Just now", "12 min ago", "3 hours ago", "Yesterday", "Monday", "In August". */
export function formatRelativeDay(value: string, now = new Date()): string {
  const date = parseVaultimeDate(value);
  const minutes = Math.floor((now.getTime() - date.getTime()) / 60_000);
  if (minutes < 2) return "Just now";
  if (minutes < 60) return `${minutes} min ago`;

  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const startOfDay = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const days = Math.round((startOfToday.getTime() - startOfDay.getTime()) / 86_400_000);
  if (days === 0) {
    const hours = Math.floor(minutes / 60);
    return hours === 1 ? "An hour ago" : `${hours} hours ago`;
  }
  if (days === 1) return "Yesterday";
  if (days < 7) return date.toLocaleDateString(UI_LOCALE, { weekday: "long" });

  const month = date.toLocaleDateString(UI_LOCALE, { month: "long" });
  return date.getFullYear() === now.getFullYear()
    ? `In ${month}`
    : `In ${month} ${date.getFullYear()}`;
}

/** Whole hours for lists: "40 min", "3 h", "142 h". */
export function formatHoursShort(ms: number): string {
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h`;
}

/** Stopwatch style: "01:24:10". */
export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(Math.floor(total / 3600))}:${pad(Math.floor((total % 3600) / 60))}:${pad(total % 60)}`;
}

/** Hours and minutes with a space: "4 h 05", "38 min". */
export function formatHoursMinutes(ms: number): string {
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")}`;
}
