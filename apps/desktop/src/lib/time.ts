// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

export function parseVaultimeDate(value: string): Date {
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
  return parseVaultimeDate(value).toLocaleDateString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatCalendarDay(value: string): string {
  return parseVaultimeDate(value).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function formatLongDate(value: string): string {
  return parseVaultimeDate(value).toLocaleDateString(undefined, {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: "numeric",
  });
}

export function formatWeekday(value: string): string {
  return parseVaultimeDate(value).toLocaleDateString(undefined, {
    weekday: "short",
  });
}
