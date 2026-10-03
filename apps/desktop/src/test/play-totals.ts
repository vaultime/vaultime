// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// What the core's `get_play_totals` answers, worked out from sessions alone
// for tests and the browser preview. The core places time by the session
// events, this spreads each session evenly over its span instead.

import { countsAsPlay, toDayKey } from "@/lib/session-stats";
import { weekdayOf } from "@/lib/stats";
import { parseVaultimeDate } from "@/lib/time";
import type { PlayBucket, PlayTotal, Session } from "@/lib/types";

const SLICE_MS = 15 * 60 * 1000;

function bucketOf(start: Date, bucket: PlayBucket): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  switch (bucket) {
    case "day":
      return toDayKey(start);
    case "month":
      return `${start.getFullYear()}-${pad(start.getMonth() + 1)}`;
    case "year":
      return String(start.getFullYear());
    case "hour_of_week":
      return `${weekdayOf(start)}-${start.getHours()}`;
  }
}

function dayStart(key: string): number {
  const [year, month, day] = key.split("-").map(Number);
  return new Date(year, month - 1, day).getTime();
}

export function estimatePlayTotals(
  sessions: Session[],
  from: string,
  to: string,
  bucket: PlayBucket,
  gameId: string | null = null,
  now = new Date(),
): PlayTotal[] {
  const [rangeStart, rangeEnd] = [dayStart(from), dayStart(to)];
  const totals = new Map<string, PlayTotal>();
  for (const session of sessions) {
    if (!countsAsPlay(session) || (gameId && session.game_id !== gameId)) continue;
    const start = parseVaultimeDate(session.started_at_wall).getTime();
    const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall).getTime() : now.getTime();
    const span = Math.max(end - start, 1);
    for (let slice = Math.floor(start / SLICE_MS) * SLICE_MS; slice < Math.max(end, start + 1); slice += SLICE_MS) {
      if (slice < rangeStart || slice >= rangeEnd) continue;
      const overlap = end > start ? Math.min(end, slice + SLICE_MS) - Math.max(start, slice) : 1;
      const share = overlap / span;
      const key = `${bucketOf(new Date(slice), bucket)}|${session.game_id}`;
      const total = totals.get(key) ?? {
        bucket: bucketOf(new Date(slice), bucket),
        game_id: session.game_id,
        runtime_ms: 0,
        active_ms: 0,
        idle_ms: 0,
      };
      total.runtime_ms += session.runtime_ms * share;
      total.active_ms += session.active_ms * share;
      total.idle_ms += session.idle_ms * share;
      totals.set(key, total);
    }
  }
  return [...totals.values()]
    .map((total) => ({
      ...total,
      runtime_ms: Math.round(total.runtime_ms),
      active_ms: Math.round(total.active_ms),
      idle_ms: Math.round(total.idle_ms),
    }))
    .sort((a, b) => a.bucket.localeCompare(b.bucket) || a.game_id.localeCompare(b.game_id));
}
