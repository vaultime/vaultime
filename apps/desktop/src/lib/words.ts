// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { NUMBER_WORDS_MAX } from "@/lib/constants";
import { UI_LOCALE } from "@/lib/time";

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

export function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** "invalid email or password" becomes "Invalid email or password." */
export function sentence(text: string): string {
  const trimmed = capitalize(text.trim());
  return /[.!?]$/.test(trimmed) ? trimmed : `${trimmed}.`;
}

/** "1 game", "3 games". */
export function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}
