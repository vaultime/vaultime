// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** The message of an error, or the value itself as text, starting with a capital. */
export function describeError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return message.charAt(0).toUpperCase() + message.slice(1);
}

/** A number that stays the same for the same text, to pick colors or wording without randomness. */
export function stableHash(text: string): number {
  let hash = 0;
  for (const char of text) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return hash;
}
