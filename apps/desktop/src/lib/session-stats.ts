// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { RECENT_DAYS } from "@/lib/constants";
import { parseVaultimeDate, UI_LOCALE } from "@/lib/time";
import type { Session } from "@/lib/types";

export interface ActivityPoint {
  key: string;
  label: string;
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
  sessionsCount: number;
}

export function buildDailyActivity(
  sessions: Session[],
  days: number,
): ActivityPoint[] {
  const today = new Date();
  today.setHours(0, 0, 0, 0);

  const points = new Map<string, ActivityPoint>();

  for (let offset = days - 1; offset >= 0; offset -= 1) {
    const day = new Date(today);
    day.setDate(today.getDate() - offset);
    const key = toDayKey(day);
    points.set(key, {
      key,
      label: day.toLocaleDateString(UI_LOCALE, {
        month: "short",
        day: "numeric",
      }),
      runtimeMs: 0,
      activeMs: 0,
      idleMs: 0,
      sessionsCount: 0,
    });
  }

  for (const session of sessions) {
    const key = toDayKey(parseVaultimeDate(session.started_at_wall));
    const point = points.get(key);
    if (!point) {
      continue;
    }

    point.runtimeMs += session.runtime_ms;
    point.activeMs += session.active_ms;
    point.idleMs += session.idle_ms;
    point.sessionsCount += 1;
  }

  return [...points.values()];
}

/** The local calendar day, "2026-09-29". */
function toDayKey(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export interface RecentPlay {
  runtimeMs: number;
  activeMs: number;
  sessionsCount: number;
  /** Days with at least one session. */
  daysCount: number;
  runtimeByGame: Map<string, number>;
  longest: Session | null;
}

/** Totals for the last `days` calendar days, today included. */
export function summarizeRecentPlay(sessions: Session[], days = RECENT_DAYS, now = new Date()): RecentPlay {
  const cutoff = new Date(now.getFullYear(), now.getMonth(), now.getDate() - (days - 1));
  const summary: RecentPlay = {
    runtimeMs: 0,
    activeMs: 0,
    sessionsCount: 0,
    daysCount: 0,
    runtimeByGame: new Map(),
    longest: null,
  };
  const playedDays = new Set<string>();

  for (const session of sessions) {
    const started = parseVaultimeDate(session.started_at_wall);
    if (started < cutoff) continue;
    summary.runtimeMs += session.runtime_ms;
    summary.activeMs += session.active_ms;
    summary.sessionsCount += 1;
    summary.runtimeByGame.set(
      session.game_id,
      (summary.runtimeByGame.get(session.game_id) ?? 0) + session.runtime_ms,
    );
    playedDays.add(toDayKey(started));
    if (!summary.longest || session.runtime_ms > summary.longest.runtime_ms) {
      summary.longest = session;
    }
  }

  summary.daysCount = playedDays.size;
  return summary;
}
