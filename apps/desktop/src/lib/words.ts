// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { HOUR_MS, MINUTE_MS, NUMBER_WORDS_MAX } from "@/lib/constants";
import { UI_LOCALE, type DayPart } from "@/lib/time";

const ONES = [
  "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
  "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
];
const TENS = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

/** Numbers as words for prose, "one hundred forty-two". Digits past NUMBER_WORDS_MAX. */
export function numberWords(value: number): string {
  const n = Math.floor(value);
  if (n < 0 || n > NUMBER_WORDS_MAX) return n.toLocaleString(UI_LOCALE);
  if (n < ONES.length) return ONES[n];
  if (n < 100) return TENS[Math.floor(n / 10)] + (n % 10 ? `-${ONES[n % 10]}` : "");
  const rest = n % 100;
  return `${ONES[Math.floor(n / 100)]} hundred${rest ? ` ${numberWords(rest)}` : ""}`;
}

/** "one session", "twenty-one sessions". */
export function countWords(count: number, noun: string): string {
  return `${numberWords(count)} ${noun}${count === 1 ? "" : "s"}`;
}

/** "twenty-eight hours", "forty minutes": whole hours, or whole minutes under an hour. */
export function amountWords(ms: number): string {
  if (ms < HOUR_MS) return countWords(Math.floor(ms / MINUTE_MS), "minute");
  return countWords(Math.floor(ms / HOUR_MS), "hour");
}

/** "eleven hours and twenty minutes", "forty minutes". */
export function durationWords(ms: number): string {
  const hours = Math.floor(ms / HOUR_MS);
  const minutes = Math.floor((ms % HOUR_MS) / MINUTE_MS);
  if (hours === 0) return countWords(minutes, "minute");
  return minutes > 0 ? `${countWords(hours, "hour")} and ${countWords(minutes, "minute")}` : countWords(hours, "hour");
}

/** How prose names each part of the day. */
export const DAY_PART_WORDS: Record<DayPart, { adjective: string; adverb: string; noun: string }> = {
  morning: { adjective: "morning", adverb: "in the morning", noun: "morning" },
  afternoon: { adjective: "afternoon", adverb: "in the afternoon", noun: "afternoon" },
  evening: { adjective: "evening", adverb: "in the evening", noun: "evening" },
  night: { adjective: "late night", adverb: "late at night", noun: "night" },
};

export function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** "invalid email or password" becomes "Invalid email or password." */
export function sentence(text: string): string {
  const trimmed = capitalize(text.trim());
  return /[.!?]$/.test(trimmed) ? trimmed : `${trimmed}.`;
}

/** "this PC" or "another PC", or null while the id of this PC is not known. */
export function whichPc(deviceId: string | null | undefined, thisDeviceId: string | null): string | null {
  if (!deviceId || !thisDeviceId) return null;
  return deviceId === thisDeviceId ? "this PC" : "another PC";
}

/** "1 game", "3 games". */
export function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}
