// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { warn } from "@tauri-apps/plugin-log"
import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"
import type { CoreError, CoreErrorKind } from "@/lib/types"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

function isCoreError(error: unknown): error is CoreError {
  return (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    "message" in error &&
    typeof error.message === "string"
  );
}

/** The message of an error, or the value itself as text, starting with a capital. */
export function describeError(error: unknown): string {
  const message = error instanceof Error ? error.message : isCoreError(error) ? error.message : String(error);
  return message.charAt(0).toUpperCase() + message.slice(1);
}

/** What kind of failure the core reported, or null for any other error. */
export function errorKind(error: unknown): CoreErrorKind | null {
  return isCoreError(error) ? error.kind : null;
}

/**
 * A catch handler for work whose failure the page has no place to show, like
 * saving a setting as the window closes, so the failure still reaches the log.
 */
export function logFailure(what: string) {
  return (error: unknown) => {
    warn(`${what} failed: ${describeError(error)}`).catch(() => {});
  };
}

/** A number that stays the same for the same text, to pick colors or wording without randomness. */
export function stableHash(text: string): number {
  let hash = 0;
  for (const char of text) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return hash;
}
