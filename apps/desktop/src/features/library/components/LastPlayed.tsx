// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useMinute } from "@/features/library/library-context";
import { lastPlayedLine } from "@/lib/sentences";
import { formatRelativeDay } from "@/lib/time";

/**
 * "5 min ago", or "Last played 5 min ago" as a sentence. On its own, so a
 * new minute renders only this text.
 */
export function LastPlayed({ at, sentence = false }: { at: string | null; sentence?: boolean }) {
  useMinute();
  if (sentence) return lastPlayedLine(at);
  return at ? formatRelativeDay(at) : "Not played yet";
}
