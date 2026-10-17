// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { DAY_MS, DAYS_PER_WEEK } from "@/lib/constants";
import { parseVaultimeDate, startOfDay, startOfWeek, toDayKey } from "@/lib/time";
import type { GameStatusChange, JournalWeekPlay, PlayRun, SideBySideGroup, StripPart } from "@/lib/types";

/** One day of the journal, as the core worked it out, with the status changes of games on it. */
export interface JournalDay {
  /** Local midnight at the start of the day. */
  start: Date;
  /** Sessions that started on this day, in the order they started. */
  sessionIds: string[];
  /** Every session's parts on the strip, in the order they began. */
  parts: StripPart[];
  runs: PlayRun[];
  groups: SideBySideGroup[];
  changes: GameStatusChange[];
  /** Time with a game running, games side by side counted once. */
  playedMs: number;
}

/** The element of a session's entry or its line on a later day, for jumps between them. */
export function entryId(sessionId: string, dayKey: string): string {
  return `journal-${sessionId}-${dayKey}`;
}

/** How many weeks back the week of `day` ("2026-09-28") lies, 0 for this week or no day. */
export function weeksBack(day: string | null): number {
  const [year, month, date] = (day ?? "").split("-").map(Number);
  if (!year || !month || !date) return 0;
  const thisWeek = startOfWeek(new Date());
  const asked = startOfWeek(new Date(year, month - 1, date));
  // Rounded, since a week with a clock change is an hour longer or shorter.
  return Math.min(0, Math.round((asked.getTime() - thisWeek.getTime()) / (DAYS_PER_WEEK * DAY_MS)));
}

/** The Monday of the week `offset` weeks from the one that holds `now`, 0 for that week. */
export function weekStart(offset: number, now: Date): Date {
  return startOfDay(startOfWeek(now), offset * DAYS_PER_WEEK);
}

/**
 * The core's days of the week from `start`, with the status changes of the
 * week on their days, newest day first. A day with only changes has no
 * strip.
 */
export function journalDays(week: JournalWeekPlay, changes: GameStatusChange[], start: Date): JournalDay[] {
  const end = startOfDay(start, DAYS_PER_WEEK);
  const days = new Map<string, JournalDay>(
    week.days.map((day) => [
      day.day,
      {
        start: parseVaultimeDate(day.day),
        sessionIds: day.session_ids,
        parts: day.parts,
        runs: day.runs,
        groups: day.side_by_side,
        changes: [],
        playedMs: day.played_ms,
      },
    ]),
  );
  for (const change of changes) {
    const changed = parseVaultimeDate(change.changed_at);
    if (change.status === "none" || changed < start || changed >= end) continue;
    const key = toDayKey(changed);
    const day = days.get(key) ?? {
      start: startOfDay(changed),
      sessionIds: [],
      parts: [],
      runs: [],
      groups: [],
      changes: [],
      playedMs: 0,
    };
    day.changes.push(change);
    days.set(key, day);
  }
  return [...days.values()].sort((a, b) => b.start.getTime() - a.start.getTime());
}
