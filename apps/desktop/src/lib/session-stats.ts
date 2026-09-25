// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import type { Game, Session } from "@/lib/types";
import { parseVaultimeDate } from "@/lib/time";

export interface SessionTotals {
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
  sessionsCount: number;
  averageActiveMs: number;
  activeRatio: number;
  lastPlayedAt: string | null;
}

export interface ActivityPoint {
  key: string;
  label: string;
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
  sessionsCount: number;
}

export interface RankedGameStat {
  game: Game;
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
  sessionsCount: number;
  lastPlayedAt: string | null;
}

export interface SessionDayGroup {
  key: string;
  label: string;
  sessions: Session[];
  runtimeMs: number;
  activeMs: number;
  idleMs: number;
}

export function getSessionTotals(sessions: Session[]): SessionTotals {
  let runtimeMs = 0;
  let activeMs = 0;
  let idleMs = 0;
  let lastPlayedAt: string | null = null;

  for (const session of sessions) {
    runtimeMs += session.runtime_ms;
    activeMs += session.active_ms;
    idleMs += session.idle_ms;

    if (!lastPlayedAt || session.started_at_wall > lastPlayedAt) {
      lastPlayedAt = session.started_at_wall;
    }
  }

  return {
    runtimeMs,
    activeMs,
    idleMs,
    sessionsCount: sessions.length,
    averageActiveMs:
      sessions.length > 0 ? Math.round(activeMs / sessions.length) : 0,
    activeRatio: runtimeMs > 0 ? activeMs / runtimeMs : 0,
    lastPlayedAt,
  };
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
      label: day.toLocaleDateString(undefined, {
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
    const key = session.started_at_wall.slice(0, 10);
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

export function groupSessionsByDay(sessions: Session[]): SessionDayGroup[] {
  const groups = new Map<string, SessionDayGroup>();

  for (const session of sessions) {
    const key = session.started_at_wall.slice(0, 10);
    const existing = groups.get(key) ?? {
      key,
      label: parseVaultimeDate(session.started_at_wall).toLocaleDateString(
        undefined,
        {
          weekday: "long",
          month: "long",
          day: "numeric",
          year: "numeric",
        },
      ),
      sessions: [],
      runtimeMs: 0,
      activeMs: 0,
      idleMs: 0,
    };

    existing.sessions.push(session);
    existing.runtimeMs += session.runtime_ms;
    existing.activeMs += session.active_ms;
    existing.idleMs += session.idle_ms;

    groups.set(key, existing);
  }

  return [...groups.values()].sort((a, b) => b.key.localeCompare(a.key));
}

export function rankGamesByActiveTime(
  games: Game[],
  sessions: Session[],
  limit = 5,
): RankedGameStat[] {
  const statsByGame = new Map<string, RankedGameStat>();

  for (const game of games) {
    statsByGame.set(game.id, {
      game,
      runtimeMs: 0,
      activeMs: 0,
      idleMs: 0,
      sessionsCount: 0,
      lastPlayedAt: null,
    });
  }

  for (const session of sessions) {
    const stat = statsByGame.get(session.game_id);
    if (!stat) {
      continue;
    }

    stat.runtimeMs += session.runtime_ms;
    stat.activeMs += session.active_ms;
    stat.idleMs += session.idle_ms;
    stat.sessionsCount += 1;

    if (
      !stat.lastPlayedAt ||
      session.started_at_wall > stat.lastPlayedAt
    ) {
      stat.lastPlayedAt = session.started_at_wall;
    }
  }

  return [...statsByGame.values()]
    .filter((stat) => stat.activeMs > 0 || stat.sessionsCount > 0)
    .sort((a, b) => {
      if (b.activeMs !== a.activeMs) {
        return b.activeMs - a.activeMs;
      }

      return b.runtimeMs - a.runtimeMs;
    })
    .slice(0, limit);
}

function toDayKey(date: Date): string {
  return date.toISOString().slice(0, 10);
}
